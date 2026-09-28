#!/usr/bin/env python3
"""Compile and execute the same real-runtime worker with Rust and Elm 0.19.1."""
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
runner = r'''
const sentinel={existingApplication:true};
globalThis.Elm=sentinel;
const loaded=require(process.argv[1]);
if(!loaded.Elm?.Main?.init)throw Error('CommonJS Elm export missing');
if(globalThis.Elm!==sentinel || Object.keys(sentinel).length!==1)throw Error('CommonJS polluted global Elm');
const browser={};
require('node:vm').runInNewContext(require('node:fs').readFileSync(process.argv[1],'utf8'),browser);
if(!browser.Elm?.Main?.init)throw Error('Browser Elm export missing');
const Elm=loaded.Elm;
let badFlags=false;
try { Elm.Main.init({flags:{start:'wrong'}}); } catch(e) { badFlags=true; }
const validFlags=JSON.parse(process.argv[2]);
const invalidFields=[];
const missingFields=[];
for (const key of Object.keys(validFlags)) {
  const invalid={...validFlags,[key]:typeof validFlags[key]==='string'?42:'wrong'};
  try { Elm.Main.init({flags:invalid}); } catch(e) { invalidFields.push(key); }
  const missing={...validFlags}; delete missing[key];
  try { Elm.Main.init({flags:missing}); } catch(e) { missingFields.push(key); }
}
const app=Elm.Main.init({flags:validFlags});
const seen=[];
const listener=x=>seen.push(x);
app.ports.outgoing.subscribe(listener);
app.ports.incoming.send(42);
app.ports.incoming.send(-7);
let badPort=false;
try { app.ports.incoming.send('wrong'); } catch(e) { badPort=true; }
setTimeout(()=>{
  app.ports.outgoing.unsubscribe(listener);
  app.ports.incoming.send(99);
  setTimeout(()=>console.log(JSON.stringify({seen,badFlags,badPort,invalidFields,missingFields})),20);
},20);
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-worker-') as directory:
    root = Path(directory)
    shutil.copytree(crate/'tests/programs/worker', root, dirs_exist_ok=True)
    original = (root/'Main.elm').read_text()
    variants = {
        'record': ('{ start : Int }', '', {'start':5}),
        'closed_empty_row': ('Extended {}', 'type alias Extended r = { r | start : Int }', {'start':5}),
        'closed_row': ('Extended { label : String }', 'type alias Extended r = { r | start : Int }', {'start':5,'label':'hello'}),
        'row_chain': ('Extended (Extra { label : String })', 'type alias Extended r = { r | start : Int }\ntype alias Extra r = { r | enabled : Bool }', {'start':5,'label':'hello','enabled':True}),
    }
    for name, (flags_type, aliases, flags) in variants.items():
        (root/'Main.elm').write_text(original.replace('Program { start : Int } Int Msg', f'Program ({flags_type}) Int Msg') + '\n' + aliases + '\n')
        native = root/'rust.js'
        reference = root/'elm.js'
        # Establish that the reference accepts this source before checking Rust.
        subprocess.run([str(args.elm.resolve()), 'make', 'Main.elm', '--output='+str(reference)], cwd=root, env={**os.environ, 'GHCRTS':'-N1 -A16m -c'}, check=True, capture_output=True, text=True, timeout=60)
        compiled = subprocess.run([str(crate/'target/release/planexpo-elm'), 'link-js', str(root/'elm.json'), 'Main.elm', str(native)], capture_output=True, text=True, timeout=60)
        assert compiled.returncode == 0, (name, compiled.stdout, compiled.stderr)
        # Exercise conventional build-tool invocation from a child directory.
        child = root/'build'
        child.mkdir(exist_ok=True)
        made = child/'nested/make.js'
        compiled = subprocess.run([str(crate/'target/release/planexpo-elm'), 'make', '../Main.elm', '--output', 'nested/make.js', '--report=json'], cwd=child, capture_output=True, text=True, timeout=60)
        assert compiled.returncode == 0, (name, compiled.stdout, compiled.stderr)
        assert not compiled.stdout and not compiled.stderr
        assert made.read_bytes() == native.read_bytes(), name
        checked = subprocess.run([str(crate/'target/release/planexpo-elm'), 'make', '../Main.elm', '--output=/dev/null'], cwd=child, capture_output=True, text=True, timeout=60)
        assert checked.returncode == 0, (name, checked.stderr)
        results=[]
        for script in [native, reference]:
            result=subprocess.run(['node','-e',runner,str(script),json.dumps(flags)], check=True, capture_output=True, text=True, timeout=10)
            results.append(json.loads(result.stdout))
        expected={'seen':[47,-2], 'badFlags':True, 'badPort':True, 'invalidFields':list(flags), 'missingFields':list(flags)}
        assert results[0] == results[1] == expected, (name, results)
        print(json.dumps({'variant':name, 'rust':results[0], 'elm':results[1], 'match':True}), flush=True)
    previous = native.read_bytes()
    (root/'Main.elm').write_text('module Main exposing (..)\nbroken = unknown\n')
    for output in [str(native), '/dev/null']:
        failed = subprocess.run([str(crate/'target/release/planexpo-elm'), 'make', 'Main.elm', '--output='+output, '--report=json'], cwd=root, capture_output=True, text=True, timeout=60)
        assert failed.returncode != 0 and not failed.stdout
        diagnostic = json.loads(failed.stderr)
        assert diagnostic['type'] == 'compile-errors', failed.stderr
        problem = diagnostic['errors'][0]['problems'][0]
        assert problem['title'] == 'NAMING ERROR', problem
        assert problem['region'] == {'start':{'line':2,'column':10}, 'end':{'line':2,'column':17}}, problem
        assert native.read_bytes() == previous, 'a failed compilation replaced the previous bundle'
    print('make: invalid sources fail with JSON diagnostics and preserve existing output', flush=True)
