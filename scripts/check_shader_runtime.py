#!/usr/bin/env python3
"""Execute shader objects and their production record-field translations."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
args = p.parse_args()
shader = '// quotes: " and \' and \\ slash\r\nattribute vec3 position; uniform float time; uniform float eval; varying vec2 uv; void main() { gl_Position = vec4(position, 1.0); }'
source = '''module Main exposing (..)
import Platform
shader = [glsl|''' + shader + '''|]
attributes = { position = 17 }
uniforms = { time = 42, eval = 7 }
main : Program () ( a, b, c ) Never
main = Platform.worker { init = \\() -> ( (shader, attributes, uniforms), Cmd.none ), update = \\msg _ -> never msg, subscriptions = \\_ -> Sub.none }
'''
# Let inference determine the shader's phantom type arguments and the model.
source = source.replace('main : Program () ( a, b, c ) Never\n', '')
runner = r'''
const fs=require('fs'),vm=require('vm');
let code=fs.readFileSync(process.argv[1],'utf8');
const index=code.lastIndexOf('_Platform_export(');
if(index<0)throw Error('missing export');
code=code.slice(0,index)+'scope.__probe={shader:$author$project$Main$shader,attributes:$author$project$Main$attributes,uniforms:$author$project$Main$uniforms};'+code.slice(index);
const context={console:{warn(){}}};vm.runInNewContext(code,context);
const p=context.__probe;
const translate=(mapping,record)=>Object.fromEntries(Object.entries(mapping).map(([name,key])=>{if(!(key in record))throw Error('missing mapped field '+name);return [name,record[key]]}));
console.log(JSON.stringify({src:p.shader.src,attributes:translate(p.shader.attributes,p.attributes),uniforms:translate(p.shader.uniforms,p.uniforms)}));
'''
with tempfile.TemporaryDirectory(prefix='elm-shader-runtime-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    (root/'Main.elm').write_text(source)
    expected = {'src':shader.replace('\r',''),'attributes':{'position':17},'uniforms':{'time':42,'_eval':7}}
    for compiler in [args.elm.resolve(),crate/'target/release/planexpo-elm']:
        for options in [[],['--optimize']]:
            output = root/'out.js'
            run = subprocess.run([str(compiler),'make','Main.elm','--output='+str(output),*options],cwd=root,env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=60)
            assert run.returncode == 0, run.stderr
            result = subprocess.run(['node','-e',runner,str(output)],capture_output=True,text=True,timeout=10,check=True)
            assert json.loads(result.stdout) == expected, result.stdout
print('PASS: official/Rust shader source, attributes and uniforms in development and production')
