#!/usr/bin/env python3
"""Compare plain/ANSI install errors and make dependency controls with Elm 0.19.1.

Uses only synthetic isolated caches and a refused local proxy; does not cover
online registry errors or argument-parser diagnostics. For make controls, compare
error status and stderr (plain/ANSI); stdout progress is retained but not asserted.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
from compiler_test_support import run_terminal


def registry(names):
    data = struct.pack('>qq', len(names), len(names))
    for name in sorted(names, key=lambda s: tuple(s.split('/'))):
        for part in name.split('/'):
            value = part.encode()
            data += bytes([len(value)]) + value
        data += bytes([1, 0, 0])+struct.pack('>q', 0)
    return data


def main():
    crate = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elm', type=Path, required=True)
    parser.add_argument('--rust', type=Path, default=crate/'target/release/planexpo-elm')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    names = ['elm/core', 'elm/json', 'elm-explorations/test', 'author/pkg', 'author/pkgs',
             'author/pack', 'author-extra/pkg', 'author/extra-pkg', 'x/test', 'z/test']
    queries = ['author/pgg', 'elm/josn', 'ELM/jsno', 'random/test',
               'an-author-with-a-long-name/a-package-with-an-equally-long-name', 'author/pkg']
    results = []
    with tempfile.TemporaryDirectory(prefix='elm-install-errors-') as temporary:
        base = Path(temporary)
        home = base/'home'
        cache = home/'0.19.1/packages'
        cache.mkdir(parents=True)
        for name in names:
            root = cache/name/'1.0.0'
            (root/'src').mkdir(parents=True)
            value = {'type': 'package', 'name': name, 'summary': 'Install error fixture.',
                     'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': [],
                     'elm-version': '0.20.0 <= v < 0.21.0' if name == 'author/pkg' else '0.19.0 <= v < 0.20.0',
                     'dependencies': {}, 'test-dependencies': {}}
            (root/'elm.json').write_text(json.dumps(value))
        for kind in ['application', 'package']:
            for query in queries+['empty/registry', 'make/constraints']:
                (cache/'registry.dat').write_bytes(registry([] if query == 'empty/registry' else names))
                root = base/(kind+'-'+query.replace('/', '-'))
                (root/'src').mkdir(parents=True)
                if kind == 'application':
                    outline = {'type': kind, 'source-directories': ['src'], 'elm-version': '0.19.1',
                               'dependencies': {'direct': {'elm/core': '1.0.0', 'elm/json': '1.0.0'}, 'indirect': {}},
                               'test-dependencies': {'direct': {}, 'indirect': {}}}
                else:
                    outline = {'type': kind, 'name': 'author/project', 'summary': 'Install error fixture.',
                               'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': [],
                               'elm-version': '0.19.0 <= v < 0.20.0',
                               'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}
                original = json.dumps(outline)
                if query == 'make/constraints':
                    if kind == 'application':
                        outline['dependencies']['direct']['author/pkg'] = '1.0.0'
                    else:
                        outline['dependencies']['author/pkg'] = '1.0.0 <= v < 2.0.0'
                    original = json.dumps(outline)
                    (root/'src/Main.elm').write_text('module Main exposing (value)\nvalue = 1\n')
                (root/'elm.json').write_text(original)
                env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1'}
                for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
                    env[key] = 'http://127.0.0.1:9'
                env['no_proxy'] = env['NO_PROXY'] = ''
                observations = []
                command = ['make', 'src/Main.elm', '--output=/dev/null'] if query == 'make/constraints' else ['install', query]
                for compiler in [args.elm.resolve(), args.rust.resolve()]:
                    result = subprocess.run([str(compiler), *command], cwd=root, env=env,
                                            input='', text=True, capture_output=True, timeout=30)
                    observations.append({'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
                    ansi_code, ansi = run_terminal([str(compiler), *command], root, env)
                    observations[-1].update(ansi_code=ansi_code, ansi=ansi)
                    assert (root/'elm.json').read_text() == original
                fields = ['code', 'stderr', 'ansi_code', 'ansi']
                if query != 'make/constraints':
                    fields.append('stdout')
                results.append({'case': kind+':'+query, 'compared_fields': fields,
                                'passed': all(observations[0][key] == observations[1][key] for key in fields),
                                'stdout_matches': observations[0]['stdout'] == observations[1]['stdout'],
                                'official': observations[0], 'rust': observations[1]})
    report = {'scope': __doc__, 'compiler_sha256': hashlib.sha256(args.rust.read_bytes()).hexdigest(),
              'passed': all(r['passed'] for r in results), 'cases': len(results), 'results': results}
    args.report.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
    raise SystemExit(not report['passed'])


if __name__ == '__main__':
    main()
