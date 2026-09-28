#!/usr/bin/env python3
"""Differential HTTP checks for reactor, with isolated project and package cache."""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--rust', type=Path, required=True)
p.add_argument('--report', type=Path, required=True)
p.add_argument('--browser', action='store_true')
p.add_argument('--port-format', choices=['decimal','hex','octal','wrapped'], default='decimal')
p.add_argument('--chromium', default='/snap/bin/chromium')
a = p.parse_args()
hashes = {str(b): hashlib.sha256(b.read_bytes()).hexdigest() for b in [a.elm, a.rust]}
results = []
crate = Path(__file__).resolve().parents[1]

def request(port, path, method='GET'):
    connection = http.client.HTTPConnection('127.0.0.1', port, timeout=60)
    try:
        connection.request(method, path)
        response = connection.getresponse()
        return {'status': response.status, 'type': response.getheader('Content-Type'), 'location': response.getheader('Location'), 'body': response.read().decode(errors='replace')}
    finally:
        connection.close()

def flags(body, module):
    prefix = 'Elm.'+module+'.init({ flags: '
    return json.JSONDecoder().raw_decode(body.split(prefix, 1)[1])[0]

with tempfile.TemporaryDirectory(prefix='elm-reactor-test-') as folder:
    root = Path(folder)
    project = root/'project'
    (project/'src').mkdir(parents=True)
    cache = root/'home/0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm')))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    versions = {'core': '1.0.5', 'json': '1.1.3', 'html': '1.0.0', 'virtual-dom': '1.0.3'}
    for name, version in versions.items():
        shutil.copytree(original/'elm'/name/version, cache/'elm'/name/version)
    (project/'elm.json').write_text(json.dumps({'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/'+name:version for name,version in versions.items()},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    (project/'README.md').write_text('# Reactor fixture\n\nA local project.')
    (project/'hello world.txt').write_text('hello é\n')
    (project/'style.css').write_text('body { color: green; }')
    (project/'example.rs').write_text('fn main() {}\n')
    source = 'module Main exposing (main)\nimport Html\nmain = Html.text "reactor works"\n'
    (project/'src/Main.elm').write_text(source)
    (project/'src/Broken.elm').write_text('module Broken exposing (main)\nmain =\n')
    servers = []
    env = dict(os.environ, ELM_HOME=str(root/'home'), GHCRTS='-N1 -A16m -c')
    try:
        for compiler in [a.elm, a.rust]:
            with socket.socket() as reservation:
                reservation.bind(('127.0.0.1', 0))
                port = reservation.getsockname()[1]
            port_text = {'decimal':str(port),'hex':hex(port),'octal':oct(port),'wrapped':str(port+2**64)}[a.port_format]
            process = subprocess.Popen([str(compiler), 'reactor', '--port='+port_text], cwd=project, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            servers.append((port, process))
            deadline = time.monotonic()+15
            while True:
                if process.poll() is not None: raise RuntimeError(process.communicate())
                try:
                    request(port, '/_elm/styles.css')
                    break
                except (OSError, http.client.HTTPException):
                    if time.monotonic() >= deadline: raise
                    time.sleep(0.05)
        for path, method in [('/', 'GET'), ('/src/', 'GET'), ('/src', 'GET'), ('/hello%20world.txt', 'GET'), ('/hello%20world.txt', 'HEAD'), ('/style.css', 'GET'), ('/example.rs', 'GET'), ('/_elm/styles.css', 'GET'), ('/missing', 'GET'), ('/src/Broken.elm', 'GET'), ('/src/Main.elm', 'GET')]:
            responses = [request(port, path, method) for port, _ in servers]
            if path in ['/', '/src/']:
                for response in responses:
                    response['body'] = flags(response['body'], 'Index')
                    response['body']['dirs'].sort()
                    response['body']['files'].sort(key=lambda value:value['name'])
            elif path == '/src/Broken.elm':
                for response in responses:
                    response['body'] = flags(response['body'], 'Errors')
            elif path == '/src/Main.elm':
                # Generated JS differs; both pages must contain the compiled app.
                for response in responses:
                    response['body'] = 'compiled' if 'reactor works' in response['body'] and 'Initialization Error' in response['body'] else response['body']
            results.append({'path':path,'method':method,'match':responses[0]==responses[1],'official':responses[0],'rust':responses[1]})
            print(path, method, results[-1]['match'], flush=True)
        for body in ['main =', 'main =   \n', 'greet name =\n\n']:
            (project/'src/Broken.elm').write_text('module Broken exposing (..)\n'+body)
            responses = [request(port, '/src/Broken.elm') for port, _ in servers]
            for response in responses: response['body'] = flags(response['body'], 'Errors')
            results.append({'path':'/src/Broken.elm','source':body,'method':'GET','match':responses[0]==responses[1],'official':responses[0],'rust':responses[1]})
            print('unfinished definition', repr(body), results[-1]['match'], flush=True)
        (project/'src/Broken.elm').write_text('module Broken exposing (main)\nmain =\n')
        if a.browser:
            for index, (path, expected) in enumerate([('/', 'Reactor fixture'), ('/src/', 'Main.elm'), ('/missing', 'Page not found'), ('/src/Broken.elm', 'UNFINISHED DEFINITION'), ('/src/Main.elm', 'reactor works'), ('/src/Main.elm', 'reactor refreshed')]):
                if index == 5:
                    (project/'src/Main.elm').write_text(source.replace('reactor works', 'reactor refreshed'))
                snapshots = []
                for compiler, (port, _) in enumerate(servers):
                    run = subprocess.run(['node', str(crate/'scripts/probe_reactor_browser.mjs'), a.chromium, str(root/f'profile-{index}-{compiler}'), f'http://127.0.0.1:{port}{path}'], capture_output=True, text=True, timeout=55, check=True)
                    snapshots.append(json.loads(run.stdout))
                match = snapshots[0] == snapshots[1] and all(expected in value['text'] for value in snapshots)
                results.append({'path':path,'method':'BROWSER','expected':expected,'match':match,'official':snapshots[0],'rust':snapshots[1]})
                print(path, 'BROWSER', match, flush=True)
        (project/'elm.json').rename(project/'saved-manifest.json')
        responses = [request(port, '/src/Main.elm') for port, _ in servers]
        for response in responses: response['body'] = flags(response['body'], 'Errors')
        results.append({'path':'/src/Main.elm without elm.json','method':'GET','match':responses[0]==responses[1],'official':responses[0],'rust':responses[1]})
        print('no project', results[-1]['match'], flush=True)
    finally:
        for _, process in servers:
            process.terminate()
            try: process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
assert hashes == {str(b): hashlib.sha256(b.read_bytes()).hexdigest() for b in [a.elm, a.rust]}, 'compiler changed during validation'
a.report.write_text(json.dumps({'binaries':hashes,'port_format':a.port_format,'cases':results},indent=2)+'\n')
assert all(result['match'] for result in results), 'HTTP behavior differs; inspect report'
