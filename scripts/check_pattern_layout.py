#!/usr/bin/env python3
"""Compare pattern layout acceptance across contexts with Elm 0.19.1.

Every valid fixture is designed to typecheck: function/lambda/let patterns are
irrefutable; case patterns are wrapped in Just with a wildcard fallback. Both
compilers execute make, and Rust's parse command is checked independently.
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
    ('tuple-next', '(x,\n{pad}y)', '(1,2)'),
    ('tuple-end', '(x,y\n{pad})', '(1,2)'),
    ('record-next', '{{x,\n{pad}y}}', '{x=1,y=2}'),
    ('record-end', '{{x,y\n{pad}}}', '{x=1,y=2}'),
    ('alias', '(x as\n{pad}whole)', '1'),
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-pattern-layout-') as directory:
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
    for context, floor in [('function',1),('lambda',1),('let',9),('case',5),('local-function',9),('local-lambda',9),('nested-case',17)]:
        choices = templates + ([('list-next','[x,\n{pad}y]',''),('list-end','[x,y\n{pad}]',''),('cons','(x ::\n{pad}xs)','')] if context in ['case','nested-case'] else [])
        for label, template, value in choices:
            for column in [1,2,4,5,8,9,12,13,16,17,18,21]:
                pattern = template.format(pad=' '*(column-1))
                if context == 'function': body = f'f {pattern} = 1'
                elif context == 'lambda': body = f'f = \\{pattern} -> 1'
                elif context == 'let': body = f'f = let\n        {pattern} = {value}\n    in 1'
                elif context == 'local-function': body = f'f = let\n        g {pattern} = 1\n    in 1'
                elif context == 'local-lambda': body = f'f = let\n        g = \\{pattern} -> 1\n    in 1'
                elif context == 'nested-case': body = f'f value =\n    let\n        g v =\n            case v of\n                Just ({pattern}) -> 1\n                _ -> 0\n    in g value'
                else: body = f'f value = case value of\n    Just ({pattern}) -> 1\n    _ -> 0'
                name = f'Fixture{len(results)}'
                source = f'module {name} exposing (..)\n{body}\n'
                entry = name+'.elm'
                (project/entry).write_text(source)
                row = {'case':f'{context}-{label}-column-{column}','source':source,'expected':column>floor}
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
