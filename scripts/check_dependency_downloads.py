#!/usr/bin/env python3
"""Opt-in network check: compile applications and packages from empty ELM_HOME."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--report', type=Path)
args = parser.parse_args()
crate = Path(__file__).resolve().parents[1]
rust = crate / 'target/release/planexpo-elm'
results = []
with tempfile.TemporaryDirectory(prefix='elm-acquisition-') as temp:
    root = Path(temp)
    for kind in ['application', 'package']:
        project = root / kind
        home = root / (kind + '-home')
        home.mkdir()
        project.mkdir()
        if kind == 'application':
            shutil.copytree(crate / 'tests/programs/worker', project, dirs_exist_ok=True)
            arguments = ['Main.elm', '--output=compiled.js']
            expected = ['elm/core/1.0.5', 'elm/json/1.1.3']
        else:
            (project / 'src').mkdir()
            (project / 'src/Library.elm').write_text('module Library exposing (answer)\nanswer : Int\nanswer = 42\n')
            (project / 'elm.json').write_text(json.dumps({
                'type': 'package', 'name': 'author/acquisition-test',
                'summary': 'A package for checking dependency acquisition',
                'license': 'BSD-3-Clause', 'version': '1.0.0',
                'exposed-modules': ['Library'], 'elm-version': '0.19.0 <= v < 0.20.0',
                'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}))
            arguments = []
            expected = ['elm/core/1.0.5']
        env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1 -A16m -c'}
        def run(binary, environment):
            result = subprocess.run([str(binary), 'make', *arguments], cwd=project,
                                    env=environment, capture_output=True, text=True, timeout=180)
            assert result.returncode == 0, (kind, str(binary), result.stdout, result.stderr)
        assert not list(home.iterdir())
        run(rust, env)
        for package in expected:
            assert (home / '0.19.1/packages' / package / 'src').is_dir(), package
        assert (home / '0.19.1/packages/registry.dat').is_file()
        # A changed Elm source must still compile when all HTTP proxies refuse
        # connections. This cannot be satisfied by reusing a previous JS output.
        source = project / ('Main.elm' if kind == 'application' else 'src/Library.elm')
        with source.open('a') as out:
            out.write('\n-- Offline recompilation after dependency acquisition\n')
        offline = {**env, **{k: 'http://127.0.0.1:9' for k in
                            ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'http_proxy', 'https_proxy', 'all_proxy']},
                   'NO_PROXY': '', 'no_proxy': ''}
        run(rust, offline)
        run(args.elm.resolve(), env)
        if kind == 'application':
            check = subprocess.run(['python3', str(crate / 'scripts/check_worker_runtime.py'),
                                    '--elm', str(args.elm.resolve())], env=env,
                                   capture_output=True, text=True, timeout=180)
            assert check.returncode == 0, check.stdout + check.stderr
            variants = [json.loads(line) for line in check.stdout.splitlines() if line.startswith('{')]
            assert len(variants) == 4 and all(v['match'] for v in variants)
        else:
            variants = []
        result = {'kind': kind, 'cold_make': True, 'offline_recompile': True,
                  'official_accepts': True, 'packages': expected, 'runtime_variants': variants}
        results.append(result)
        print(json.dumps(result), flush=True)
report = {'passed': True, 'binary_sha256': hashlib.sha256(rust.read_bytes()).hexdigest(), 'results': results}
if args.report:
    args.report.write_text(json.dumps(report, indent=2) + '\n')
