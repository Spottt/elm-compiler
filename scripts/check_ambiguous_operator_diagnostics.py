#!/usr/bin/env python3
"""Compare ambiguous imported operator diagnostics with Elm (non-TTY).

Uses isolated core package fixtures to expose competing operators.
Compares complete streams and source regions; ANSI styles remain outside scope.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--rust', type=Path, default=crate/'target/debug/planexpo-elm')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
cases = [
    ('infix', 'import Alpha exposing (..)\nimport Beta exposing (..)\n1 %% 2\n'),
    ('function', 'import Alpha exposing (..)\nimport Beta exposing (..)\n(%%)\n'),
    ('three-imports', 'import Alpha exposing (..)\nimport Beta exposing (..)\nimport Gamma exposing (..)\n1 %% 2\n'),
    ('unicode-prefix', 'import Alpha exposing (..)\nimport Beta exposing (..)\n("é", 1 %% 2)\n'),
    ('repeated-symbol', 'import Alpha exposing (..)\nimport Beta exposing (..)\n("%%", 1 %% 2)\n'),
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-repl-diagnostics-') as tmp:
    root = Path(tmp)
    home = root/'home'
    cache = home/'0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    for package in ['core/1.0.5', 'json/1.1.3']:
        shutil.copytree(original/'elm'/package, cache/'elm'/package)
    core = cache/'elm/core/1.0.5'
    for cached in ['artifacts.dat', 'docs.json']:
        (core/cached).unlink(missing_ok=True)
    manifest = json.loads((core/'elm.json').read_text())
    manifest['exposed-modules']['Test operators'] = ['Alpha', 'Beta', 'Gamma']
    (core/'elm.json').write_text(json.dumps(manifest))
    for module in ['Alpha', 'Beta', 'Gamma']:
        (core/'src'/(module+'.elm')).write_text('module '+module+' exposing ((%%))\ninfix left 5 (%%) = choose\nchoose a b = a\n')
    env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1', 'NO_PROXY': '', 'no_proxy': ''}
    for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    for name, source in cases:
        project = root/name
        project.mkdir()
        (project/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
        runs = []
        for binary in [args.elm, args.rust]:
            run = subprocess.run([str(binary.resolve()), 'repl', '--no-colors'], cwd=project, env=env,
                input=source+':exit\n', text=True, capture_output=True, timeout=30)
            runs.append({'code': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
        results.append({'case': name, 'passed': runs[0] == runs[1] and runs[0]['stderr'].count('-- AMBIGUOUS NAME ') == 1 and 'Alpha.%%' in runs[0]['stderr'] and 'Beta.%%' in runs[0]['stderr'],
                        'official': runs[0], 'rust': runs[1]})
report = {'scope': __doc__, 'passed': all(r['passed'] for r in results), 'cases': len(results), 'results': results,
          'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest()}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
raise SystemExit(not report['passed'])
