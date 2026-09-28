#!/usr/bin/env python3
"""Compare import suggestions after four concurrent compiler processes finish."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']:
    parser.add_argument('--' + key, type=Path, required=True)
args = parser.parse_args()
binaries = [args.elm.resolve(), args.rust.resolve()]
hashes = [hashlib.sha256(binary.read_bytes()).hexdigest() for binary in binaries]
modules = ['Widget', 'Puzzle', 'Example', 'Zebra']
results = []
with tempfile.TemporaryDirectory(prefix='elm-import-concurrent-') as directory:
    root = Path(directory)
    for index, binary in enumerate(binaries):
        project = root / str(index)
        project.mkdir()
        home = project / 'home'
        packages = home / '0.19.1/packages'
        packages.mkdir(parents=True)
        original = Path(os.environ.get('ELM_HOME', Path.home() / '.elm')) / '0.19.1/packages'
        shutil.copy2(original / 'registry.dat', packages / 'registry.dat')
        for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
            shutil.copytree(original / package, packages / package)
        shutil.copy2(Path(__file__).resolve().parents[1] / 'tests/programs/worker/elm.json', project / 'elm.json')
        env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1'}
        for name in modules:
            (project / (name + '.elm')).write_text('module ' + name + ' exposing (value)\nvalue = 1\n')
        def compile_module(name):
            return subprocess.run([str(binary), 'make', name + '.elm', '--output=/dev/null', '--report=json'], cwd=project, env=env, text=True, capture_output=True, timeout=90)
        # Warm dependencies before the simultaneous project-cache writes.
        (project / 'Warm.elm').write_text('module Warm exposing (value)\nvalue = 0\n')
        warm = compile_module('Warm')
        assert warm.returncode == 0, warm.stderr
        with ThreadPoolExecutor(max_workers=4) as pool:
            builds = list(pool.map(compile_module, modules))
        assert all(result.returncode == 0 for result in builds), [(r.returncode,r.stderr) for r in builds]
        reports = []
        for name in modules:
            typo = name[:-2] + name[-1] + name[-2]
            (project / 'Main.elm').write_text('module Main exposing (value)\nimport ' + typo + '\nvalue = 1\n')
            result = compile_module('Main')
            assert result.returncode == 1, result.stderr
            report = json.loads(result.stderr)
            for error in report['errors']:
                error['path'] = str(Path(error['path']).relative_to(project))
            message = report['errors'][0]['problems'][0]['message']
            rendered = ''.join(chunk if isinstance(chunk, str) else chunk['string'] for chunk in message)
            assert name in rendered, (name, report)
            reports.append(report)
        results.append(reports)
assert hashes == [hashlib.sha256(binary.read_bytes()).hexdigest() for binary in binaries]
checks = [{'module': name, 'passed': results[0][i] == results[1][i], 'elm': results[0][i], 'rust': results[1][i]} for i,name in enumerate(modules)]
report = {'scope': __doc__, 'binary_sha256': hashes, 'processes_per_compiler': 4, 'checks': checks, 'passed': all(row['passed'] for row in checks)}
args.report.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'checks': len(checks), 'passed': report['passed']}))
raise SystemExit(not report['passed'])
