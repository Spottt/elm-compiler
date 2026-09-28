#!/usr/bin/env python3
"""Compare project Elm version checks and complete diagnostics with Elm 0.19.1."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
binary = crate / 'target/release/planexpo-elm'
original = Path(os.environ.get('ELM_HOME', str(Path.home() / '.elm'))) / '0.19.1/packages'
cases = [('application', v, valid) for v, valid in [
    ('0.19.1', True), ('65536.19.1', True), ('0.18.0', False), ('0.19.0', False),
    ('0.20.0', False), ('65536.19.0', False), ('65535.65535.65535', False)]]
for lo in ['<', '<=']:
    for hi in ['<', '<=']:
        cases.append(('package', f'0.20.0 {lo} v {hi} 0.21.0', False))
cases += [('package', v, valid) for v, valid in [
    ('0.19.0 <= v < 0.20.0', True), ('0.19.1 <= v < 0.20.0', True),
    ('0.19.1 < v < 0.20.0', False), ('0.18.0 < v < 0.19.1', False),
    ('0.18.0 < v <= 0.19.1', True), ('65536.20.0 <= v < 65536.21.0', False),
    ('65534.65535.65535 <= v <= 65535.65535.65535', False)]]
results, failures = [], []
with tempfile.TemporaryDirectory(prefix='elm-version-parity-') as tmp:
    root = Path(tmp)
    cache = root / 'home/0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copyfile(original / 'registry.dat', cache / 'registry.dat')
    for name in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(original / name / 'src', cache / name / 'src')
        shutil.copyfile(original / name / 'elm.json', cache / name / 'elm.json')
    env = {**os.environ, 'ELM_HOME': str(root / 'home'), 'GHCRTS':'-N1 -A16m -c'}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    env.update(no_proxy='', NO_PROXY='')
    for index, (kind, version, expected) in enumerate(cases):
        project = root / str(index)
        (project / 'src').mkdir(parents=True)
        (project / 'src/Main.elm').write_text('module Main exposing (answer)\nanswer = 42\n')
        if kind == 'application':
            config = {'type':kind,'elm-version':version,'source-directories':['src'],
                      'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},
                      'test-dependencies':{'direct':{},'indirect':{}}}
        else:
            config = {'type':kind,'name':'author/fixture','summary':'Version fixture.',
                      'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
                      'elm-version':version,'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
        (project / 'elm.json').write_text(json.dumps(config))
        row = {'kind':kind,'version':version,'expected':expected}
        for label, executable in [('official',args.elm.resolve()),('rust',binary)]:
            run = subprocess.run([str(executable),'make','src/Main.elm','--output=/dev/null','--report=json'],
                                 cwd=project,env=env,capture_output=True,text=True,timeout=20)
            row[label] = {'accepted':run.returncode == 0}
            if row[label]['accepted'] != expected: failures.append(f'{index}/{label}: acceptance')
            if not expected:
                row[label]['diagnostic'] = json.loads(run.stderr)
                terminal = subprocess.run([str(executable),'make','src/Main.elm','--output=/dev/null'],
                                          cwd=project,env=env,capture_output=True,text=True,timeout=20)
                row[label]['terminal'] = terminal.stderr.rstrip()
                if terminal.returncode == 0: failures.append(f'{index}/{label}: terminal acceptance')
        if row['official'] != row['rust']: failures.append(f'{index}: output differs')
        results.append(row)
report = {'cases':len(results),'failures':failures,'results':results,
          'compiler_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}
if args.report: args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
