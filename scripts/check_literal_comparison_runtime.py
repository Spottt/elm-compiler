#!/usr/bin/env python3
"""Compare the results of comparisons against literals with official Elm.

Comparisons with a literal operand are emitted as direct JavaScript operators.
The program evaluates each form on ordinary and special numbers (NaN, signed
zeros, infinities) and on the strings, characters and booleans derived from
them, in development and --optimize builds; every result must equal official
Elm's.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--rust', type=Path)
parser.add_argument('--report', type=Path)
args = parser.parse_args()
crate = Path(__file__).resolve().parents[1]
compilers = {'elm': args.elm.resolve(), 'rust': (args.rust or crate/'target/release/planexpo-elm').resolve()}
source = r'''port module Main exposing (main)
import Platform
port incoming : (Float -> msg) -> Sub msg
port outgoing : List Bool -> Cmd msg
unit u = [ u == (), () == u, u /= () ]
calculate f =
    let
        i = round f
        s = String.fromFloat f
        c = Char.fromCode (if isNaN f then 97 else clamp 0 200 (round f))
        b = f > 0
    in
    [ f == 0, 0 == f, f /= 0, 0 /= f, f == 0.0, f /= 0.0, f == 0x00, f == 1, 1 == f, f /= 1
    , f == 1.5, f == 1e3, f == -1, f /= -1, (==) f 0, (f |> (==) 0), (/=) 0 <| f
    , f < 1, f <= 1, f > 1, f >= 1, 1 < f, 1 <= f, 1 > f, 1 >= f
    , f < 0, f <= 0, f > 0, f >= 0, f > 1.5, f < -1, f >= -1, (<) f 1
    , i == 0, i /= 0, i == 10, i > 10, i <= 10, 0 == i, 10 >= i
    , s == "0", s /= "0", s == "NaN", "1" == s, s < "1", s <= "1", s > "1", s >= "NaN", "a" > s
    , c == 'a', c /= 'a', 'a' == c, c < 'a', c <= 'a', c > 'a', c >= 'a', 'b' > c
    , b == True, b == False, True == b, False == b, b /= True, b /= False, True /= b, False /= b
    ] ++ unit ()
main : Program () () Float
main = Platform.worker
    { init = \_ -> ((), Cmd.none)
    , update = \value model -> (model, outgoing (calculate value))
    , subscriptions = \_ -> incoming identity
    }
'''
runner = r'''
const app=require(process.argv[1]).Elm.Main.init({flags:null});
const values=[0,-0,1,-1,0.5,1.5,10,11,97,98,1000,NaN,Infinity,-Infinity];
const outputs=[];
app.ports.outgoing.subscribe(value=>outputs.push(value.map(Number).join('')));
for(const value of values)app.ports.incoming.send(value);
setTimeout(()=>console.log(JSON.stringify(Object.fromEntries(values.map((v,i)=>[Object.is(v,-0)?'-0':String(v),outputs[i]])))),30);
'''
report = {'modes': {}, 'match': True}
with tempfile.TemporaryDirectory(prefix='elm-rust-literal-comparison-') as directory:
    for mode, flags in {'development': [], 'optimize': ['--optimize']}.items():
        results = {}
        for name, binary in compilers.items():
            root = Path(directory)/mode/name
            root.mkdir(parents=True)
            shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
            (root/'Main.elm').write_text(source)
            compiled = subprocess.run([str(binary), 'make', 'Main.elm', '--output=main.js', *flags], cwd=root,
                                      env={**os.environ, 'GHCRTS': '-N1 -A16m -c'}, capture_output=True, text=True, timeout=120)
            assert compiled.returncode == 0, (mode, name, compiled.stdout, compiled.stderr)
            run = subprocess.run(['node', '-e', runner, str(root/'main.js')], capture_output=True, text=True, timeout=30)
            assert run.returncode == 0, (mode, name, run.stderr[-3000:])
            results[name] = json.loads(run.stdout)
        differences = {value: [results['elm'][value], results['rust'].get(value)]
                       for value in results['elm'] if results['elm'][value] != results['rust'].get(value)}
        report['modes'][mode] = {'values': len(results['elm']), 'comparisons': len(next(iter(results['elm'].values()))),
                                 'elm': results['elm'], 'differences': differences, 'match': not differences}
        report['match'] &= not differences
if args.report:
    args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({mode: {key: row[key] for key in ('values', 'comparisons', 'differences', 'match')}
                  for mode, row in report['modes'].items()}))
raise SystemExit(0 if report['match'] else 1)
