#!/usr/bin/env python3
"""Compare make failure summaries and complete diagnostics (decoded JSON or exact terminal text); dependency progress is recorded separately."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
for name in ('elm', 'rust', 'report'):
    p.add_argument('--' + name, type=Path, required=True)
a = p.parse_args()
sha = hashlib.sha256(a.rust.read_bytes()).hexdigest()
results = []
crate = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='make-failure-streams-') as directory:
    root = Path(directory)
    cache = root / 'home/0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home() / '.elm')) / '0.19.1/packages'
    shutil.copy2(original / 'registry.dat', cache / 'registry.dat')
    for package, version in [('elm/core', '1.0.5'), ('elm/json', '1.1.3')]:
        shutil.copytree(original / package / version, cache / package / version)
    env = {**os.environ, 'ELM_HOME': str(root / 'home'), 'GHCRTS': '-N1', 'PLANEXPO_ELM_STATS': ''}
    cases = {
        'syntax': {'Main.elm': 'module Main exposing (..)\nx = [1,\n'},
        'type': {'Main.elm': 'module Main exposing (..)\nx = String.length 1\n'},
        'two-problems-one-module': {'Main.elm': 'module Main exposing (..)\nx = String.length 1\ny = 1 + "a"\n'},
        'two-modules': {'Main.elm': 'module Main exposing (..)\nimport Broken\nx = String.length 1\n', 'Broken.elm': 'module Broken exposing (..)\nx = String.length True\n'},
        'missing-entry': {},
        'independent-modules': {'Main.elm': 'module Main exposing (..)\nimport Broken\nimport Other\nx = 1\n', 'Broken.elm': 'module Broken exposing (..)\nx = String.length True\n', 'Other.elm': 'module Other exposing (..)\nx = String.length 1\n'},
    }
    for name, files in cases.items():
        project = root / name
        project.mkdir()
        shutil.copyfile(crate / 'tests/programs/worker/elm.json', project / 'elm.json')
        for path, source in files.items():
            (project / path).write_text(source)
        for json_report in (False, True):
            runs = []
            for compiler in (a.elm, a.rust):
                flags = ['--report=json'] if json_report else []
                run = subprocess.run([str(compiler.resolve()), 'make', 'Main.elm', '--output=/dev/null', *flags], cwd=project, env=env, capture_output=True, timeout=60)
                runs.append({'code': run.returncode, 'stdout': run.stdout.decode(), 'stderr': run.stderr.decode()})
            expected = runs[0]['stdout']
            if 'Compiling ...' in expected:
                expected = expected[expected.index('Compiling ...'):]
            actual = runs[1]['stdout']
            if 'Compiling ...' in actual:
                actual = actual[actual.index('Compiling ...'):]
            diagnostics_match = (json.loads(runs[0]['stderr']) == json.loads(runs[1]['stderr'])) if json_report else runs[0]['stderr'] == runs[1]['stderr']
            passed = runs[0]['code'] == runs[1]['code'] == 1 and expected == actual and diagnostics_match
            merged = []
            if not json_report:
                for compiler in (a.elm, a.rust):
                    capture = subprocess.run([str(compiler.resolve()), 'make', 'Main.elm', '--output=/dev/null'], cwd=project, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60)
                    text = capture.stdout.decode()
                    if 'Compiling ...' in text:
                        text = text[text.index('Compiling ...'):]
                    merged.append({'code':capture.returncode, 'output':text})
                passed = passed and merged[0] == merged[1]
            results.append({'case': name, 'json': json_report, 'passed': passed, 'official': runs[0], 'rust': runs[1], 'merged':merged})
            print(name, json_report, 'PASS' if passed else 'FAIL', flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest() == sha
report = {'scope': __doc__, 'compiler_sha256': sha, 'cases': len(results),
          'passed': all(r['passed'] for r in results), 'results': results}
a.report.write_text(json.dumps(report, indent=2) + '\n')
raise SystemExit(not report['passed'])
