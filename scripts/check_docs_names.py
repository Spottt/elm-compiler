#!/usr/bin/env python3
"""Compare @docs syntax and export-list validation with actual Elm --docs output.

Build the docs_comments example separately. This checks list syntax and name validation,
not type rendering, complete diagnostic reports, or a Rust make --docs command.
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
def module(overview, exposing='value', declaration=None):
    if declaration is None:
        declaration = "{-| Value. -}\nvalue : Int\nvalue = 1\n"
    return f"module Main exposing ({exposing})\n{{-|{overview}-}}\n" + declaration

cases = [(name, module(overview)) for name, overview in [
    ('simple', '@docs value'),
    ('inline', 'Text @docs value more prose.'),
    ('code-span', '`@docs value`'),
    ('inline-without-left-boundary', 'prefix@docs value'),
    ('no-space-after-marker', '@docs(value)\n@docs value'),
    ('missing-name', '@docs'),
    ('missing-all', 'No names.'),
    ('extra-name', '@docs value, extra'),
    ('duplicate-name', '@docs value, value'),
    ('duplicate-marker', '@docs value\n@docs value'),
    ('unknown-duplicate', '@docs value, extra, extra'),
    ('trailing-comma', '@docs value,'),
    ('reserved', '@docs if'),
    ('underscore', '@docs _value'),
    ('qualified', '@docs Main.value'),
    ('suffix-underscore', '@docs_ ignored\n@docs value'),
    ('suffix-letter', '@docsValue ignored\n@docs value'),
    ('suffix-number', '@docs2 ignored\n@docs value'),
    ('suffix-unicode', '@docsé ignored\n@docs value'),
    ('suffix-nonascii-digit', '@docs² ignored\n@docs value'),
    ('block-comment-tab', '@docs {- tab\there -} value'),
    ('line-comment-tab', '@docs -- tab\there\nvalue'),
    ('suffix-apostrophe', "@docs' ignored\n@docs value"),
    ('comma-column-one', '@docs value\n, ignored'),
    ('comma-indented', '@docs value\n , ignored'),
    ('spaces-comments', '@docs {- spacing {- nested -} -} value'),
    ('line-comment', '@docs -- spacing\nvalue'),
    ('nested-doc-spacing', '@docs {-| spacing -} value'),
    ('tab-before-name', '@docs\tvalue'),
    ('tab-after-name', '@docs value\t'),
    ('tab-in-prose', 'Some\tprose\n@docs value'),
    ('crlf', '@docs\r\nvalue\r\n'),
    ('operator-extra', '@docs value, (|>)'),
    ('operator-dotdot', '@docs value, (..)'),
    ('operator-pipe', '@docs value, (|)'),
    ('operator-space', '@docs value, (+ )'),
    ('empty-list', '@docs ,'),
    ('punctuation-after-name', '@docs value. prose'),
    ('comma-then-linebreak', '@docs value,\nextra'),
]]
cases.extend([
    ('implicit-exports', module('@docs value', '..')),
    ('missing-overview', 'module Main exposing (value)\nimport String\n{-| Value. -}\nvalue : Int\nvalue = 1\n'),
    ('two-values', module('@docs other\n@docs value', 'value, other', '{-| Value. -}\nvalue : Int\nvalue = 1\n{-| Other. -}\nother : Int\nother = 2\n')),
    ('unicode-name', module('@docs café', 'café', '{-| Café. -}\ncafé : Int\ncafé = 1\n')),
    ('union-open', module('@docs Choice', 'Choice(..)', '{-| Choice. -}\ntype Choice = A | B\n')),
    ('union-closed', module('@docs Choice', 'Choice', '{-| Choice. -}\ntype Choice = A | B\n')),
    ('constructor-is-not-doc-name', module('@docs Choice, A', 'Choice(..)', '{-| Choice. -}\ntype Choice = A | B\n')),
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
        config={'type':'package','name':'author/fixture','summary':'Documentation comment fixture.',
                'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
                'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
        (project/'elm.json').write_text(json.dumps(config))
        path=project/'src/Main.elm';path.write_text(source)
        run=subprocess.run([str(args.elm.resolve()),'make','--docs=docs.json','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
        inspected = subprocess.run([str(extractor),str(path),'--validate-names'],capture_output=True,text=True,timeout=30)
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
