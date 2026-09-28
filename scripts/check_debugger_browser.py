#!/usr/bin/env python3
"""Compare real --debug bundles, cached compilation and browser message history.

Requires Node with WebSocket, Chromium and an Elm package cache. Creates isolated
projects, ELM_HOME and browser profiles; never uses the application's dev server.
"""
import os
os.environ["PLANEXPO_ELM_STATS"] = "1"
import argparse
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import re
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
from functools import partial

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--rust', type=Path, required=True)
p.add_argument('--chromium', default='/snap/bin/chromium')
p.add_argument('--report', type=Path, required=True)
p.add_argument('--ports', action='store_true', help='exercise incoming/outgoing ports while paused and during history replay')
a = p.parse_args()
crate = Path(__file__).resolve().parents[1]
hashes = {str(b): hashlib.sha256(b.read_bytes()).hexdigest() for b in (a.elm, a.rust)}
versions = {'core': '1.0.5', 'json': '1.1.3', 'browser': '1.0.2', 'html': '1.0.0', 'virtual-dom': '1.0.3', 'time': '1.0.0', 'url': '1.0.0'}
source = '''module Main exposing (main)
import Browser
import Html exposing (button, div, text)
import Html.Attributes exposing (id)
import Html.Events exposing (onClick)
type alias Payload = { step : Int, label : Char, crumbs : List Int }
type Msg = Increment Payload | Reset
main : Program () Int Msg
main = Browser.KIND
    { init = INIT
    , update = \\msg model -> (case msg of
        Increment payload -> model + payload.step
        Reset -> 0
        , Cmd.none)
    , subscriptions = \\_ -> Sub.none
    , view = \\model -> VIEW
    EXTRA
    }
'''
view = """div [] [ button [ id "increment", onClick (Increment { step = 1, label = 'é', crumbs = [3,2] }) ] [ text "Increment" ], button [ id "reset", onClick Reset ] [ text "Reset" ], div [ id "count" ] [ text (String.fromInt model) ] ]"""
page = '''<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div><script src="bundle.js"></script><script>
const errors=[]; window.addEventListener('error',e=>errors.push(e.message));
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
let popout; const open=window.open.bind(window); window.open=(...args)=>(popout=open(...args));
(async()=>{try {
 Elm.Main.init({node:document.getElementById('app'),flags:null});
 await pause(100);
 document.getElementById('increment').click();
 await pause(100);
 document.getElementById('increment').click();
 await pause(200);
 const count=document.getElementById('count').innerText;
 const corner=document.querySelector('div[style*="position: fixed"]');
 const messages=corner?.innerText;
 if(!corner) throw Error('Debugger corner missing');
 corner.click(); await pause(300);
 if(!popout) throw Error('Debugger window missing');
 const history=popout.document.body.innerText;
 const key=key=>popout.document.dispatchEvent(new KeyboardEvent('keydown',{key,bubbles:true}));
 key('ArrowDown'); await pause(100); key('ArrowDown'); await pause(200);
 const previousCount=document.getElementById('count').innerText;
 key('ArrowUp'); await pause(100); key('ArrowUp'); await pause(200);
 const resumedCount=document.getElementById('count').innerText;
 let exported;
 const anchorClick=HTMLAnchorElement.prototype.click;
 HTMLAnchorElement.prototype.click=function(){
   if(this.hasAttribute('download')) exported={name:this.download,data:JSON.parse(decodeURIComponent(this.href.split(',').slice(1).join(',')))};
   else anchorClick.call(this);
 };
 const action=name=>Array.from(popout.document.querySelectorAll('*')).find(node=>node.textContent===name).click();
 action('Export'); await pause(100);
 if(!exported) throw Error('No history exported');
 document.getElementById('reset').click(); await pause(200);
 const resetCount=document.getElementById('count').innerText;
 popout.HTMLInputElement.prototype.click=function(){};
 action('Import'); await pause(100);
 const input=popout.document.querySelector('input[type=file]');
 const transfer=new DataTransfer();
 transfer.items.add(new File([JSON.stringify(exported.data)],exported.name,{type:'text/plain'}));
 input.files=transfer.files;
 input.dispatchEvent(new Event('change',{bubbles:true})); await pause(400);
 const importedCount=document.getElementById('count').innerText;
 window.__debugResult={done:true,count,messages,history,previousCount,resumedCount,resetCount,importedCount,exported,errors};
} catch(e) {window.__debugResult={done:true,errors:[...errors,String(e.stack)]};}})();
</script></body></html>'''
if a.ports:
    source = source.replace('module Main', 'port module Main').replace(
        'type Msg = Increment Payload | Reset',
        'port incoming : (Int -> msg) -> Sub msg\nport outgoing : Int -> Cmd msg\ntype Msg = Increment Payload | Reset | Received Int'
    ).replace('    , update = \\msg model -> (case msg of\n        Increment payload -> model + payload.step\n        Reset -> 0\n        , Cmd.none)', '    , update = update').replace('\\_ -> Sub.none', '\\_ -> incoming Received')
    source += '\nupdate msg model =\n    let\n        next =\n            case msg of\n                Increment payload -> model + payload.step\n                Reset -> 0\n                Received value -> model + value\n    in\n    (next, outgoing next)\n'
    page = page.replace(' Elm.Main.init(', ' const app=Elm.Main.init(').replace(
        ' await pause(100);\n document.getElementById',
        ' const outgoing=[]; app.ports.outgoing.subscribe(value=>outgoing.push(value));\n app.ports.incoming.send(0);\n await pause(100);\n document.getElementById', 1
    ).replace(
        " const previousCount=document.getElementById('count').innerText;",
        " const previousCount=document.getElementById('count').innerText;\n app.ports.incoming.send(7); await pause(200);\n const pausedCount=document.getElementById('count').innerText;\n const outgoingWhilePaused=outgoing.slice();"
    ).replace('importedCount,exported,errors}', 'importedCount,pausedCount,outgoingWhilePaused,outgoing,exported,errors}')

class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

def run(command, cwd, env):
    result = subprocess.run([str(s) for s in command], cwd=cwd, env=env, capture_output=True, text=True, timeout=120)
    if result.returncode:
        raise RuntimeError(result.stdout+result.stderr)
    return result

def metadata(path, rust):
    js = path.read_text()
    start = js.rfind('{"types"' if rust else '{"versions"')
    if start < 0:
        raise RuntimeError('debug metadata missing in '+str(path))
    return json.JSONDecoder().raw_decode(js[start:])[0]

results = []
with tempfile.TemporaryDirectory(prefix='elm-debugger-browser-') as folder:
    root = Path(folder)
    home = root/'home'
    cache = home/'0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm')))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    for name, version in versions.items():
        shutil.copytree(original/'elm'/name/version, cache/'elm'/name/version)
    env = dict(os.environ, ELM_HOME=str(home), GHCRTS='-N1 -A16m -c')
    server = ThreadingHTTPServer(('127.0.0.1', 0), partial(QuietHandler, directory=str(root)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        for kind in ['element', 'document', 'application']:
            project = root/kind
            (project/'src').mkdir(parents=True)
            (project/'elm.json').write_text(json.dumps({'type': 'application', 'source-directories': ['src'], 'elm-version': '0.19.1', 'dependencies': {'direct': {'elm/'+k: v for k, v in versions.items()}, 'indirect': {}}, 'test-dependencies': {'direct': {}, 'indirect': {}}}))
            elm_source = source.replace('KIND', kind).replace('INIT', '\\_ _ _ -> (0, Cmd.none)' if kind == 'application' else '\\_ -> (0, Cmd.none)').replace('VIEW', view if kind == 'element' else '{ title = "Counter", body = ['+view+'] }').replace('EXTRA', ', onUrlRequest = \\_ -> Reset, onUrlChange = \\_ -> Reset' if kind == 'application' else '')
            (project/'src/Main.elm').write_text(elm_source)
            run([a.elm, 'make', 'src/Main.elm', '--debug', '--output=official.js'], project, env)
            run([a.rust, 'make', 'src/Main.elm', '--debug', '--incremental', '--output=rust.js'], project, env)
            expected = metadata(project/'official.js', False)
            assert metadata(project/'rust.js', True) == expected, kind+' metadata'
            before = (project/'rust.js').read_bytes()
            (project/'src/Main.elm').write_text(elm_source+'\n-- force recompilation with cached dependency types\n')
            rebuild = run([a.rust, 'make', 'src/Main.elm', '--debug', '--incremental', '--output=rust.js'], project, env)
            reused = re.search(r'Reused types for ([1-9][0-9]*) module', rebuild.stdout)
            assert reused, kind+' did not reuse dependency type caches'
            assert metadata(project/'rust.js', True) == expected, kind+' cached metadata'
            assert before == (project/'rust.js').read_bytes(), kind+' cached output'
            snapshots = []
            for compiler in ['official', 'rust']:
                shutil.copy2(project/(compiler+'.js'), project/'bundle.js')
                (project/'index.html').write_text(page)
                response = run(['node', crate/'scripts/probe_debugger_browser.mjs', a.chromium, root/(kind+'-'+compiler+'-profile'), f'http://127.0.0.1:{server.server_port}/{kind}/index.html'], project, env)
                snapshots.append(json.loads(response.stdout))
            result = {'kind': kind, 'metadata_match': True, 'cached_output_match': True, 'reused_type_modules': int(reused[1]), 'official': snapshots[0], 'rust': snapshots[1], 'match': snapshots[0] == snapshots[1]}
            results.append(result)
            print(kind, json.dumps(result), flush=True)
    finally:
        server.shutdown()
        server.server_close()
assert hashes == {str(b): hashlib.sha256(b.read_bytes()).hexdigest() for b in (a.elm, a.rust)}, 'compiler changed during validation'
a.report.write_text(json.dumps({'binaries': hashes, 'ports': a.ports, 'cases': results}, indent=2)+'\n')
for result in results:
    actual = result['rust']
    assert result['match'] and not actual['errors'], 'debugger runtime mismatch'
    assert actual['count'] == '2' and actual['messages'] == ('3' if a.ports else '2')
    assert actual['previousCount'] == '1' and actual['resetCount'] == '0'
    expected_latest = '9' if a.ports else '2'
    assert actual['resumedCount'] == expected_latest and actual['importedCount'] == expected_latest
    assert actual['exported']['data']['history'][1 if a.ports else 0]['a']['label'] == 'é'
    if a.ports:
        assert actual['pausedCount'] == '1'
        assert actual['outgoingWhilePaused'] == [0, 1, 2, 9]
        assert actual['outgoing'] == [0, 1, 2, 9, 0], 'history replay emitted user commands'
