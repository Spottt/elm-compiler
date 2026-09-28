#!/usr/bin/env python3
"""Compare Debug.todo's real runtime errors, including source locations."""
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
runner = '''
const loaded = require(process.argv[1]);
try { loaded.Elm.Main.init({flags:null}); console.log(JSON.stringify({error:null})); }
catch (error) { console.log(JSON.stringify({error:error.message})); }
'''
cases = {
    'direct': ('import Debug', 'Debug.todo "runtime probe"', ''),
    'import_alias': ('import Debug as D', 'D.todo "runtime probe"', ''),
    'exposed': ('import Debug exposing (todo)', 'todo "runtime probe"', ''),
    'value_alias': ('import Debug', 'crash "runtime probe"', '\ncrash = Debug.todo\n'),
}
with tempfile.TemporaryDirectory(prefix='elm-rust-debug-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    for name, (imports, expression, extra) in cases.items():
        source = f'''module Main exposing (main)
import Platform
{imports}
main : Program () () Never
main = Platform.worker
    {{ init = \\_ -> {expression}
    , update = \\msg _ -> never msg
    , subscriptions = \\_ -> Sub.none
    }}
{extra}'''
        (root/'Main.elm').write_text(source)
        native, reference = root/'rust.js', root/'elm.js'
        subprocess.run([str(crate/'target/release/planexpo-elm'), 'link-js', str(root/'elm.json'), 'Main.elm', str(native)], check=True, capture_output=True, text=True)
        subprocess.run([str(args.elm.resolve()), 'make', 'Main.elm', '--output='+str(reference)], cwd=root, env={**os.environ, 'GHCRTS':'-N1 -A16m -c'}, check=True, capture_output=True, text=True)
        results = []
        for bundle in (native, reference):
            run = subprocess.run(['node', '-e', runner, str(bundle)], check=True, capture_output=True, text=True, timeout=10)
            results.append(json.loads(run.stdout))
        assert results[0] == results[1], (name, results)
        assert 'TODO in module `Main` on line' in results[0]['error'], results
        assert results[0]['error'].endswith('runtime probe'), results
        print(f'{name}: identical runtime error and location')
