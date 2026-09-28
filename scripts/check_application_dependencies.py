#!/usr/bin/env python3
"""Compare application dependency validation with Elm 0.19.1 in isolated caches."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
base = {'type': 'application', 'elm-version': '0.19.1', 'source-directories': ['src'],
        'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}},
        'test-dependencies': {'direct': {}, 'indirect': {}}}
sections = [('dependencies', 'direct'), ('dependencies', 'indirect'),
            ('test-dependencies', 'direct'), ('test-dependencies', 'indirect')]
cases = [('valid', base, None, True)]
for a in range(4):
    for b in range(a + 1, 4):
        config = copy.deepcopy(base)
        del config['dependencies']['direct']['elm/json']
        for section, category in [sections[a], sections[b]]:
            config[section][category]['elm/json'] = '1.1.3'
        cases.append((f'duplicate-{a}-{b}', config, None, (a, b) == (1, 2)))
config = copy.deepcopy(base)
del config['dependencies']['direct']['elm/json']
config['dependencies']['indirect']['elm/json'] = '1.1.3'
config['test-dependencies']['direct']['elm/json'] = '1.0.0'
cases.append(('unequal-allowed-overlap', config, None, False))
for test in [False, True]:
    config = copy.deepcopy(base)
    config['test-dependencies' if test else 'dependencies']['direct']['elm/html'] = '1.0.0'
    cases.append((f'missing-transitive-{test}', config, None, False))
config = copy.deepcopy(base)
config['dependencies']['direct']['elm/html'] = '1.0.0'
config['dependencies']['indirect']['elm/virtual-dom'] = '1.0.3'
cases.append(('complete-transitive', config, None, True))
cases.append(('incompatible-transitive-version', base, ('json/1.1.3', 'dependencies', {'elm/core': '2.0.0 <= v < 3.0.0'}), False))
cases.append(('incompatible-compiler-version', base, ('json/1.1.3', 'elm-version', '0.20.0 <= v < 0.21.0'), False))
cases.append(('uncached-transitive-dependency', base, ('json/1.1.3', 'dependencies', {'elm/core':'1.0.0 <= v < 2.0.0','author/absent':'1.0.0 <= v < 2.0.0'}), False))
config = copy.deepcopy(base)
del config['dependencies']['direct']['elm/core']
cases.append(('missing-core', config, None, False))
results = []
failures = []
original_home = Path(os.environ.get('ELM_HOME', str(Path.home() / '.elm')))
with tempfile.TemporaryDirectory(prefix='elm-application-deps-') as tmp:
    root = Path(tmp)
    for index, (name, config, mutation, expected) in enumerate(cases):
        project = root / str(index)
        (project / 'src').mkdir(parents=True)
        (project / 'src/Main.elm').write_text('module Main exposing (answer)\nanswer : Int\nanswer = 42\n')
        (project / 'elm.json').write_text(json.dumps(config))
        home = project / 'home'
        packages = home / '0.19.1/packages'
        packages.mkdir(parents=True)
        shutil.copyfile(original_home / '0.19.1/packages/registry.dat', packages / 'registry.dat')
        for package in ['core/1.0.5', 'json/1.1.3', 'html/1.0.0', 'virtual-dom/1.0.3']:
            source = original_home / '0.19.1/packages/elm' / package
            target = packages / 'elm' / package
            shutil.copytree(source / 'src', target / 'src')
            shutil.copyfile(source / 'elm.json', target / 'elm.json')
        if mutation:
            package, key, value = mutation
            path = packages / 'elm' / package / 'elm.json'
            metadata = json.loads(path.read_text())
            metadata[key] = value
            path.write_text(json.dumps(metadata))
        env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1 -A16m -c'}
        row = {'case': name, 'expected': expected}
        for compiler, binary in [('official', args.elm.resolve()), ('rust', crate / 'target/release/planexpo-elm')]:
            result = subprocess.run([str(binary), 'make', 'src/Main.elm', '--output=/dev/null', '--report=json'],
                                    cwd=project, env=env, capture_output=True, text=True, timeout=60)
            row[compiler] = {'accepted': result.returncode == 0, 'stderr': result.stderr}
            if not expected:
                terminal = subprocess.run([str(binary), 'make', 'src/Main.elm', '--output=/dev/null'],
                                          cwd=project, env=env, capture_output=True, text=True, timeout=60)
                row[compiler]['terminal'] = terminal.stderr.rstrip()
                if terminal.returncode == 0: failures.append(name + ': terminal unexpectedly accepted')
        if row['official']['accepted'] != expected or row['rust']['accepted'] != expected:
            failures.append(name)
        if not expected:
            if json.loads(row['official']['stderr']) != json.loads(row['rust']['stderr']):
                failures.append(name + ': diagnostic differs')
            if row['official']['terminal'] != row['rust']['terminal']:
                failures.append(name + ': terminal differs')
        results.append(row)
report = {'cases': len(results), 'failures': failures, 'results': results}
if args.report:
    args.report.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'cases': len(results), 'failures': failures}))
raise SystemExit(bool(failures))
