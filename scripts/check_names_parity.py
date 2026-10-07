#!/usr/bin/env python3
"""Name-resolution acceptance parity against Elm, in an isolated project.

This does not assert inferred type equivalence. Every negative case targets name
resolution/canonicalization; positive cases also compile with the reference.
"""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile

POSITIVE = [
    'x = 1',
    'f x = let walk y = walk y in walk x',
    'f xs = case xs of\n    [] -> []\n    h :: t -> t',
    'import A as X\nx = X.value',
    'import A exposing (T(..), Row)\nx = One (Row 1)',
    'import A exposing (value)\nf value = value',
    'import A exposing (value)\nvalue = 2',
    'type Tree a = Node (List (Tree a)) | Leaf a',
    'type alias Row r = { r | x : Int }\nf : Row r -> Int\nf r = r.x',
    'f (a,b) = let (c,d) = (a,b) in (c,d)',
    'f x = case x of\n    Just y -> (\\z -> y) y\n    Nothing -> 0',
    'import A exposing (..)\nimport B exposing (..)\nx = 1',
]
NEGATIVE = [
    'x = missing',
    'f x = \\x -> x',
    'f x x = x',
    'f (x,x) = x',
    'f x = let f y = y in f x',
    'f x = { a=x, a=x }',
    'x = 1\nx = 2',
    'import A as X\nx = A.value',
    'import A\nx = A.private',
    'import A exposing (private)\nx = 1',
    'import A exposing (value)\nimport B exposing (value)\nx = value',
    'import A exposing (..)\nimport B exposing (..)\nx = value',
    'import Opaque\nx = Opaque.Secret',
    'import A exposing (Row)\nf (Row x) = x',
    'import A exposing (Row(..))\nx = 1',
    'type T = One Int\nf One = 1',
    'type T = One Int\nf (One x y) = x',
    'type alias A = List',
    'type alias A = Missing',
    'type alias A = { x : missing }',
    'type alias A a = Int',
    'type alias A = List B\ntype alias B = A',
    'type alias A = A',
    'type alias A = { x:Int, x:Int }',
    'x = (1,2,3,4)',
    'x = Elm.Kernel.Bytes.width',
]
HEADERS = [
    'module {module} exposing (missing)\nx = 1',
    'module {module} exposing (Missing)\nx = 1',
    'module {module} exposing (x,x)\nx = 1',
    'module {module} exposing (A(..))\ntype alias A = Int',
]
p = argparse.ArgumentParser()
p.add_argument('--elm',type=pathlib.Path,required=True)
a = p.parse_args()
binary = pathlib.Path(__file__).resolve().parents[1]/'target/release/planexpo-elm'
elm = a.elm.resolve()
failures=[]
with tempfile.TemporaryDirectory(prefix='elm-rust-names-parity-') as directory:
    root=pathlib.Path(directory)
    (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    (root/'A.elm').write_text('module A exposing (value, T(..), Row)\nvalue = 1\nprivate = 0\ntype T = One Row\ntype alias Row = { x : Int }\n')
    (root/'B.elm').write_text('module B exposing (value)\nvalue = 2\n')
    (root/'Opaque.elm').write_text('module Opaque exposing (Secret)\ntype Secret = Secret\n')
    cases=[(True,s,False) for s in POSITIVE]+[(False,s,False) for s in NEGATIVE]+[(False,s,True) for s in HEADERS]
    for i,(expected,body,is_header) in enumerate(cases):
        module=f'Fixture{i}'
        source=body.format(module=module) if is_header else f'module {module} exposing (..)\n{body}\n'
        path=root/f'{module}.elm';path.write_text(source)
        native=subprocess.run([str(binary),'names',str(root/'elm.json'),path.name],capture_output=True,text=True,timeout=30)
        reference=subprocess.run([str(elm),'make',path.name,'--output=/dev/null','--report=json'],cwd=root,capture_output=True,text=True,timeout=30,env={**os.environ,'GHCRTS':'-N1 -A16m -c'})
        if reference.returncode not in (0,1):
            if os.environ.get('ELM_REFERENCE_RELEASE'):
                print(f'SKIPPED, the reference compiler crashed: {body!r}', flush=True)
                continue
            raise SystemExit(reference.stderr)
        titles=[]
        if reference.returncode:
            try: report=json.loads(reference.stderr)
            except ValueError: raise SystemExit(reference.stderr)
            titles=[p['title'] for e in report.get('errors',[]) for p in e['problems']]
        if (native.returncode==0)!=expected or (reference.returncode==0)!=expected:
            failures.append({'body':body,'expected':expected,'rust':native.returncode,'elm':reference.returncode,'rust_error':native.stderr,'elm_titles':titles})
        elif not expected and (not titles or 'TYPE MISMATCH' in titles):
            failures.append({'body':body,'error':'reference rejection is not a proven canonicalization failure','elm_titles':titles})
print(json.dumps({'positive':len(POSITIVE),'negative':len(NEGATIVE)+len(HEADERS),'failures':failures},indent=2))
raise SystemExit(bool(failures))
