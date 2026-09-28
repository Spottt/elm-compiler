#!/usr/bin/env python3
"""Compare output write failures after valid compilation and error precedence.

Runs only in disposable projects; protects sentinel files and the previous good
bundle. Repeats each failed build, repairs each destination, retries the same
output and executes the recovered worker in Node. The incremental variant also
verifies an output-cache hit after recovery. Warm package verification isolates
the compilation/output streams. Structured
JSON diagnostics are compared as objects; plain and ANSI documents are exact.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from diff_test_process import run_command

p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']:
    p.add_argument('--'+key, type=Path, required=True)
p.add_argument('--incremental', action='store_true', help='Exercise Rust incremental caches, including repeated failures and recovery')
a = p.parse_args()
binaries = [a.elm.resolve(), a.rust.resolve()]
hashes = [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
crate = Path(__file__).resolve().parents[1]
rows = []
cache_probes = []
with tempfile.TemporaryDirectory(prefix='elm-output-io-') as directory:
    root = Path(directory)
    packages = root/'home/0.19.1/packages'
    packages.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', packages/'registry.dat')
    for name in ['elm/core/1.0.5','elm/json/1.1.3']:
        shutil.copytree(original/name, packages/name)
    env = {**os.environ, 'ELM_HOME':str(root/'home'), 'GHCRTS':'-N1', 'PLANEXPO_ELM_STATS':''}
    for kind in ['directory','parent-file','readonly-file','readonly-parent','deep-parent-file','deep-readonly-parent','source-before-output']:
        for mode in ['plain','json','ansi','ansi-stdout']:
            runs = []
            for i, binary in enumerate(binaries):
                project = root/f'{kind}-{mode}-{i}'
                project.mkdir()
                command = [str(binary),'make','Main.elm'] + (['--incremental'] if i == 1 and a.incremental else [])
                source = (crate/'tests/programs/worker/Main.elm').read_text()
                shutil.copyfile(crate/'tests/programs/worker/elm.json', project/'elm.json')
                (project/'Main.elm').write_text(source)
                warm = run_command(command+['--output=good.js','--report=json'],project,env)
                assert warm['code'] == 0, warm
                good = (project/'good.js').read_bytes()
                source += '\n-- force output compilation\n'
                if kind == 'source-before-output': source += 'bad : Int\nbad = "wrong"\n'
                (project/'Main.elm').write_text(source)
                output = 'out.js'
                if kind in ['directory','source-before-output']:
                    (project/output).mkdir()
                    sentinel = project/output/'keep'
                elif kind in ['readonly-parent','deep-readonly-parent']:
                    (project/'parent').mkdir()
                    sentinel = project/'parent/keep'
                    output = 'parent/nested/out.js' if kind == 'readonly-parent' else 'parent/middle/nested/out.js'
                elif kind in ['parent-file','deep-parent-file']:
                    sentinel = project/'parent'
                    output = 'parent/out.js' if kind == 'parent-file' else 'parent/middle/nested/out.js'
                else:
                    sentinel = project/output
                sentinel.write_bytes(b'preserved')
                if kind == 'readonly-file': sentinel.chmod(0o444)
                if kind in ['readonly-parent','deep-readonly-parent']: (project/'parent').chmod(0o555)
                try:
                    run = run_command(command+['--output='+output]+(['--report=json'] if mode=='json' else []),project,env,mode.startswith('ansi'),'stdout' if mode=='ansi-stdout' else 'stderr')
                    repeated = run_command(command+['--output='+output]+(['--report=json'] if mode=='json' else []),project,env,mode.startswith('ansi'),'stdout' if mode=='ansi-stdout' else 'stderr')
                    assert repeated['code'] == 1, repeated
                    assert sentinel.read_bytes() == b'preserved', 'Destination content changed'
                    assert (project/'good.js').read_bytes() == good, 'Previous bundle changed'
                finally:
                    if kind == 'readonly-file': sentinel.chmod(0o644)
                    if kind in ['readonly-parent','deep-readonly-parent']: (project/'parent').chmod(0o755)
                run = {k:v.replace(str(project),'<PROJECT>') if isinstance(v,str) else v for k,v in run.items()}
                if mode == 'json' and kind == 'source-before-output':
                    run['stderr'] = json.loads(run['stderr'])
                repeated = {k:v.replace(str(project),'<PROJECT>') if isinstance(v,str) else v for k,v in repeated.items()}
                if mode == 'json' and kind == 'source-before-output':
                    repeated['stderr'] = json.loads(repeated['stderr'])
                run['repeated'] = repeated
                # Repair only owned fixture paths, then retry the same output.
                if kind in ['directory','source-before-output']:
                    shutil.rmtree(project/output)
                elif kind in ['parent-file','deep-parent-file']:
                    sentinel.unlink()
                if kind == 'source-before-output':
                    (project/'Main.elm').write_text((crate/'tests/programs/worker/Main.elm').read_text()+'\n-- repaired source\n')
                recovered = run_command(command+['--output='+output,'--report=json'],project,env)
                assert recovered == {'code':0,'stdout':'','stderr':''}, recovered
                assert (project/'good.js').read_bytes() == good, 'Recovery changed previous bundle'
                runner = "const assert=require('node:assert/strict');const values=[];const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});app.ports.outgoing.subscribe(x=>values.push(x));app.ports.incoming.send(2);setTimeout(()=>{assert.deepEqual(values,[7]);console.log(JSON.stringify(values));},20);"
                executed = subprocess.run(['node','-e',runner,str(project/output)],capture_output=True,text=True,timeout=10,check=True)
                run['recovery'] = {**recovered,'values':json.loads(executed.stdout)}
                if i == 1 and a.incremental:
                    probe = run_command(command+['--output='+output],project,{**env,'PLANEXPO_ELM_STATS':'1'})
                    assert probe['code'] == 0 and probe['stderr'] == '', probe
                    assert 'Output cache: (1 cached).' in probe['stdout'], probe
                    assert (project/'good.js').read_bytes() == good
                    cache_probes.append({'case':kind,'mode':mode,'output_cache_hits':1})
                runs.append(run)
            row = {'case':kind,'mode':mode,'passed':runs[0]==runs[1] and runs[0]['code']==1,'official':runs[0],'rust':runs[1]}
            rows.append(row)
            print(kind,mode,'PASS' if row['passed'] else 'FAIL',flush=True)
assert hashes == [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'scope':__doc__,'binary_sha256':hashes,'incremental':a.incremental,'cache_probes':cache_probes,'passed':all(r['passed'] for r in rows),'cases':rows},indent=2)+'\n')
raise SystemExit(not all(r['passed'] for r in rows))
