#!/usr/bin/env python3
"""Compare REPL editing behavior on a real pseudo-terminal (requires pexpect).

Checks submitted values and control flow, not byte-identical terminal redraws.
Persistent history, signals during evaluation and Windows terminals
are outside this suite's scope.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import tempfile

import pexpect

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--rust', type=Path, default=crate/'target/debug/planexpo-elm')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()


def run(binary):
    transcript = io.StringIO()
    steps = []
    error = None
    with tempfile.TemporaryDirectory(prefix='elm-repl-terminal-') as tmp:
        root = Path(tmp)
        (root/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
        home = root/'home'
        original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
        cache = home/'0.19.1/packages'
        cache.mkdir(parents=True)
        shutil.copy2(original/'registry.dat', cache/'registry.dat')
        for package in ['core/1.0.5', 'json/1.1.3']:
            shutil.copytree(original/'elm'/package, cache/'elm'/package)
        env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1',
               'TERM': 'xterm', 'NO_PROXY': '', 'no_proxy': ''}
        for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
            env[key] = 'http://127.0.0.1:9'
        child = pexpect.spawn(str(binary.resolve()), ['repl', '--no-colors'],
                              cwd=tmp, env=env, encoding='utf-8', timeout=30,
                              dimensions=(24, 100))
        child.logfile_read = transcript

        def prompt():
            child.expect_exact('> ')

        def result(value, label):
            child.expect_exact(value)
            prompt()
            steps.append(label)

        try:
            prompt()
            child.send('1 + 3\x7f2\n')
            result('3 : number', 'backspace-edit')
            child.send('\x1b[A\n')
            result('3 : number', 'previous-line')
            child.send('1 + 3\x1b[D\x042\n')
            result('3 : number', 'cursor-and-delete')
            child.send('identity : message -> message\n')
            child.expect_exact('| ')
            child.send('x = x\n')
            child.expect_exact('| ')
            child.send('\n')
            result('<function> : message -> message', 'annotation-prefill')
            child.send('iden\t()\n')
            result('() : ()', 'complete-declaration')
            child.send('identical = ()\n')
            result('() : ()', 'completion-second-candidate')
            child.send('ide\tty ()\n')
            result('() : ()', 'complete-common-prefix')
            child.send('type Box = Box\n')
            prompt()
            child.send('Bo\t\n')
            result('Box : Box', 'complete-type')
            child.send('import Maybe\n')
            prompt()
            child.send('import May\tas M\n')
            prompt()
            child.send('M.Just ()\n')
            result('Just () : M.Maybe ()', 'complete-import-with-space')
            child.send(':he\t\n')
            child.expect_exact('Valid commands include:')
            prompt()
            steps.append('complete-command')
            child.send('[1,\n')
            child.expect_exact('| ')
            child.sendcontrol('c')
            prompt()
            steps.append('cancel-continuation')
            child.send('identity ()\n')
            result('() : ()', 'state-survives-cancel')
            child.send('unfinished')
            child.expect_exact('unfinished')
            child.sendcontrol('c')
            prompt()
            child.send('identity ()\n')
            result('() : ()', 'cancel-first-line')
            child.send('[1,\n')
            child.expect_exact('| ')
            # Continuations start with indentation. Clear it before sending EOF.
            child.sendcontrol('u')
            child.sendcontrol('d')
            prompt()
            steps.append('continuation-eof')
            child.sendcontrol('d')
            child.expect(pexpect.EOF)
            child.close()
            assert child.exitstatus == 0, (child.exitstatus, child.signalstatus)
            steps.append('exit-eof')
        except (pexpect.ExceptionPexpect, AssertionError) as exc:
            error = str(exc)
        finally:
            if child.isalive():
                child.close(force=True)
    return {'passed': error is None, 'steps': steps, 'error': error,
            'transcript': transcript.getvalue()}


runs = {'official': run(args.elm), 'rust': run(args.rust)}
passed = all(r['passed'] for r in runs.values()) and runs['official']['steps'] == runs['rust']['steps']
report = {'scope': __doc__, 'passed': passed, 'runs': runs,
          'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest()}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'passed': passed, 'steps': {key: len(run['steps']) for key, run in runs.items()}}))
raise SystemExit(not passed)
