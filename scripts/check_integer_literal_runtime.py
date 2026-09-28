#!/usr/bin/env python3
"""Compare 64-bit integer literal accumulation and pattern matching with Elm."""
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
binary = crate/'target/release/planexpo-elm'
source = r'''port module Main exposing (main)
import Platform
port outgoing : List Int -> Cmd msg
check n = case n of
    18446744073709551616 -> 7
    _ -> 9
main : Program () () Never
main = Platform.worker
    { init = \_ -> ((), outgoing [ 9223372036854775808, 18446744073709551615, 18446744073709551616, 18446744073709551617, 0x10000000000000000, check 0 ])
    , update = \_ model -> (model, Cmd.none)
    , subscriptions = \_ -> Sub.none
    }
'''
runner = r'''
const Elm=require(process.argv[1]).Elm;
const app=Elm.Main.init({flags:null});
app.ports.outgoing.subscribe(value=>console.log(JSON.stringify(value)));
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-integer-literals-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    (root/'Main.elm').write_text(source)
    for mode in ['development','production']:
        results = []
        for name in ['elm', 'rust']:
            output = root/f'{name}.js'
            command = ([str(args.elm.resolve()), 'make', 'Main.elm', *(['--optimize'] if mode == 'production' else []), '--output='+str(output)] if name == 'elm' else
                       [str(binary), ('link-js-prod' if mode == 'production' else 'link-js'), str(root/'elm.json'), 'Main.elm', str(output)])
            compiled = subprocess.run(command, cwd=root, env={**os.environ, 'GHCRTS':'-N1 -A16m -c'}, capture_output=True, text=True, timeout=60)
            assert compiled.returncode == 0, (name, compiled.stdout, compiled.stderr)
            run = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, timeout=10)
            assert run.returncode == 0, (name, run.stderr)
            results.append(json.loads(run.stdout))
        assert results[0] == results[1], results
        print(json.dumps({'mode':mode,'outputs':results,'matches':True}))

    # Preserve the observed reference bug separately from executable parity.
    negative = source.replace('outgoing [ 9223372036854775808', 'outgoing [ -9223372036854775808')
    (root/'Main.elm').write_text(negative)
    output = root/'reference-negative.js'
    compiled = subprocess.run([str(args.elm.resolve()), 'make', 'Main.elm', '--optimize', '--output='+str(output)], cwd=root, env={**os.environ,'GHCRTS':'-N1 -A16m -c'}, capture_output=True,text=True,timeout=60)
    assert compiled.returncode == 0, compiled.stderr
    checked = subprocess.run(['node','--check',str(output)],capture_output=True,text=True,timeout=10)
    assert checked.returncode != 0 and '--9223372036854775808' in checked.stderr, checked.stderr
    print('Reference limitation: direct negation of overflowing integer emits invalid JavaScript')
    native = root/'native-negative.js'
    compiled = subprocess.run([str(binary),'link-js-prod',str(root/'elm.json'),'Main.elm',str(native)],capture_output=True,text=True,timeout=60)
    assert compiled.returncode == 0, compiled.stderr
    run = subprocess.run(['node','-e',runner,str(native)],capture_output=True,text=True,timeout=10)
    assert run.returncode == 0, run.stderr
    expected = [-results[1][0], *results[1][1:]]
    assert json.loads(run.stdout) == expected, (run.stdout,expected)
    print('Rust keeps the overflowing negative literal parenthesized, producing valid JavaScript')
