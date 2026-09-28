#!/usr/bin/env python3
"""Verify real cache hits, transitive invalidation and runtime output in isolation."""
import os
os.environ["PLANEXPO_ELM_STATS"] = "1"
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
binary = crate/'target/release/planexpo-elm'
home = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm')))
runner = r'''
const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});
app.ports.outgoing.subscribe(value=>console.log(JSON.stringify(value)));
app.ports.incoming.send(1);
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-cache-runtime-') as directory:
    root = Path(directory)
    project = root/'project'
    shutil.copytree(crate/'tests/programs/worker', project)
    local_home = root/'elm-home'
    for package, version in [('elm/core', '1.0.5'), ('elm/json', '1.1.3')]:
        relative = Path('0.19.1/packages')/package/version
        shutil.copytree(home/relative, local_home/relative)
    # A fresh project validates against the registry, including offline builds.
    shutil.copyfile(home/'0.19.1/packages/registry.dat', local_home/'0.19.1/packages/registry.dat')
    compiler = root/'compiler'
    shutil.copy2(binary, compiler)
    env = {**os.environ, 'ELM_HOME': str(local_home)}
    source = project/'Main.elm'
    source.write_text(source.read_text().replace('import Platform', 'import Platform\nimport Offset').replace('value + model', 'value + model + Offset.value'))
    dependency = project/'Offset.elm'
    dependency.write_text('module Offset exposing (value)\nvalue = 1\n')
    output = project/'result.js'
    cache_directory = project/'elm-stuff/planexpo-rust/v1'

    def compile(expected_cached, expected_value, *extra):
        result = subprocess.run([str(compiler), 'make', 'Main.elm', '--output='+str(output), *extra], cwd=project, env=env, capture_output=True, text=True, timeout=60)
        assert result.returncode == 0, result.stderr
        assert f'({int(expected_cached)} cached)' in result.stdout, result.stdout
        run = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, check=True, timeout=10)
        assert json.loads(run.stdout) == expected_value, run.stdout
        return output.read_bytes()

    first = compile(False, 7)
    output.unlink()
    assert compile(True, 7) == first  # A missing output is restored from cache.
    stat = dependency.stat()
    dependency.write_text(dependency.read_text().replace('= 1', '= 2'))
    os.utime(dependency, ns=(stat.st_atime_ns, stat.st_mtime_ns))
    second = compile(False, 8)  # Same length and mtime, different dependency bytes.
    assert second != first
    assert compile(True, 8) == second
    assert compile(False, 8, '--no-cache') == second
    for path in [project/'elm.json', local_home/'0.19.1/packages/elm/core/1.0.5/elm.json']:
        path.write_text(path.read_text()+'\n')
        assert compile(False, 8) == second
    kernel = local_home/'0.19.1/packages/elm/core/1.0.5/src/Elm/Kernel/Utils.js'
    kernel.write_text(kernel.read_text()+'\n// cache invalidation fixture\n')
    third = compile(False, 8)
    assert third != second
    assert compile(True, 8) == third
    # Trailing bytes are harmless to the executable, but change its identity.
    with compiler.open('ab') as stream:
        stream.write(b'\0cache-test')
    assert compile(False, 8) == third
    cache_file, = cache_directory.glob('*.cache')
    data = bytearray(cache_file.read_bytes())
    data[-1] ^= 1
    cache_file.write_bytes(data)
    assert compile(False, 8) == third
    assert compile(True, 8) == third
    cache_file.unlink()
    cache_directory.rmdir()
    cache_directory.write_text('cache cannot be a directory')
    assert compile(False, 8) == third  # Cache I/O failure is nonfatal.
    source.write_text('module Main exposing (..)\nbroken = unknown\n')
    failed = subprocess.run([str(compiler), 'make', 'Main.elm', '--output='+str(output), '--report=json'], cwd=project, env=env, capture_output=True, text=True, timeout=60)
    assert failed.returncode != 0
    diagnostic = json.loads(failed.stderr)
    assert diagnostic['type'] == 'compile-errors', diagnostic
    problem = diagnostic['errors'][0]['problems'][0]
    assert problem['title'] == 'NAMING ERROR', problem
    assert problem['region'] == {'start':{'line':2,'column':10}, 'end':{'line':2,'column':17}}, problem
    assert output.read_bytes() == third
    print('PASS: output restoration, transitive same-mtime edits, manifests, kernels, compiler identity, corruption, bypass, cache I/O failure and invalid sources')
