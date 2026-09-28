#!/usr/bin/env python3
"""Compare offline package metadata rejection and cache cleanup with Elm 0.19.1."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import struct
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
original = Path(os.environ.get('ELM_HOME', str(Path.home() / '.elm'))) / '0.19.1/packages'
binary = crate / 'target/release/planexpo-elm'
results, failures = [], []
cases = ['valid', 'missing-license', 'unknown-license', 'missing-test-dependencies',
         'invalid-json', 'invalid-utf8', 'application-outline', 'corrupt-without-sources',
         'valid-without-sources', 'long-name', 'max-length-name']
with tempfile.TemporaryDirectory(prefix='elm-metadata-cache-') as tmp:
    for kind in ['application', 'package']:
        for case in cases:
            row = {'kind': kind, 'case': case}
            corrupt = case not in ['valid', 'valid-without-sources']
            for compiler, executable in [('official', args.elm.resolve()), ('rust', binary)]:
                project = Path(tmp) / kind / case / compiler
                (project / 'src').mkdir(parents=True)
                (project / 'src/Main.elm').write_text('module Main exposing (answer)\nanswer = 42\n')
                if kind == 'application':
                    config = {'type':'application', 'elm-version':'0.19.1', 'source-directories':['src'],
                              'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},
                              'test-dependencies':{'direct':{},'indirect':{}}}
                else:
                    config = {'type':'package','name':'author/fixture','summary':'Metadata parity fixture.',
                              'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
                              'elm-version':'0.19.0 <= v < 0.20.0',
                              'dependencies':{'elm/core':'1.0.5 <= v < 1.0.6','elm/json':'1.1.3 <= v < 1.1.4'},
                              'test-dependencies':{}}
                custom = None
                if case in ['long-name', 'max-length-name']:
                    custom = ('author/a-package-with-a-long-name-to-exercise-wrapping' if case == 'long-name'
                              else 'a' * 255 + '/' + 'b' * 255)
                    deps = config['dependencies']['direct'] if kind == 'application' else config['dependencies']
                    deps[custom] = '1.0.0' if kind == 'application' else '1.0.0 <= v < 2.0.0'
                (project / 'elm.json').write_text(json.dumps(config))
                home = project / 'home'
                cache = home / '0.19.1/packages'
                cache.mkdir(parents=True)
                shutil.copyfile(original / 'registry.dat', cache / 'registry.dat')
                for name in ['elm/core/1.0.5','elm/json/1.1.3']:
                    shutil.copytree(original / name / 'src', cache / name / 'src')
                    shutil.copyfile(original / name / 'elm.json', cache / name / 'elm.json')
                root = cache / 'elm/json/1.1.3'
                if custom:
                    packages = [(custom, (1,0,0)), ('elm/core',(1,0,5)), ('elm/json',(1,1,3))]
                    data = struct.pack('>qq',len(packages),len(packages))
                    for name, version in packages:
                        for part in name.split('/'):
                            encoded = part.encode(); data += bytes([len(encoded)]) + encoded
                        data += bytes(version) + struct.pack('>q',0)
                    (cache / 'registry.dat').write_bytes(data)
                    target = cache / custom / '1.0.0'
                    shutil.copytree(root, target)
                    root = target

                path = root / 'elm.json'
                value = json.loads(path.read_text())
                if case in ['missing-license', 'corrupt-without-sources', 'long-name', 'max-length-name']:
                    del value['license']
                elif case == 'unknown-license': value['license'] = 'Unknown-License'
                elif case == 'missing-test-dependencies': del value['test-dependencies']
                elif case == 'application-outline': value = config | {'type':'application'}
                payload = json.dumps(value).encode()
                if case == 'invalid-json': payload = b'not json'
                if case == 'invalid-utf8': payload = b'\xff\xfe'
                path.write_bytes(payload)
                if case.endswith('without-sources'): shutil.rmtree(root / 'src')
                (root / 'KEEP').write_text('preserve')
                env = {**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1 -A16m -c'}
                for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
                    env[key] = 'http://127.0.0.1:9'
                env.update(no_proxy='', NO_PROXY='')
                run = subprocess.run([str(executable),'make','src/Main.elm','--output=/dev/null','--report=json'],
                                     cwd=project,env=env,capture_output=True,timeout=20)
                outcome = {'accepted':run.returncode == 0,'metadata_exists':path.exists(),
                           'sources_exist':(root / 'src').exists(),'other_files_preserved':(root / 'KEEP').read_text()=='preserve',
                           'stderr':run.stderr.decode(errors='replace')}
                if corrupt:
                    outcome['diagnostic'] = json.loads(run.stderr)
                    # Restore precisely the same corruption for terminal rendering.
                    path.write_bytes(payload)
                    terminal = subprocess.run([str(executable),'make','src/Main.elm','--output=/dev/null'],
                                              cwd=project,env=env,capture_output=True,timeout=20)
                    outcome['terminal'] = terminal.stderr.decode(errors='replace').rstrip()
                    if terminal.returncode == 0 or path.exists():
                        failures.append(f'{kind}/{case}/{compiler}: terminal retry did not reject and remove metadata')
                row[compiler] = outcome
                expected = (case == 'valid', not corrupt, not case.endswith('without-sources'), True)
                actual = tuple(outcome[k] for k in ['accepted','metadata_exists','sources_exist','other_files_preserved'])
                if actual != expected: failures.append(f'{kind}/{case}/{compiler}: {actual} != {expected}')
            if corrupt:
                for output in ['diagnostic', 'terminal']:
                    if row['official'][output] != row['rust'][output]:
                        failures.append(f'{kind}/{case}: {output} differs')
            results.append(row)
report = {'cases':len(results),'failures':failures,'results':results,
          'compiler_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}
if args.report: args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
