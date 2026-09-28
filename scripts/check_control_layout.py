#!/usr/bin/env python3
"""Compare control-expression layout against Elm with explicit indentation thresholds."""
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
    ('if-condition', 'if\n{pad}True then 1 else 2'),
    ('if-then', 'if True\n{pad}then 1 else 2'),
    ('if-else', 'if True then 1\n{pad}else 2'),
    ('lambda-arrow', '\\item\n{pad}-> item'),
    ('case-of', 'case True\n{pad}of _ -> 1'),
    ('case-arrow', 'case True of _\n{pad}-> 1'),
    ('case-second-branch', 'case True of\n{branchpad}True -> 1\n{pad}False -> 2'),
    ('let-first', 'let\n{pad}innerValue = 1 in innerValue'),
    ('let-in', 'let innerValue = 1\n{pad}in innerValue'),
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-control-layout-') as directory:
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
            for column in [1,2,4,5,8,9,10,13,16,17,18,21,24,28,40]:
                expr = template.format(pad=' '*(column-1), branchpad=' '*(floor+3))
                if context == 'value': body = f'value = {expr}'
                elif context == 'local-value': body = f'f = let\n        value = {expr}\n    in value'
                elif context == 'case': body = f'f v = case v of\n    _ -> {expr}'
                else: body = f'f v =\n    let\n        g x =\n            case x of\n                _ -> {expr}\n    in g v'
                name = f'Fixture{len(results)}'
                source = f'module {name} exposing (..)\nbase = {{x=0}}\n{body}\n'
                entry = name+'.elm'
                (project/entry).write_text(source)
                threshold = floor
                if label in ['let-first','case-arrow']:
                    marker = 'let\n' if label == 'let-first' else '_\n'
                    at = source.rindex(marker)
                    threshold = at - source.rfind('\n',0,at)
                expected = column == floor+4 if label == 'case-second-branch' else column > threshold
                # A dedented branch can belong to the enclosing case. It parses,
                # but this fixture then has an incomplete inner case and a redundant outer branch.
                parse_expected = expected or (label == 'case-second-branch' and context in ['case','nested-case'] and column == floor)
                row = {'case':f'{context}-{label}-column-{column}','source':source,'expected':expected,'parse_expected':parse_expected}
                for compiler, executable in [('official',args.elm.resolve()),('rust',binary)]:
                    run = subprocess.run([str(executable),'make',entry,'--output=/dev/null','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
                    row[compiler] = {'returncode':run.returncode,'accepted':run.returncode==0,'stderr':run.stderr if run.returncode else ''}
                run = subprocess.run([str(binary),'parse',str(project/entry)],cwd=project,env=env,capture_output=True,text=True,timeout=30)
                row['rust_parse'] = {'returncode':run.returncode,'accepted':run.returncode==0,'stderr':run.stderr if run.returncode else ''}
                row['matches'] = all(row[key]['returncode'] in (0,1) and row[key]['accepted']==(row['parse_expected'] if key == 'rust_parse' else row['expected']) for key in ['official','rust','rust_parse'])
                if parse_expected != expected:
                    reference = json.loads(row['official']['stderr'])
                    titles = {problem['title'] for error in reference['errors'] for problem in error['problems']}
                    row['reference_passed_syntax'] = {'REDUNDANT PATTERN','MISSING PATTERNS'} <= titles
                    row['matches'] = row['matches'] and row['reference_passed_syntax']
                results.append(row)
assert hashlib.sha256(binary.read_bytes()).hexdigest() == sha
report = {'scope':__doc__,'compiler_sha256':sha,'cases':len(results),'failures':[r['case'] for r in results if not r['matches']],'results':results}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:report[k] for k in ['cases','failures']}))
raise SystemExit(bool(report['failures']))
