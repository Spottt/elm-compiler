#!/usr/bin/env python3
"""Differential syntax fixtures against the real Elm 0.19.1 compiler.

Positive fixtures must typecheck as well. Negative fixtures intentionally have
syntax errors; a type error alone is not accepted as evidence of syntax parity.
Each source gets a unique module name so no previous compiled output can mask a
failure. The temporary project and compiler invocations are sequential.
"""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile

POSITIVE = [
    'value = 42',
    'value = 0xFE',
    'value = 1.25e-2',
    'value = """first\nsecond"""',
    r"value = '\u{1F600}'",
    'f x = x + 2 * 3\nvalue = f -1',
    'f x = x.a\nvalue = f { a = 1 }',
    'value = List.map .a [ { a = 1 }, { a = 2 } ]',
    'value = (\\(x,y) -> x + y) (1,2)',
    'value = if True then 1 else if False then 2 else 3',
    'value =\n    let\n        f : Int -> Int\n        f x = x + 1\n        ( a, b ) = ( 1, 2 )\n    in\n    f (a + b)',
    'f xs =\n    case xs of\n        [] -> []\n        h :: t as whole -> whole\nvalue = f [1,2]',
    'type Choice a = One a | None\nf c =\n    case c of\n        One x -> x\n        None -> 0\nvalue = f (One 1)',
    'type alias Row r = { r | a : Int }\nf : Row r -> Int\nf row = row.a\nvalue = f {a=1,b=2}',
    'f model = { model | a = model.a + 1 }\nvalue = f {a=0}',
    'value = List.map ((+) 2) [1,2,3]',
    'value = identity <| if True then 1 else 2',
    'value = identity <| let x = 1 in x',
    'value = case (1,2) of\n    ( a, b ) -> a + b',
    'value = { a = [1,2], b = (True, "yes") }',
]
NEGATIVE = [
    'value =', 'value = [1,]', 'value = (1,)', 'value = { a = }',
    'value = if True then 1', 'value = let x = 1 in',
    'value = case 1 of', 'value = \\ -> 1', 'type alias X =',
    'value =\n1', 'value = - 1', 'value = . a', 'f _named = 1',
    'f 1.5 = 1', 'value : Int', 'value : Int\nother = 1',
    'value = 01', 'value = 1.', 'value = 1e+', 'value = 0x',
    'value = "\\q"', "value = 'ab'", 'value = {- unclosed',
    'value = let\n x = 1\n  y = 2\n in x',
]
HEADER_NEGATIVE = [
    'module {module} exposing ()\n',
    'module {module} exposing (value,)\nvalue = 1\n',
    'module {module} exposing (T(C))\ntype T = C\n',
    'module {module} exposing ((=))\nvalue = 1\n',
]
# Parse.Module.exposing checks indentation after each item/delimiter, while
# parenthesized operator names require byte adjacency (no spaces or comments).
HEADER_POSITIVE = [
    'module {module} exposing (\n    value\n    )\nvalue = 1\n',
    'module {module} exposing (value\n    , other)\nvalue = 1\nother = 2\n',
    'module {module} exposing (T (\n    ..\n    ))\ntype T = C\n',
    'module {module} exposing (\n    ..\n    )\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing ((+))\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing (\n    identity\n    )\nvalue = 1\n',
]
HEADER_NEGATIVE.extend([
    'module {module} exposing (\nvalue)\nvalue = 1\n',
    'module {module} exposing (value\n)\nvalue = 1\n',
    'module {module} exposing (value,\nother)\nvalue = 1\nother = 2\n',
    'module {module} exposing (value\n, other)\nvalue = 1\nother = 2\n',
    'module {module} exposing (\n..)\nvalue = 1\n',
    'module {module} exposing (..\n)\nvalue = 1\n',
    'module {module} exposing (T\n(..))\ntype T = C\n',
    'module {module} exposing (T(\n..))\ntype T = C\n',
    'module {module} exposing (T(..\n))\ntype T = C\n',
    'module {module} exposing (T(..)\n)\ntype T = C\n',
    'module {module} exposing (..)\nimport Basics exposing (\nidentity)\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing (( +))\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing ((+ ))\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing (({{- comment -}}+))\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing ((+{{- comment -}}))\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing ((\n    +))\nvalue = 1\n',
])
HEADER_POSITIVE.extend([
    'module {module} exposing (..)\nimport\n    Basics\n    as\n    B\n    exposing\n    (..)\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics as B\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing (..)\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics -- comment\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics\n{{- comment -}}\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics as B\n    exposing (..)\nvalue = 1\n',
])
HEADER_NEGATIVE.extend([
    'module {module} exposing (..)\nimport\nBasics\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics\nas B\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics as\nB\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics\nexposing (..)\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics as B\nexposing (..)\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics exposing\n(..)\nvalue = 1\n',
    'module {module} exposing (..)\nimport\n\rBasics\nvalue = 1\n',
    'module {module} exposing (..)\nimport Basics',
    'module {module} exposing (..)\nimport Basics as B',
    'module {module} exposing (..)\nimport Basics exposing (..)',
    'module {module} exposing (..)\nimport Basics -- comment',
    'module {module} exposing (..)\nimport Basics\n  ',
    'module {module} exposing (..)\nimport Basics\n{{- comment -}}',
])
HEADER_NEGATIVE.extend(['module {module} exposing (..)\n', 'module {module} exposing (..)\nimport Basics\n', 'module {module} exposing (..)\n{{- comment -}}\n'])
# Local destructuring uses Pattern.term, whereas case branches use Pattern.expression.
for pattern in ['Box x', 'Box _', '(x,y) as pair', '{x} as record', '() as unit', '_ :: _']:
    NEGATIVE.append('type Box = Box Int\nvalue =\n    let\n        '+pattern+' = Box 1\n    in 1')
for pattern, expression, result in [
    ('(Box x)', 'Box 1', 'x'),
    ('((Box x) as box)', 'Box 1', 'x'),
    ('((x,y) as pair)', '(1,2)', 'x'),
    ('({x} as record)', '{x=1}', 'x'),
    ('(() as unit)', '()', 'unit'),
    ('Box (Box x)', 'Box (Box 1)', 'x'),
]:
    if pattern == 'Box (Box x)':
        POSITIVE.append('type Box a = Box a\nvalue = case '+expression+' of\n    '+pattern+' -> '+result)
    else:
        POSITIVE.append('type Box a = Box a\nvalue =\n    let\n        '+pattern+' = '+expression+'\n    in '+result)
# Control expressions terminate an operator chain; outer operators require parentheses.
for prefix in ['', '1 + ', 'if True then 1 else ', 'let x = 1 in ', '\\x -> ']:
    NEGATIVE.append('value = '+prefix+'case True of\n    _ -> 2\n  + 3')
POSITIVE.extend([
    'value = (case True of\n    _ -> 2\n  ) + 3',
    'value = 1 + (case True of\n    _ -> 2\n  ) + 3',
    'value = 1 + case True of\n    _ -> 2 + 3',
    'value = (if True then 1 else 2) + 3',
    'value = (let x = 1 in x) + 3',
])
parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=pathlib.Path, required=True)
args = parser.parse_args()
binary = pathlib.Path(__file__).resolve().parents[1] / 'target/release/planexpo-elm'
elm = args.elm.resolve()
failures = []
with tempfile.TemporaryDirectory(prefix='elm-rust-syntax-parity-') as tmp:
    root = pathlib.Path(tmp)
    (root/'elm.json').write_text(json.dumps({
        'type':'application', 'source-directories':['.'], 'elm-version':'0.19.1',
        'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},
        'test-dependencies':{'direct':{},'indirect':{}},
    }))
    fixtures = [(True,b,True) for b in HEADER_POSITIVE]+[(True,b,False) for b in POSITIVE]+[(False,b,False) for b in NEGATIVE]+[(False,b,True) for b in HEADER_NEGATIVE]
    for i, (expected, body, is_header) in enumerate(fixtures):
        module = f'Fixture{i}'
        path = root/f'{module}.elm'
        path.write_text(body.format(module=module) if is_header else f'module {module} exposing (..)\n{body}\n')
        native = subprocess.run([str(binary),'parse',str(path)], capture_output=True,text=True,timeout=30)
        reference = subprocess.run([str(elm),'make',str(path),'--output=/dev/null','--report=json'],
                                   cwd=root,capture_output=True,text=True,timeout=30,
                                   env={**os.environ,'GHCRTS':'-N1 -A16m -c'})
        if reference.returncode not in (0,1):
            raise SystemExit(f'Elm failed unexpectedly: {reference.stderr}')
        titles=[]
        if reference.returncode:
            try:
                report=json.loads(reference.stderr)
                titles=[p['title'] for e in report.get('errors',[]) for p in e['problems']]
            except ValueError:
                raise SystemExit(f'Elm did not return diagnostics: {reference.stderr}')
        if (native.returncode==0)!=expected or (reference.returncode==0)!=expected:
            failures.append({'body':body,'expected':expected,'rust':native.returncode,'elm':reference.returncode,'rust_error':native.stderr,'elm_titles':titles})
        elif not expected and (not titles or any(t in ['TYPE MISMATCH','NAMING ERROR','MISSING PATTERNS'] for t in titles)):
            failures.append({'body':body,'reason':'reference rejection is not proven to be syntax','elm_titles':titles})
print(json.dumps({'positive':len(POSITIVE)+len(HEADER_POSITIVE),'negative':len(NEGATIVE)+len(HEADER_NEGATIVE),'failures':failures},indent=2))
raise SystemExit(bool(failures))
