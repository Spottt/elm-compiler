#!/usr/bin/env python3
"""Compare wide union constructors, reusable partials and pattern extraction."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=Path, required=True)
args = parser.parse_args()
crate = Path(__file__).resolve().parents[1]
runner = r'''
const app=require(process.argv[1]).Elm.Main.init({flags:null});
app.ports.outgoing.subscribe(value=>console.log(JSON.stringify(value)));
app.ports.incoming.send(0);
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-constructors-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    for arity in [1, 2, 9, 10, 26, 27, 53, 54, 260, 300]:
        names = [f'v{i}' for i in range(arity)]
        source = f'''port module Main exposing (main)
import Platform
type Huge = Huge {' '.join(['Int'] * arity)}
prefix = Huge {' '.join(str(i) for i in range(arity - 1))}
flatten (Huge {' '.join(names)}) = [{','.join(names)}]
port incoming : (Int -> msg) -> Sub msg
port outgoing : List Int -> Cmd msg
main : Program () () Int
main = Platform.worker
    {{ init = \\_ -> ((), Cmd.none)
    , update = \\_ model -> (model, outgoing (flatten (prefix 71) ++ flatten (prefix 99)))
    , subscriptions = \\_ -> incoming identity
    }}
'''
        (root/'Main.elm').write_text(source)
        outputs = []
        for compiler, name in [(args.elm.resolve(), 'elm'), (crate/'target/release/planexpo-elm', 'rust')]:
            output = root/f'{name}.js'
            compiled = subprocess.run([str(compiler), 'make', 'Main.elm', '--output='+str(output)], cwd=root, env={**os.environ, 'GHCRTS':'-N1 -A16m -c'}, capture_output=True, text=True, timeout=60)
            assert compiled.returncode == 0, (arity, name, compiled.stdout, compiled.stderr)
            result = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, check=True, timeout=10)
            outputs.append(json.loads(result.stdout))
        expected = list(range(arity - 1)) + [71] + list(range(arity - 1)) + [99]
        assert outputs[0] == outputs[1] == expected, (arity, outputs)
        print(f'{arity} arguments: identical fields, pattern extraction and reusable partial application', flush=True)
