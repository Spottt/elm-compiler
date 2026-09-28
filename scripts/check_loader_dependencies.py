#!/usr/bin/env python3
"""Verify the private loader sidecar on cold/warm builds and errors without changing CLI output."""
import argparse, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--rust', type=Path, required=True)
p.add_argument('--report', type=Path, required=True)
a = p.parse_args()
binary = a.rust.resolve()
identity = hashlib.sha256(binary.read_bytes()).hexdigest()
home = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))
manifest = {'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
rows = []
with tempfile.TemporaryDirectory(prefix='elm-loader-dependencies-') as temporary:
    root = Path(temporary)
    packages = root/'home/0.19.1/packages'
    packages.mkdir(parents=True)
    shutil.copy2(home/'0.19.1/packages/registry.dat', packages/'registry.dat')
    for package in ['elm/core/1.0.5','elm/json/1.1.3']:
        shutil.copytree(home/'0.19.1/packages'/package, packages/package)
    env = dict(os.environ, ELM_HOME=str(root/'home'))
    env.pop('PLANEXPO_ELM_DEPENDENCIES_FILE', None)
    (root/'src').mkdir()
    (root/'elm.json').write_text(json.dumps(manifest))
    (root/'src/Helper.elm').write_text('module Helper exposing (value)\nvalue : Int\nvalue = 1\n')
    (root/'src/Main.elm').write_text('module Main exposing (main)\nimport Helper\nmain : Program () Int Never\nmain = Platform.worker { init = \\() -> (Helper.value, Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }\n')
    command = [str(binary),'make','src/Main.elm','--incremental','--report=json','--output=out.js']
    sidecar = root/'dependencies.json'
    for name in ['cold', 'warm', 'type-error', 'cached-error', 'repair', 'unwritable-sidecar']:
        if name == 'type-error': (root/'src/Helper.elm').write_text('module Helper exposing (value)\nvalue : Int\nvalue = "wrong"\n')
        if name == 'repair': (root/'src/Helper.elm').write_text('module Helper exposing (value)\nvalue : Int\nvalue = 2\n')
        sidecar.unlink(missing_ok=True)
        target = root if name == 'unwritable-sidecar' else sidecar
        result = subprocess.run(command, cwd=root, env=dict(env,PLANEXPO_ELM_DEPENDENCIES_FILE=str(target)), capture_output=True, timeout=60)
        expected_exit = 1 if 'error' in name else 0
        assert result.returncode == expected_exit, (name,result.stderr)
        if name != 'unwritable-sidecar':
            report = json.loads(sidecar.read_text())
            assert report['version'] == 1
            files = set(report['files'])
            assert all(Path(file).is_absolute() for file in files)
            assert {str(root/'elm.json'),str(root/'src/Main.elm'),str(root/'src/Helper.elm')} <= files
            assert any(file.endswith('/Basics.elm') for file in files)
        output = (root/'out.js').read_bytes() if expected_exit == 0 else None
        reference = subprocess.run(command, cwd=root, env=env, capture_output=True, timeout=60)
        assert (result.returncode,result.stdout,result.stderr) == (reference.returncode,reference.stdout,reference.stderr), name
        if output is not None: assert output == (root/'out.js').read_bytes()
        rows.append({'case':name,'exit':result.returncode,'unchanged_output':True})
assert hashlib.sha256(binary.read_bytes()).hexdigest() == identity
a.report.parent.mkdir(parents=True,exist_ok=True)
a.report.write_text(json.dumps({'compiler_sha256':identity,'rows':rows},indent=2)+'\n')
print(f'{len(rows)} dependency/CLI checks passed')
