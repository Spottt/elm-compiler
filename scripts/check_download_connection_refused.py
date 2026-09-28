#!/usr/bin/env python3
"""Offline differential IPv4/IPv6 connection diagnostic test.

IPv6 must match the official bracketed-host resolver failure, even when a
loopback archive server is listening. Acceptance and complete diagnostics are
compared, not exempted.

Only process-specific socket descriptor numbers are normalized for comparison.

No system trust store, hosts file, user package cache or development server is
modified. Both compilers connect only to a loopback proxy that serves fixtures.
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
modes = ('archive-refused', 'archive-refused-ipv6', 'archive-listening-ipv6')
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
    archive_servers = []
    archive_errors = []
    class Archive(socketserver.BaseRequestHandler):
        def handle(self):
            try:
                self.request.settimeout(5)
                request = b''
                while b'\r\n\r\n' not in request:
                    chunk = self.request.recv(4096)
                    if not chunk:
                        raise ValueError('incomplete archive request')
                    request += chunk
                    if len(request) > 16384:
                        raise ValueError('oversized archive request')
                if request.split(b'\r\n', 1)[0] != b'GET /source.zip HTTP/1.1':
                    raise ValueError('unexpected archive request')
                self.request.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: '
                    + str(len(archive_bytes)).encode()
                    + b'\r\nConnection: close\r\n\r\n' + archive_bytes)
            except (OSError, ValueError) as error:
                archive_errors.append(repr(error))
    class IPv6ArchiveServer(socketserver.ThreadingTCPServer):
        address_family = socket.AF_INET6
        daemon_threads = True
    endpoints = {}
    for mode, family, host, url_host in [
        ('archive-refused', socket.AF_INET, '127.0.0.1', '127.0.0.1'),
        ('archive-refused-ipv6', socket.AF_INET6, '::1', '[::1]'),
        ('archive-listening-ipv6', socket.AF_INET6, '::1', '[::1]'),
    ]:
        if mode == 'archive-listening-ipv6':
            archive_server = IPv6ArchiveServer((host, 0), Archive)
            archive_servers.append(archive_server)
            threading.Thread(target=archive_server.serve_forever, daemon=True).start()
            refused = archive_server.socket
        else:
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
                    if method != 'CONNECT' or target not in ('package.elm-lang.org:443', 'archive.test:443'):
                        raise ValueError('unexpected proxy destination: ' + target)
                    connects.append(target)
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
                        requests.append({'host': target, 'method': method, 'path': path,'accept_encoding':encoding})
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
               'SSL_CERT_DIR':str(root/'empty-certs'), 'NO_PROXY':'127.0.0.1,::1,[::1]', 'no_proxy':'127.0.0.1,::1,[::1]',
               **{key:proxy for key in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy']}}
        try:
            result = subprocess.run([str(binary),'make','Main.elm','--output=/dev/null',
                                     *(['--report=json'] if json_mode else [])],cwd=project,env=env,
                                    capture_output=True,timeout=90)
            return {'case':mode,'compiler':label,'json':json_mode,'code':result.returncode,
                    'stdout':result.stdout.decode(),'stderr':result.stderr.decode(),'requests':requests,
                    'server_errors':server_errors,'connects':connects,'tls_failures':tls_failures,'sources_published':(cache/'elm/url/1.0.0/src').exists(),
                    'verification_cached':(project/'elm-stuff/planexpo-rust/dependencies-built-v1.json').exists()}
        finally:
            server.shutdown()
            server.server_close()
    cases = [(mode, label, binary, json_mode) for mode in modes
             for json_mode in (False,True) for label,binary in binaries]
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
            results = list(pool.map(run,cases))
    finally:
        for archive_server in archive_servers:
            archive_server.shutdown()
            archive_server.server_close()
        for reserved_socket in reserved_sockets:
            reserved_socket.close()
    checks = []
    for mode in modes:
        for json_mode in (False, True):
            pair = [r for r in results if r['json'] == json_mode and r['case'] == mode]
            def normalize(text):
                return re.sub(r'<socket: [0-9]+>', '<socket: FD>', text)
            behavior = all(r['code'] == 1 and not r['sources_published'] and not r['verification_cached']
                           and not r['server_errors'] and not r['tls_failures']
                           and any(q['path'].endswith('/endpoint.json') for q in r['requests'])
                           and (not json_mode or not r['stdout']) for r in pair)
            descriptors = all('Network.Socket.connect:' not in r['stderr'] or re.search(r'<socket: [0-9]+>', r['stderr']) for r in pair)
            try:
                messages = [json.loads(normalize(r['stderr'])) if json_mode else normalize(r['stderr']) for r in pair]
                equal = messages[0] == messages[1]
            except ValueError:
                equal = False
            checks.append({'case':mode,'json':json_mode,'behavior_passed':behavior,
                           'outcome_equal': all(pair[0][key] == pair[1][key] for key in ('code', 'sources_published', 'verification_cached')),
                           'socket_descriptor_valid':bool(descriptors),'diagnostic_equal':bool(descriptors) and equal})

unchanged = hashlib.sha256(args.rust.read_bytes()).hexdigest() == sha
report = {'scope':__doc__,'rust_sha256':sha,'binary_unchanged':unchanged,'checks':checks,'results':results,'archive_server_errors':archive_errors,
          'passed':unchanged and not archive_errors and all(c['behavior_passed'] and c['diagnostic_equal'] for c in checks)}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':report['passed'],'checks':checks}))
raise SystemExit(not report['passed'])
