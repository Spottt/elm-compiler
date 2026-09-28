#!/usr/bin/env python3
"""Compare numeric optimization semantics with Elm --optimize."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=Path, required=True)
args = parser.parse_args()
crate = Path(__file__).resolve().parents[1]
binary = crate/'target/release/planexpo-elm'
source = r'''port module Main exposing (main)
import Platform
port incoming : (( Float, Float ) -> msg) -> Sub msg
port outgoing : List Float -> Cmd msg
shadow a b = a - b
calculate (a, b) =
    let
        partial = (+) a
        alias = (*)
    in
    [ a + b, a - b, a * b, a / b, partial b, alias a b, shadow a b ]
main : Program () () ( Float, Float )
main = Platform.worker
    { init = \_ -> ((), Cmd.none)
    , update = \pair model -> (model, outgoing (calculate pair))
    , subscriptions = \_ -> incoming identity
    }
'''
runner = r'''
const Elm=require(process.argv[1]).Elm;
const app=Elm.Main.init({flags:null});
const values=[[7,3],[-7,3],[1.25,-2.5],[0,-0],[-0,-0],[1,0],[0,0],[2147483647,2],[9007199254740991,2]];
const encode=x=>Number.isNaN(x)?'NaN':Object.is(x,-0)?'-0':x===Infinity?'Infinity':x===-Infinity?'-Infinity':x;
const outputs=[];
app.ports.outgoing.subscribe(value=>outputs.push(value.map(encode)));
for(const pair of values)app.ports.incoming.send(pair);
setTimeout(()=>{
 const expected=values.map(([a,b])=>[a+b,a-b,a*b,a/b,a+b,a*b,a-b].map(encode));
 if(JSON.stringify(outputs)!==JSON.stringify(expected))throw Error(JSON.stringify({outputs,expected}));
 console.log(JSON.stringify(outputs));
},30);
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-arithmetic-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    (root/'Main.elm').write_text(source)
    results = []
    for name in ['elm', 'rust']:
        output = root/f'{name}.js'
        command = ([str(args.elm.resolve()), 'make', 'Main.elm', '--optimize', '--output='+str(output)] if name == 'elm' else
                   [str(binary), 'link-js-prod', str(root/'elm.json'), 'Main.elm', str(output)])
        compiled = subprocess.run(command, cwd=root, env={**os.environ, 'GHCRTS':'-N1 -A16m -c'}, capture_output=True, text=True, timeout=60)
        assert compiled.returncode == 0, (name, compiled.stdout, compiled.stderr)
        run = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, check=True, timeout=10)
        results.append(json.loads(run.stdout))
    assert results[0] == results[1], results
    print('Arithmetic: operators, aliases, partial applications, large numbers, NaN, infinity and negative zero match Elm --optimize')
