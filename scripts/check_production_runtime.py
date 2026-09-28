#!/usr/bin/env python3
"""Compare production flags, ports and runtime containers with Elm --optimize."""
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
import Array exposing (Array)
import Platform
type alias Payload = { count : Int, maybe : Maybe Int, items : List Int, array : Array Int, pair : (Int, String), unit : () }
type alias Detail = { rareCount : Int }
adjust ({ rareCount } as record) = { record | rareCount = rareCount }
port incoming : (Payload -> msg) -> Sub msg
port outgoing : Payload -> Cmd msg
main : Program Payload Payload Payload
main = Platform.worker
    { init = \flags -> (flags, Cmd.none)
    , update = \payload model -> (model, outgoing
        { payload | count = (.rareCount) (adjust (Detail (payload.count + model.count)))
        , maybe = Maybe.map ((+) 1) payload.maybe
        , items = List.reverse payload.items
        , array = Array.map ((+) 1) payload.array
        , pair = Tuple.mapFirst ((+) 1) payload.pair
        })
    , subscriptions = \_ -> incoming identity
    }
'''
runner = r'''
const Elm=require(process.argv[1]).Elm;
const payload={count:5,maybe:7,items:[1,2,3],array:[4,5],pair:[6,'hello'],unit:null};
const app=Elm.Main.init({flags:payload});
const outputs=[];
app.ports.outgoing.subscribe(value=>outputs.push(value));
app.ports.incoming.send(payload);
app.ports.incoming.send({...payload,maybe:null,items:[],array:[]});
let badFlags=false,badPort=false;
try{Elm.Main.init({flags:{...payload,count:'wrong'}});}catch(e){badFlags=true;}
try{app.ports.incoming.send({...payload,unit:42});}catch(e){badPort=true;}
setTimeout(()=>console.log(JSON.stringify({outputs,badFlags,badPort})),30);
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-production-') as directory:
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
    expected = {'outputs':[
        {'count':10,'maybe':8,'items':[3,2,1],'array':[5,6],'pair':[7,'hello'],'unit':None},
        {'count':10,'maybe':None,'items':[],'array':[],'pair':[7,'hello'],'unit':None},
    ],'badFlags':True,'badPort':True}
    assert results[0] == results[1] == expected, results
    print('Production: flags, ports, Maybe, List, Array, tuples and unit match Elm --optimize', flush=True)
    for expression in ['Debug.log "value" 1', 'Debug.toString 1', 'Debug.todo "missing"', 'D.log "value" 1']:
        negative = source.replace('import Platform', 'import Platform\nimport Debug as D').replace('(flags, Cmd.none)', 'if value == value then (flags, Cmd.none) else (flags, Cmd.none)')
        (root/'Main.elm').write_text(negative+'\nvalue = '+expression+'\n')
        rust = subprocess.run([str(binary), 'link-js-prod', str(root/'elm.json'), 'Main.elm', str(root/'bad.js')], capture_output=True, text=True, timeout=60)
        elm = subprocess.run([str(args.elm.resolve()), 'make', 'Main.elm', '--optimize', '--output='+str(root/'bad-reference.js'), '--report=json'], cwd=root, env={**os.environ,'GHCRTS':'-N1 -A16m -c'}, capture_output=True, text=True, timeout=60)
        assert rust.returncode and 'Debug.' in rust.stderr, (expression, rust.stderr)
        assert elm.returncode and 'DEBUG' in elm.stderr, (expression, elm.stderr)
    print('Production: Debug references, including aliases, rejected by both compilers')
