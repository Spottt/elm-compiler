#!/usr/bin/env python3
"""Check Rust REPL recovery after complete CLI errors, not reference liveness parity.

Elm's evaluation with this malformed proxy exceeded the observation timeout.
Only the subsequent help/reset/exit transcript is compared to the reference;
Rust must print the observed proxy exception without internal diagnostic markers.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']:
    parser.add_argument('--'+key, type=Path, required=True)
args = parser.parse_args()
binaries = [args.elm.resolve(), args.rust.resolve()]
hashes = [hashlib.sha256(p.read_bytes()).hexdigest() for p in binaries]
exception = 'elm: HttpExceptionContentWrapper {unHttpExceptionContentWrapper = InvalidProxyEnvironmentVariable "http_proxy" "not a proxy"}\n'
rows = []
with tempfile.TemporaryDirectory(prefix='elm-repl-raw-recovery-') as directory:
    root = Path(directory)
    home = root/'home'
    cache = home/'0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(original/package, cache/package)
    env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1', 'no_proxy': '', 'NO_PROXY': ''}
    for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
        env[key] = 'not a proxy'
    manifest = Path(__file__).resolve().parents[1]/'tests/programs/worker/elm.json'
    for flags in [[], ['--no-colors']]:
        for label, evaluations in [('expression', '1 + 2\n'), ('declaration', 'x = 1\n'), ('repeated', '1 + 2\n3 + 4\n')]:
            runs = []
            commands = ':reset\n:help\n:exit\n'
            for index, binary in enumerate(binaries):
                project = root/(label+str(len(flags))+str(index))
                project.mkdir()
                shutil.copy2(manifest, project/'elm.json')
                run = subprocess.run([str(binary), 'repl', *flags],
                    input=(evaluations if index else '')+commands,
                    cwd=project, env=env, text=True, capture_output=True, timeout=20)
                runs.append({'code': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
            reference, rust = runs
            attempts = evaluations.count('\n')
            expected_stdout = reference['stdout'].replace('> ', '> '*attempts+'> ', 1)
            passed = (reference['code'] == rust['code'] == 0 and not reference['stderr']
                and rust['stdout'] == expected_stdout and rust['stderr'] == exception*attempts)
            rows.append({'case': label, 'flags': flags, 'passed': passed,
                'reference_without_evaluation': reference, 'rust': rust})
assert hashes == [hashlib.sha256(p.read_bytes()).hexdigest() for p in binaries]
report = {'scope': __doc__, 'binary_sha256': hashes,
    'passed': all(row['passed'] for row in rows), 'results': rows}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'cases': len(rows), 'failures': [r['case'] for r in rows if not r['passed']]}))
raise SystemExit(not report['passed'])
