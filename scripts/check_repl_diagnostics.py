#!/usr/bin/env python3
"""Compare complete REPL syntax and type diagnostic streams with Elm (non-TTY).

This suite covers malformed input and syntax errors in imported modules;
Default colors and --no-colors are compared over pipes; terminal editing and
arbitrary interactive sessions are not covered by these cases.
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
    ('list-comma', '[1,\n\n'),
    ('tuple-comma', '(1,\n\n'),
    ('record-value', '{ a =\n\n'),
    ('if-branch', 'if True then\n\n'),
    ('lambda-body', '\\x ->\n\n'),
    ('leading-zero', '01\n\n'),
    ('character-width', "'ab'\n\n"),
    ('unfinished-string', '"hello\n\n'),
    ('imported-syntax', 'import Broken\n'),
    ('after-definition', 'x = ()\n[1,\n\n'),
    ('mixed-list', '[1, "two"]\n'),
    ('if-condition-type', 'if 1 then 2 else 3\n'),
    ('if-branch-type', 'if True then 1 else "two"\n'),
    ('call-argument-type', 'String.length 1\n'),
    ('call-non-function', '1 2\n'),
    ('record-missing-field', '{ a = 1 }.b\n'),
    ('infinite-type', '\\x -> x x\n'),
    ('case-missing-pattern', 'case Just 1 of\n  Nothing -> 0\n\n'),
    ('annotation-mismatch', 'x : String\nx = 1\n\n'),
    ('retained-definition-type', 'x = 1\nString.length x\nx + 1\n'),
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
        (project/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
        if name == 'imported-syntax':
            (project/'Broken.elm').write_text('module Broken exposing (..)\nbad = [1,\n')
        for color_flags in (['--no-colors'], []):
            runs = []
            for binary in [args.elm, args.rust]:
                run = subprocess.run([str(binary.resolve()), 'repl', *color_flags], cwd=project, env=env,
                    input=source+':exit\n', text=True, capture_output=True, timeout=30)
                runs.append({'code': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
            results.append({'case': name, 'colors': not color_flags, 'passed': runs[0] == runs[1] and '-- ' in runs[0]['stderr'],
                            'official': runs[0], 'rust': runs[1]})
report = {'scope': __doc__, 'passed': all(r['passed'] for r in results), 'cases': len(results), 'results': results,
          'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest()}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
raise SystemExit(not report['passed'])
