#!/usr/bin/env python3
"""Differential acceptance checks for expression inference, not code generation.

Positive fixtures are compiled by Elm 0.19.1. Negative fixtures must be rejected
for type errors by Elm as well as by the Rust inference pass. Pattern coverage and main validation have a separate differential suite.
Ports and effect-manager lifecycle validation have separate suites.
"""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile

POSITIVE = [
    'f x = g x\ng y = f [y]',
    'g y = f [y]\nf x = g x',
    'outer n = let f x = g x\n              g y = f [y]\n          in f n',
    'f n x = if n == 0 then 0 else g (n - 1) x\ng n y = f n [y]',
    'f : a -> b\nf x = g x\ng y = f [y]',
    'f x = g x\ng : a -> b\ng y = f [y]',
    'f : a -> b\nf x = g x\ng : a -> b\ng y = f [y]',
    'identity x = x\npair = (identity (), identity [])',
    'pair = (identity (), identity [])\nidentity x = x',
    'f a = let identity x = x in (identity a, identity [])',
    'f x = g x\ng y = f y',
    'f : a -> a\nf x = x',
    'f : a -> a\nf x =\n    let\n        g : a -> a\n        g y = x\n    in g x',
    'get row = row.value\npair = (get { value=() }, get { value=[] })',
    'f : { row | a : (), b : () } -> ()\nf r = let value = r.a in value',
    'f { value } = value',
    'f r = { r | value = () }\nx = f {value=(),other=[]}',
    'type Box a = Box a\nunbox (Box value) = value\nx = unbox (Box ())',
    'type alias Pair a = { right : a, left : () }\nx = Pair [] ()',
    'type alias N number = number\nx : N String\nx = "hello"',
    'type Tree a = Leaf a | Branch (List (Tree a))\nx = Branch [Leaf ()]',
    'f x = let (a,b) = (x,[]) in (a,b)',
    'f x = case x of\n    Just a -> [a]\n    Nothing -> []',
    'f : number -> number\nf x = x + 1',
    'f : comparable -> comparable -> Bool\nf x y = x < y',
    'f : appendable -> appendable\nf x = x ++ x',
    'f x = (x ++ x) < x',
    'x = 0xDEADBEEF\ny = 1e3\nz = -2.5',
    'f xs = case xs of\n    [] -> 0\n    h :: t -> h',
    'f xs = case xs of\n    (a :: b) as whole -> whole\n    [] -> []',
    'f : a -> a\nf x = let unused : b -> b\n          unused y = y\n      in x',
]
NEGATIVE = [
    'f : a -> a\nf x = ()',
    'f : a -> b\nf x = x',
    'f : a -> a\nf x = x + 1',
    'f : comparable -> comparable\nf x = x + 1',
    'f : appendable -> Bool\nf x = x < x',
    'f : a -> a\nf x =\n    let\n        g : a -> a\n        g y = ()\n    in g x',
    'f x = x x',
    'f x = (x (),x [])',
    'f x = let g y = x y in (g (), g [])',
    'f r = { r | value = () }\nx = f {}',
    'f r = { r | value = () }\nx = f {value=[]}',
    'get r = r.value\nx = get {}',
    'x = [(),[]]',
    'x = if () then 1 else 2',
    'x = if True then () else []',
    'x = (1,2) < (1,2,3)',
    'x = [{value=1}] < []',
    'x = True ++ False',
    'x = 1 ++ 2',
    'x = -"no"',
    'type Box a = Box a\nf (Box a) = a\nx = f ()',
    'type Box a = Box a\nx : Box Int\nx = Box "no"',
    'type alias Pair = { right : String, left : Int }\nx = Pair 1 "no"',
    'f x = case x of\n    Just a -> ()\n    Nothing -> []',
    'f xs = case xs of\n    [] -> ()\n    1 :: rest -> []\n    _ -> ()',
    'f x = let (a,b) = x in (a (), a [])',
    'f x = f [x]',
    'f : a -> b\nf x = f [x]',
    'f x = g [x]\ng y = f y',
    'z x = a x\na y = z [y]',
]
parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=pathlib.Path, required=True)
parser.add_argument('--report', type=pathlib.Path)
args = parser.parse_args()
elm = args.elm.resolve()
binary = pathlib.Path(__file__).resolve().parents[1] / 'target/release/planexpo-elm'
failures = []
with tempfile.TemporaryDirectory(prefix='elm-rust-inference-parity-') as directory:
    root = pathlib.Path(directory)
    (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    for index, (expected, body) in enumerate([(True,s) for s in POSITIVE]+[(False,s) for s in NEGATIVE]):
        module = f'Fixture{index}'
        source = root/f'{module}.elm'
        source.write_text(f'module {module} exposing (..)\n{body}\n')
        native = subprocess.run([str(binary),'check',str(root/'elm.json'),source.name],capture_output=True,text=True,timeout=30)
        reference = subprocess.run([str(elm),'make',source.name,'--output=/dev/null','--report=json'],cwd=root,capture_output=True,text=True,timeout=30,env={**os.environ,'GHCRTS':'-N1 -A16m -c'})
        titles=[]
        if reference.returncode:
            try: report=json.loads(reference.stderr)
            except ValueError: raise SystemExit(reference.stderr)
            titles=[p['title'] for e in report.get('errors',[]) for p in e['problems']]
        if (native.returncode==0)!=expected or (reference.returncode==0)!=expected:
            failures.append({'body':body,'expected':expected,'rust':native.returncode,'elm':reference.returncode,'rust_error':native.stderr,'elm_titles':titles})
        elif not expected and (not titles or not any(t in ['TYPE MISMATCH','INFINITE TYPE'] for t in titles)):
            failures.append({'body':body,'error':'reference rejection is not a proven type error','elm_titles':titles})
report = {'positive':len(POSITIVE),'negative':len(NEGATIVE),'failures':failures}
if args.report:
    args.report.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report,indent=2))
raise SystemExit(bool(failures))
