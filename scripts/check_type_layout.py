#!/usr/bin/env python3
"""Compare type layout acceptance in aliases, annotations, constructors and ports.

Valid fixtures must typecheck; both compilers' make and Rust parse must match
an explicit indentation threshold. Package caches and sources are isolated.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path, required=True)
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
binary = crate/'target/release/planexpo-elm'
sha = hashlib.sha256(binary.read_bytes()).hexdigest()
templates = [
    ('unit-open', '(\n{pad})', '()'),
    ('unit-gap', '({pad})', '()'),
    ('tuple-next', '(Int,\n{pad}Int)', '(1,2)'),
    ('tuple-end', '(Int,Int\n{pad})', '(1,2)'),
    ('tuple-comma', '(Int\n{pad},Int)', '(1,2)'),
    ('record-open', '{{\n{pad}x : Int}}', '{x=1}'),
    ('record-empty', '{{\n{pad}}}', '{}'),
    ('record-next', '{{x:Int,\n{pad}y:Int}}', '{x=1,y=2}'),
    ('record-end', '{{x:Int,y:Int\n{pad}}}', '{x=1,y=2}'),
    ('record-colon', '{{x\n{pad}:Int}}', '{x=1}'),
    ('function', '(Int ->\n{pad}Int)', 'identity'),
    ('extension-bar', '{{r\n{pad}|x:Int}}', ''),
    ('extension-field', '{{r |\n{pad}x:Int}}', ''),
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-type-layout-') as directory:
    root = Path(directory)
    packages = root/'home/0.19.1/packages'
    original = Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
    packages.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat', packages/'registry.dat')
    for name, version in [('elm/core','1.0.5'),('elm/json','1.1.3')]:
        shutil.copytree(original/name/version/'src',packages/name/version/'src')
        shutil.copyfile(original/name/version/'elm.json',packages/name/version/'elm.json')
    project = root/'project'
    project.mkdir()
    shutil.copyfile(crate/'tests/programs/worker/elm.json',project/'elm.json')
    env = {**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1 -A16m -c','no_proxy':'','NO_PROXY':''}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    for context, floor in [('annotation',1),('alias',1),('constructor',1),('port',1),('local-annotation',9)]:
        for label, template, value in templates:
            if label.startswith('extension-') and context not in ['alias','constructor']:
                continue
            if context == 'port' and label == 'function':
                continue  # Functions are intentionally not valid port payloads.
            for column in [1,2,4,5,8,9,10,13]:
                typ = template.format(pad=' '*(column-1))
                if context == 'annotation': body = f'value : {typ}\nvalue = {value}'
                elif context == 'alias': body = f'type alias T{" r" if label.startswith("extension-") else ""} = {typ}'
                elif context == 'constructor': body = f'type T{" r" if label.startswith("extension-") else ""} = T {typ}'
                elif context == 'port': body = f'port send : {typ} -> Cmd msg'
                else: body = f'f = let\n        value : {typ}\n        value = {value}\n    in 1'
                name = f'Fixture{len(results)}'
                prefix = 'port ' if context == 'port' else ''
                source = f'{prefix}module {name} exposing (..)\n{body}\n'
                entry = name+'.elm'
                (project/entry).write_text(source)
                row = {'case':f'{context}-{label}-column-{column}','source':source,'expected':(column == 1 if label == 'unit-gap' else label != 'unit-open' and column>floor)}
                for compiler, executable in [('official',args.elm.resolve()),('rust',binary)]:
                    run = subprocess.run([str(executable),'make',entry,'--output=/dev/null','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
                    row[compiler] = {'returncode':run.returncode,'accepted':run.returncode==0,'stderr':run.stderr if run.returncode else ''}
                run = subprocess.run([str(binary),'parse',str(project/entry)],cwd=project,env=env,capture_output=True,text=True,timeout=30)
                row['rust_parse'] = {'returncode':run.returncode,'accepted':run.returncode==0,'stderr':run.stderr if run.returncode else ''}
                row['matches'] = all(row[key]['returncode'] in (0,1) and row[key]['accepted']==row['expected'] for key in ['official','rust','rust_parse'])
                results.append(row)
assert hashlib.sha256(binary.read_bytes()).hexdigest() == sha
report = {'scope':__doc__,'compiler_sha256':sha,'cases':len(results),'failures':[r['case'] for r in results if not r['matches']],'results':results}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:report[k] for k in ['cases','failures']}))
raise SystemExit(bool(report['failures']))
