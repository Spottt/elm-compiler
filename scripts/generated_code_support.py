"""Compile the runtime benchmark program with official Elm and this compiler."""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess

CRATE = Path(__file__).resolve().parents[1]
PROGRAM = CRATE/'tests/programs/runtime-performance'
MODES = {'development': [], 'optimize': ['--optimize']}
COMPILERS = ('elm', 'rust')


def arguments(description):
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument('--elm', type=Path, required=True, help='Official elm binary')
    parser.add_argument('--rust', type=Path, default=CRATE/'target/release/planexpo-elm',
                        help='Compiler under test (default: target/release/planexpo-elm)')
    parser.add_argument('--report', type=Path, help='Write the full JSON report to this file')
    return parser


def binaries(args):
    found = {'elm': args.elm.resolve(), 'rust': args.rust.resolve()}
    for name, binary in found.items():
        if not binary.is_file():
            raise SystemExit(f'{name} compiler not found: {binary}')
    return found


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compile_bundles(compilers, directory, program=PROGRAM):
    """Return {mode: {compiler: bundle path}}, each compiler in its own project copy."""
    bundles = {}
    for name in COMPILERS:
        project = Path(directory)/name
        shutil.copytree(program, project, ignore=shutil.ignore_patterns('elm-stuff'))
        for mode, flags in MODES.items():
            bundle = project/f'{mode}.js'
            run = subprocess.run(
                [str(compilers[name]), 'make', 'src/Main.elm', '--output='+str(bundle), *flags],
                cwd=project, env={**os.environ, 'GHCRTS': '-N1 -A16m -c'},
                capture_output=True, text=True, timeout=300)
            if run.returncode != 0 or not bundle.is_file():
                raise SystemExit(f'{name} failed to compile ({mode}):\n{run.stdout}{run.stderr}')
            bundles.setdefault(mode, {})[name] = bundle
    return bundles
