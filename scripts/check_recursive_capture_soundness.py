#!/usr/bin/env python3
"""Preserve rejection of a captured-parameter bug accepted by Elm 0.19.1.

This is deliberately a soundness check, not an equality-of-acceptance check.
The historical bundle emits undefined through an outgoing port declared Int.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--compiler', type=Path, default=crate/'target/release/planexpo-elm')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
rust = args.compiler.resolve()
elm = args.elm.resolve()
body = '''outer z =
    let
        f x = g z
        g y = if List.isEmpty z then f [y] else String.length z
    in
    f []
'''
template = (crate/'tests/programs/worker/Main.elm').read_text()
runner = "const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});app.ports.outgoing.subscribe(value=>console.log(JSON.stringify({value,type:typeof value})));app.ports.incoming.send(1);"
report = {'scope': 'Keep captured parameters monomorphic; reproduce historical acceptance and invalid outgoing Int value on a terminating call. This known historical defect is not desired compatibility.', 'source': body, 'expression': 'outer [()]', 'compiler_sha256': hashlib.sha256(rust.read_bytes()).hexdigest(), 'elm_sha256': hashlib.sha256(elm.read_bytes()).hexdigest(), 'results': []}
with tempfile.TemporaryDirectory(prefix='elm-capture-soundness-') as directory:
    project = Path(directory)
    (project/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
    (project/'Main.elm').write_text(template.replace('value + model', 'outer [()]') + '\n' + body)
    for optimize in [False, True]:
        row = {'mode': 'production' if optimize else 'development'}
        for name, binary in [('rust', rust), ('elm', elm)]:
            output = project/(name+'.js')
            flags = ['--optimize'] if optimize else []
            run = subprocess.run([str(binary), 'make', 'Main.elm', *flags, '--report=json', '--output='+str(output)], cwd=project, env={**os.environ, 'GHCRTS':'-N1 -A16m -c'}, capture_output=True, text=True, timeout=30)
            row[name] = {'accepted': run.returncode == 0}
            if name == 'rust':
                assert run.returncode != 0, 'Rust accepted incompatible uses of a captured parameter'
                row[name]['diagnostic'] = json.loads(run.stderr)
                diagnostic = row[name]['diagnostic']
                assert diagnostic['type'] == 'compile-errors', diagnostic
                problems = [problem for error in diagnostic['errors'] for problem in error['problems']]
                assert len(problems) == 1 and problems[0]['title'] == 'TYPE MISMATCH', diagnostic
                message = problems[0]['message']
                highlighted = [part['string'] for part in message if isinstance(part, dict) and part.get('color') == 'yellow']
                assert len(highlighted) == 2 and highlighted[0].startswith('List ') and highlighted[1] == 'String', diagnostic
                text = ''.join(part if isinstance(part, str) else part['string'] for part in message)
                assert 'argument to `length`' in text and 'String.length z' in text, diagnostic
            else:
                assert run.returncode == 0, run.stderr
                execution = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, check=True, timeout=10)
                row[name]['runtime'] = json.loads(execution.stdout)
                assert row[name]['runtime'] == {'type': 'undefined'}
        report['results'].append(row)
args.report.write_text(json.dumps(report, indent=2)+'\n')
print('PASS: Rust rejects the captured-parameter defect; historical Elm emits undefined in both modes.')
