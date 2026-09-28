#!/usr/bin/env python3
"""Compare Elm diff argument handling without a project.

Checks both output streams, exit status, precedence, and package-cache effects.
With --ansi also compares exact terminal escape sequences on stderr.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
from diff_test_process import run_command
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--rust', type=Path, required=True)
p.add_argument('--report', type=Path, required=True)
p.add_argument('--ansi', action='store_true')
a = p.parse_args()
binaries = [a.elm.resolve(), a.rust.resolve()]
before = hashlib.sha256(binaries[1].read_bytes()).hexdigest()
cases = [
    ['--help'], ['garbage', '--help'], ['--help', '--wat'],
    ['--wat', '--help'], ['elm/core', '1.0.0', '2.0.0', '--help'],
    ['--help', '--report=json'], ['--help', '--help'],
    ['--wat'], ['--report=json'], ['--report', 'json'], ['-h'],
    ['--'], ['-'], ['--help=true'], ['--help=false'],
    ['garbage', '--wat'], ['elm/core', '1.0.0', '2.0.0', '--wat'],
    ['--wat', '--other'], ['--other', '--wat'],
]
cases += [[arg] for arg in ['', 'wat', '1', '1.2', '1.2.3.4', '01.2.3', '1..3', '1.2.x', '١.٢.٣']]
cases += [
    ['elm/core', '1.0.0'], ['elm/core', '1.0.0', 'bad'],
    ['elm/core', 'bad', '1.0.0'], ['1.0.0', 'bad'],
    ['1.0.0', 'bad', '2.0.0'], ['a', 'b', 'c', 'd'],
    ['elm/core', '1.0.0', '2.0.0', 'extra'],
    ['1.0.0', '2.0.0', '3.0.0'], ['1.0.0', '2.0.0', '3.0.0', 'extra'],
]
cases = [(args, False) for args in cases] + [
    (['1.0.0', '2.0.0', '3.0.0'], True),
    (['65536.0.0', '2.0.0', 'extra'], True),
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-diff-arguments-') as temp:
    root = Path(temp)
    for case_index, (args, cached) in enumerate(cases):
        observed = []
        for index, binary in enumerate(binaries):
            home = root / str(case_index) / str(index)
            if cached:
                original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm')) / '0.19.1/packages/registry.dat'
                destination = home/'0.19.1/packages/registry.dat'
                destination.parent.mkdir(parents=True)
                shutil.copy2(original, destination)
            env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1',
                   'http_proxy': 'http://127.0.0.1:1', 'https_proxy': 'http://127.0.0.1:1',
                   'HTTP_PROXY': 'http://127.0.0.1:1', 'HTTPS_PROXY': 'http://127.0.0.1:1'}
            run = run_command([str(binary), 'diff', *args], root, env, a.ansi)
            observed.append({**run, 'cache_files': sorted(str(p.relative_to(home)) for p in home.rglob('*'))})
        passed = observed[0] == observed[1]
        results.append({'args': args, 'cached': cached, 'passed': passed, 'official': observed[0], 'rust': observed[1]})
        print(args, 'PASS' if passed else 'FAIL', flush=True)
assert hashlib.sha256(binaries[1].read_bytes()).hexdigest() == before, 'Compiler changed during validation'
report = {'scope': __doc__, 'ansi': a.ansi, 'cases': len(results), 'passed': all(x['passed'] for x in results),
          'compiler_sha256': before, 'results': results}
a.report.write_text(json.dumps(report, indent=2) + '\n')
raise SystemExit(not report['passed'])
