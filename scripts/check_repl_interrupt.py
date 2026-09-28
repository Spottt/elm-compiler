#!/usr/bin/env python3
"""Linux REPL cancellation checks (requires pexpect and /proc).

Compare runtime cancellation and rollback with Elm. Separately stop a Rust
compilation worker, interrupt its parent, and verify worker removal and rollback.
Reference zombies may take up to two seconds to be reaped, but any live process
at the prompt fails; Rust must be synchronously reaped. This does not establish
Windows or every signal-timing edge-case parity.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import signal
import tempfile
import time
import pexpect

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--rust', type=Path, default=crate/'target/debug/planexpo-elm')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()


def run(binary, stop_compiler=False):
    transcript = io.StringIO()
    steps = []
    reaping = []
    worker = None
    error = None
    with tempfile.TemporaryDirectory(prefix='elm-repl-interrupt-') as tmp:
        root = Path(tmp)
        home = root/'home'
        cache = home/'0.19.1/packages'
        cache.mkdir(parents=True)
        original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
        shutil.copy2(original/'registry.dat', cache/'registry.dat')
        for package in ['core/1.0.5', 'json/1.1.3']:
            shutil.copytree(original/'elm'/package, cache/'elm'/package)
        (root/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
        wrapper = root/'node'
        # The temporary path contains no shell metacharacters. exec preserves PID.
        wrapper.write_text('#!/bin/sh\necho $$ > '+str(root/'node.pid')+'\nexec '+shutil.which('node')+'\n')
        wrapper.chmod(0o755)
        env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1', 'TERM': 'xterm', 'NO_PROXY': '', 'no_proxy': ''}
        for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
            env[key] = 'http://127.0.0.1:9'
        child = pexpect.spawn(str(binary.resolve()), ['repl', '--no-colors', '--interpreter='+str(wrapper)],
                              cwd=tmp, env=env, encoding='utf8', timeout=20)
        child.logfile_read = transcript
        try:
            child.expect_exact('> ')
            child.sendline('x = 1')
            child.expect_exact('1 : number')
            child.expect_exact('> ')
            child.sendline('x = let loop n = loop n in case Debug.log "RUNNING" () of () -> loop 0')
            child.expect_exact('RUNNING: ()')
            node_pid = int((root/'node.pid').read_text())
            child.sendcontrol('c')
            child.expect_exact('<cancelled>')
            child.expect_exact('> ')
            # Haskell's withCreateProcess can reap an already-dead interpreter
            # asynchronously. Do not confuse that zombie with live JavaScript.
            # Rust's process-group guard promises synchronous wait(), so retain
            # the original stronger assertion for the Rust run.
            process = Path('/proc', str(node_pid))
            started = time.monotonic()
            states = []
            if stop_compiler:
                assert not process.exists(), 'Rust interpreter was not synchronously reaped'
            else:
                while process.exists():
                    try:
                        state = (process/'stat').read_text().split(') ', 1)[1].split()[0]
                    except FileNotFoundError:
                        break
                    states.append(state)
                    assert state == 'Z', f'interpreter still active after cancellation: {state}'
                    assert time.monotonic()-started < 2, 'interpreter zombie was not reaped within two seconds'
                    time.sleep(0.001)
            assert not process.exists(), 'interpreter survived cancellation'
            reaping.append({'states': states, 'seconds': time.monotonic()-started})
            steps.append('cancel-running-javascript-and-reap')
            child.sendline('x')
            child.expect_exact('1 : number')
            child.expect_exact('> ')
            steps.append('runtime-rollback')
            if stop_compiler:
                (root/'Slow.elm').write_text('module Slow exposing (..)\n'+''.join('v'+str(i)+' = ()\n' for i in range(10000)))
                child.sendline('import Slow')
                deadline = time.monotonic()+10
                children = Path('/proc', str(child.pid), 'task', str(child.pid), 'children')
                while worker is None and time.monotonic() < deadline:
                    for pid in children.read_text().split():
                        try:
                            if b'--internal-repl-compile' in Path('/proc', pid, 'cmdline').read_bytes():
                                worker = int(pid)
                                os.kill(worker, signal.SIGSTOP)
                                break
                        except ProcessLookupError:
                            continue
                    if worker is None:
                        time.sleep(0.001)
                assert worker is not None, 'did not observe compilation worker'
                child.sendcontrol('c')
                child.expect_exact('<cancelled>')
                child.expect_exact('> ')
                assert not Path('/proc', str(worker)).exists(), 'compilation worker survived cancellation'
                steps.append('cancel-stopped-compilation-and-reap')
                worker = None
                child.sendline('x')
                child.expect_exact('1 : number')
                child.expect_exact('> ')
                steps.append('compilation-rollback')
            child.sendcontrol('d')
            child.expect(pexpect.EOF)
            child.close()
            assert child.exitstatus == 0, (child.exitstatus, child.signalstatus)
        except (AssertionError, OSError, pexpect.ExceptionPexpect) as exc:
            error = str(exc)
        finally:
            if worker is not None:
                try:
                    os.kill(worker, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            if child.isalive():
                child.close(force=True)
    return {'passed': error is None, 'steps': steps, 'reaping': reaping, 'error': error, 'transcript': transcript.getvalue()}


runs = {'official': run(args.elm), 'rust': run(args.rust, True)}
report = {'scope': __doc__, 'passed': all(r['passed'] for r in runs.values()), 'runs': runs,
          'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest()}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'passed': report['passed'], 'steps': {key: len(run['steps']) for key, run in runs.items()}}))
raise SystemExit(not report['passed'])
