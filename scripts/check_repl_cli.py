#!/usr/bin/env python3
"""Compare REPL CLI output, flags and cached standalone startup with Elm.

Does not exercise terminal editing/history/signals, online acquisition, or ANSI
argument diagnostics on a TTY. Each case compares status and both streams.
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
    ('help', ['--help'], '', False),
    ('commands', ['--no-colors'], ':help\n:unknown extra\nport broken\n:reset\n:quit\n', False),
    ('eof', [], '', False),
    ('invalid-proxy-eof', [], '', False),
    ('invalid-proxy-exit', [], ':exit\n', False),
    ('invalid-proxy-help', [], ':help\n:exit\n', False),
    ('continuation-eof', [], '[1,\n', False),
    ('value-colors', [], 'Just ()\n:exit\n', False),
    ('value-plain', ['--no-colors'], 'Just ()\n:exit\n', False),
    ('interpreter', ['--interpreter='+shutil.which('node'), '--no-colors'], '1 + 2\n:exit\n', False),
    ('standalone', ['--no-colors'], '1 + 2\nimport Html\nHtml.text "hello"\n:exit\n', True),
    ('annotation-names', ['--no-colors'],
     'identity : message -> message\nidentity x = x\n\nidentity\n'
     'keep : { row | field : item } -> { row | field : item }\nkeep x = x\n\nkeep\n:exit\n', False),
    ('missing-default-interpreter', [], '', False),
    ('fallback-nodejs', ['--no-colors'], '1 + 2\n:exit\n', False),
]
for label, flags in [
    ('unknown-flag', ['--bogus']),
    ('nearby-flag', ['--interprter=x']),
    ('transposed-flag', ['--no-cloors']),
    ('missing-value', ['--interpreter']),
    ('missing-before-flag', ['--interpreter', '--no-colors']),
    ('boolean-value', ['--no-colors=yes']),
    ('duplicate-boolean', ['--no-colors', '--no-colors']),
    ('duplicate-interpreter', ['--interpreter=node', '--interpreter=nodejs']),
    ('unexpected-argument', ['foo']),
    ('unexpected-arguments', ['foo', 'bar']),
    ('flag-before-argument-error', ['foo', '--bad']),
    ('known-flag-error-priority', ['--no-colors=yes', '--interpreter']),
    ('help-overrides-errors', ['--bogus', '--help', 'foo']),
    ('missing-interpreter', ['--interpreter=/not/an/executable']),
    ('empty-interpreter', ['--interpreter=']),
    ('unsupported-json-report', ['--report=json']),
    ('unsupported-json-report-spaced', ['--report', 'json']),
]:
    cases.append((label, flags, '', False))
results = []
with tempfile.TemporaryDirectory(prefix='elm-repl-cli-') as directory:
    root = Path(directory)
    empty_path = root/'empty-path'
    empty_path.mkdir()
    fallback_path = root/'fallback-path'
    fallback_path.mkdir()
    (fallback_path/'nodejs').symlink_to(shutil.which('node'))
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    home = root/'home'
    cache = home/'0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    for package in ['core/1.0.5', 'json/1.1.3', 'html/1.0.0', 'virtual-dom/1.0.3']:
        shutil.copytree(original/'elm'/package, cache/'elm'/package)
    env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1', 'NO_PROXY': '', 'no_proxy': ''}
    for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    for label, flags, source, standalone in cases:
        project = root/label
        project.mkdir()
        if not standalone:
            (project/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
        case_env = dict(env)
        if label.startswith('invalid-proxy-'):
            for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
                case_env[key] = 'not a proxy'
        if label == 'missing-default-interpreter':
            case_env['PATH'] = str(empty_path)
        elif label == 'fallback-nodejs':
            case_env['PATH'] = str(fallback_path)
        runs = [subprocess.run([str(binary.resolve()), 'repl', *flags], input=source,
            cwd=project, env=case_env, text=True, capture_output=True, timeout=90) for binary in [args.elm, args.rust]]
        expected, actual = [{'code': p.returncode, 'stdout': p.stdout, 'stderr': p.stderr} for p in runs]
        passed = expected == actual and (not standalone or not (project/'elm.json').exists())
        results.append({'case': label, 'passed': passed, 'official': expected, 'rust': actual})
report = {'scope': __doc__, 'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest(),
    'passed': all(r['passed'] for r in results), 'cases': len(results), 'results': results}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
raise SystemExit(not report['passed'])
