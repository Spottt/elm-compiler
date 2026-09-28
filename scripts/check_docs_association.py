#!/usr/bin/env python3
"""Compare documentation association and placement with actual Elm --docs output.

Build the docs_comments example separately. This checks comment association and invalid placements, not type rendering,
@docs validation, or a Rust implementation of make --docs.
"""
import argparse
import hashlib
import json
import os
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
header = "module Main exposing (value)\n{-| Overview.\n@docs value\n-}\n"
value = "{-| Value. -}\nvalue : Int\nvalue = 1\n"
cases = [
    ('no-imports', header + value),
    ('imports', header + 'import String\n' + value),
    ('ordinary-comments', header + '{- ordinary {-| nested -} -}\n' + value),
    ('two-values', header.replace('(value)', '(value, other)').replace('@docs value', '@docs value, other') + value + '{-| Other. -}\nother : Int\nother = 2\n'),
    ('types', 'module Main exposing (Choice(..), Row)\n{-| Types.\n@docs Choice, Row\n-}\n{-| Choice. -}\ntype Choice = A | B\n{-| Row. -}\ntype alias Row = { x : Int }\n'),
    ('annotation-gap', header + 'value : Int\n{-| Wrong. -}\nvalue = 1\n'),
    ('expression-comment', header + '{-| Value. -}\nvalue : Int\nvalue =\n    {-| Wrong. -}\n    1\n'),
    ('trailing-comment', header + value + '{-| Trailing. -}\n'),
    ('duplicate-comment', header + '{-| Extra. -}\n' + value),
    ('indented-comment', header + '  ' + value),
    ('comment-before-import', header + '{-| Wrong. -}\nimport String\n' + value),
    ('comment-inside-import', header + 'import {-| Wrong. -} String\n' + value),
]
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
        config={'type':'package','name':'author/fixture','summary':'Documentation comment fixture.',
                'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
                'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
        (project/'elm.json').write_text(json.dumps(config))
        path=project/'src/Main.elm';path.write_text(source)
        run=subprocess.run([str(args.elm.resolve()),'make','--docs=docs.json','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
        inspected = subprocess.run([str(extractor),str(path),'--associated'],capture_output=True,text=True,timeout=30)
        expected = actual = None
        if run.returncode == 0:
            docs=json.loads((project/'docs.json').read_text())[0]
            expected={'overview':docs['comment'], 'declarations':{
                entry['name']:entry['comment'] for category in ['values','aliases','unions'] for entry in docs[category]
            }}
        if inspected.returncode == 0:
            actual=json.loads(inspected.stdout)
        if (run.returncode == 0) != (inspected.returncode == 0) or expected != actual:
            failures.append(name)
        results.append({'case':name,'official_accepted':run.returncode==0,'rust_accepted':inspected.returncode==0,
                        'expected':expected,'actual':actual,'official_stderr':run.stderr,'rust_stderr':inspected.stderr})
report={'scope':__doc__,'cases':len(results),'failures':failures,'results':results,
        'extractor_sha256':hashlib.sha256(extractor.read_bytes()).hexdigest()}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
