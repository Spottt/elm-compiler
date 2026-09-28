#!/usr/bin/env python3
"""Compare documentation comment extraction with actual Elm --docs output.

Build the docs_comments example separately. This checks extraction only, not a
Rust implementation of make --docs or declaration/type association.
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
cases=[
    ('plain',' Module docs.\n@docs value\n',' Value docs. '),
    ('unicode',' Éléments 😀.\n@docs value\n',' Café et λ. '),
    ('crlf',' Module\r\n@docs value\r\n',' First\r\nsecond\r\n'),
    ('carriage-returns',' Module\n@docs value\n',' a\rb\rc '),
    ('nested',' Module {- nested -}\n@docs value\n',' Value {- nested {- deeper -} -} docs. '),
    ('nested-doc-marker',' Module\n@docs value\n',' {-| inner is text -} '),
    ('quotes-backslashes',' A "quote" and \\slash.\n@docs value\n',' "quoted" \\ \\n '),
    ('empty-value-comment','\n@docs value\n',''),
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
    for index,(name,overview,comment) in enumerate(cases):
        project=root/str(index);(project/'src').mkdir(parents=True)
        config={'type':'package','name':'author/fixture','summary':'Documentation comment fixture.',
                'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
                'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
        (project/'elm.json').write_text(json.dumps(config))
        source='module Main exposing (value)\n{-|'+overview+'-}\n{- ordinary {-| ignored -} -}\n{-|'+comment+'-}\nvalue : String\nvalue = """{-| not documentation -}"""\n'
        path=project/'src/Main.elm';path.write_text(source)
        run=subprocess.run([str(args.elm.resolve()),'make','--docs=docs.json','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
        if run.returncode:
            failures.append(name+': official rejected fixture')
            results.append({'case':name,'stderr':run.stderr})
            continue
        docs=json.loads((project/'docs.json').read_text())[0]
        expected=[docs['comment'],docs['values'][0]['comment']]
        actual=json.loads(subprocess.check_output([str(extractor),str(path)],text=True))
        if actual!=expected:failures.append(name)
        results.append({'case':name,'expected':expected,'actual':actual})
report={'scope':__doc__,'cases':len(results),'failures':failures,'results':results,
        'extractor_sha256':hashlib.sha256(extractor.read_bytes()).hexdigest()}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
