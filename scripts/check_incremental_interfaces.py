#!/usr/bin/env python3
"""Exercise real module edits through cached/full code generation and execution.

Optional Elm 0.19.1 oracle covers all fixtures. Operator declarations live in an
isolated copy of a kernel package; the user's package cache is never modified.
"""
import os
os.environ["PLANEXPO_ELM_STATS"] = "1"
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=Path)
parser.add_argument('--report', type=Path)
args = parser.parse_args()
crate = Path(__file__).resolve().parents[1]
binary = crate / 'target/release/planexpo-elm'
oracle = args.elm.resolve() if args.elm else None
runner = "const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});app.ports.outgoing.subscribe(x=>console.log(JSON.stringify(x)));app.ports.incoming.send(1);"
# Each edit changes only Api.elm. Bridge and Main must be reconsidered according
# to actual public interfaces, not a source hash propagated to every dependent.
cases = [
    ('record alias', 'Api.data.n', [
        ('module Api exposing (Row, data)\ntype alias Row = { n : Int }\ndata : Row\ndata = { n = 1 }\n', 1, None),
        ('module Api exposing (Row, data)\ntype alias Row = { n : Int }\ndata : Row\ndata = { n = 2 }\n', 2, 1),
        ('module Api exposing (Row, data)\ntype alias Row = { n : Int, extra : () }\ndata : Row\ndata = { n = 3, extra = () }\n', 3, 2),
        ('module Api exposing (Row, data)\ntype alias Row = { n : String }\ndata : Row\ndata = { n = "bad" }\n', None, None),
    ], True),
    ('alias parameters', 'Tuple.first Api.data', [
        ('module Api exposing (Pair, data)\ntype alias Pair a b = (a,b)\ndata : Pair Int String\ndata = (1,"x")\n', 1, None),
        ('module Api exposing (Pair, data)\ntype alias Pair a b = (b,a)\ndata : Pair Int String\ndata = ("x",1)\n', None, None),
    ], True),
    ('constructor coverage', 'case Api.choice of\n        Api.First -> 1\n        Api.Second -> 2', [
        ('module Api exposing (Choice(..), choice)\ntype Choice = First | Second\nchoice = First\n', 1, None),
        ('module Api exposing (Choice(..), choice)\ntype Choice = First | Second\nchoice = Second\n', 2, 1),
        ('module Api exposing (Choice(..), choice)\ntype Choice = First | Second | Third\nchoice = Third\n', None, None),
    ], True),
    ('constructor payload', 'case Api.choice of\n        Api.Choice n -> n', [
        ('module Api exposing (Choice(..), choice)\ntype Choice = Choice Int\nchoice = Choice 1\n', 1, None),
        ('module Api exposing (Choice(..), choice)\ntype Choice = Choice String\nchoice = Choice "bad"\n', None, None),
    ], True),
    ('constructor exposure', 'case Api.choice of\n        Api.Choice n -> n', [
        ('module Api exposing (Choice(..), choice)\ntype Choice = Choice Int\nchoice = Choice 1\n', 1, None),
        ('module Api exposing (Choice, choice)\ntype Choice = Choice Int\nchoice = Choice 1\n', None, None),
    ], True),
    ('polymorphic constraint', 'List.length (Api.identity [()])', [
        ('module Api exposing (identity)\nidentity x = x\n', 1, None),
        ('module Api exposing (identity)\nidentity x = x + 1\n', None, None),
    ], True),
    ('open record field', 'Api.get { value = 1, extra = () }', [
        ('module Api exposing (get)\nget r = r.value\n', 1, None),
        ('module Api exposing (get)\nget r = r.extra\n', None, None),
    ], True),
    ('record constructor order', 'Api.Row 1 2 |> .first', [
        ('module Api exposing (Row)\ntype alias Row = { first : Int, second : Int }\n', 1, None),
        ('module Api exposing (Row)\ntype alias Row = { second : Int, first : Int }\n', 2, None),
    ], True),
    ('constructor order', 'case Api.choice of\n        Api.First -> 1\n        Api.Second -> 2', [
        ('module Api exposing (Choice(..), choice)\ntype Choice = First | Second\nchoice = First\n', 1, None),
        ('module Api exposing (Choice(..), choice)\ntype Choice = Second | First\nchoice = First\n', 1, None),
    ], True),
    ('operator implementation target', '1 |= 2', [
        ('module Api exposing ((|=))\ninfix left 4 (|=) = choose\nchoose a b = a * 10 + b\nselect a b = b * 10 + a\n', 12, None),
        ('module Api exposing ((|=))\ninfix left 4 (|=) = select\nchoose a b = a * 10 + b\nselect a b = b * 10 + a\n', 21, None),
    ], True),
    ('operator associativity', '1 |= 2 |= 3', [
        ('module Api exposing ((|=))\ninfix left 4 (|=) = combine\ncombine a b = a * 10 + b\n', 123, None),
        ('module Api exposing ((|=))\ninfix right 4 (|=) = combine\ncombine a b = a * 10 + b\n', 33, 2),
    ], True),
    ('operator precedence', '1 + 2 |= 3', [
        ('module Api exposing ((|=))\ninfix left 4 (|=) = combine\ncombine a b = a * 10 + b\n', 33, None),
        ('module Api exposing ((|=))\ninfix left 7 (|=) = combine\ncombine a b = a * 10 + b\n', 24, 2),
    ], True),
]

results = []

def execute(output, expected):
    run = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, timeout=10, check=True)
    assert json.loads(run.stdout) == expected, run.stdout

for optimize in [False, True]:
    for name, expression, edits, historical in cases:
        with tempfile.TemporaryDirectory(prefix='elm-api-incremental-') as directory:
            project = Path(directory)
            shutil.copytree(crate / 'tests/programs/worker', project, dirs_exist_ok=True)
            environment = dict(os.environ)
            api_path = project / 'Api.elm'
            kernel_package = None
            if name.startswith('operator '):
                original_home = Path(os.environ.get('ELM_HOME', str(Path.home() / '.elm')))
                isolated_home = project / 'elm-home'
                packages = isolated_home / '0.19.1/packages'
                for package, version in [('elm/core', '1.0.5'), ('elm/json', '1.1.3')]:
                    source_package = original_home / '0.19.1/packages' / package / version
                    target = packages / package / version
                    shutil.copytree(source_package / 'src', target / 'src')
                    shutil.copy2(source_package / 'elm.json', target / 'elm.json')
                shutil.copy2(original_home / '0.19.1/packages/registry.dat', packages / 'registry.dat')
                kernel_package = packages / 'elm/json/1.1.3'
                manifest = kernel_package / 'elm.json'
                config = json.loads(manifest.read_text())
                config['exposed-modules'].append('Api')
                manifest.write_text(json.dumps(config))
                api_path = kernel_package / 'src/Api.elm'
                environment['ELM_HOME'] = str(isolated_home)
            main = project / 'Main.elm'
            main.write_text(main.read_text().replace('import Platform', 'import Platform\nimport Bridge').replace('value + model', 'Bridge.value'))
            (project / 'Bridge.elm').write_text('module Bridge exposing (value)\nimport Api exposing (..)\nvalue : Int\nvalue = ' + expression + '\n')
            output = project / 'cached.js'
            full = project / 'full.js'
            flags = ['--optimize'] if optimize else []
            def compile(path, extra):
                return subprocess.run([str(binary), 'make', 'Main.elm', *flags, '--output=' + str(path), *extra], cwd=project, env=environment, capture_output=True, text=True, timeout=60)
            def clear_output_cache():
                for file in (project / 'elm-stuff/planexpo-rust/v1').glob('*.cache'):
                    file.unlink()
            warm = None
            baseline = None
            for source, expected, misses in edits:
                api_path.write_text(source)
                clear_output_cache()
                previous = output.read_bytes() if output.exists() else None
                cached = compile(output, ['--incremental'])
                uncached = compile(full, ['--no-cache'])
                assert (cached.returncode == 0) == (expected is not None), (name, cached.stderr)
                assert (uncached.returncode == 0) == (expected is not None), (name, uncached.stderr)
                if expected is None:
                    assert output.read_bytes() == previous, 'invalid API edit overwrote last successful output'
                    again = compile(output, ['--incremental'])
                    assert again.returncode != 0, 'cached invalid API edit was accepted on retry'
                    assert output.read_bytes() == previous
                else:
                    assert output.read_bytes() == full.read_bytes(), (name, 'cached/full JavaScript differs')
                    if baseline is None:
                        baseline = output.read_bytes()
                    execute(output, expected)
                    hits = int(re.search(r'Reused types for (\d+)', cached.stdout)[1])
                    if misses is not None:
                        assert hits == warm - misses, (name, hits, warm, misses)
                    clear_output_cache()
                    again = compile(output, ['--incremental'])
                    assert again.returncode == 0, again.stderr
                    warm = int(re.search(r'Reused types for (\d+)', again.stdout)[1])
                    assert warm > 0
                    assert output.read_bytes() == full.read_bytes()
                if oracle and historical:
                    if kernel_package:
                        # Official Elm assumes released package sources immutable.
                        # Invalidate only this test copy after editing its source.
                        (kernel_package / 'artifacts.dat').unlink(missing_ok=True)
                        shutil.rmtree(project / 'elm-stuff/0.19.1', ignore_errors=True)
                    reference = project / 'historical.js'
                    result = subprocess.run([str(oracle), 'make', 'Main.elm', *flags, '--output=' + str(reference)], cwd=project, env={**environment, 'GHCRTS': '-N1 -A16m -c'}, capture_output=True, text=True, timeout=60)
                    assert (result.returncode == 0) == (expected is not None), (name, result.stderr)
                    if expected is not None:
                        execute(reference, expected)
            # Restore the original API after valid or invalid edits. Intermediate
            # artifacts must not poison a later successful compilation.
            api_path.write_text(edits[0][0])
            clear_output_cache()
            recovered = compile(output, ['--incremental'])
            assert recovered.returncode == 0, recovered.stderr
            assert output.read_bytes() == baseline, 'API restoration changed the original JavaScript'
            execute(output, edits[0][1])
            results.append({
                'scenario': name,
                'mode': 'production' if optimize else 'development',
                'edits': len(edits),
                'historical_oracle': bool(oracle and historical),
                'warm_modules': warm,
                'baseline_sha256': hashlib.sha256(baseline).hexdigest(),
                'passed': True,
            })
            print(f'PASS: {name}, {"production" if optimize else "development"}, oracle={bool(oracle and historical)}', flush=True)

if args.report:
    args.report.write_text(json.dumps({
        'scope': 'Cached/full JavaScript equality, execution, interface invalidation, rejection retries and recovery after API edits. Historical Elm acceptance and execution for all fixtures, including operators in an isolated kernel package.',
        'compiler_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'results': results,
    }, indent=2) + '\n')
