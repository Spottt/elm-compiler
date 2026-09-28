#!/usr/bin/env python3
"""Compare application registry revalidation triggers with Elm 0.19.1."""
import argparse
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm',type=Path,required=True)
p.add_argument('--report',type=Path)
args=p.parse_args()
crate=Path(__file__).resolve().parents[1]
original_home=Path(os.environ.get('ELM_HOME',str(Path.home()/'.elm')))
results=[]
with tempfile.TemporaryDirectory(prefix='elm-app-resolution-') as tmp:
    for compiler,binary in [('official',args.elm.resolve()),('rust',crate/'target/release/planexpo-elm')]:
        root=Path(tmp)/compiler;(root/'src').mkdir(parents=True)
        home=root/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
        shutil.copyfile(original_home/'0.19.1/packages/registry.dat',cache/'registry.dat')
        for package in ['core/1.0.5','json/1.1.3']:
            origin=original_home/'0.19.1/packages/elm'/package;target=cache/'elm'/package
            shutil.copytree(origin/'src',target/'src');shutil.copyfile(origin/'elm.json',target/'elm.json')
        (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['src'],'elm-version':'0.19.1',
            'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
        (root/'src/Main.elm').write_text('module Main exposing (answer)\nanswer : Int\nanswer = 42\n')
        env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1 -A16m -c',
             **{k:'http://127.0.0.1:9' for k in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy']},'NO_PROXY':'','no_proxy':''}
        def run(phase,expected):
            r=subprocess.run([str(binary),'make','src/Main.elm','--output=/dev/null','--report=json'],cwd=root,env=env,capture_output=True,text=True,timeout=60)
            row={'compiler':compiler,'phase':phase,'accepted':r.returncode==0,'expected':expected,'stderr':r.stderr};results.append(row)
            assert row['accepted']==expected,row
            if not expected: assert json.loads(r.stderr)['type']=='error',row
        run('initial-offline-validation',True)
        # Both compilers must retain the already-verified project despite this
        # changed shared registry. Source-only recompilation must not re-solve.
        (cache/'registry.dat').write_bytes(struct.pack('>qq',0,0))
        with (root/'src/Main.elm').open('a') as f:f.write('\n-- Source-only recompilation\n')
        run('unchanged-manifest-reuses-validation',True)
        with (root/'elm.json').open('a') as f:f.write('\n')
        run('changed-manifest-revalidates-registry',False)
report={'passed':True,'cases':len(results),'results':results}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':True,'cases':len(results)}))
