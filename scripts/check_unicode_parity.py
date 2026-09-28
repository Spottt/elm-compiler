#!/usr/bin/env python3
"""Compare identifier categories with the official Elm 0.19.1 compiler."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, required=True)
args = parser.parse_args()
compilers = [args.elm.resolve(), crate/'target/release/planexpo-elm']
# Letter categories, derived Alphabetic/Lowercase/Uppercase properties, marks,
# numeric letters, ASCII-only digits, and characters introduced after GHC 8.4.3.
characters = 'aZéÉǅǈǋǲ中ªºʰᵃⅰⅠ⒜Ⓐ\u0345\u05b0\u093e\u2160\u24d0\u1c90\uab70\U000104b0\U0001e900\U00010400\U00010428٠0_'
results = []
with tempfile.TemporaryDirectory(prefix='elm-unicode-parity-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    for char in characters:
        for form, body in [('lower', f'{char}value = ()'),
                           ('upper', f'type Box = {char}Value'),
                           ('inner', f'value{char}suffix = ()')]:
            target = {'lower':f'{char}value', 'upper':f'{char}Value', 'inner':f'value{char}suffix'}[form]
            source = ('port module Fixture exposing (..)\nimport Platform\nimport Debug\n'
                      + body + '\nport result : String -> Cmd msg\n'
                      + 'main : Program () () Never\nmain = Platform.worker { init = \\() -> ((), result (Debug.toString '
                      + target + ')), update = \\msg _ -> never msg, subscriptions = \\_ -> Sub.none }\n')
            (root/'Fixture.elm').write_text(source)
            accepted = []
            for compiler in compilers:
                run = subprocess.run([str(compiler),'make','Fixture.elm','--output=/dev/null','--report=json'],
                    cwd=root,env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=30)
                assert run.returncode in (0,1), run.stderr
                if run.returncode:
                    error = json.loads(run.stderr)
                    assert error.get('type') in ('error','compile-errors'), error
                accepted.append(run.returncode == 0)
            if all(accepted):
                values = []
                for index, compiler in enumerate(compilers):
                    output = root/f'output{index}.js'
                    build = subprocess.run([str(compiler),'make','Fixture.elm','--output='+str(output)],
                        cwd=root,env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=30)
                    assert build.returncode == 0, build.stderr
                    run = subprocess.run(['node','-e',
                        'require(process.argv[1]).Elm.Fixture.init({flags:null}).ports.result.subscribe(v=>console.log(JSON.stringify(v)))',
                        str(output)],capture_output=True,text=True,timeout=10,check=True)
                    values.append(json.loads(run.stdout))
                assert values[0] == values[1], (char, form, values)
            results.append({'codepoint':f'U+{ord(char):04X}','form':form,'accepted':accepted})
failures = [row for row in results if row['accepted'][0] != row['accepted'][1]]
print(json.dumps({'cases':len(results),'failures':failures},ensure_ascii=False,indent=2))
raise SystemExit(bool(failures))
