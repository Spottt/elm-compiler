#!/usr/bin/env python3
"""Compare declaration layout against Elm with explicit indentation thresholds."""
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
    ('value-equals', 'value\n{pad}= 1'),
    ('function-arg', 'value\n{pad}arg = arg'),
    ('function-equals', 'value arg\n{pad}= arg'),
    ('annotation-colon', 'value\n{pad}: Int\n{decl}value = 1'),
    ('annotation-type', 'value :\n{pad}Int\n{decl}value = 1'),
    ('value-body', 'value =\n{pad}1'),
]
top_templates = [
    ('type-name', 'type\n{pad}T = T'),
    ('type-alias', 'type\n{pad}alias T = Int'),
    ('alias-name', 'type alias\n{pad}T = Int'),
    ('type-param', 'type T\n{pad}a = T a'),
    ('alias-param', 'type alias T\n{pad}a = List a'),
    ('type-equals', 'type T a\n{pad}= T a'),
    ('alias-equals', 'type alias T a\n{pad}= List a'),
    ('type-bar', 'type T = A\n{pad}| B'),
    ('type-variant', 'type T = A |\n{pad}B'),
    ('port-name', 'port\n{pad}send : Int -> Cmd msg'),
    ('port-colon', 'port send\n{pad}: Int -> Cmd msg'),
    ('port-type', 'port send :\n{pad}Int -> Cmd msg'),
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-declaration-layout-') as directory:
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
    for context, floor in [('value',1),('local-value',9),('nested-value',17)]:
        selected = templates + top_templates if context == 'value' else templates + [('destruct-equals', '(value, other)\n{pad}= (1,2)')]
        for label, template in selected:
            for column in [1,2,4,5,8,9,10,13,16,17,18,21]:
                decl = ' '*(floor-1)
                definition = template.format(pad=' '*(column-1), decl=decl)
                if context == 'value': body = definition
                elif context == 'local-value': body = f'f =\n    let\n        {definition}\n    in value'
                else: body = f'f =\n    let\n        g =\n            let\n                {definition}\n            in value\n    in g'
                name = f'Fixture{len(results)}'
                prefix = 'port ' if label.startswith('port-') else ''
                source = f'{prefix}module {name} exposing (..)\n{body}\n'
                entry = name+'.elm'
                (project/entry).write_text(source)
                expected = column > floor
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
