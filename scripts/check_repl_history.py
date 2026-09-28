#!/usr/bin/env python3
"""Verify default Haskeline history format and cross-compiler recall (pexpect).

Checks 100-entry retention, duplicates, Unicode, blank-line filtering, pipe
behavior and recall across sessions. Custom .haskeline preferences, concurrent
writers, malformed history files and abnormal termination are not covered.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import pexpect

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--rust', type=Path, default=crate/'target/debug/planexpo-elm')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
results = []


def terminal(binary, root, env, lines=None, recall=False):
    child = pexpect.spawn(str(binary.resolve()), ['repl', '--no-colors'],
                          cwd=str(root), env=env, encoding='utf-8', timeout=15,
                          dimensions=(24, 100))
    try:
        child.expect_exact('> ')
        for line in lines or []:
            child.sendline(line)
            child.expect_exact('> ')
        if recall:
            child.send('\x1b[A\n')
            child.expect_exact('<reset>')
            child.expect_exact('> ')
        child.sendcontrol('d')
        child.expect(pexpect.EOF)
        child.close()
        assert child.exitstatus == 0, (child.exitstatus, child.signalstatus)
    finally:
        if child.isalive():
            child.close(force=True)


for writer_name, writer in [('official', args.elm), ('rust', args.rust)]:
    for reader_name, reader in [('official', args.elm), ('rust', args.rust)]:
        with tempfile.TemporaryDirectory(prefix='elm-repl-history-') as tmp:
            root = Path(tmp)
            home = root/'home'
            home.mkdir()
            env = {**os.environ, 'ELM_HOME': str(home), 'HOME': str(home),
                   'GHCRTS': '-N1', 'TERM': 'xterm'}
            history = home/'0.19.1/repl/history'
            label = writer_name+'-to-'+reader_name
            error = None
            snapshots = {}
            try:
                terminal(writer, root, env, [':héllo', ':help', ':help', '', '   ', ':reset'])
                snapshots['written'] = history.read_text()
                expected = ':reset\n:help\n:help\n:héllo\n'
                assert snapshots['written'] == expected, snapshots['written']
                terminal(reader, root, env, recall=True)
                snapshots['recalled'] = history.read_text()
                assert snapshots['recalled'] == ':reset\n'+expected, snapshots['recalled']
                run = subprocess.run([str(reader.resolve()), 'repl'], cwd=root, env=env,
                    input=':help\n:quit\n', text=True, capture_output=True, timeout=15)
                assert run.returncode == 0, run.stderr
                assert history.read_text() == snapshots['recalled']
                seed = ''.join(':unknown'+str(i)+'\n' for i in range(105))
                history.write_text(seed)
                terminal(reader, root, env)
                snapshots['limited'] = history.read_text()
                assert snapshots['limited'] == ''.join(seed.splitlines(keepends=True)[:100])
            except (AssertionError, OSError, pexpect.ExceptionPexpect) as exc:
                error = str(exc)
            results.append({'case': label, 'passed': error is None, 'error': error, 'history': snapshots})
report = {'scope': __doc__, 'passed': all(r['passed'] for r in results),
          'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest(), 'results': results}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
raise SystemExit(not report['passed'])
