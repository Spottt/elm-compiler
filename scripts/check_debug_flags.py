#!/usr/bin/env python3
"""Compare --debug/--optimize conflict diagnostics with Elm 0.19.1."""
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
results = []
with tempfile.TemporaryDirectory(prefix='elm-debug-flags-') as folder:
    root = Path(folder)
    env = dict(os.environ, ELM_HOME=str(root/'home'))
    for has_outline in [False, True]:
        if has_outline: (root/'elm.json').write_text('not JSON')
        for flags in [['--debug', '--optimize'], ['--optimize', '--debug'], ['--debug', '--debug', '--optimize'], ['--optimize', '--optimize', '--debug']]:
            for json_args in [[], ['--report=json'], ['--report', 'json']]:
                for ansi in [False, True]:
                    outputs = []
                    for binary in binaries:
                        result = run_command([str(binary), 'make', 'Missing.elm', *flags, *json_args], root, env, ansi=ansi)
                        code, stdout, stderr = (result[key] for key in ['code', 'stdout', 'stderr'])
                        outputs.append((code, stdout, json.loads(stderr) if json_args and stderr.startswith('{') else stderr))
                    results.append({'outline': has_outline, 'flags': flags, 'report': json_args, 'ansi': ansi, 'match': outputs[0] == outputs[1], 'official': outputs[0], 'rust': outputs[1]})
assert hashes == {str(b): hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries}, 'binary changed during comparison'
a.report.write_text(json.dumps({'binaries': hashes, 'cases': results}, indent=2)+'\n')
for result in results:
    print(result['flags'], result['report'], result['ansi'], result['match'])
assert all(r['match'] and r['rust'][0] == 1 for r in results)
