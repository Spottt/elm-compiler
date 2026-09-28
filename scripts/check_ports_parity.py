#!/usr/bin/env python3
"""Compare port signatures and serialization restrictions with the official Elm compiler."""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile

POSITIVE = ['port send : String -> Cmd msg',
 'port receive : (Int -> msg) -> Sub msg',
 'import Json.Encode exposing (Value)\nport send : Value -> Cmd msg',
 'import Array exposing (Array)\nport send : Array (Maybe (List Bool)) -> Cmd msg',
 'type alias Data = { name : String, count : Int }\nport send : Data -> Cmd msg',
 'port send : ((), Float, Bool) -> Cmd msg',
 'port send : String -> Cmd number',
 'port receive : (String -> number) -> Sub number']
NEGATIVE = ['port send : Cmd msg',
 'port send : Int -> String -> Cmd msg',
 'port send : String -> Cmd Int',
 'port receive : (String -> a) -> Sub b',
 'port receive : String -> Sub msg',
 'port receive : (String -> Int) -> Sub Int',
 'port send : String -> msg',
 'port send : Char -> Cmd msg',
 'port send : a -> Cmd msg',
 'port send : (Int -> Int) -> Cmd msg',
 'port send : { r | value : Int } -> Cmd msg',
 'type Custom = Custom\nport send : Custom -> Cmd msg',
 'port send : List (Maybe Char) -> Cmd msg',
 'type alias Row a = { a | value : Int }\nport send : Row {} -> Cmd msg']
parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=pathlib.Path, required=True)
args = parser.parse_args()
elm = args.elm.resolve()
binary = pathlib.Path(__file__).resolve().parents[1] / 'target/release/planexpo-elm'
failures = []
with tempfile.TemporaryDirectory(prefix='elm-rust-ports-parity-') as directory:
    root = pathlib.Path(directory)
    (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    for index, (expected, body) in enumerate([(True,s) for s in POSITIVE]+[(False,s) for s in NEGATIVE]):
        module = f'Fixture{index}'
        source = root/f'{module}.elm'
        source.write_text(f'port module {module} exposing (..)\n{body}\n')
        native = subprocess.run([str(binary),'check',str(root/'elm.json'),source.name],capture_output=True,text=True,timeout=30)
        reference = subprocess.run([str(elm),'make',source.name,'--output=/dev/null','--report=json'],cwd=root,capture_output=True,text=True,timeout=30,env={**os.environ,'GHCRTS':'-N1 -A16m -c'})
        titles=[]
        if reference.returncode:
            try: report=json.loads(reference.stderr)
            except ValueError: raise SystemExit(reference.stderr)
            titles=[p['title'] for e in report.get('errors',[]) for p in e['problems']]
        if (native.returncode==0)!=expected or (reference.returncode==0)!=expected:
            failures.append({'body':body,'expected':expected,'rust':native.returncode,'elm':reference.returncode,'rust_error':native.stderr,'elm_titles':titles})
        elif not expected and (not titles or not any(t in ['BAD PORT','PORT ERROR'] for t in titles)):
            failures.append({'body':body,'error':'reference rejection is not the expected validation phase','elm_titles':titles})
print(json.dumps({'positive':len(POSITIVE),'negative':len(NEGATIVE),'failures':failures},indent=2))
raise SystemExit(bool(failures))
