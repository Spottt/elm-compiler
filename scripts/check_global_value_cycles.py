#!/usr/bin/env python3
"""Strict acceptance parity for immediate versus delayed global value cycles.

All accepted fixtures are checked, not executed: Elm permits some cycles through
function calls which do not terminate. Compile development, production and the
check CLI so emission cannot hide an inference validation gap.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

CASES = [
    (False, 'x = x'),
    (False, 'x = y\ny = x'),
    (False, 'x : ()\nx = x'),
    (False, 'x = let unused = x in ()'),
    (False, 'x = let y = x in y'),
    (False, 'x = (\\ignored -> ()) x'),
    (False, 'x = case () of\n    _ -> x'),
    (False, 'x = if True then () else x'),
    (False, 'x = { x | value = () }'),
    (False, 'type Box = Box (() -> Box)\nx = if True then Box (\\_ -> x) else y\ny = x'),
    (False, 'x = let (a,b) = (x,()) in b'),
    (False, 'x = List.length [x]'),
    (False, 'x = y\ny = z\nz = x'),
    (False, 'x = y\ny = x\nunused arg = x'),
    (True, 'type Ring = Ring (() -> Ring)\na = Ring (\\_ -> b)\nb = Ring (\\_ -> a)'),
    (True, 'x = f ()\nf arg = x'),
    (True, 'x = let f arg = x in ()'),
    (True, 'x = let f arg = x in f ()'),
    (True, 'f = \\arg -> f arg'),
    (True, 'f : a -> b\nf = \\arg -> f arg'),
    (True, 'type Ring = Ring (() -> Ring)\na = Ring (let f arg = a in f)'),
    (False, 'x = let x = () in x'),
    (False, 'x = case () of\n    x -> x'),
    (False, 'x = (\\x -> x) ()'),
    (True, 'type Ring = Ring (() -> Ring)\na = Ring (\\_ -> b)\nb = a'),
    (True, 'x = y\ny = ()'),
]


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elm', type=Path, required=True)
    parser.add_argument('--compiler', type=Path, default=root/'target/release/planexpo-elm')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    binaries = {'elm': args.elm.resolve(), 'rust': args.compiler.resolve()}
    report = {'scope': __doc__, 'sha256': {k: hashlib.sha256(v.read_bytes()).hexdigest() for k,v in binaries.items()}, 'results': [], 'failures': []}
    with tempfile.TemporaryDirectory(prefix='elm-global-cycles-') as directory:
        project = Path(directory)
        (project/'elm.json').write_bytes((root/'tests/programs/worker/elm.json').read_bytes())
        for expected, body in CASES:
            # A real entry is required for JS output by Elm 0.19. Keep every
            # original cycle fixture and add an independent terminating worker.
            source = ('module Fixture exposing (..)\nimport List\n'
                      'import Platform\nimport Platform.Cmd as Cmd\nimport Platform.Sub as Sub\n'
                      + body + '\n'
                      + r'main = Platform.worker { init = \() -> ((), Cmd.none), update = \_ model -> (model, Cmd.none), subscriptions = \_ -> Sub.none }'
                      + '\n')
            (project/'Fixture.elm').write_text(source)
            row = {'source': source, 'expected': expected}
            for compiler, mode in [(compiler, mode) for compiler in ['elm','rust'] for mode in ['check','development','production']]:
                command = [str(binaries[compiler])]
                if compiler == 'rust' and mode == 'check':
                    command += ['check', str(project/'elm.json'), 'Fixture.elm']
                else:
                    command += ['make','Fixture.elm','--report=json','--output='+('/dev/null' if mode == 'check' else str(project/'fixture.js'))]
                    if mode == 'production':
                        command += ['--optimize']
                result = subprocess.run(command, cwd=project, env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=30)
                accepted = result.returncode == 0
                row[compiler+'-'+mode] = {'accepted':accepted,'returncode':result.returncode,'stderr':result.stderr if not accepted else ''}
                if accepted != expected or result.returncode not in (0,1):
                    report['failures'].append({'source':source,'compiler':compiler,'mode':mode,'expected':expected,'outcome':row[compiler+'-'+mode]})
            report['results'].append(row)
    args.report.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'cases':len(report['results']),'failures':len(report['failures'])}))
    raise SystemExit(bool(report['failures']))


if __name__ == '__main__':
    main()
