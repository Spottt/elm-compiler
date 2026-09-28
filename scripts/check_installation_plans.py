#!/usr/bin/env python3
"""Compare offline install plans with Elm in isolated projects.

By default decline proposals; --accept also compares the complete manifest
written by Elm. --rust-cli exercises Rust installation and verification instead
of the read-only plan probe. Compare status, stdout and manifests; diagnostic
stderr text, ANSI output, and online downloads are not compared.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile


def main():
    crate = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elm', type=Path, required=True)
    parser.add_argument('--probe', type=Path, default=crate/'target/debug/examples/installation_plan')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--accept', action='store_true', help='Accept only in isolated fixtures and compare complete manifests')
    parser.add_argument('--rust-cli', type=Path, help='Exercise the actual Rust install command instead of the read-only probe')
    args = parser.parse_args()
    results = []
    original_home = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))
    for case in ['preserve', 'patch', 'minor', 'major', 'indirect', 'backtrack', 'test-dependencies', 'downgrade', 'remove-indirect', 'no-solution',
                 'promote-indirect', 'promote-test', 'promote-test-indirect', 'already-installed',
                 'package-add', 'package-promote', 'package-installed', 'broken-dependency']:
        with tempfile.TemporaryDirectory(prefix='elm-install-plan-') as temporary:
            root = Path(temporary)
            (root/'src').mkdir()
            home = root/'home'
            cache = home/'0.19.1/packages'
            versions = {}

            def package(name, version, dependencies):
                versions.setdefault(name, []).append(tuple(map(int, version.split('.'))))
                path = cache/name/version
                (path/'src').mkdir(parents=True)
                if name in ['elm/core', 'elm/json']:
                    shutil.copytree(original_home/'0.19.1/packages'/name/version, path, dirs_exist_ok=True)
                    return
                dependencies = {'elm/core': '1.0.0 <= v < 2.0.0', **dependencies}
                (path/'elm.json').write_text(json.dumps({'type': 'package', 'name': name, 'version': version,
                    'summary': 'Installation solver regression fixture.', 'license': 'BSD-3-Clause',
                    'elm-version': '0.19.0 <= v < 0.20.0', 'exposed-modules': [],
                    'dependencies': dependencies, 'test-dependencies': {}}))

            package('elm/core', '1.0.5', {})
            package('elm/json', '1.1.3', {'elm/core': '1.0.0 <= v < 2.0.0'})
            for version in ['1.0.0', '1.0.1', '1.0.2', '1.1.0', '1.2.0', '2.0.0', '3.0.0']:
                package('author/base', version, {})
            lower = {'patch': '1.0.1', 'minor': '1.1.0', 'major': '2.0.0', 'no-solution': '4.0.0'}.get(case, '1.0.0')
            package('author/new', '1.0.0', {'author/base': lower+' <= v < 5.0.0'})
            if case == 'broken-dependency':
                manifest = cache/'author/new/1.0.0/elm.json'
                value = json.loads(manifest.read_text())
                value['exposed-modules'] = ['Broken']
                manifest.write_text(json.dumps(value))
                (manifest.parent/'src/Broken.elm').write_text('module Broken exposing (value)\nvalue : Int\nvalue = "wrong"\n')
            direct = {'elm/core': '1.0.5', 'elm/json': '1.1.3', 'author/base': '1.0.0'}
            indirect, test_direct, test_indirect = {}, {}, {}
            if case == 'downgrade':
                direct['author/base'] = '2.0.0'
                manifest = cache/'author/new/1.0.0/elm.json'
                value = json.loads(manifest.read_text())
                value['dependencies']['author/base'] = '1.0.0 <= v < 2.0.0'
                manifest.write_text(json.dumps(value))
            if case == 'remove-indirect':
                package('author/old', '1.0.0', {})
                indirect['author/old'] = '1.0.0'
                for name, version, dependency, constraint in [
                    ('author/base', '1.0.0', 'author/old', '1.0.0 <= v < 2.0.0'),
                    ('author/new', '1.0.0', 'author/base', '1.0.1 <= v < 2.0.0')]:
                    manifest = cache/name/version/'elm.json'
                    value = json.loads(manifest.read_text())
                    value['dependencies'][dependency] = constraint
                    manifest.write_text(json.dumps(value))
            if case == 'backtrack':
                package('author/new', '2.0.0', {'author/base': '2.0.0 <= v < 3.0.0'})
            if case == 'indirect':
                direct.pop('author/base')
                direct['author/holder'] = '1.0.0'
                indirect['author/base'] = '1.0.0'
                package('author/holder', '1.0.0', {'author/base': '1.0.0 <= v < 4.0.0'})
                package('author/holder', '2.0.0', {'author/base': '2.0.0 <= v < 4.0.0'})
                manifest = cache/'author/new/1.0.0/elm.json'
                value = json.loads(manifest.read_text())
                value['dependencies']['author/base'] = '2.0.0 <= v < 4.0.0'
                manifest.write_text(json.dumps(value))
            if case == 'test-dependencies':
                direct.pop('author/base')
                test_direct['author/tests'] = '1.0.0'
                test_indirect['author/base'] = '1.0.0'
                package('author/tests', '1.0.0', {'author/base': '1.0.0 <= v < 2.0.0'})
            if case == 'promote-indirect':
                package('author/holder', '1.0.0', {'author/new': '1.0.0 <= v < 2.0.0'})
                direct['author/holder'] = '1.0.0'
                indirect['author/new'] = '1.0.0'
            if case == 'promote-test':
                test_direct['author/new'] = '1.0.0'
            if case == 'promote-test-indirect':
                package('author/tests', '1.0.0', {'author/new': '1.0.0 <= v < 2.0.0'})
                test_direct['author/tests'] = '1.0.0'
                test_indirect['author/new'] = '1.0.0'
            if case == 'already-installed':
                direct['author/new'] = '1.0.0'
            registry = struct.pack('>qq', sum(map(len, versions.values())), len(versions))
            for name in sorted(versions, key=lambda n: tuple(n.split('/'))):
                for part in name.split('/'):
                    encoded = part.encode()
                    registry += bytes([len(encoded)]) + encoded
                ordered = sorted(versions[name], reverse=True)
                registry += bytes(ordered[0])+struct.pack('>q', len(ordered)-1)
                registry += b''.join(bytes(v) for v in ordered[1:])
            (cache/'registry.dat').write_bytes(registry)
            original = {'type': 'application', 'source-directories': ['src'], 'elm-version': '0.19.1',
                        'dependencies': {'direct': direct, 'indirect': indirect},
                        'test-dependencies': {'direct': test_direct, 'indirect': test_indirect}}
            if case.startswith('package-'):
                original = {'type': 'package', 'name': 'author/project', 'version': '1.0.0',
                            'summary': 'Installation plan test project.', 'license': 'BSD-3-Clause',
                            'elm-version': '0.19.0 <= v < 0.20.0', 'exposed-modules': [],
                            'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0', 'author/base': '1.0.0 <= v < 2.0.0'},
                            'test-dependencies': {}}
                if case == 'package-promote':
                    original['test-dependencies']['author/new'] = '1.0.0 <= v < 3.0.0'
                if case == 'package-installed':
                    original['dependencies']['author/new'] = '1.0.0 <= v < 3.0.0'
            path = root/'elm.json'
            content = json.dumps(original)
            path.write_text(content)
            env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1'}
            for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
                env[key] = 'http://127.0.0.1:9'
            env['no_proxy'] = env['NO_PROXY'] = ''
            if args.rust_cli:
                rust_root = root/'rust-project'
                (rust_root/'src').mkdir(parents=True)
                (rust_root/'elm.json').write_text(content)
                # Installing dependencies must not compile unfinished project code.
                (rust_root/'src/Unfinished.elm').write_text('module Unfinished exposing (..)\nx = ???\n')
                rust = subprocess.run([str(args.rust_cli.resolve()), 'install', 'author/new'], cwd=rust_root,
                                      env=env, input='y\n' if args.accept else 'n\n',
                                      capture_output=True, text=True, timeout=120)
                rust_manifest = json.loads((rust_root/'elm.json').read_text())
                if not args.accept or rust.returncode != 0:
                    assert (rust_root/'elm.json').read_text() == content, 'Failed or declined Rust installation changed original bytes'
            else:
                rust = subprocess.run([str(args.probe.resolve()), str(home), str(path), 'author/new'],
                                      capture_output=True, text=True, timeout=45)
            official = subprocess.run([str(args.elm.resolve()), 'install', 'author/new'], cwd=root, env=env,
                                      input='y\n' if args.accept else 'n\n', capture_output=True, text=True, timeout=45)
            if not args.accept:
                assert path.read_text() == content, 'Declining installation changed elm.json'
            elif official.returncode != 0:
                assert json.loads(path.read_text()) == original, 'Failed installation changed elm.json'
            if args.rust_cli:
                plan = rust_manifest
                passed = (official.returncode == rust.returncode and rust_manifest == json.loads(path.read_text())
                          and official.stdout == rust.stdout)
            elif case == 'no-solution':
                passed = official.returncode != 0 and rust.returncode != 0 and 'CANNOT FIND COMPATIBLE VERSION LOCALLY' in official.stderr
                plan = None
            else:
                selected = ({**original['dependencies'], **original['test-dependencies']} if case.startswith('package-')
                            else {**direct, **indirect, **test_direct, **test_indirect})
                section = None
                changes = 0
                for line in official.stdout.splitlines():
                    if line.strip() in ['Add:', 'Change:', 'Remove:']:
                        section = line.strip()
                    version = r'\d+\.\d+\.\d+(?: <= v < \d+\.\d+\.\d+)?'
                    match = re.fullmatch(r'\s+([a-zA-Z0-9-]+/[a-z0-9-]+)\s+('+version+r')(?:\s+=>\s+('+version+r'))?\s*', line)
                    if match:
                        name, old, new = match.groups()
                        if section == 'Remove:':
                            selected.pop(name)
                        else:
                            selected[name] = new or old
                        changes += 1
                plan = json.loads(rust.stdout) if rust.returncode == 0 else None
                rust_selected = ({**plan['dependencies'], **plan['test-dependencies']} if case.startswith('package-')
                                 else {**plan['dependencies']['direct'], **plan['dependencies']['indirect'],
                                       **plan['test-dependencies']['direct'], **plan['test-dependencies']['indirect']}) if plan else None
                no_version_change = case.startswith('promote-') or case in ['already-installed', 'package-promote', 'package-installed']
                expected_prompt = ('It is already installed!' if case in ['already-installed', 'package-installed']
                                   else 'I found it in your elm.json file,')
                observed_plan = expected_prompt in official.stdout if no_version_change else changes > 0
                passed = official.returncode == 0 and observed_plan and rust_selected == selected
                if args.accept and case != 'broken-dependency':
                    passed = passed and plan == json.loads(path.read_text())
                if args.accept and case == 'broken-dependency':
                    passed = official.returncode != 0 and 'PROBLEM BUILDING DEPENDENCIES' in official.stderr
            results.append({'case': case, 'passed': passed, 'official_stdout': official.stdout,
                            'official_stderr': official.stderr, 'rust_stdout': rust.stdout, 'rust_stderr': rust.stderr, 'rust_plan': plan})
    report = {'scope': __doc__, 'accepted_official_plans': args.accept,
              'rust_cli': bool(args.rust_cli),
              'binary_sha256': hashlib.sha256((args.rust_cli or args.probe).read_bytes()).hexdigest(),
              'passed': all(r['passed'] for r in results), 'cases': len(results), 'results': results}
    args.report.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
    raise SystemExit(not report['passed'])


if __name__ == '__main__':
    main()
