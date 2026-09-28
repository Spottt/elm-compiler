#!/usr/bin/env python3
"""Compare documentation declaration requirements with actual Elm --docs output.

Build the docs_comments example separately. This checks required annotations/comments and exported comment selection,
not type rendering, complete diagnostic reports, or a Rust make --docs command.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tempfile

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm',type=Path,required=True)
p.add_argument('--report',type=Path)
args=p.parse_args()
crate=Path(__file__).resolve().parents[1]
extractor=crate/'target/release/examples/docs_comments'
original=Path(os.environ.get('ELM_HOME',str(Path.home()/'.elm')))/'0.19.1/packages'
def module(exports, body, overview=None):
    if overview is None: overview = '@docs ' + exports.replace('(..)', '')
    return f"module Main exposing ({exports})\n{{-|{overview}-}}\nimport String\n" + body

cases = []
for annotation in [False, True]:
    for comment in [False, True]:
        body = ('{-| Value. -}\n' if comment else '') + ('value : Int\n' if annotation else '') + 'value = 1\n'
        cases.append((f'value-annotation-{annotation}-comment-{comment}', module('value', body)))
for kind, exports, declaration in [
    ('alias', 'Row', 'type alias Row = { x : Int }\n'),
    ('union-open', 'Choice(..)', 'type Choice = A | B\n'),
    ('union-closed', 'Choice', 'type Choice = A | B\n'),
]:
    for comment in [False, True]:
        cases.append((f'{kind}-comment-{comment}', module(exports, ('{-| Type. -}\n' if comment else '') + declaration)))
for annotation in [False, True]:
    for comment in [False, True]:
        body = 'infix left 5 (<+>) = combine\n' + ('{-| Adds. -}\n' if comment else '') + ('combine : Int -> Int -> Int\n' if annotation else '') + 'combine a b = a + b\n'
        cases.append((f'operator-annotation-{annotation}-comment-{comment}', module('(<+>)', body)))
valid = '{-| Value. -}\nvalue : Int\nvalue = 1\n'
cases.extend([
    ('empty-comment', module('value', valid.replace('{-| Value. -}', '{-|-}'))),
    ('private-value', module('value', valid + 'private = 2\n')),
    ('private-types', module('value', valid + 'type Hidden = Hidden\ntype alias Internal = Int\n')),
    ('private-documentation', module('value', valid + '{-| Private. -}\nprivate = 2\n')),
    ('multiple-errors', module('alpha, beta, gamma', 'alpha = 1\nbeta : Int\nbeta = 2\ngamma = 3\n')),
    ('names-before-definitions', module('value', 'value = 1\n', '@docs unknown')),
    ('operator-and-function', module('(<+>), combine', 'infix left 5 (<+>) = combine\n{-| Adds. -}\ncombine : Int -> Int -> Int\ncombine a b = a + b\n')),
    ('operator-and-function-no-annotation', module('(<+>), combine', 'infix left 5 (<+>) = combine\ncombine a b = a + b\n')),
    ('unused-private-operator', module('value', 'infix left 5 (<+>) = combine\ncombine a b = a + b\n' + valid)),
])
results,failures=[],[]
with tempfile.TemporaryDirectory(prefix='elm-doc-comments-') as tmp:
    root=Path(tmp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat',cache/'registry.dat')
    shutil.copytree(original/'elm/core/1.0.5/src',cache/'elm/core/1.0.5/src')
    shutil.copyfile(original/'elm/core/1.0.5/elm.json',cache/'elm/core/1.0.5/elm.json')
    env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1 -A16m -c'}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:env[key]='http://127.0.0.1:9'
    env.update(no_proxy='',NO_PROXY='')
    for index,(name,source) in enumerate(cases):
        project=root/str(index);(project/'src').mkdir(parents=True)
        config={'type':'package','name':'elm/fixture','summary':'Documentation comment fixture.',
                'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
                'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
        (project/'elm.json').write_text(json.dumps(config))
        path=project/'src/Main.elm';path.write_text(source)
        run=subprocess.run([str(args.elm.resolve()),'make','--docs=docs.json','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
        inspected = subprocess.run([str(extractor),str(path),'--validate-definitions'],capture_output=True,text=True,timeout=30)
        expected = actual = None
        if run.returncode == 0:
            docs=json.loads((project/'docs.json').read_text())[0]
            expected={'overview':docs['comment'], 'declarations':{
                entry['name']:entry['comment'] for category in ['values','aliases','unions','binops'] for entry in docs[category]
            }}
        if inspected.returncode == 0:
            actual=json.loads(inspected.stdout)
        if (run.returncode == 0) != (inspected.returncode == 0) or expected != actual:
            failures.append(name)
        official_titles = []
        if run.returncode:
            error = json.loads(run.stderr)
            official_titles = [problem['title'] for entry in error.get('errors', []) for problem in entry['problems']]
        rust_titles = [{'NoAnnotation':'NO TYPE ANNOTATION', 'NoComment':'NO DOCS'}[kind] for kind in re.findall(r'NoAnnotation|NoComment', inspected.stderr)]
        if rust_titles and rust_titles != official_titles:
            failures.append(name + ': diagnostic categories')
        results.append({'official_titles':official_titles,'rust_definition_titles':rust_titles,'case':name,'official_accepted':run.returncode==0,'rust_accepted':inspected.returncode==0,
                        'expected':expected,'actual':actual,'official_stderr':run.stderr,'rust_stderr':inspected.stderr})
report={'scope':__doc__,'cases':len(results),'failures':failures,'results':results,
        'extractor_sha256':hashlib.sha256(extractor.read_bytes()).hexdigest()}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
