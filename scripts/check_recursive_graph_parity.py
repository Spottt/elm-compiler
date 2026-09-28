#!/usr/bin/env python3
"""Compare recursive inference across graph orders, patterns and lexical scopes.

Acceptance parity only: runtime and captured-variable soundness have separate
suites. Each run uses an isolated project and retains every mismatch.
"""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import subprocess
import tempfile


def fixtures():
    for size in (2, 3, 4):
        for names in itertools.permutations(('f', 'g', 'h', 'i')[:size]):
            for grow in range(size):
                for reverse in (False, True):
                    definitions = [name + ' x = ' + names[(i + 1) % size]
                                   + (' [x]' if i == grow else ' x')
                                   for i, name in enumerate(names)]
                    if reverse:
                        definitions.reverse()
                    yield 'top-cycle', '\n'.join(definitions)
                    yield 'local-cycle', local(definitions, names[0])
                    anchored = [line.replace(' = ', ' = let ignored = anchor in ', 1)
                                if line.startswith(names[-1] + ' ') else line
                                for line in definitions]
                    yield 'local-anchor', local(['anchor = ()'] + anchored, names[0])
                    yield 'local-tuple-anchor', local(['(anchor, zulu) = ((), ())'] + anchored, names[0])
                    yield 'local-record-anchor', local(['{anchor, zulu} = {anchor=(), zulu=()}'] + anchored, names[0])
    patterns = [
        'f x = g x\ng {value} = f {value=[value]}',
        'f x = g x\ng (a,b) = f ([a],b)',
        'f x = g x\ng (a,b) = f (a,[b])',
        'f x = g x\ng ((a,b) as whole) = f ([a],b)',
        'f x = g x\ng row = f {row | value=[row.value]}',
        'f x = g x\ng y = case y of\n    {value} -> f {value=[value]}',
        'f (x as whole) = g whole\ng y = f [y]',
        'f x = g x\ng (y as whole) = f [whole]',
        'f x = g x\ng (a,b) = f [(a,b)]',
        'f (a,b) = g (a,b)\ng y = f [y]',
        'f x = g x\ng {value} = f [{value=value}]',
        'f x = g x\ng y = (\\z -> f [z]) y',
        'f x = g x\ng y = let z = y in f [z]',
        'f x = g x\ng y = let z a = a in f [z y]',
        'f x = g x\ng y = case y of\n    z -> f [z]',
    ]
    for names in itertools.permutations(('f', 'g', 'h')):
        for owner, target, growth in itertools.product(range(3), range(3), (False, True)):
            definitions = []
            for i, name in enumerate(names):
                body = names[(i + 1) % 3] + ' x'
                if i == owner:
                    body = 'if True then ' + body + ' else ' + names[target] + (' [x]' if growth else ' x')
                definitions.append(name + ' x = ' + body)
            patterns.append('\n'.join(definitions))
    for body in patterns:
        yield 'top-pattern', body
        yield 'local-pattern', local(body.splitlines(), 'f')


def local(lines, entry):
    return 'outer input =\n    let\n' + ''.join('        ' + line + '\n' for line in lines) + '    in\n    ' + entry + ' input'


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elm', type=Path, required=True)
    parser.add_argument('--compiler', type=Path, default=root / 'target/release/planexpo-elm')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    binaries = {'rust': args.compiler.resolve(), 'elm': args.elm.resolve()}
    report = {'scope': __doc__, 'sha256': {key: hashlib.sha256(path.read_bytes()).hexdigest()
                                         for key, path in binaries.items()}, 'results': [], 'failures': []}
    with tempfile.TemporaryDirectory(prefix='elm-recursive-graph-') as directory:
        project = Path(directory)
        (project / 'elm.json').write_bytes((root / 'tests/programs/worker/elm.json').read_bytes())
        for category, body in fixtures():
            source = 'module Fixture exposing (..)\n' + body + '\n'
            (project / 'Fixture.elm').write_text(source)
            row = {'category': category, 'source': source}
            for name, binary in binaries.items():
                result = subprocess.run([str(binary), 'make', 'Fixture.elm', '--output=/dev/null', '--report=json'],
                                        cwd=project, env={**os.environ, 'GHCRTS': '-N1 -A16m -c'},
                                        capture_output=True, text=True, timeout=30)
                if result.returncode not in (0, 1):
                    raise RuntimeError((name, result.returncode, result.stderr))
                row[name] = {'accepted': result.returncode == 0,
                             'diagnostic': json.loads(result.stderr) if result.returncode else None}
            report['results'].append(row)
            if len(report['results']) % 100 == 0:
                print(json.dumps({'completed': len(report['results']), 'failures': len(report['failures'])}), flush=True)
            if row['rust']['accepted'] != row['elm']['accepted']:
                report['failures'].append(row)
    assert report['sha256'] == {key: hashlib.sha256(path.read_bytes()).hexdigest() for key, path in binaries.items()}, 'compiler changed during comparison'
    report['cases'] = len(report['results'])
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'cases': report['cases'], 'failures': len(report['failures'])}))
    raise SystemExit(bool(report['failures']))


if __name__ == '__main__':
    main()
