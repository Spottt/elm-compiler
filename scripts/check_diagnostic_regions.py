#!/usr/bin/env python3
"""Real-source regions and terminal rendering for rejected Elm programs."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, default=crate.parent/'front/node_modules/elm/bin/elm')
parser.add_argument('--rust', type=Path, default=crate/'target/release/planexpo-elm')
args = parser.parse_args()
compilers = [args.elm.resolve(), args.rust.resolve()]
# Match the other differential suites: the reference can race on cold d.dat
# initialization with multiple capabilities, before reporting the source error.
env = {**os.environ, 'GHCRTS': '-N1 -A16m -c'}
cases = [
    ('broken xs = case xs of\n    [1, "bad"] -> ()\n    _ -> ()\n', '"bad"'),
    ('broken =\n    let\n        (a, b) = 1\n    in\n    a\n', '1'),
    ('broken = [1, "bad"]\n', '"bad"'),
    ('broken = ("é", [1, "bad"])\n', '"bad"'),
    ('broken : Int\nbroken = "bad"\n', '"bad"'),
    ('broken =\n    let\n        inside = [1, "bad"]\n    in\n    inside\n', '"bad"'),
]
with tempfile.TemporaryDirectory(prefix='elm-diagnostic-regions-') as directory:
    root = Path(directory)
    shutil.copytree(crate/'tests/programs/worker', root, dirs_exist_ok=True)
    path = root/'Broken.elm'
    for body, fragment in cases:
        source = 'module Broken exposing (..)\n-- café ☃\n' + body
        path.write_text(source)
        reports = []
        terminals = []
        for compiler in compilers:
            result = subprocess.run([str(compiler),'make','Broken.elm','--output=/dev/null','--report=json'],cwd=root,
                env=env,capture_output=True,text=True,timeout=60)
            assert result.returncode != 0, (compiler, body)
            try:
                report = json.loads(result.stderr)
            except json.JSONDecodeError as error:
                raise AssertionError({'compiler': str(compiler), 'source': source,
                                      'returncode': result.returncode,
                                      'stdout': result.stdout, 'stderr': result.stderr}) from error
            assert report['type'] == 'compile-errors', report
            reports.append(report)
            problem = report['errors'][0]['problems'][0]
            region = problem['region']
            lines = source.splitlines(keepends=True)
            def offset(position):
                return sum(len(line) for line in lines[:position['line']-1]) + position['column']-1
            selected = source[offset(region['start']):offset(region['end'])]
            assert selected == fragment, (selected, fragment, report)
            assert problem['title'] == 'TYPE MISMATCH'
            plain = subprocess.run([str(compiler),'make','Broken.elm','--output=/dev/null'],cwd=root,env=env,capture_output=True,text=True,timeout=60)
            assert plain.returncode != 0 and ('^' in plain.stderr or '|>' in plain.stderr) and fragment in plain.stderr, plain.stderr
            terminals.append(plain.stderr)
        assert reports[0] == reports[1], reports
        assert terminals[0] == terminals[1], terminals
    print('PASS: official/Rust rejection, exact AST regions, Unicode columns, nested definitions, annotation mismatch, terminal snippets and JSON')
