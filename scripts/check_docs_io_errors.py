#!/usr/bin/env python3
"""Compare documentation output failures, exact streams and repaired artifacts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
from diff_test_process import run_command

p = argparse.ArgumentParser(description=__doc__)
for name in ['elm', 'rust', 'report']:
    p.add_argument('--'+name, type=Path, required=True)
a = p.parse_args()
binaries = [a.elm.resolve(), a.rust.resolve()]
hashes = [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
source = 'module Main exposing (value)\n{-| @docs value -}\n{-| Value. -}\nvalue : Int\nvalue = 1\n'
config = {'type':'package','name':'author/fixture','summary':'Documentation IO fixture.',
          'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
          'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
rows = []
with tempfile.TemporaryDirectory(prefix='elm-doc-io-') as tmp:
    root = Path(tmp)
    packages = root/'home/0.19.1/packages'
    packages.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', packages/'registry.dat')
    shutil.copytree(original/'elm/core/1.0.5', packages/'elm/core/1.0.5')
    env = {**os.environ, 'ELM_HOME':str(root/'home'), 'GHCRTS':'-N1', 'PLANEXPO_ELM_STATS':''}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    env.update(no_proxy='', NO_PROXY='')
    for kind in ['directory','readonly-file','missing-parent']:
        for mode in ['plain','json','ansi','ansi-stdout']:
            runs = []
            for i, binary in enumerate(binaries):
                project = root/f'{kind}-{mode}-{i}'
                (project/'src').mkdir(parents=True)
                (project/'elm.json').write_text(json.dumps(config))
                (project/'src/Main.elm').write_text(source)
                command = [str(binary),'make']
                warm = run_command(command+['--docs=good.json','--report=json'],project,env)
                assert warm == {'code':0,'stdout':'','stderr':''}, warm
                good = (project/'good.json').read_bytes()
                (project/'src/Main.elm').write_text(source.replace('Value.', 'Updated value.'))
                output = 'missing/docs.json' if kind == 'missing-parent' else 'docs.json'
                sentinel = project/'keep'
                if kind == 'directory':
                    (project/output).mkdir()
                    sentinel = project/output/'keep'
                elif kind == 'readonly-file':
                    sentinel = project/output
                sentinel.write_bytes(b'preserved')
                if kind == 'readonly-file': sentinel.chmod(0o444)
                try:
                    run = run_command(command+['--docs='+output]+(['--report=json'] if mode=='json' else []),project,env,mode.startswith('ansi'),'stdout' if mode=='ansi-stdout' else 'stderr')
                    assert run['code'] == 1, run
                    assert sentinel.read_bytes() == b'preserved'
                    assert (project/'good.json').read_bytes() == good
                finally:
                    if kind == 'readonly-file': sentinel.chmod(0o644)
                if kind == 'directory': shutil.rmtree(project/output)
                if kind == 'missing-parent': (project/'missing').mkdir()
                recovery = run_command(command+['--docs='+output,'--report=json'],project,env)
                assert recovery == {'code':0,'stdout':'','stderr':''}, recovery
                assert (project/'good.json').read_bytes() == good
                artifact = json.loads((project/output).read_text())
                assert artifact[0]['values'][0]['comment'].strip() == 'Updated value.', artifact
                run['recovery'] = {'streams':recovery,'documentation':artifact}
                runs.append(run)
            passed = runs[0] == runs[1]
            rows.append({'case':kind,'mode':mode,'passed':passed,'official':runs[0],'rust':runs[1]})
            print(kind,mode,'PASS' if passed else 'FAIL',flush=True)
assert hashes == [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'scope':__doc__,'binary_sha256':hashes,'passed':all(r['passed'] for r in rows),'cases':rows},indent=2)+'\n')
raise SystemExit(not all(r['passed'] for r in rows))
