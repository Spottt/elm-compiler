#!/usr/bin/env python3
"""Compare a structured Rust REPL session with Elm's interactive session.

Rust inputs are explicitly classified. Tests state replacement, rollback after
compile/runtime failures, reevaluation and reset, not terminal input handling
or exact diagnostic text.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--probe', type=Path, default=crate/'target/debug/examples/repl_session')
parser.add_argument('--report', type=Path, required=True)
parser.add_argument('--rust-cli', type=Path, help='Compare the real repl command, including prompts')
parser.add_argument('--classify', action='store_true', help='Classify raw source lines and include multiline inputs')
args = parser.parse_args()
inputs = [
    {'kind': 'declaration', 'name': 'x', 'source': 'x = 1'},
    {'kind': 'expression', 'source': 'x'},
    {'kind': 'declaration', 'name': 'x', 'source': 'x = "bad" + 1', 'reject': True},
    {'kind': 'expression', 'source': 'x'},
    {'kind': 'declaration', 'name': 'x', 'source': 'x = Debug.todo "boom"', 'reject': True},
    {'kind': 'expression', 'source': 'x'},
    {'kind': 'declaration', 'name': 'y', 'source': 'y = x + 2'},
    {'kind': 'declaration', 'name': 'x', 'source': 'x = 5'},
    {'kind': 'expression', 'source': 'y'},
    {'kind': 'declaration', 'name': 'x', 'source': 'x = "break dependent y"', 'reject': True},
    {'kind': 'expression', 'source': 'y'},
    {'kind': 'import', 'name': 'Maybe', 'source': 'import Maybe as M'},
    {'kind': 'expression', 'source': 'M.Just ()'},
    {'kind': 'import', 'name': 'Maybe', 'source': 'import Maybe exposing (Missing)', 'reject': True},
    {'kind': 'expression', 'source': 'M.Just ()'},
    {'kind': 'type', 'name': 'Box', 'source': 'type alias Box item = { item : item }'},
    {'kind': 'expression', 'source': 'Box ()'},
    {'kind': 'type', 'name': 'Box', 'source': 'type alias Box item = { item : Missing item }', 'reject': True},
    {'kind': 'expression', 'source': 'Box ()'},
    {'kind': 'reset'},
    {'kind': 'expression', 'source': 'x', 'reject': True},
    {'kind': 'expression', 'source': '()'},
]
if args.classify or args.rust_cli:
    inputs.extend([
        {'kind': 'declaration', 'source': 'double : Int -> Int\ndouble n = n + n\n'},
        {'kind': 'expression', 'source': 'double 21'},
        {'kind': 'expression', 'source': 'let\n  local = 7\nin\n  local + 1\n'},
        {'kind': 'expression', 'source': 'if True then\n', 'reject': True},
        {'kind': 'expression', 'source': 'double 2'},
    ])
with tempfile.TemporaryDirectory(prefix='elm-repl-session-') as directory:
    root = Path(directory)
    home = root/'home'
    cache = home/'0.19.1/packages'
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    for package in ['core/1.0.5', 'json/1.1.3']:
        shutil.copytree(original/'elm'/package, cache/'elm'/package)
    manifest = root/'elm.json'
    manifest.write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
    input_path = root/'inputs.json'
    input_path.write_text(json.dumps(inputs))
    env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1', 'NO_PROXY': '', 'no_proxy': ''}
    for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    official = subprocess.run([str(args.elm.resolve()), 'repl', '--no-colors'], cwd=root, env=env,
        input='\n'.join(':reset' if i['kind'] == 'reset' else i['source'] for i in inputs)+'\n:exit\n', text=True, capture_output=True, timeout=120)
    if args.rust_cli:
        rust = subprocess.run([str(args.rust_cli.resolve()), 'repl', '--no-colors'], cwd=root, env=env,
            input='\n'.join(':reset' if i['kind'] == 'reset' else i['source'] for i in inputs)+'\n:exit\n', text=True, capture_output=True, timeout=120)
    else:
        rust = subprocess.run([str(args.probe.resolve()), str(home), str(manifest), str(input_path)]+(['--classify'] if args.classify else []),
            cwd=root, env=env, text=True, capture_output=True, timeout=120)
    outcomes = json.loads(rust.stdout) if rust.returncode == 0 and not args.rust_cli else []
    expected = re.sub(r'^(?:[>|] )+', '', official.stdout.split('\n', 3)[-1], flags=re.MULTILINE)
    actual = ''.join(row.get('stdout', '') for row in outcomes)
    rejected = [i for i, row in enumerate(outcomes) if not row['ok']]
    passed = (official.returncode == rust.returncode == 0 and len(outcomes) == len(inputs)
        and rejected == [i for i, item in enumerate(inputs) if item.get('reject')] and actual == expected
        and all(marker in official.stderr for marker in ['TYPE MISMATCH', 'boom', 'NAMING ERROR']))
    if args.rust_cli:
        actual = rust.stdout
        expected = official.stdout
        passed = (official.returncode == rust.returncode == 0 and actual == expected
                  and 'boom' in rust.stderr and all(marker in official.stderr for marker in ['TYPE MISMATCH', 'boom', 'NAMING ERROR']))
    report = {'cli': bool(args.rust_cli), 'scope': __doc__, 'probe_sha256': hashlib.sha256((args.rust_cli or args.probe).read_bytes()).hexdigest(),
        'passed': passed, 'classified_inputs': args.classify, 'steps': len(inputs), 'inputs': inputs, 'outcomes': outcomes,
        'official_stdout': official.stdout, 'official_stderr': official.stderr,
        'rust_stderr': rust.stderr, 'expected_stdout_without_prompts': expected, 'rust_stdout': actual}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'passed': passed, 'steps': len(inputs), 'rejected_indices': rejected}))
raise SystemExit(not passed)
