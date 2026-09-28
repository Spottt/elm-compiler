#!/usr/bin/env python3
"""Offline differential certificate rejection tests using process-scoped TLS authorities.

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
modes = ('success', 'archive-tls-name', 'archive-tls-issuer', 'archive-tls-expired', 'archive-tls-future', 'archive-tls-purpose', 'archive-tls-signature', 'archive-tls-critical', 'archive-tls-expired-name', 'archive-tls-expired-issuer', 'archive-tls-signature-name', 'archive-tls-signature-expired')
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
    (root/'wrong-name-extensions').write_text((root/'extensions').read_text().replace(',DNS:archive.test',''))
    openssl('x509','-req','-in','server.csr','-CA','ca.pem','-CAkey','ca.key','-CAcreateserial',
            '-out','wrong-name.pem','-days','1','-extfile','wrong-name-extensions')
    wrong_name = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    wrong_name.load_cert_chain(root/'wrong-name.pem',root/'server.key')
    openssl('req','-x509','-newkey','rsa:2048','-nodes','-keyout','other-ca.key','-out','other-ca.pem',
            '-days','1','-subj','/CN=Untrusted parity CA','-addext','basicConstraints=critical,CA:TRUE',
            '-addext','keyUsage=critical,keyCertSign,cRLSign')
    openssl('x509','-req','-in','server.csr','-CA','other-ca.pem','-CAkey','other-ca.key','-CAcreateserial',
            '-out','wrong-issuer.pem','-days','1','-extfile','extensions')
    wrong_issuer = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    wrong_issuer.load_cert_chain(root/'wrong-issuer.pem',root/'server.key')
    (root/'index').write_text('')
    (root/'serial').write_text('1000\n')
    (root/'ca.cnf').write_text(f"[ca]\ndefault_ca=local\n[local]\ndatabase={root}/index\n"
        f"serial={root}/serial\nnew_certs_dir={root}\ncertificate={root}/ca.pem\n"
        f"private_key={root}/ca.key\ndefault_md=sha256\npolicy=subject\nunique_subject=no\n"
        "[subject]\ncommonName=supplied\n")
    certificate_contexts = {'archive-tls-name':wrong_name, 'archive-tls-issuer':wrong_issuer}
    for name, start, end in [('expired','20000101000000Z','20010101000000Z'),
                             ('future','20990101000000Z','21000101000000Z')]:
        openssl('ca','-batch','-notext','-config','ca.cnf','-extfile','extensions',
                '-in','server.csr','-out',name+'.pem','-startdate',start,'-enddate',end)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(root/(name+'.pem'),root/'server.key')
        certificate_contexts['archive-tls-'+name] = context
    (root/'wrong-purpose-extensions').write_text((root/'extensions').read_text().replace('serverAuth','clientAuth'))
    openssl('x509','-req','-in','server.csr','-CA','ca.pem','-CAkey','ca.key','-CAcreateserial',
            '-out','wrong-purpose.pem','-days','1','-extfile','wrong-purpose-extensions')
    wrong_purpose = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    wrong_purpose.load_cert_chain(root/'wrong-purpose.pem',root/'server.key')
    certificate_contexts['archive-tls-purpose'] = wrong_purpose
    openssl('x509','-in','server.pem','-outform','DER','-out','bad-signature.der')
    signature = bytearray((root/'bad-signature.der').read_bytes())
    signature[-1] ^= 1
    (root/'bad-signature.der').write_bytes(signature)
    openssl('x509','-inform','DER','-in','bad-signature.der','-out','bad-signature.pem')
    bad_signature = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    bad_signature.load_cert_chain(root/'bad-signature.pem',root/'server.key')
    certificate_contexts['archive-tls-signature'] = bad_signature
    (root/'critical-extensions').write_text((root/'extensions').read_text()+'1.2.3.4=critical,DER:01:01:FF\n')
    openssl('x509','-req','-in','server.csr','-CA','ca.pem','-CAkey','ca.key','-CAcreateserial',
            '-out','critical.pem','-days','1','-extfile','critical-extensions')
    critical = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    critical.load_cert_chain(root/'critical.pem',root/'server.key')
    certificate_contexts['archive-tls-critical'] = critical
    openssl('ca','-batch','-notext','-config','ca.cnf','-extfile','wrong-name-extensions',
            '-in','server.csr','-out','expired-name.pem','-startdate','20000101000000Z','-enddate','20010101000000Z')
    # Same certificate dates and host; sign with an untrusted issuer.
    (root/'other-ca.cnf').write_text((root/'ca.cnf').read_text().replace('certificate='+str(root/'ca.pem'),'certificate='+str(root/'other-ca.pem')).replace('private_key='+str(root/'ca.key'),'private_key='+str(root/'other-ca.key')))
    openssl('ca','-batch','-notext','-config','other-ca.cnf','-extfile','extensions',
            '-in','server.csr','-out','expired-issuer.pem','-startdate','20000101000000Z','-enddate','20010101000000Z')
    for name, original in [('signature-name','wrong-name'),('signature-expired','expired')]:
        openssl('x509','-in',original+'.pem','-outform','DER','-out',name+'.der')
        data = bytearray((root/(name+'.der')).read_bytes())
        data[-1] ^= 1
        (root/(name+'.der')).write_bytes(data)
        openssl('x509','-inform','DER','-in',name+'.der','-out',name+'.pem')
    for name in ['expired-name','expired-issuer','signature-name','signature-expired']:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(root/(name+'.pem'),root/'server.key')
        certificate_contexts['archive-tls-'+name] = context
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
               'SSL_CERT_DIR':str(root/'empty-certs'), 'NO_PROXY':'', 'no_proxy':'',
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
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        results = list(pool.map(run,cases))
    checks = []
    for mode in modes:
        for json_mode in (False,True):
            pair = [r for r in results if r['case']==mode and r['json']==json_mode]
            success = mode == 'success'
            def expected_success(result):
                # Observed reference behavior, not a parity exemption: diagnostic
                # comparison below still fails when Rust rejects this certificate.
                return success or (mode in ('archive-tls-purpose', 'archive-tls-critical') and result['compiler'] == 'elm')
            passed = all(r['code']==(0 if expected_success(r) else 1) and r['sources_published']==expected_success(r)
                         and r['verification_cached'] == (expected_success(r) and r['compiler'] == 'rust')
                         and not r['server_errors'] and (expected_success(r) or (not r['verification_cached'] and bool(r['tls_failures'])))
                         and any(q['path'].endswith('/endpoint.json') for q in r['requests'])
                         and (mode.startswith('endpoint-') or 'archive.test:443' in r['connects'])
                         and all(q['accept_encoding']=='gzip' for q in r['requests'])
                         and (not json_mode or not r['stdout']) for r in pair)
            if json_mode and not success:
                try: equal = json.loads(pair[0]['stderr']) == json.loads(pair[1]['stderr'])
                except ValueError: equal = False
            else: equal = pair[0]['stderr'] == pair[1]['stderr']
            checks.append({'case':mode,'json':json_mode,'behavior_passed':passed,'outcome_equal':all(pair[0][key] == pair[1][key] for key in ('code','sources_published')),'diagnostic_equal':equal})
unchanged = hashlib.sha256(args.rust.read_bytes()).hexdigest() == sha
report = {'scope':__doc__,'rust_sha256':sha,'binary_unchanged':unchanged,'checks':checks,'results':results,
          'passed':unchanged and all(c['behavior_passed'] and c['outcome_equal'] and c['diagnostic_equal'] for c in checks)}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':report['passed'],'checks':checks}))
raise SystemExit(not report['passed'])
