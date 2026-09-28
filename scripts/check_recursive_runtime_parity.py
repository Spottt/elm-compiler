#!/usr/bin/env python3
"""Probe recursive inference with terminating calls and nearby invalid programs.

This is a strict differential suite: known acceptance gaps remain failures.
Successful compilation alone is insufficient; accepted fixtures must emit Int 0.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
crate = Path(__file__).resolve().parents[1]
rust = crate / 'target/release/planexpo-elm'
elm = args.elm.resolve()
f = 'f n x = if n == 0 then 0 else g (n - 1) x'
g = 'g n y = f n [y]'
cases = [
    ('implicit recursion', f + '\n' + g, 'f 3 [()]', True),
    ('reversed declarations', g + '\n' + f, 'f 3 [()]', True),
    ('local recursion', 'outer counter values =\n    let\n        ' + f + '\n        ' + g + '\n    in\n    f counter values', 'outer 3 [()]', True),
    ('annotated f', 'f : Int -> List a -> Int\n' + f + '\n' + g, 'f 3 [()]', True),
    ('annotated g', f + '\ng : Int -> a -> Int\n' + g, 'f 3 [()]', True),
    ('both annotated', 'f : Int -> List a -> Int\n' + f + '\ng : Int -> a -> Int\n' + g, 'f 3 [()]', True),
    ('invalid call argument', f + '\n' + g, 'f 3 2', False),
    ('numeric base case', f.replace('then 0', 'then x + 1') + '\n' + g, 'f 3 [()]', False),
    ('annotated numeric base case', 'f : Int -> Int -> Int\n' + f.replace('then 0', 'then x + 1') + '\n' + g, 'f 3 2', False),
    ('inconsistent parameter uses', f + '\ng n y = if List.isEmpty y then f n [y] else String.length y', 'f 3 [()]', False),
    ('growth in first function', f.replace('g (n - 1) x', 'g (n - 1) [x]') + '\ng n y = f n y', 'f 3 [()]', False),
    ('renamed recursion', 'z n x = if n == 0 then 0 else a (n - 1) x\na n y = z n [y]', 'z 3 [()]', False),
]
runner = "const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});app.ports.outgoing.subscribe(value=>console.log(JSON.stringify({value,type:typeof value})));app.ports.incoming.send(1);"
report = {'scope': 'Terminating mutually recursive worker fixtures, production and development; differential acceptance and actual outgoing port value/type. Historical name/order sensitivity is retained, not treated as a reason to weaken occurs checks.', 'compiler_sha256': hashlib.sha256(rust.read_bytes()).hexdigest(), 'elm_sha256': hashlib.sha256(elm.read_bytes()).hexdigest(), 'results': [], 'failures': []}
with tempfile.TemporaryDirectory(prefix='elm-recursive-runtime-') as directory:
    project = Path(directory)
    (project / 'elm.json').write_bytes((crate / 'tests/programs/worker/elm.json').read_bytes())
    template = (crate / 'tests/programs/worker/Main.elm').read_text()
    for optimize in [False, True]:
        for name, body, expression, expected in cases:
            (project / 'Main.elm').write_text(template.replace('value + model', expression) + '\n' + body + '\n')
            row = {'name': name, 'source': body, 'expression': expression, 'mode': 'production' if optimize else 'development', 'expected': expected}
            for compiler, binary in [('rust', rust), ('elm', elm)]:
                output = project / (compiler + '.js')
                flags = ['--optimize'] if optimize else []
                result = subprocess.run([str(binary), 'make', 'Main.elm', *flags, '--report=json', '--output=' + str(output)], cwd=project, env={**os.environ, 'GHCRTS': '-N1 -A16m -c'}, capture_output=True, text=True, timeout=30)
                outcome = {'accepted': result.returncode == 0}
                if result.returncode == 0:
                    execution = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, timeout=10, check=True)
                    outcome['runtime'] = json.loads(execution.stdout)
                    assert outcome['runtime'] == {'value': 0, 'type': 'number'}, (name, compiler, outcome)
                else:
                    outcome['diagnostics'] = json.loads(result.stderr)
                    if compiler == 'elm':
                        titles = [problem['title'] for error in outcome['diagnostics'].get('errors', []) for problem in error['problems']]
                        assert titles and all(title in ['TYPE MISMATCH', 'INFINITE TYPE'] for title in titles), (name, titles)
                row[compiler] = outcome
            assert row['elm']['accepted'] == expected, (name, 'unexpected oracle result', row)
            report['results'].append(row)
            if row['rust']['accepted'] != expected:
                report['failures'].append({'name': name, 'mode': row['mode']})
            print(name, row['mode'], 'rust=', row['rust']['accepted'], 'elm=', row['elm']['accepted'], flush=True)
args.report.write_text(json.dumps(report, indent=2) + '\n')
raise SystemExit(bool(report['failures']))
