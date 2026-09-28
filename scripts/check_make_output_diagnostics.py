#!/usr/bin/env python3
"""Compare output-related make errors and complete compilation streams; JSON decoded, dependency progress recorded separately."""
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
with tempfile.TemporaryDirectory(prefix='make-output-diagnostics-') as directory:
    root = Path(directory)
    cache = root / 'home/0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home() / '.elm')) / '0.19.1/packages'
    shutil.copy2(original / 'registry.dat', cache / 'registry.dat')
    for package, version in [('elm/core', '1.0.5'), ('elm/json', '1.1.3')]:
        shutil.copytree(original / package / version, cache / package / version)
    env = {**os.environ, 'ELM_HOME': str(root / 'home'), 'GHCRTS': '-N1', 'PLANEXPO_ELM_STATS': ''}
    cases = {
        'long-name': ({'LongNestedModuleName/VeryLongModuleName.elm': 'module LongNestedModuleName.VeryLongModuleName exposing (..)\nx = 1\n'}, ['LongNestedModuleName/VeryLongModuleName.elm'], 'out.js'),
        'reversed-modules': ({'Zebra.elm': 'module Zebra exposing (..)\nx = 1\n', 'Alpha.elm': 'module Alpha exposing (..)\nx = 2\n'}, ['Zebra.elm','Alpha.elm'], 'out.js'),
        'mixed-main': ({'Main.elm': (crate/'tests/programs/worker/Main.elm').read_text(), 'Other.elm': 'module Other exposing (..)\nx = 2\n'}, ['Main.elm','Other.elm'], 'out.js'),
        'no-main-js': ({'Main.elm': 'module Main exposing (..)\nx = 1\n'}, ['Main.elm'], 'out.js'),
        'no-main-html': ({'Main.elm': 'module Main exposing (..)\nx = 1\n'}, ['Main.elm'], 'out.html'),
        'multiple-no-main': ({'Main.elm': 'module Main exposing (..)\nx = 1\n', 'Other.elm': 'module Other exposing (..)\nx = 2\n'}, ['Main.elm','Other.elm'], 'out.js'),
        'multiple-html': ({'Main.elm': 'module Main exposing (..)\nx = 1\n', 'Other.elm': 'module Other exposing (..)\nx = 2\n'}, ['Main.elm','Other.elm'], 'out.html'),
        'type-before-no-main': ({'Main.elm': 'module Main exposing (..)\nx = String.length True\n'}, ['Main.elm'], 'out.js'),
        'type-before-multiple-html': ({'Main.elm': 'module Main exposing (..)\nx = String.length True\n', 'Other.elm': 'module Other exposing (..)\nx = 2\n'}, ['Main.elm','Other.elm'], 'out.html'),
    }
    for name, (files, entries, output) in cases.items():
        project = root / name
        project.mkdir()
        shutil.copyfile(crate / 'tests/programs/worker/elm.json', project / 'elm.json')
        for path, source in files.items():
            (project / path).parent.mkdir(parents=True, exist_ok=True)
            (project / path).write_text(source)
        for json_report in (False, True):
            shutil.rmtree(project / 'elm-stuff', ignore_errors=True)
            runs = []
            for compiler in (a.elm, a.rust):
                (project / output).unlink(missing_ok=True)
                flags = ['--report=json'] if json_report else []
                run = subprocess.run([str(compiler.resolve()), 'make', *entries, '--output='+output, *flags], cwd=project, env=env, capture_output=True, timeout=60)
                runs.append({'code': run.returncode, 'stdout': run.stdout.decode(), 'stderr': run.stderr.decode(), 'generated': (project / output).exists()})
            expected = runs[0]['stdout']
            if 'Compiling ...' in expected:
                expected = expected[expected.index('Compiling ...'):]
            actual = runs[1]['stdout']
            if 'Compiling ...' in actual:
                actual = actual[actual.index('Compiling ...'):]
            diagnostics_match = (json.loads(runs[0]['stderr']) == json.loads(runs[1]['stderr'])) if json_report else runs[0]['stderr'] == runs[1]['stderr']
            passed = runs[0]['code'] == runs[1]['code'] == 1 and expected == actual and diagnostics_match
            passed &= not any(run['generated'] for run in runs)
            results.append({'case': name, 'json': json_report, 'passed': passed, 'official': runs[0], 'rust': runs[1]})
            print(name, json_report, 'PASS' if passed else 'FAIL', flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest() == sha
report = {'scope': __doc__, 'compiler_sha256': sha, 'cases': len(results),
          'passed': all(r['passed'] for r in results), 'results': results}
a.report.write_text(json.dumps(report, indent=2) + '\n')
raise SystemExit(not report['passed'])
