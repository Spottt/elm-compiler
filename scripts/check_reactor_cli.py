#!/usr/bin/env python3
"""Exact text/ANSI CLI comparison for reactor. Valid ports have an extra argument
so these cases never start a server; HTTP startup is checked by check_reactor.py.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tempfile
from diff_test_process import run_command

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--rust', type=Path, required=True)
p.add_argument('--report', type=Path, required=True)
a = p.parse_args()
binaries = [a.elm, a.rust]
hashes = {str(b): hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries}
cases = [
    ['--help'], ['--bad','--help'], ['--port','--help'], ['extra','--help'],
    ['--port'], ['--port='], ['--port',''], ['--port=nope'], ['--port','nope'],
    ['--bad'], ['--por=8000'], ['--report=json'], ['--port','--bad'], ['--port','-1'],
    ['--bad','--port=nope'], ['extra','--port=nope'], ['--port=nope','--bad'],
    ['--port=8000','--port=nope'], ['--port','8000','--port','9000'],
    ['one'], ['one','two'], ['one','--port','8000','two'], ['-'], ['-p'], ['--'],
]
for value in ['8000', '0', '+8000', '0x1f40', '0X1F40', '0o17500', '0O17500', '0b111', '(8000)', '(( 8000 ))', ' 8000 ', '\n8000\t', '- 8000', '(-1)', '(- (1))', '1_000', '1.0', '18446744073709559616', '-18446744073709559616', '9223372036854775808']:
    cases.append(['--port='+value, 'extra'])
results = []
with tempfile.TemporaryDirectory(prefix='elm-reactor-cli-') as root:
    env = dict(os.environ, ELM_HOME=str(Path(root)/'home'))
    for args in cases:
        for ansi in [False, True]:
            responses = [run_command([str(binary), 'reactor', *args], root, env, ansi=ansi) for binary in binaries]
            results.append({'args': args, 'ansi': ansi, 'match': responses[0] == responses[1], 'official': responses[0], 'rust': responses[1]})
assert hashes == {str(b): hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries}, 'compiler changed during validation'
a.report.write_text(json.dumps({'binaries': hashes, 'cases': results}, indent=2)+'\n')
for result in results:
    if not result['match']: print(json.dumps(result, ensure_ascii=False))
print(sum(r['match'] for r in results), '/', len(results), 'matched')
assert all(r['match'] for r in results)
