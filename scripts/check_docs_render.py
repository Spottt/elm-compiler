#!/usr/bin/env python3
"""Compare complete module documentation JSON with Elm 0.19.1.

By default, build --release --example docs_project first. With --public-cli,
test the public make --docs command instead. Both exercise real project name
resolution and type checks. Comparisons use decoded JSON, not file bytes.
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
p.add_argument('--report', type=Path)
p.add_argument('--public-cli', action='store_true')
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
extractor = crate / ('target/release/planexpo-elm' if args.public_cli else 'target/release/examples/docs_project')
original = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm'))) / '0.19.1/packages'

def alias(parameters, tipe, imports=''):
    return f'module Main exposing (Shape)\n{{-| Shape docs.\n@docs Shape\n-}}\n{imports}{{-| Definition. -}}\ntype alias Shape {parameters} = {tipe}\n'

cases = [(name, {'Main': alias(params, tipe, imports)}, ['Main']) for name, params, tipe, imports in [
    ('unit', '', '()', ''),
    ('empty-record', '', '{}', ''),
    ('int', '', 'Int', ''),
    ('float', '', 'Float', ''),
    ('string', '', 'String', ''),
    ('bool', '', 'Bool', ''),
    ('char', '', 'Char', ''),
    ('nested-list', 'a', 'List (List a)', ''),
    ('nested-maybe-result', 'a b', 'Maybe (Result a b)', ''),
    ('function-argument', 'a b', '(a -> b) -> a -> b', ''),
    ('function-type-argument', 'a b', 'List (a -> b)', ''),
    ('tuple', 'a', '(a, List a)', ''),
    ('triple', 'a b', '(a -> b, List (a, b), ())', ''),
    ('record-order', 'a b', '{ z : a, a : b }', ''),
    ('extensible-record', 'r a', '{ r | z : List a, a : a -> a }', ''),
    ('nested-record', 'a', '{ outer : { value : a }, fn : a -> a }', ''),
    ('qualified-import-alias', 'a', 'D.Dict String a', 'import Dict as D\n'),
    ('exposed-import', 'a', 'Array a', 'import Array exposing (Array)\n'),
    ('core-alias', 'msg', 'Cmd msg', ''),
]]
cases.extend([
    ('local-alias', {'Main': 'module Main exposing (Box, identity)\n{-| @docs Box, identity -}\n{-| Box. -}\ntype alias Box a = { value : a }\n{-| Identity. -}\nidentity : Box a -> Box a\nidentity x = x\n'}, ['Main']),
    ('union-cases', {'Main': 'module Main exposing (Choice(..), Closed)\n{-| @docs Choice, Closed -}\n{-| Choice. -}\ntype Choice a = Nothing | Just a | Apply (a -> a) | Pair a a\n{-| Closed. -}\ntype Closed = Secret Int\n'}, ['Main']),
    ('foreign-alias', {'Main': alias('a', 'H.Box a', 'import Helper as H\n'), 'Helper': 'module Helper exposing (Box)\ntype alias Box a = { value : a }\n'}, ['Main']),
    ('operator', {'Main': 'module Main exposing ((<+>))\n{-| @docs (<+>) -}\ninfix right 7 (<+>) = combine\n{-| Adds. -}\ncombine : Int -> Int -> Int\ncombine a b = a + b\n'}, ['Main']),
    ('multiple-modules', {'Main': alias('', 'Int'), 'Other': alias('', 'String').replace('module Main', 'module Other')}, ['Other', 'Main']),
])
results, failures = [], []
with tempfile.TemporaryDirectory(prefix='elm-doc-render-') as tmp:
    root = Path(tmp)
    cache = root/'home/0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat', cache/'registry.dat')
    shutil.copytree(original/'elm/core/1.0.5/src', cache/'elm/core/1.0.5/src')
    shutil.copyfile(original/'elm/core/1.0.5/elm.json', cache/'elm/core/1.0.5/elm.json')
    env = {**os.environ, 'ELM_HOME': str(root/'home'), 'GHCRTS': '-N1 -A16m -c'}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    env.update(no_proxy='', NO_PROXY='')
    for index, (name, modules, exposed) in enumerate(cases):
        project = root/str(index)
        (project/'src').mkdir(parents=True)
        config = {'type':'package','name':'elm/fixture','summary':'Documentation rendering fixture.',
                  'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':exposed,
                  'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
        (project/'elm.json').write_text(json.dumps(config))
        for module, source in modules.items():
            (project/'src'/f'{module}.elm').write_text(source)
        official = subprocess.run([str(args.elm.resolve()),'make','--docs=docs.json','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
        rust = subprocess.run(([str(extractor),'make','--docs=rust-docs.json','--report=json'] if args.public_cli else [str(extractor),*[f'src/{module}.elm' for module in exposed]]),cwd=project,env=env,capture_output=True,text=True,timeout=30)
        expected = json.loads((project/'docs.json').read_text()) if official.returncode == 0 else None
        actual = json.loads((project/'rust-docs.json').read_text() if args.public_cli else rust.stdout) if rust.returncode == 0 else None
        if official.returncode or rust.returncode or expected != actual:
            failures.append(name)
        results.append({'case':name,'official_returncode':official.returncode,'rust_returncode':rust.returncode,
                        'expected':expected,'actual':actual,'official_stderr':official.stderr,'rust_stderr':rust.stderr})
report = {'public_cli':args.public_cli, 'scope':__doc__,'cases':len(results),'failures':failures,'results':results,
          'extractor_sha256':hashlib.sha256(extractor.read_bytes()).hexdigest()}
if args.report:
    args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
