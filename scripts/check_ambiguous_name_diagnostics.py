#!/usr/bin/env python3
"""Compare complete ambiguous-name diagnostics with Elm (non-TTY).

Covers values, types, constructors, patterns and colliding import aliases.
Operator ambiguities and terminal ANSI styles are not covered here.
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
    ('unqualified-value', 'import Alpha exposing (..)\nimport Beta exposing (..)\nvalue\n'),
    ('unqualified-variant', 'import Alpha exposing (..)\nimport Beta exposing (..)\nToken\n'),
    ('unqualified-type', 'import Alpha exposing (..)\nimport Beta exposing (..)\ntype alias Example = Token\n'),
    ('qualified-value', 'import Alpha as X\nimport Beta as X\nX.value\n'),
    ('qualified-variant', 'import Alpha as X\nimport Beta as X\nX.Token\n'),
    ('qualified-type', 'import Alpha as X\nimport Beta as X\ntype alias Example = X.Token\n'),
    ('pattern-variant', 'import Alpha exposing (..)\nimport Beta exposing (..)\n\\Token -> ()\n'),
    ('three-imports', 'import Alpha as X\nimport Beta as X\nimport Gamma as X\nX.value\n'),
    ('module-name-alias', 'import Alpha\nimport Beta as Alpha\nAlpha.value\n'),
    ('three-exposed-imports', 'import Alpha exposing (..)\nimport Beta exposing (..)\nimport Gamma exposing (..)\nvalue\n'),
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
    env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1', 'NO_PROXY': '', 'no_proxy': ''}
    for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    for name, source in cases:
        project = root/name
        project.mkdir()
        for module in ['Alpha', 'Beta', 'Gamma']:
            (project/(module+'.elm')).write_text('module '+module+' exposing (..)\nvalue = ()\ntype Token = Token\n')
        (project/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
        runs = []
        for binary in [args.elm, args.rust]:
            run = subprocess.run([str(binary.resolve()), 'repl', '--no-colors'], cwd=project, env=env,
                input=source+':exit\n', text=True, capture_output=True, timeout=30)
            runs.append({'code': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
        results.append({'case': name, 'passed': runs[0] == runs[1] and '-- ' in runs[0]['stderr'],
                        'official': runs[0], 'rust': runs[1]})
report = {'scope': __doc__, 'passed': all(r['passed'] for r in results), 'cases': len(results), 'results': results,
          'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest()}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
raise SystemExit(not report['passed'])
