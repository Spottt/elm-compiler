#!/usr/bin/env python3
"""Compare failed/blocked module sets and error categories across independent modules.

Syntax/lexical cases compare module identities only. Message prose, exact regions,
and syntax/lexical categories are outside this suite.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
p.add_argument('--rust', type=Path)
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
rust = (args.rust or crate / 'target/release/planexpo-elm').resolve()
original = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm'))) / '0.19.1/packages'
cases = [
    ('worker-dependencies', {'Alpha':'value : Int\nvalue = \"bad\"', 'Beta':'value : Int\nvalue = \"bad\"', 'Main':'import Alpha\nimport Beta\nmain = Platform.worker { init = \\_ -> ((), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }'}),
    ('two-coverage', {'Alpha':'value (Just x) = x', 'Beta':'value xs = case xs of\n    [] -> 0'}),
    ('coverage-and-main', {'Alpha':'value (Just x) = x', 'Beta':'main = 42'}),
    ('two-types', {'Alpha':'value : Int\nvalue = "bad"', 'Beta':'value : Int\nvalue = "also bad"'}),
    ('three-types', {'Zebra':'value : Int\nvalue = "bad"', 'Alpha':'value : Int\nvalue = "bad"', 'Middle':'value : Int\nvalue = "bad"'}),
    ('two-names', {'Alpha':'value = missing', 'Beta':'value = absent'}),
    ('mixed-name-type', {'Alpha':'value = missing', 'Beta':'value : Int\nvalue = "bad"'}),
    ('blocked-chain', {'Alpha':'value : Int\nvalue = "bad"', 'Beta':'import Alpha\nvalue = Alpha.value', 'Middle':'import Beta\nvalue = Beta.value', 'Other':'value : Int\nvalue = "bad"'}),
    ('shared-valid-dependency', {'Common':'identity x = x\nvalue = identity 1', 'Alpha':'import Common\nvalue : Int\nvalue = Common.identity "bad"', 'Beta':'import Common\nvalue : Int\nvalue = Common.identity "bad"'}),
    ('healthy-after-failure', {'Alpha':'value : Int\nvalue = "bad"', 'Good':'identity x = x\nvalue = (identity 1, identity "ok")', 'Zebra':'value : Int\nvalue = "bad"'}),
]
cases.extend([
    ('verified-prefix-coverage', {
        'Common':'identity x = x',
        'Alpha':'import Common\nvalue (Just x) = Common.identity x',
        'Main':'import Alpha\nmain = Platform.worker { init = \\_ -> (Alpha.value (Just 1), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }',
    }),
    ('verified-prefix-type', {
        'Common':'identity x = x',
        'Alpha':'import Common\nvalue : Int\nvalue = Common.identity "bad"',
        'Main':'import Alpha\nmain = Platform.worker { init = \\_ -> (Alpha.value, Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }',
    }),
])
syntax_cases = [
    ('syntax-independent', {'Alpha':'value =', 'Beta':'value ='}),
    ('syntax-and-type', {'Alpha':'value =', 'Beta':'value : Int\nvalue = "bad"'}),
    ('syntax-blocked-importer', {'Alpha':'value =', 'Beta':'import Alpha\nvalue = Alpha.value', 'Other':'value ='}),
    ('syntax-in-both-importer-and-dependency', {'Alpha':'value =', 'Beta':'import Alpha\nvalue ='}),
]
cases.extend(syntax_cases)
cases.extend([
    ('lexical-root-skips-imports', {'Alpha':'value = \"unterminated', 'Main':'import Alpha\nvalue = \"unterminated'}),
    ('lexical-nested-modules', {'Alpha.Inner':'value = \"unterminated', 'Beta.Inner':'value = \"unterminated'}),
    ('lexical-strings', {'Alpha':'value = "unterminated', 'Beta':'value = "unterminated'}),
    ('lexical-comments', {'Alpha':'value = 1\n{- unfinished', 'Beta':'value = 1\n{- unfinished'}),
    ('lexical-number-and-type', {'Alpha':'value = 0x', 'Beta':'value : Int\nvalue = "bad"'}),
    ('lexical-blocked-importer', {'Alpha':'value = "unterminated', 'Beta':'import Alpha\nvalue = Alpha.value', 'Other':'value = "unterminated'}),
    ('lexical-importer-and-dependency', {'Alpha':'value = "unterminated', 'Beta':'import Alpha\nvalue = "unterminated'}),
    ('lexical-before-missing-import', {'Alpha':'import Missing\nvalue = "unterminated', 'Beta':'value = "unterminated'}),
])
cases.extend([
    ('header-absent-multiple', {'Alpha':'value = 1', 'Beta':'value = 1'}),
    ('header-imported', {'Alpha':'module Alpha exposing (', 'Main':'module Main exposing (..)\nimport Alpha\nmain = Alpha.value'}),
    ('header-and-type', {'Alpha':'module Alpha exposing (', 'Beta':'module Beta exposing (..)\nvalue : Int\nvalue = \"bad\"'}),
    ('header-blocked-importer', {'Alpha':'module Alpha exposing (', 'Beta':'module Beta exposing (..)\nimport Alpha\nvalue = Alpha.value', 'Other':'module Other exposing ('}),
    ('header-wrong-declared-name', {'Alpha':'module Wrong exposing (', 'Beta':'module Different exposing ('}),
    ('header-exposing', {'Alpha':'module Alpha exposing (', 'Beta':'module Beta exposing ('}),
    ('header-missing-name', {'Alpha':'module exposing (..)\nvalue = 1', 'Beta':'module exposing (..)\nvalue = 1'}),
    ('header-lower-name', {'Alpha':'module alpha exposing (..)\nvalue = 1', 'Beta':'module beta exposing (..)\nvalue = 1'}),
    ('header-lexical', {'Alpha':'module Alpha exposing ("unfinished', 'Beta':'module Beta exposing ("unfinished'}),
    ('header-nested', {'Alpha.Inner':'module Alpha.Inner exposing (', 'Beta.Inner':'module Beta.Inner exposing ('}),
])
results, failures = [], []
with tempfile.TemporaryDirectory(prefix='elm-module-errors-') as directory:
    root = Path(directory)
    cache = root/'home/0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat', cache/'registry.dat')
    for package, version in [('elm/core','1.0.5'), ('elm/json','1.1.3')]:
        shutil.copytree(original/package/version/'src', cache/package/version/'src')
        shutil.copyfile(original/package/version/'elm.json', cache/package/version/'elm.json')
    env = {**os.environ, 'ELM_HOME':str(root/'home'), 'GHCRTS':'-N1 -A16m -c', 'no_proxy':'', 'NO_PROXY':''}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    for name, sources in cases:
        project = root/name
        project.mkdir()
        shutil.copyfile(crate/'tests/programs/worker/elm.json', project/'elm.json')
        for module, body in sources.items():
            path = project/(module.replace('.', '/')+'.elm')
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(body + '\n' if name.startswith('header-') else f'module {module} exposing (..)\n{body}\n')
        command = ['make', *(["Main.elm"] if 'Main' in sources else [module.replace('.', '/')+'.elm' for module in sources]), '--output=result.js', '--report=json']
        outputs = []
        for binary in [args.elm.resolve(), rust]:
            output_path = project/'result.js'
            output_path.write_text('previous successful bundle')
            run = subprocess.run([str(binary), *command], cwd=project, env=env, capture_output=True, text=True, timeout=60)
            if output_path.read_text() != 'previous successful bundle':
                failures.append(name + ': output overwritten')
            report = json.loads(run.stderr)
            errors = [(error['name'], [problem['title'] for problem in error['problems']]) for error in report.get('errors', [])]
            outputs.append({'returncode':run.returncode, 'errors':errors, 'report':report})
        compare_categories = not name.startswith(('syntax-', 'lexical-', 'header-'))
        expected = outputs[0]['errors'] if compare_categories else [error[0] for error in outputs[0]['errors']]
        actual = outputs[1]['errors'] if compare_categories else [error[0] for error in outputs[1]['errors']]
        if outputs[0]['returncode'] != outputs[1]['returncode'] or expected != actual:
            failures.append(name)
        incremental = []
        for temperature in ['cold', 'warm']:
            run = subprocess.run([str(rust), *command, '--incremental'], cwd=project, env=env, capture_output=True, text=True, timeout=60)
            cached_report = json.loads(run.stderr)
            equal = run.returncode == outputs[1]['returncode'] and cached_report == outputs[1]['report']
            preserved = output_path.read_text() == 'previous successful bundle'
            if not equal or not preserved:
                failures.append(name + ': incremental ' + temperature)
            incremental.append({'temperature':temperature, 'equal_to_uncached':equal, 'output_preserved':preserved})
        results.append({'categories_compared':compare_categories, 'case':name, 'official':outputs[0], 'rust':outputs[1], 'incremental':incremental})
report = {'scope':__doc__, 'cases':len(results), 'failures':failures, 'results':results,
          'compiler_sha256':hashlib.sha256(rust.read_bytes()).hexdigest()}
if args.report:
    args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'cases':len(results), 'failures':failures}))
raise SystemExit(bool(failures))
