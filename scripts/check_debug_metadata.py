#!/usr/bin/env python3
"""Compare type extraction with metadata embedded by the official --debug compiler.

This tests extraction only, not Rust debugger integration or browser behavior.
All projects and package caches are isolated from developer builds.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', required=True, type=Path)
parser.add_argument('--probe', required=True, type=Path)
parser.add_argument('--report', required=True, type=Path)
args = parser.parse_args()
binaries = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in (args.elm, args.probe)}
cases = {
    'recursive': 'type alias Payload a = { value : a }\ntype Chain a = End | Link a (Chain a)\ntype Msg = Got (Payload (Chain ())) | Again Msg\ntype Unused = Unused',
    'hidden-and-phantom': 'type Hidden = Hidden\ntype alias Payload a = { hidden : Hidden, value : a }\ntype Phantom original = Phantom\ntype Msg = Got (Payload ()) (Phantom ())',
    'list-long-record': 'type Msg = Got (List ((), (), ())) { aVeryLongFieldNameThatUsesSpace : (), anotherLongFieldNameThatUsesSpace : (), lastLongFieldNameThatUsesSpace : () }',
    'function-precedence': 'type Msg = Got ((() -> ()) -> ()) (() -> () -> ())',
    'mutual-recursion': 'type Left a = Left a (Right a)\ntype Right b = Right (Left b)\ntype Msg = Got (Left ())',
    'nested-aliases': 'type alias Inner a = { value : a }\ntype alias Outer b = Inner (List b)\ntype Msg = Got (Outer ())',
    'parameter-order': 'type Pair z a = Pair a z\ntype Msg = Got (Pair () (List ()))',
    'record-order': 'type Msg = Got { z : (), a : () }',
}
results = []
with tempfile.TemporaryDirectory(prefix='elm-debug-metadata-') as folder:
    root = Path(folder)
    home = root/'home'
    cache = home/'0.19.1/packages'
    cache.mkdir(parents=True)
    source_cache = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm')))/'0.19.1/packages'
    shutil.copy2(source_cache/'registry.dat', cache/'registry.dat')
    for package, version in [('core', '1.0.5'), ('json', '1.1.3')]:
        shutil.copytree(source_cache/'elm'/package/version, cache/'elm'/package/version)
    env = dict(os.environ, ELM_HOME=str(home))
    for label, declarations in cases.items():
        project = root/label
        project.mkdir()
        (project/'elm.json').write_text(json.dumps({
            'type': 'application', 'source-directories': ['.'], 'elm-version': '0.19.1',
            'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}},
            'test-dependencies': {'direct': {}, 'indirect': {}}
        }))
        source = project/'declarations.elm'
        source.write_text(declarations)
        (project/'Main.elm').write_text('module Main exposing (main)\n'+declarations+'''\nmain : Program () () Msg
main = Platform.worker { init = \\_ -> ((), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }
''')
        official = subprocess.run([str(args.elm), 'make', 'Main.elm', '--debug', '--output=main.js'], cwd=project, env=env, capture_output=True, text=True, timeout=120)
        if official.returncode:
            raise RuntimeError(official.stdout+official.stderr)
        js = (project/'main.js').read_text()
        start = js.rfind('{"versions"')
        if start < 0:
            raise RuntimeError('official debugger metadata not found')
        expected, _ = json.JSONDecoder().raw_decode(js[start:])
        probe = subprocess.run([str(args.probe), str(source)], capture_output=True, text=True, timeout=30, check=True)
        actual = json.loads(probe.stdout)
        results.append({'case': label, 'match': actual == expected, 'expected': expected, 'actual': actual})
assert binaries == {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in (args.elm, args.probe)}, 'binary changed during validation'
args.report.write_text(json.dumps({'binaries': binaries, 'scope': 'metadata extraction only', 'cases': results}, indent=2)+'\n')
for result in results:
    print(result['case'], 'PASS' if result['match'] else 'FAIL')
raise SystemExit(0 if all(r['match'] for r in results) else 1)
