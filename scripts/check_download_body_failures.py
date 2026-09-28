#!/usr/bin/env python3
"""Offline differential download tests using a process-scoped local TLS authority.

No system trust store, hosts file, user package cache or development server is
modified. Both compilers connect only to a loopback proxy that serves fixtures.
"""
import argparse
import concurrent.futures
import hashlib
import gzip
import io
import json
import os
from pathlib import Path
import shutil
import socketserver
import ssl
import subprocess
import tempfile
import threading
import zipfile
import zlib

parser = argparse.ArgumentParser(description=__doc__)
for key in ('rust', 'elm', 'report'):
    parser.add_argument('--' + key, type=Path, required=True)
args = parser.parse_args()
binaries = [('elm', args.elm.resolve()), ('rust', args.rust.resolve())]
sha = hashlib.sha256(args.rust.read_bytes()).hexdigest()
modes = ('success','endpoint-short','archive-short','chunk-success','endpoint-chunk-short',
         'endpoint-chunk-eof','endpoint-chunk-size','archive-chunk-short',
         'endpoint-chunk-terminator','endpoint-chunk-cr',
         'gzip-success','endpoint-gzip','archive-gzip','gzip-chunk-success',
         'endpoint-gzip-header','endpoint-gzip-crc','endpoint-gzip-short','archive-gzip-crc','gzip-short-success','endpoint-gzip-half',
         'endpoint-gzip-isize','endpoint-gzip-deflate',
         'endpoint-gzip-members','archive-gzip-members','gzip-members-registry','archive-gzip-tail',
         'gzip-optional-success','endpoint-gzip-header-crc',
         'endpoint-gzip-reserved-32','endpoint-gzip-reserved-64','endpoint-gzip-reserved-128',
         'endpoint-gzip-ce-upper','endpoint-gzip-ce-alias','endpoint-gzip-ce-list',
         'endpoint-gzip-ce-spaces','endpoint-gzip-ce-duplicate',
         'archive-gzip-mime-gzip','archive-gzip-mime-xgzip','archive-gzip-mime-octet')
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
    endpoint = json.dumps({'url': 'https://archive.test/source.zip',
                           'hash': hashlib.sha1(archive_bytes).hexdigest()}).encode()

    def run(case):
        mode, label, binary, json_mode = case
        requests, server_errors = [], []
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
                    self.request.sendall(b'HTTP/1.1 200 Connection established\r\n\r\n')
                    with tls.wrap_socket(self.request, server_side=True) as stream:
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
                            short = mode == 'archive-short'
                        elif path.endswith('/endpoint.json'):
                            body = endpoint
                            short = mode == 'endpoint-short'
                        elif path.startswith('/all-packages/since/'):
                            body, short = b'[]', False
                        else:
                            raise ValueError('unexpected request: ' + path)
                        compressed = mode in ('gzip-success','gzip-chunk-success','gzip-short-success','gzip-members-registry','gzip-optional-success') or (mode=='endpoint-gzip' and path.endswith('/endpoint.json')) or (mode=='archive-gzip' and target=='archive.test:443') or (mode.startswith('endpoint-gzip-') and path.endswith('/endpoint.json')) or (mode.startswith('archive-gzip-') and target=='archive.test:443')
                        encoding_header = b'Content-Encoding: gzip\r\n' if compressed else b''
                        if mode.startswith('endpoint-gzip-ce-') and path.endswith('/endpoint.json'):
                            encodings = {'upper': b'GZip', 'alias': b'x-gzip', 'list': b'gzip, gzip',
                                         'spaces': b' gzip ', 'duplicate': b'gzip\r\nContent-Encoding: identity'}
                            encoding_header = b'Content-Encoding: ' + encodings[mode.rsplit('-',1)[1]] + b'\r\n'
                        if mode.startswith('archive-gzip-mime-') and target == 'archive.test:443':
                            types = {'gzip': b'application/gzip', 'xgzip': b'application/x-gzip', 'octet': b'application/octet-stream'}
                            encoding_header += b'Content-Type: ' + types[mode.rsplit('-',1)[1]] + b'\r\n'
                        if compressed:
                            if mode.endswith('-gzip-members') or mode == 'gzip-members-registry':
                                split = len(body)//2
                                # Neither member contains the full JSON/ZIP payload.
                                # An empty member also checks continued decoding.
                                body = (gzip.compress(body[:split], mtime=0)
                                        + gzip.compress(b'', mtime=0)
                                        + gzip.compress(body[split:], mtime=0))
                            else:
                                body = gzip.compress(body, mtime=0)
                            if mode in ('gzip-optional-success','endpoint-gzip-header-crc'):
                                # FEXTRA, FNAME, FCOMMENT and FHCRC, including non-ASCII bytes.
                                header = (body[:3] + bytes([30]) + body[4:10]
                                          + b'\x04\x00test' + b'file-\xff.json\x00comment\x00')
                                checksum = zlib.crc32(header) & 0xffff
                                if mode == 'endpoint-gzip-header-crc':checksum ^= 1
                                body = header + checksum.to_bytes(2,'little') + body[10:]
                            if mode == 'archive-gzip-tail':body += gzip.compress(b'ignored trailing member' * 10000, mtime=0)
                            if mode.endswith('-gzip-header'):body = b'BAD' + body[3:]
                            elif mode.endswith('-gzip-crc'):body = body[:-8] + bytes([body[-8] ^ 255]) + body[-7:]
                            elif mode.endswith('-gzip-short') or mode=='gzip-short-success':body = body[:-5]
                            elif mode.endswith('-gzip-half'):body = body[:len(body)//2]
                            elif mode.endswith('-gzip-isize'):body = body[:-4] + bytes([body[-4] ^ 255]) + body[-3:]
                            elif '-gzip-reserved-' in mode:body = body[:3] + bytes([body[3] | int(mode.rsplit('-',1)[1])]) + body[4:]
                            elif mode.endswith('-gzip-deflate'):body = body[:10] + bytes([body[10] | 6]) + body[11:]
                        sent = body[:len(body)//2] if short else body
                        chunk_target = (mode.startswith('endpoint-chunk-') and path.endswith('/endpoint.json')) or (mode.startswith('archive-chunk-') and target=='archive.test:443')
                        if mode in ('chunk-success','gzip-chunk-success') or chunk_target:
                            size = f'{len(body):x}'.encode()
                            if mode.endswith('-chunk-short'):
                                wire = size + b'\r\n' + body[:len(body)//2]
                            elif mode.endswith('-chunk-eof'):
                                wire = size + b'\r\n' + body + b'\r\n'
                            elif mode.endswith('-chunk-terminator'):
                                wire = size + b'\r\n' + body
                            elif mode.endswith('-chunk-cr'):
                                wire = size + b'\r\n' + body + b'\r'
                            elif mode.endswith('-chunk-size'):
                                wire = b'NOTHEX\r\n' + body + b'\r\n0\r\n\r\n'
                            else:
                                wire = size + b'\r\n' + body + b'\r\n0\r\n\r\n'
                            stream.sendall(b'HTTP/1.1 200 OK\r\n' + encoding_header + b'Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n' + wire)
                        else:
                            stream.sendall(b'HTTP/1.1 200 OK\r\n' + encoding_header + b'Content-Length: ' + str(len(body)).encode()
                                           + b'\r\nConnection: close\r\n\r\n' + sent)
                        # Send a TLS close_notify; only the HTTP body is truncated.
                        try:
                            stream.unwrap().close()
                        except (ssl.SSLError, OSError):
                            pass
                except (OSError, ValueError) as error:
                    server_errors.append(repr(error))
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
               'SSL_CERT_DIR':str(root/'empty-certs'), 'NO_PROXY':'', 'no_proxy':'',
               **{key:proxy for key in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy']}}
        try:
            result = subprocess.run([str(binary),'make','Main.elm','--output=/dev/null',
                                     *(['--report=json'] if json_mode else [])],cwd=project,env=env,
                                    capture_output=True,timeout=90)
            return {'case':mode,'compiler':label,'json':json_mode,'code':result.returncode,
                    'stdout':result.stdout.decode(),'stderr':result.stderr.decode(),'requests':requests,
                    'server_errors':server_errors,'sources_published':(cache/'elm/url/1.0.0/src').exists(),
                    'verification_cached':(project/'elm-stuff/planexpo-rust/dependencies-built-v1.json').exists()}
        finally:
            server.shutdown()
            server.server_close()
    cases = [(mode, label, binary, json_mode) for mode in modes
             for json_mode in (False,True) for label,binary in binaries]
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        results = list(pool.map(run,cases))
    checks = []
    for mode in modes:
        for json_mode in (False,True):
            pair = [r for r in results if r['case']==mode and r['json']==json_mode]
            success = mode in ('success','chunk-success','gzip-success','endpoint-gzip','archive-gzip','gzip-chunk-success','endpoint-gzip-short','gzip-short-success','archive-gzip-tail','gzip-optional-success','endpoint-gzip-ce-spaces','endpoint-gzip-ce-duplicate','archive-gzip-mime-gzip','archive-gzip-mime-xgzip','archive-gzip-mime-octet')
            passed = all(r['code']==(0 if success else 1) and r['sources_published']==success
                         and not r['server_errors'] and (success or not r['verification_cached'])
                         and (mode == 'gzip-members-registry' or any(q['path'].endswith('/endpoint.json') for q in r['requests']))
                         and (mode == 'gzip-members-registry' or mode.startswith('endpoint-') or any(q['host']=='archive.test:443' for q in r['requests']))
                         and all(q['accept_encoding']=='gzip' for q in r['requests'])
                         and (not json_mode or not r['stdout']) for r in pair)
            if json_mode and not success:
                try: equal = json.loads(pair[0]['stderr']) == json.loads(pair[1]['stderr'])
                except ValueError: equal = False
            else: equal = pair[0]['stderr'] == pair[1]['stderr']
            checks.append({'case':mode,'json':json_mode,'behavior_passed':passed,'diagnostic_equal':equal})
unchanged = hashlib.sha256(args.rust.read_bytes()).hexdigest() == sha
report = {'scope':__doc__,'rust_sha256':sha,'binary_unchanged':unchanged,'checks':checks,'results':results,
          'passed':unchanged and all(c['behavior_passed'] and c['diagnostic_equal'] for c in checks)}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':report['passed'],'checks':checks}))
raise SystemExit(not report['passed'])
