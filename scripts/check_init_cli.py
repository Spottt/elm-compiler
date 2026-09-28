#!/usr/bin/env python3
"""Compare interactive init, terminal streams and generated manifests; no live downloads."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
from diff_test_process import run_command
import tempfile


def main():
    crate = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elm', type=Path, required=True)
    parser.add_argument('--rust', type=Path, default=crate/'target/release/planexpo-elm')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--ansi', action='store_true')
    parser.add_argument('--terminal-stream', choices=['stdout', 'stderr'], default='stderr')
    args = parser.parse_args()
    compilers = [args.elm.resolve(), args.rust.resolve()]
    cached = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    results = []
    with tempfile.TemporaryDirectory(prefix='elm-init-parity-') as temporary:
        base = Path(temporary)
        home = base/'home'
        packages = home/'0.19.1/packages'
        packages.mkdir(parents=True)
        shutil.copy2(cached/'registry.dat', packages/'registry.dat')
        for name in ['core', 'browser', 'html', 'json', 'virtual-dom', 'time', 'url']:
            shutil.copytree(cached/'elm'/name, packages/'elm'/name)
        missing = base/'missing'
        (missing/'0.19.1/packages').mkdir(parents=True)
        shutil.copy2(cached/'registry.dat', missing/'0.19.1/packages/registry.dat')
        for label, answer in [('install-eof', ''), ('install-retry-eof', 'maybe\n'), ('install-decline', 'n\n'), ('eof', ''), ('retry-eof', 'maybe\n'), ('decline', 'n\n'), ('retry', 'N\nno\nyes\nn\n'),
                              ('default', '\n'), ('yes', 'y\n'), ('capital-yes', 'Y\n'),
                              ('retry-yes', 'yes\ny\n'), ('existing', ''),
                              ('offline-no-solution', 'y\n'), ('help', '')]:
            observations = []
            for index, compiler in enumerate(compilers):
                root = base/f'{label}-{index}'
                root.mkdir()
                (root/'src').mkdir()
                (root/'src/keep.txt').write_text('preserved')
                initial_manifest = None
                if label.startswith('install-'):
                    shutil.copyfile(crate/'tests/programs/worker/elm.json', root/'elm.json')
                    initial_manifest = (root/'elm.json').read_bytes()
                if label == 'existing':
                    (root/'elm.json').write_text('original')
                env = {**os.environ, 'ELM_HOME': str(missing if label == 'offline-no-solution' else home), 'GHCRTS': '-N1'}
                for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
                    env[key] = 'http://127.0.0.1:9'
                env['no_proxy'] = env['NO_PROXY'] = ''
                command = [str(compiler)] + (['install', 'elm/url'] if label.startswith('install-') else ['init'] + (['--help'] if label == 'help' else []))
                process = run_command(command, root, env, args.ansi, args.terminal_stream, input_text=answer)
                assert (root/'src/keep.txt').read_text() == 'preserved'
                if initial_manifest is not None:
                    assert (root/'elm.json').read_bytes() == initial_manifest
                manifest = (root/'elm.json').read_text() if (root/'elm.json').exists() else None
                if manifest and label != 'existing':
                    manifest = json.loads(manifest)
                observations.append({'returncode': process['code'], 'stdout': process['stdout'],
                                     'stderr': process['stderr'], 'manifest': manifest})
            results.append({'case': label, 'passed': observations[0] == observations[1],
                            'official': observations[0], 'rust': observations[1]})
    report = {'scope': __doc__, 'ansi': args.ansi, 'terminal_stream': args.terminal_stream, 'compiler_sha256': hashlib.sha256(compilers[1].read_bytes()).hexdigest(),
              'cases': len(results), 'passed': all(r['passed'] for r in results), 'results': results}
    args.report.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({'cases': len(results), 'failures': [r['case'] for r in results if not r['passed']]}))
    raise SystemExit(not report['passed'])


if __name__ == '__main__':
    main()
