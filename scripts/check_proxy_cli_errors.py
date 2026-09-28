#!/usr/bin/env python3
"""Invalid proxy startup: exact stdout/stderr and status for public CLI commands.

All commands use a malformed proxy, an isolated project and private package
cache; they must fail at proxy validation before any network request.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
from diff_test_process import run_command

p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']:
    p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
binaries = [('elm', a.elm.resolve()), ('rust', a.rust.resolve())]
hashes = [hashlib.sha256(b.read_bytes()).hexdigest() for _, b in binaries]
commands = [('make', ['make', 'Main.elm', '--output=/dev/null']),
    ('make-json', ['make', 'Main.elm', '--output=/dev/null', '--report=json']),
    ('init', ['init']), ('install', ['install', 'elm/url']),
    ('diff', ['diff', 'elm/core', '1.0.4', '1.0.5']), ('bump', ['bump'])]
rows = []
with tempfile.TemporaryDirectory(prefix='elm-proxy-cli-') as directory:
    root = Path(directory)
    home = root/'home'
    packages = home/'0.19.1/packages'
    packages.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', packages/'registry.dat')
    for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(original/package, packages/package)
    env = {**os.environ, 'ELM_HOME':str(home), 'GHCRTS':'-N1',
        'http_proxy':'not a proxy', 'https_proxy':'not a proxy',
        'HTTP_PROXY':'not a proxy', 'HTTPS_PROXY':'not a proxy',
        'no_proxy':'', 'NO_PROXY':''}
    for label, command in commands:
        for mode in ['plain', 'ansi', 'ansi-stdout']:
            runs = []
            for compiler, binary in binaries:
                project = root/f'{label}-{mode}-{compiler}'
                project.mkdir()
                if label != 'init':
                    (project/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
                    (project/'Main.elm').write_text('module Main exposing (..)\nx = 1\n')
                result = run_command([str(binary), *command], project, env, mode!='plain', 'stdout' if mode=='ansi-stdout' else 'stderr', input_text='y\n')
                assert result['code'] == 1 and 'InvalidProxyEnvironmentVariable' in result['stderr'], result
                runs.append(result)
            row = {'command':label,'mode':mode,'passed':runs[0]==runs[1],'elm':runs[0],'rust':runs[1]}
            rows.append(row)
            if not row['passed']: print(json.dumps(row), flush=True)
assert hashes == [hashlib.sha256(b.read_bytes()).hexdigest() for _, b in binaries]
a.report.write_text(json.dumps({'scope':__doc__,'binary_sha256':hashes,'passed':all(r['passed'] for r in rows),'cases':rows},indent=2)+'\n')
print('checks',len(rows),'passed',sum(r['passed'] for r in rows))
raise SystemExit(not all(r['passed'] for r in rows))
