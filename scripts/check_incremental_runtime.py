#!/usr/bin/env python3
"""Compare incremental type reuse to a full compile after edits and corruption."""
import os
os.environ["PLANEXPO_ELM_STATS"] = "1"
import json
import hashlib
import argparse
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

crate=Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--compiler', type=Path, default=crate/'target/release/planexpo-elm')
parser.add_argument('--implicit-incremental', action='store_true', help='verify a wrapper that enables incremental make itself')
parser.add_argument('--development', action='store_true')
args = parser.parse_args()
mode_flags=[] if args.development else ['--optimize']
binary=args.compiler.resolve()
incremental_flags=[] if args.implicit_incremental else ['--incremental']
runner="const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});app.ports.outgoing.subscribe(x=>console.log(JSON.stringify(x)));app.ports.incoming.send(1);"
with tempfile.TemporaryDirectory(prefix='elm-incremental-runtime-') as directory:
    project=Path(directory)
    shutil.copytree(crate/'tests/programs/worker',project,dirs_exist_ok=True)
    source=project/'Main.elm'
    source.write_text(source.read_text().replace('import Platform','import Platform\nimport Offset').replace('value + model','value + model + Offset.value'))
    dependency=project/'Offset.elm';dependency.write_text('module Offset exposing (value)\nvalue = 1\n')
    output=project/'result.js'
    def compile(expected, minimum_hits=0):
        # Force code generation; this suite exercises the typed-module cache.
        for file in (project/'elm-stuff/planexpo-rust/v1').glob('*.cache'):file.unlink()
        result=subprocess.run([str(binary),'make','Main.elm',*mode_flags,*incremental_flags,'--output='+str(output)],cwd=project,capture_output=True,text=True,timeout=60)
        assert result.returncode==0,result.stderr
        hits=int(re.search(r'Reused types for (\d+)',result.stdout)[1])
        assert hits>=minimum_hits,result.stderr
        run=subprocess.run(['node','-e',runner,str(output)],capture_output=True,text=True,check=True,timeout=10)
        assert json.loads(run.stdout)==expected,run.stdout
        full=project/'full.js'
        subprocess.run([str(binary),'make','Main.elm',*mode_flags,'--no-cache','--output='+str(full)],cwd=project,capture_output=True,text=True,check=True,timeout=60)
        assert full.read_bytes()==output.read_bytes(),'incremental output differs from full compile'
        return hits
    assert compile(7)==0
    warm=compile(7,1)
    source.write_text(source.read_text()+'\n-- root edit\n')
    assert compile(7,1)==warm-1
    dependency.write_text('module Offset exposing (value)\nvalue = 2\n')
    assert compile(8,1)==warm-1, 'implementation edit unnecessarily invalidated dependents'
    source.write_text(source.read_text().replace('exposing (main)', 'exposing (booleanChecks)'))
    assert compile(8,1)==warm-1
    assert compile(8,1)==warm, 'cache failed to retain unexposed main and ports'
    # Missing/malformed optional interface hashes must fall back to recomputing
    # the interface without discarding an otherwise valid inferred-type graph.
    magic = b'PLANEXPO-ELM-CACHE-1\n'
    for fingerprint in [None, ['invalid']]:
        for file in (project/'elm-stuff/planexpo-rust/types-v1').glob('*.cache'):
            data = file.read_bytes()
            assert data.startswith(magic)
            artifact = json.loads(data[len(magic)+64:])
            artifact['interface_fingerprint'] = fingerprint
            payload = json.dumps(artifact).encode()
            file.write_bytes(data[:len(magic)+32] + hashlib.sha256(payload).digest() + payload)
        assert compile(8,1)==warm, 'optional fingerprint fallback lost valid cached types'
    profile = subprocess.run([str(binary), 'profile', str(project/'elm.json'), 'Main.elm', '--incremental', *(['--development'] if args.development else [])], cwd=project, capture_output=True, text=True, check=True, timeout=60)
    profile = json.loads(profile.stdout)
    assert profile['cached_type_modules']==warm
    assert profile['javascript_bytes']==output.stat().st_size
    assert all('interface_ms' in module for module in profile['modules'])
    if args.development:
        assert profile['cached_generated_modules'] > 0, profile
        for file in (project/'elm-stuff/planexpo-rust/types-v1/generated-v1').glob('*.cache'):
            data=bytearray(file.read_bytes());data[-1]^=1;file.write_bytes(data)
        assert compile(8,1)==warm, 'corrupt generated code must rebuild without losing valid type artifacts'
    for file in (project/'elm-stuff/planexpo-rust/types-v1').glob('*.cache'):
        data=bytearray(file.read_bytes());data[-1]^=1;file.write_bytes(data)
    assert compile(8)==0
    before=output.read_bytes()
    dependency.write_text('module Offset exposing (value)\nvalue = []\n')
    failed=subprocess.run([str(binary),'make','Main.elm',*mode_flags,*incremental_flags,'--output='+str(output)],cwd=project,capture_output=True,text=True,timeout=60)
    assert failed.returncode!=0,failed.stderr
    assert output.read_bytes()==before
    print(f'PASS: {warm} reusable modules; edits and corrupted artifacts match full compilation; invalid dependent types rejected')
