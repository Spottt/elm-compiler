#!/usr/bin/env python3
"""Compare acceptance of deeply nested, ignored manifest values with Elm 0.19.1."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
for name in ['elm', 'rust', 'report']:
    parser.add_argument('--' + name, type=Path, required=True)
args = parser.parse_args()
binaries = {'elm': args.elm.resolve(), 'rust': args.rust.resolve()}
sha = {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()}
config = {'type': 'application', 'elm-version': '0.19.1', 'source-directories': ['src'],
          'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}},
          'test-dependencies': {'direct': {}, 'indirect': {}}}
results = []
with tempfile.TemporaryDirectory(prefix='outline-depth-') as temp:
    root = Path(temp)
    home = root / 'home'
    shutil.copytree(Path(os.environ.get('ELM_HOME', Path.home() / '.elm')) / '0.19.1/packages', home / '0.19.1/packages')
    (root / 'src').mkdir()
    (root / 'src/Main.elm').write_text('module Main exposing (x)\nx = 1\n')
    for location in ['application', 'dependencies', 'package']:
        for shape, opening, closing in [('array', '[', ']'), ('object', '{"value":', '}')]:
            for depth in [100, 125, 128, 150, 200, 1000, 10000]:
                nested = opening * depth + '0' + closing * depth
                if location == 'application':
                    source = json.dumps(config)[:-1] + ',"unused":' + nested + '}'
                elif location == 'dependencies':
                    base = json.loads(json.dumps(config)); base['dependencies']['extension'] = 'PLACEHOLDER'
                    source = json.dumps(base).replace('"PLACEHOLDER"', nested)
                else:
                    base = {'type':'package','name':'author/fixture','summary':'Fixture.','license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
                    source = json.dumps(base)[:-1] + ',"unused":' + nested + '}'
                (root / 'elm.json').write_text(source)
                row = {'case': f'{location}-{shape}-{depth}'}
                for name, binary in binaries.items():
                    run = subprocess.run([str(binary), 'make', 'src/Main.elm', '--output=/dev/null', '--report=json'],
                                         cwd=root, env={**os.environ, 'GHCRTS': '-N1', 'ELM_HOME': str(home)},
                                         capture_output=True, text=True, timeout=30)
                    row[name] = {'returncode': run.returncode, 'stderr': run.stderr}
                row['passed'] = all(row[name]['returncode'] == 0 and row[name]['stderr'] == '' for name in binaries)
                results.append(row)
                print(row['case'], 'PASS' if row['passed'] else 'FAIL', flush=True)
assert sha == {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()}
report = {'scope': __doc__, 'sha256': sha, 'cases': len(results), 'passed': all(row['passed'] for row in results), 'results': results}
args.report.write_text(json.dumps(report, indent=2) + '\n')
raise SystemExit(not report['passed'])
