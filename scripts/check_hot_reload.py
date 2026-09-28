#!/usr/bin/env python3
"""Exercise the project's actual elm-hot injector and state/port preservation.

Uses a worker in a Node VM with a minimal body placeholder, not a browser or
Webpack watcher. No frontend development server is stopped or modified.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
binary = crate/'target/release/planexpo-elm'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--injector', type=Path, default=crate.parent/'front/node_modules/elm-hot')
parser.add_argument('--compiler', type=Path, default=binary)
args = parser.parse_args()
binary = args.compiler.resolve()
injector = args.injector.resolve()
source = '''port module Main exposing (main)
import Platform
port incoming : (Int -> msg) -> Sub msg
port outgoing : Int -> Cmd msg
type Msg = Received Int
main : Program { start : Int } Int Msg
main = Platform.worker
    { init = \\flags -> (flags.start, Cmd.none)
    , update = \\(Received value) model -> (model + value, outgoing (OFFSET + model + value))
    , subscriptions = \\_ -> incoming Received
    }
'''
runner = r'''
const fs=require('fs'),vm=require('vm'),assert=require('node:assert/strict');
const {inject}=require(process.argv[1]);
const body={parentNode:{},lastChild:null};
const context=vm.createContext({console,setTimeout,clearTimeout,document:{body}});
let dispose, accepts=0;
function load(file,data){
 context.module={exports:{},hot:{data,accept(){accepts++},dispose(f){dispose=f}}};
 vm.runInContext('(function(){'+inject(fs.readFileSync(file,'utf8'))+'}).call(module.exports);',context);
 return context.module.exports.Elm;
}
(async()=>{
const first=load(process.argv[2]);
const app=first.Main.init({flags:{start:5}});
const values=[];
let received;
app.ports.outgoing.subscribe(v=>{values.push(v);if(received)received()});
async function send(value){
 await new Promise((resolve,reject)=>{
  const timer=setTimeout(()=>reject(new Error('Port response timed out')),3000);
  received=()=>{clearTimeout(timer);received=null;resolve()};
  app.ports.incoming.send(value);
 });
}
await send(2);
assert.deepEqual(values,[7]);
const data={};dispose(data);
load(process.argv[3],data);
// Retain the original application object and callback, as a real bootstrap does.
await send(3);
assert.deepEqual(values,[7,110]);
assert.equal(accepts,2);
console.log(JSON.stringify({injection:true,hot_accepts:accepts,values,state_preserved:true,ports_reconnected:true}));
})().catch(error=>{console.error(error);process.exitCode=1});
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-hot-reload-') as directory:
    project=Path(directory)
    shutil.copy2(crate/'tests/programs/worker/elm.json',project/'elm.json')
    outputs=[]
    for offset in (0,100):
        (project/'Main.elm').write_text(source.replace('OFFSET',str(offset)))
        output=project/f'main-{offset}.js'
        subprocess.run([str(binary),'make','Main.elm','--incremental','--output='+str(output)],cwd=project,check=True,capture_output=True,text=True,timeout=60)
        outputs.append(str(output))
    result=subprocess.run(['node','-e',runner,str(injector),*outputs],capture_output=True,text=True,timeout=30)
    if result.returncode:
        raise RuntimeError(result.stderr)
    print(json.dumps({'compiler_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'result':json.loads(result.stdout)}))
