#!/usr/bin/env python3
"""Compare pattern coverage, main validation and module boundaries with Elm."""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile

POSITIVE = ['type Bit = Zero | One\n'
 'f x = case x of\n'
 '    (Zero,_) -> ()\n'
 '    (_,Zero) -> ()\n'
 '    (One,One) -> ()',
 'type Box a = Box a\nf (Box a) = a',
 'f xs = case xs of\n    [] -> ()\n    a :: rest -> ()',
 'f {field} = field',
 'f value = let (x,y) = value in (x,y)',
 'f = \\() -> ()',
 'main : Program () () Never\n'
 'main = Platform.worker { init = \\flags -> ((), Cmd.none), update = \\msg model -> (model, '
 'Cmd.none), subscriptions = \\model -> Sub.none }',
 'main : Program {items : List Int, choice : Maybe String} () Never\n'
 'main = Platform.worker { init = \\flags -> ((), Cmd.none), update = \\msg model -> (model, '
 'Cmd.none), subscriptions = \\model -> Sub.none }',
 'type alias Row a = { a | value : Int }\n'
 'main : Program (Row {}) () Never\n'
 'main = Platform.worker { init = \\flags -> ((), Cmd.none), update = \\msg model -> (model, '
 'Cmd.none), subscriptions = \\model -> Sub.none }',
 "f x = case x of\n    'a' -> ()\n    '\\u{0061}' -> ()\n    _ -> ()",
 'f x = case x of\n    "a" -> ()\n    "\\u{0061}" -> ()\n    _ -> ()']
NEGATIVE = ['type Bit = Zero | One\nf x = case x of\n    (Zero,_) -> ()\n    (One,One) -> ()',
 'type Bit = Zero | One\nf x = case x of\n    (Zero,_) -> ()\n    (One,_) -> ()\n    (_,One) -> ()',
 'type Bit = Zero | One\nf Zero = ()',
 'f = \\[a] -> a',
 'f xs = let [a] = xs in a',
 'f xs = case xs of\n    [] -> ()\n    [a] -> ()',
 'f x = case x of\n    1 -> ()\n    2 -> ()',
 'f x = case x of\n    10 -> ()\n    0x0A -> ()\n    _ -> ()',
 'main = 42',
 'main x = x',
 'main : Program (Int -> Int) () Never\n'
 'main = Platform.worker { init = \\flags -> ((), Cmd.none), update = \\msg model -> (model, '
 'Cmd.none), subscriptions = \\model -> Sub.none }',
 'main : Program { r | value : Int } () Never\n'
 'main = Platform.worker { init = \\flags -> ((), Cmd.none), update = \\msg model -> (model, '
 'Cmd.none), subscriptions = \\model -> Sub.none }',
 'type Custom = Custom\n'
 'main : Program Custom () Never\n'
 'main = Platform.worker { init = \\flags -> ((), Cmd.none), update = \\msg model -> (model, '
 'Cmd.none), subscriptions = \\model -> Sub.none }',
 'main : Program Char () Never\n'
 'main = Platform.worker { init = \\flags -> ((), Cmd.none), update = \\msg model -> (model, '
 'Cmd.none), subscriptions = \\model -> Sub.none }',
 "f x = case x of\n    '\\u{000061}' -> ()\n    '\\u{0061}' -> ()\n    _ -> ()",
 'f x = case x of\n    "\\u{000061}" -> ()\n    "\\u{0061}" -> ()\n    _ -> ()']
parser = argparse.ArgumentParser()
POSITIVE += [
 'f z =\n    let\n        x a = y a\n        y a = x a\n    in\n    z',
 'f z =\n    let\n        x = y\n        y = z\n    in\n    x',
]
NEGATIVE += [
 'infix left 4 (|=) = combine\ncombine a b = a',
 'f z =\n    let\n        x = y\n        y = x\n    in\n    z',
 'f z =\n    let\n        x = g ()\n        g _ = if True then z else x\n    in\n    x',
 'f z =\n    let\n        x = g\n        g y = if True then z else x y\n    in\n    x ()',
 'f z =\n    let\n        (x,y) = (y,x)\n    in\n    z',
 'f z =\n    let\n        x = \\_ -> x\n    in\n    z',
]
parser.add_argument('--elm', type=pathlib.Path, required=True)
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
        if reference.returncode not in (0,1) and os.environ.get('ELM_REFERENCE_RELEASE'):
            print(f'SKIPPED, the reference compiler crashed: {body!r}', flush=True)
            continue
        if reference.returncode:
            try: report=json.loads(reference.stderr)
            except ValueError: raise SystemExit(reference.stderr)
            titles=[p['title'] for e in report.get('errors',[]) for p in e['problems']]
        if (native.returncode==0)!=expected or (reference.returncode==0)!=expected:
            failures.append({'body':body,'expected':expected,'rust':native.returncode,'elm':reference.returncode,'rust_error':native.stderr,'elm_titles':titles})
        elif not expected and (not titles or not any(t in (
            ['UNFINISHED PARENTHESES'] if body.startswith('infix ') else
            ['MISSING PATTERNS','UNSAFE PATTERN','REDUNDANT PATTERN','BAD MAIN TYPE','BAD FLAGS','CYCLIC VALUE']
        ) for t in titles)):
            failures.append({'body':body,'error':'reference rejection is not the expected validation phase','elm_titles':titles})
print(json.dumps({'positive':len(POSITIVE),'negative':len(NEGATIVE),'failures':failures},indent=2))
raise SystemExit(bool(failures))
