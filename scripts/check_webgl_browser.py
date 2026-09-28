#!/usr/bin/env python3
"""Compile Elm's real WebGL package and compare rendered pixels in Chromium.

Requires cached dependencies from tests/programs/webgl/elm.json and Chromium.
Uses an isolated project, package home (if supplied), and browser profile.
"""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import struct
import zlib
import tempfile
import threading

crate = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--elm-home', type=Path)
p.add_argument('--chromium', default='chromium')
p.add_argument('--report', type=Path)
args = p.parse_args()
env = {**os.environ,'GHCRTS':'-N1 -A16m -c'}
if args.elm_home: env['ELM_HOME'] = str(args.elm_home.resolve())
page = b'''<!doctype html><html><head><script>
window.__webglResult={done:false,errors:[]};
addEventListener('error',e=>__webglResult.errors.push(String(e.error||e.message)));
addEventListener('unhandledrejection',e=>__webglResult.errors.push(String(e.reason)));
</script><script src="/bundle.js"></script></head><body><div id="elm"></div><script>
try { Elm.Main.init({node:document.getElementById('elm')}); }
catch(e) { __webglResult.errors.push(String(e.stack)); }
let frames=0;
function sample(){
  requestAnimationFrame(async()=>{
    const canvas=document.querySelector('canvas');
    if(!canvas || ++frames<5){sample();return;}
    const gl=canvas.getContext('webgl')||canvas.getContext('experimental-webgl');
    if(!gl){__webglResult.errors.push('WebGL context unavailable');return;}
    gl.finish();
    const points=[[1,1],[32,32],[62,62]],pixels=[];
    for(const [x,y] of points){const bytes=new Uint8Array(4);gl.readPixels(x,y,1,1,gl.RGBA,gl.UNSIGNED_BYTE,bytes);pixels.push(Array.from(bytes));}
    const frame=new Uint8Array(canvas.width*canvas.height*4);
    gl.readPixels(0,0,canvas.width,canvas.height,gl.RGBA,gl.UNSIGNED_BYTE,frame);
    const digest=await crypto.subtle.digest('SHA-256',frame);
    const frameSha256=Array.from(new Uint8Array(digest)).map(x=>x.toString(16).padStart(2,'0')).join('');
    Object.assign(__webglResult,{done:true,pixels,frameSha256,glError:gl.getError(),width:canvas.width,height:canvas.height,renderer:gl.getParameter(gl.RENDERER)});
  });
}
sample();
</script></body></html>'''
def png_chunk(kind, data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
pixel_png = b'\x89PNG\r\n\x1a\n' + png_chunk(b'IHDR',struct.pack('>2I5B',1,1,8,6,0,0,0)) + png_chunk(b'IDAT',zlib.compress(bytes([0,191,64,128,255]))) + png_chunk(b'IEND',b'')
results=[]
references={}
with tempfile.TemporaryDirectory(prefix='elm-webgl-') as directory:
    root=Path(directory)
    shutil.copytree(crate/'tests/programs/webgl', root, dirs_exist_ok=True)
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            data=(root/'bundle.js').read_bytes() if self.path=='/bundle.js' else pixel_png if self.path=='/pixel.png' else active_page
            self.send_response(200);self.send_header('Content-Type','application/javascript' if self.path=='/bundle.js' else 'image/png' if self.path=='/pixel.png' else 'text/html');self.end_headers();self.wfile.write(data)
        def log_message(self,*_):pass
    server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
    threading.Thread(target=server.serve_forever,daemon=True).start()
    try:
        for entry in ['Main','TextureMain']:
            active_page = page.replace(b'Elm.Main.init', ('Elm.'+entry+'.init').encode())
            for name,compiler in [('official',args.elm.resolve()),('rust',crate/'target/release/planexpo-elm')]:
                for mode,options in [('development',[]),('production',['--optimize'])]:
                    compile=subprocess.run([str(compiler),'make',entry+'.elm','--output=bundle.js',*options],cwd=root,env=env,capture_output=True,text=True,timeout=120)
                    assert compile.returncode==0, (name,mode,compile.stderr)
                    with tempfile.TemporaryDirectory(prefix='elm-webgl-chromium-') as profile:
                        run=subprocess.run(['node',str(crate/'scripts/probe_webgl_browser.mjs'),args.chromium,profile,f'http://127.0.0.1:{server.server_port}/'],capture_output=True,text=True,timeout=60)
                    assert run.returncode==0, run.stderr
                    rendered=json.loads(run.stdout)
                    row={'entry':entry,'compiler':name,'mode':mode,'rendered':rendered,'bundle_sha256':hashlib.sha256((root/'bundle.js').read_bytes()).hexdigest()}
                    results.append(row)
                    assert rendered['done'] and not rendered['errors'] and rendered['glError']==0, row
                    assert (rendered['width'],rendered['height'])==(64,64),row
                    expected_pixels = ([[round(255*0.75*(p+0.5)/64),round(255*0.25*(p+0.5)/64),128,255] for p in [1,32,62]]
                                       if entry == 'Main' else [[191,64,128,255]]*3)
                    for pixel,expected_pixel in zip(rendered['pixels'],expected_pixels):
                        assert all(abs(actual-expected)<=1 for actual,expected in zip(pixel,expected_pixel)),row
                    reference = references.setdefault(entry, (rendered['pixels'],rendered['frameSha256']))
                    assert (rendered['pixels'],rendered['frameSha256'])==reference,row
                    print(f'PASS {entry} {name} {mode}: {rendered["pixels"]}',flush=True)
    finally:
        server.shutdown()
if args.report: args.report.write_text(json.dumps({'results':results,'passed':True},indent=2)+'\n')
