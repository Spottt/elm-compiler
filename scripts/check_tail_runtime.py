#!/usr/bin/env python3
"""Exercise stack-safe tail calls, argument swaps and captured iteration values."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--production', action='store_true')
args = parser.parse_args()
crate = Path(__file__).resolve().parents[1]
source = '''port module Main exposing (main)
import Platform
port incoming : (Int -> msg) -> Sub msg
port outgoing : List Int -> Cmd msg
main : Program () () Int
main = Platform.worker
    { init = \\_ -> ((), Cmd.none)
    , update = \\n model ->
        let
            partial = sum n
        in
        (model, outgoing [ sum n 0, walk (List.range 1 n) 0, swap n 1 2, partial 10, partial 20, capture 3 [], tuple (n, 0), staged (min n 100) 0, local n, enclosing 2 0 ])
    , subscriptions = \\_ -> incoming identity
    }
sum n total =
    if n == 0 then total else sum (n - 1) (total + n)
tuple (n, total) =
    if n == 0 then total else tuple (n - 1, total + n)
staged n total =
    if n == 0 then total else (staged (n - 1)) (total + n)
local limit =
    let
        base = 3
        go n total =
            if n == 0 then total else go (n - 1) (total + n + base)
    in
    go limit 0
enclosing n total =
    if n == 0 then total else
        let
            inner i acc =
                if i == 0 then acc else inner (i - 1) (acc + 1)
        in
        enclosing (n - 1) (total + inner 100000 0)
walk list total =
    case list of
        [] -> total
        head :: rest ->
            let next = total + head in
            walk rest next
swap n a b =
    if n == 0 then a * 10 + b else swap (n - 1) b a
capture n functions =
    if n == 0 then
        List.sum (List.map (\\f -> f ()) functions)
    else
        let
            current = n * 10
        in
        capture (n - 1) ((\\_ -> current + n) :: functions)
'''
runner = '''
const app=require(process.argv[1]).Elm.Main.init({flags:null});
const seen=[];app.ports.outgoing.subscribe(x=>seen.push(x));
app.ports.incoming.send(100000);app.ports.incoming.send(3);
setTimeout(()=>console.log(JSON.stringify(seen)),30);
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-tail-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    (root/'Main.elm').write_text(source)
    reference, native = root/'elm.js', root/'rust.js'
    subprocess.run([str(args.elm.resolve()), 'make', 'Main.elm', '--output='+str(reference), *(['--optimize'] if args.production else [])], cwd=root, env={**os.environ, 'GHCRTS':'-N1 -A16m -c'}, check=True, capture_output=True, text=True, timeout=60)
    subprocess.run([str(crate/'target/release/planexpo-elm'), 'link-js-prod' if args.production else 'link-js', str(root/'elm.json'), 'Main.elm', str(native)], check=True, capture_output=True, text=True, timeout=60)
    results=[]
    for bundle in (reference, native):
        run=subprocess.run(['node','-e',runner,str(bundle)],capture_output=True,text=True,timeout=15)
        assert run.returncode==0, (bundle.name,run.stderr[-3000:])
        results.append(json.loads(run.stdout))
    # Upstream emits function-scoped loop bindings: the three closures each
    # read the final current=10 and n=0. Keep that observable 0.19.1 behavior.
    assert results[0]==results[1]==[[5000050000,5000050000,12,5000050010,5000050020,30,5000050000,5050,5000350000,200000],[6,6,21,16,26,30,6,6,15,200000]],results
    print(json.dumps({'iterations':100000,'elm':results[0],'rust':results[1],'match':True}))
