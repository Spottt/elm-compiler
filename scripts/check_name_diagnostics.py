#!/usr/bin/env python3
"""Compare exact diagnostic titles and regions for name resolution failures."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
args = p.parse_args()
cases = [
    'value = missing',
    'value item item = item',
    'value (item, item) = item',
    'value (item as item) = item',
    'value { item, item } = item',
    'value = \\item item -> item',
    'value = let item = 1\n            item = 2\n        in item',
    'value = case () of\n    (item, item) -> item',
    'value = 1\nvalue = 2',
    'type Thing = One\ntype Thing = Two',
    'type Thing = One | One',
    'type Thing = One\ntype Other = One',
    'type Thing a a = One a',
    'type alias Thing a a = List a',
    'import A exposing (..)\nimport B exposing (..)\nvalue = shared',
    'import A exposing (..)\nimport B exposing (..)\nvalue Same = ()',

    'value = List.missing',
    'value = ("é", missing)',
    'value = { missing | x = 1 }',
    'value = let nested = missing in nested',
    'type alias Thing = Missing',
    'type alias Thing = Missing Int',
    'type Thing = Thing Missing',
    'value (Missing item) = item',
    'value = case () of\n    Missing -> ()',
    'value item = (\\item -> item)',
    'value = let item = 1 in (\\item -> item)',
    'value = { x = 1, x = 2 }',
    'value = { café = 1, café = 2 }',
    'type alias Thing = { x : Int, x : Int }',
    'value = (1,2,3,4)',
    'type alias Thing = List',
    'type alias Thing = List Int Int',
    'type Choice = One Int\nvalue (One a b) = a',
    'type Choice = One Int\nvalue One = ()',
]
results = []
with tempfile.TemporaryDirectory(prefix='elm-name-diagnostics-') as d:
    root = Path(d)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    for module in ['A', 'B']:
        (root/(module+'.elm')).write_text(f'module {module} exposing (..)\nshared = ()\ntype Choice = Same\n')
    for body in cases:
        source = 'module Main exposing (..)\n-- café ☃\n'+body+'\n'
        (root/'Main.elm').write_text(source)
        for output in ['/dev/null', 'output.js', 'output.html']:
            row = {'source':source, 'output':output}
            for label, binary in [('official', args.elm.resolve()), ('rust', crate/'target/release/planexpo-elm')]:
                run = subprocess.run([str(binary), 'make', 'Main.elm', '--output='+output, '--report=json'], cwd=root,
                                     env={**os.environ, 'GHCRTS':'-N1'}, capture_output=True, text=True, timeout=30)
                assert run.returncode == 1, (label, source, run.stdout, run.stderr)
                report = json.loads(run.stderr)
                row[label] = [{'title':problem['title'], 'region':problem['region']}
                              for error in report.get('errors', []) for problem in error['problems']]
                if not row[label]:
                    row[label] = report
                elif label == 'rust':
                    assert report['type'] == 'compile-errors'
                    assert report['errors'][0]['name'] == 'Main'
                    if row[label][0]['title'] == 'AMBIGUOUS NAME':
                        explanation = ''.join(
                            chunk if isinstance(chunk, str) else chunk['string']
                            for chunk in report['errors'][0]['problems'][0]['message'])
                        assert 'A.' in explanation and 'B.' in explanation, explanation
                        assert 'SymbolId' not in explanation, explanation
                    assert Path(report['errors'][0]['path']) == root/'Main.elm'
                    plain = subprocess.run([str(binary), 'make', 'Main.elm', '--output='+output], cwd=root,
                                           capture_output=True, text=True, timeout=30)
                    assert plain.returncode == 1 and '^' in plain.stderr, plain.stderr
                    assert row[label][0]['title'] in plain.stderr, plain.stderr
            row['matches'] = row['official'] == row['rust']
            results.append(row)
report = {'cases':len(results), 'failures':[r for r in results if not r['matches']], 'results':results}
if args.report:
    args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k != 'results'}, indent=2))
raise SystemExit(bool(report['failures']))
