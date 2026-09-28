#!/usr/bin/env python3
"""Compare expression delimiter layout with Elm 0.19.1.

All syntactically valid fixtures must typecheck. Explicit layout expectations
are compared with make from both compilers and Rust parse independently.
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
    ('unit-open', '(\n{pad})'), ('unit-gap', '({pad})'),
    ('operator-open', '(\n{pad}+)'), ('operator-gap', '({pad}+)'),
    ('operator-close', '(+{pad})'),
    ('unit-cr', '(\r)'), ('operator-cr-open', '(\r+)'),
    ('operator-cr-close', '(+\r)'), ('unit-comment','({{-comment-}})'),
    ('tuple-next', '(1,\n{pad}2)'), ('tuple-end', '(1,2\n{pad})'),
    ('tuple-comma', '(1\n{pad},2)'),
    ('list-open', '[\n{pad}1]'), ('list-empty', '[\n{pad}]'),
    ('list-next', '[1,\n{pad}2]'), ('list-end', '[1,2\n{pad}]'),
    ('record-open', '{{\n{pad}x=1}}'), ('record-empty', '{{\n{pad}}}'),
    ('record-next', '{{x=1,\n{pad}y=2}}'), ('record-end', '{{x=1,y=2\n{pad}}}'),
    ('record-equals', '{{x\n{pad}=1}}'),
    ('update-bar', '{{base\n{pad}|x=1}}'), ('update-field', '{{base |\n{pad}x=1}}'),
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-expression-layout-') as directory:
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
    for context, floor in [('value',1),('local-value',9),('case',5),('nested-case',17)]:
        for label, template in templates:
            for column in [1,2,4,5,8,9,10,13,16,17,18,21]:
                expr = template.format(pad=' '*(column-1))
                if context == 'value': body = f'value = {expr}'
                elif context == 'local-value': body = f'f = let\n        value = {expr}\n    in value'
                elif context == 'case': body = f'f v = case v of\n    _ -> {expr}'
                else: body = f'f v =\n    let\n        g x =\n            case x of\n                _ -> {expr}\n    in g v'
                name = f'Fixture{len(results)}'
                source = f'module {name} exposing (..)\nbase = {{x=0}}\n{body}\n'
                entry = name+'.elm'
                (project/entry).write_text(source)
                expected = (True if label in ['unit-cr','operator-cr-open'] else False if label in ['operator-cr-close','unit-comment'] else column == 1 if label in ['unit-gap','operator-gap','operator-close'] else label not in ['unit-open','operator-open'] and column > floor)
                row = {'case':f'{context}-{label}-column-{column}','source':source,'expected':expected}
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
