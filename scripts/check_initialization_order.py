#!/usr/bin/env python3
"""Compare precedence and categories of outline, registry, and project errors."""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
binary = crate / 'target/release/planexpo-elm'
app = {'type':'application','elm-version':'0.19.1','source-directories':['src'],
       'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},
       'test-dependencies':{'direct':{},'indirect':{}}}
pkg = {'type':'package','name':'author/fixture','summary':'Initialization order fixture.',
       'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
       'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
cases = []
for kind, base in [('app',app),('package',pkg)]:
    cases.append((kind+'-valid',base,'TROUBLE VERIFYING DEPENDENCIES'))
    config = copy.deepcopy(base)
    config['elm-version'] = '0.18.0' if kind == 'app' else '0.20.0 <= v < 0.21.0'
    cases.append((kind+'-version',config,'ELM VERSION MISMATCH'))
    config = copy.deepcopy(base)
    if kind == 'app': config['test-dependencies']['direct']['elm/core'] = '1.0.5'
    else: config['test-dependencies']['elm/core'] = '1.0.0 <= v < 2.0.0'
    cases.append((kind+'-duplicate',config,'ERROR IN DEPENDENCIES'))
    config = copy.deepcopy(base)
    if kind == 'app': del config['dependencies']['direct']['elm/core']
    else: del config['dependencies']['elm/core']
    cases.append((kind+'-missing-core',config,'MISSING DEPENDENCY'))
results, failures = [], []
with tempfile.TemporaryDirectory(prefix='elm-init-order-') as tmp:
    for cached in [False, True, "empty", "truncated"]:
        for name, config, title in cases:
            expected = title if cached is True or title == 'MISSING DEPENDENCY' else 'PROBLEM LOADING PACKAGE LIST'
            row = {'case':name,'registry_cached':cached,'expected_title':expected}
            for label, executable in [('official',args.elm.resolve()),('rust',binary)]:
                project = Path(tmp) / str(cached) / name / label
                (project / 'src').mkdir(parents=True)
                (project / 'src/Main.elm').write_text('module Main exposing (answer)\nanswer = 42\n')
                (project / 'elm.json').write_text(json.dumps(config,indent=2)+'\n')
                home = project / 'home'
                packages = home / '0.19.1/packages'
                packages.mkdir(parents=True)
                if cached is True: (packages / 'registry.dat').write_bytes(struct.pack('>qq',0,0))
                elif cached in ['empty', 'truncated']:
                    (packages / 'registry.dat').write_bytes(b'' if cached == 'empty' else b'\0' * 8)
                env = {**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1 -A16m -c'}
                for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
                    env[key] = 'http://127.0.0.1:9'
                env.update(no_proxy='',NO_PROXY='')
                run = subprocess.run([str(executable),'make','src/Main.elm','--output=/dev/null','--report=json'],
                                     cwd=project,env=env,capture_output=True,text=True,timeout=20)
                # Elm warns about corrupt binary caches before its JSON report.
                report = next(json.JSONDecoder().raw_decode(line)[0] for line in run.stderr.splitlines() if line.startswith('{'))
                if cached in ['empty', 'truncated']:
                    before = b'' if cached == 'empty' else b'\0' * 8
                    if (packages / 'registry.dat').read_bytes() != before:
                        failures.append(f'{cached}/{name}/{label}: changed cache after failed repair')
                row[label] = report
                path = None if expected == 'PROBLEM LOADING PACKAGE LIST' else 'elm.json'
                if run.returncode == 0 or (report['type'],report['title'],report['path']) != ('error',expected,path):
                    failures.append(f'{cached}/{name}/{label}: {report["title"]}')
            if expected != 'PROBLEM LOADING PACKAGE LIST' and row['official'] != row['rust']:
                failures.append(f'{cached}/{name}: full diagnostic differs')
            results.append(row)
report = {'cases':len(results),'failures':failures,'results':results,
          'compiler_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
          'limits':'Transport-specific HTTP error text is not compared.'}
if args.report: args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
