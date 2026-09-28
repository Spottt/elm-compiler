#!/usr/bin/env python3
"""Offline HTTPS CONNECT proxy authentication comparison.

Checks actual tunnel authorization headers against Elm 0.19.1 and verifies that
proxy credentials are absent from the tunneled origin HTTP requests. Registry
and endpoint metadata are served by a loopback TLS proxy; the final archive
uses a reserved loopback port to finish without publishing a package.
"""
import argparse
import concurrent.futures
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import socketserver
import socket
import re
import ssl
import subprocess
import tempfile
import threading
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
for key in ('rust', 'elm', 'report'):
    parser.add_argument('--' + key, type=Path, required=True)
args = parser.parse_args()
binaries = [('elm', args.elm.resolve()), ('rust', args.rust.resolve())]
sha = hashlib.sha256(args.rust.read_bytes()).hexdigest()
modes = ('https-auth-percent','https-auth-invalid-utf8','https-auth-unicode','https-auth-user-only','https-auth-empty-user','https-auth-scalar','https-auth-encoded-colon')
no_proxy_values = dict.fromkeys(modes, '')
cache_source = Path(os.environ.get('ELM_HOME', Path.home()/'.elm')) / '0.19.1/packages'

with tempfile.TemporaryDirectory(prefix='download-body-') as directory:
    root = Path(directory)
    def openssl(*arguments):
        subprocess.run(['openssl', *arguments], cwd=root, check=True, capture_output=True)
    openssl('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', 'ca.key',
            '-out', 'ca.pem', '-days', '1', '-subj', '/CN=Elm parity test CA',
            '-addext', 'basicConstraints=critical,CA:TRUE',
            '-addext', 'keyUsage=critical,keyCertSign,cRLSign')
    openssl('req', '-newkey', 'rsa:2048', '-nodes', '-keyout', 'server.key',
            '-out', 'server.csr', '-subj', '/CN=package.elm-lang.org')
    (root/'extensions').write_text('subjectAltName=DNS:package.elm-lang.org,DNS:archive.test\n'
                                  'basicConstraints=critical,CA:FALSE\n'
                                  'keyUsage=critical,digitalSignature,keyEncipherment\n'
                                  'extendedKeyUsage=serverAuth\n')
    openssl('x509', '-req', '-in', 'server.csr', '-CA', 'ca.pem', '-CAkey', 'ca.key',
            '-CAcreateserial', '-out', 'server.pem', '-days', '1', '-extfile', 'extensions')
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(root/'server.pem', root/'server.key')
    certificate_contexts = {}
    archive_buffer = io.BytesIO()
    with zipfile.ZipFile(archive_buffer, 'w', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('package/', b'')
        package = cache_source/'elm/url/1.0.0'
        for path in sorted(package.rglob('*')):
            if path.relative_to(package).parts[0] == 'src' or path.name == 'elm.json':
                if path.is_dir():
                    archive.writestr('package/' + str(path.relative_to(package)) + '/', b'')
                elif path.is_file():
                    archive.writestr('package/' + str(path.relative_to(package)), path.read_bytes())
    archive_bytes = archive_buffer.getvalue()
    reserved_sockets = []
    endpoints = {}
    for mode in modes:
        ipv6 = mode.startswith('ipv6-')
        family, host, url_host = (socket.AF_INET6, '::1', '[::1]') if ipv6 else (socket.AF_INET, '127.0.0.1', '127.0.0.1')
        refused = socket.socket(family)
        refused.bind((host, 0))
        reserved_sockets.append(refused)
        refused_url = f'http://{url_host}:{refused.getsockname()[1]}/source.zip'
        endpoints[mode] = json.dumps({'url': refused_url,
                           'hash': hashlib.sha1(archive_bytes).hexdigest()}).encode()

    def run(case):
        mode, label, binary, json_mode = case
        endpoint = endpoints[mode]
        requests, server_errors, connects, tls_failures = [], [], [], []
        proxy_archives = []
        proxy_authorization = []
        class Proxy(socketserver.BaseRequestHandler):
            def handle(self):
                try:
                    self.request.settimeout(10)
                    request = b''
                    while not request.endswith(b'\r\n\r\n'):
                        byte = self.request.recv(1)
                        if not byte:
                            return
                        request += byte
                        if len(request) > 16384:
                            raise ValueError('oversized CONNECT')
                    method, target, _ = request.split(b'\r\n', 1)[0].decode().split()
                    if method == 'GET' and target == json.loads(endpoint)['url']:
                        proxy_archives.append(target)
                        self.request.sendall(b'HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n')
                        return
                    if method != 'CONNECT' or target not in ('package.elm-lang.org:443', 'archive.test:443'):
                        raise ValueError('unexpected proxy destination: ' + target)
                    connects.append(target)
                    proxy_authorization.append(next((line.partition(b':')[2].strip().decode() for line in request.split(b'\r\n') if line.lower().startswith(b'proxy-authorization:')), None))
                    self.request.sendall(b'HTTP/1.1 200 Connection established\r\n\r\n')
                    selected_tls = certificate_contexts.get(mode,tls) if target=='archive.test:443' else tls
                    with selected_tls.wrap_socket(self.request, server_side=True) as stream:
                        request = b''
                        while b'\r\n\r\n' not in request:
                            chunk = stream.recv(4096)
                            if not chunk:
                                return
                            request += chunk
                        method, path, _ = request.split(b'\r\n', 1)[0].decode().split()
                        headers = dict(line.split(b':', 1) for line in request.split(b'\r\n')[1:] if b':' in line)
                        encoding = next((v.strip().decode() for k,v in headers.items() if k.lower()==b'accept-encoding'), '')
                        requests.append({'host': target, 'method': method, 'path': path,'accept_encoding':encoding,'proxy_authorization':next((v.strip().decode() for k,v in headers.items() if k.lower()==b'proxy-authorization'),None)})
                        if target == 'archive.test:443' and path == '/source.zip':
                            body = archive_bytes
                        elif path.endswith('/endpoint.json'):
                            body = endpoint
                        elif path.startswith('/all-packages/since/'):
                            body = b'[]'
                        else:
                            raise ValueError('unexpected request: ' + path)
                        stream.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: ' + str(len(body)).encode()
                                       + b'\r\nConnection: close\r\n\r\n' + body)
                        # Close successful TLS connections normally.
                        try:
                            stream.unwrap().close()
                        except (ssl.SSLError, OSError):
                            pass
                except (OSError, ValueError) as error:
                    if mode.startswith('archive-tls-') and isinstance(error, ssl.SSLError):tls_failures.append(repr(error))
                    else:server_errors.append(repr(error))
        class Server(socketserver.ThreadingTCPServer):
            daemon_threads = True
        server = Server(('127.0.0.1', 0), Proxy)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        project = root/f'{mode}-{label}-{json_mode}'
        project.mkdir()
        cache = project/'home/0.19.1/packages'
        cache.mkdir(parents=True)
        shutil.copy2(cache_source/'registry.dat', cache/'registry.dat')
        for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
            shutil.copytree(cache_source/package, cache/package)
        (cache/'elm/url/1.0.0').mkdir(parents=True)
        shutil.copy2(cache_source/'elm/url/1.0.0/elm.json', cache/'elm/url/1.0.0/elm.json')
        (project/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],
            'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3','elm/url':'1.0.0'},'indirect':{}},
            'test-dependencies':{'direct':{},'indirect':{}}}))
        (project/'Main.elm').write_text('module Main exposing (..)\nx = 1\n')
        proxy = f'http://127.0.0.1:{server.server_address[1]}'
        env = {**os.environ, 'ELM_HOME':str(project/'home'), 'GHCRTS':'-N1 -A16m -c',
               'SYSTEM_CERTIFICATE_PATH':str(root/'ca.pem'), 'SSL_CERT_FILE':str(root/'ca.pem'),
               'SSL_CERT_DIR':str(root/'empty-certs'), 'NO_PROXY':no_proxy_values[mode], 'no_proxy':no_proxy_values[mode],
               **{key:proxy for key in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy']}}
        exclusions = {'no_proxy':'', 'NO_PROXY':''}
        settings = {
            'https-auth-percent': {'https_proxy':proxy.replace('://', '://us%65r:p%40ss@')},
            'https-auth-invalid-utf8': {'https_proxy':proxy.replace('://', '://us%FFr:p%FFss@')},
            'https-auth-unicode': {'https_proxy':proxy.replace('://', '://us%C3%A9r:p%C3%A9ss@')},
            'https-auth-user-only': {'https_proxy':proxy.replace('://', '://user@')},
            'https-auth-empty-user': {'https_proxy':proxy.replace('://', '://:pass@')},
            'https-auth-scalar': {'https_proxy':proxy.replace('://', '://us%F0%9F%98%80r:p%E6%BC%A2ss@')},
            'https-auth-encoded-colon': {'https_proxy':proxy.replace('://', '://user%3Apass@')},
        }[mode]
        env = {key:value for key,value in env.items() if key.lower() not in ('no_proxy','http_proxy','all_proxy')}
        env.pop('REQUEST_METHOD', None)
        env.update(exclusions)
        env.update(settings)
        try:
            result = subprocess.run([str(binary),'make','Main.elm','--output=/dev/null',
                                     *(['--report=json'] if json_mode else [])],cwd=project,env=env,
                                    capture_output=True,timeout=90)
            return {'case':mode,'compiler':label,'json':json_mode,'code':result.returncode,
                    'stdout':result.stdout.decode(),'stderr':result.stderr.decode(),'requests':requests,
                    'proxy_authorization':proxy_authorization, 'proxy_environment':settings, 'archive_via_proxy':bool(proxy_archives), 'no_proxy':no_proxy_values[mode], 'server_errors':server_errors,'connects':connects,'tls_failures':tls_failures,'sources_published':(cache/'elm/url/1.0.0/src').exists(),
                    'verification_cached':(project/'elm-stuff/planexpo-rust/dependencies-built-v1.json').exists()}
        finally:
            server.shutdown()
            server.server_close()
    cases = [(mode, label, binary, json_mode) for mode in modes
             for json_mode in (False,True) for label,binary in binaries]
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        results = list(pool.map(run,cases))
    expected_authorization = {
        'https-auth-percent': 'Basic dXNlcjpwQHNz',
        'https-auth-invalid-utf8': 'Basic dXP9cjpw/XNz',
        'https-auth-unicode': 'Basic dXPpcjpw6XNz',
        'https-auth-user-only': None,
        'https-auth-empty-user': 'Basic OnBhc3M=',
        'https-auth-scalar': 'Basic dXMAcjpwInNz',
        'https-auth-encoded-colon': None,
    }
    checks = []
    for mode in modes:
        for json_mode in (False,True):
            pair = [r for r in results if r['case']==mode and r['json']==json_mode]
            behavior = all(r['code']==1 and not r['sources_published'] and not r['verification_cached']
                           and not r['server_errors'] and any(q['path'].endswith('/endpoint.json') for q in r['requests'])
                           and all(q['proxy_authorization'] is None for q in r['requests'])
                           and (not json_mode or not r['stdout'])
                           and (mode not in expected_authorization or bool(r['connects']) and r['proxy_authorization'] == [expected_authorization[mode]] * len(r['connects'])) for r in pair)
            checks.append({'case':mode,'json':json_mode,'behavior_passed':behavior,
                           'routing_equal':pair[0]['archive_via_proxy']==pair[1]['archive_via_proxy'],
                           'authorization_equal':pair[0]['proxy_authorization']==pair[1]['proxy_authorization']})

unchanged = hashlib.sha256(args.rust.read_bytes()).hexdigest() == sha
report = {'scope':__doc__,'rust_sha256':sha,'binary_unchanged':unchanged,'checks':checks,'results':results,
          'passed':unchanged and all(c['behavior_passed'] and c['routing_equal'] and c['authorization_equal'] for c in checks)}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':report['passed'],'checks':checks}))
raise SystemExit(not report['passed'])
