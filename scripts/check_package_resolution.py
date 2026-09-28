#!/usr/bin/env python3
"""Compare persisted package selections and manifest invalidation with Elm."""
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
def registry(versions):
    packages=[('author/pkg',versions),('elm/core',[(1,0,5)])]
    data=struct.pack('>qq',sum(len(v) for _,v in packages),len(packages))
    for name, vs in packages:
        for part in name.split('/'):
            encoded=part.encode();data+=bytes([len(encoded)])+encoded
        data+=bytes(vs[0])+struct.pack('>q',len(vs)-1)
        for version in vs[1:]: data+=bytes(version)
    return data

def fake_package(cache,version,value):
    root=cache/'author/pkg'/version
    (root/'src').mkdir(parents=True)
    (root/'elm.json').write_text(json.dumps({'type':'package','name':'author/pkg','summary':'Package selection regression fixture.',
        'license':'BSD-3-Clause','version':version,'exposed-modules':['Value'],'elm-version':'0.19.0 <= v < 0.20.0',
        'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}))
    (root/'src/Value.elm').write_text('module Value exposing (value)\nvalue = '+value+'\n')
results=[]
with tempfile.TemporaryDirectory(prefix='elm-resolution-parity-') as tmp:
    for compiler,binary in [('official',args.elm.resolve()),('rust',crate/'target/release/planexpo-elm')]:
        root=Path(tmp)/compiler
        (root/'src').mkdir(parents=True)
        home=root/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
        core=original_home/'0.19.1/packages/elm/core/1.0.5'
        shutil.copytree(core/'src',cache/'elm/core/1.0.5/src')
        shutil.copyfile(core/'elm.json',cache/'elm/core/1.0.5/elm.json')
        fake_package(cache,'1.0.0','42')
        (cache/'registry.dat').write_bytes(registry([(1,0,0)]))
        (root/'elm.json').write_text(json.dumps({'type':'package','name':'author/project','summary':'Project selection regression fixture.',
            'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],'elm-version':'0.19.0 <= v < 0.20.0',
            'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0','author/pkg':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}))
        (root/'src/Main.elm').write_text('module Main exposing (answer)\nimport Value\nanswer : Int\nanswer = Value.value\n')
        env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1 -A16m -c',
             **{k:'http://127.0.0.1:9' for k in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy']},'NO_PROXY':'','no_proxy':''}
        def run(phase,expected):
            result=subprocess.run([str(binary),'make','src/Main.elm','--output=/dev/null','--report=json'],cwd=root,env=env,capture_output=True,text=True,timeout=60)
            row={'compiler':compiler,'phase':phase,'accepted':result.returncode==0,'expected':expected,'stderr':result.stderr}
            results.append(row)
            assert row['accepted']==expected,row
            if not expected:
                diagnostic=json.loads(result.stderr)
                titles=[problem['title'] for error in diagnostic.get('errors',[]) for problem in error.get('problems',[])]
                assert titles == ['TYPE MISMATCH'], row
        run('initial',True)
        fake_package(cache,'1.1.0','"new version has changed type"')
        (cache/'registry.dat').write_bytes(registry([(1,1,0),(1,0,0)]))
        with (root/'src/Main.elm').open('a') as f:f.write('\n-- Force source recompilation while preserving the manifest\n')
        run('unchanged-manifest-retains-old-version',True)
        with (root/'elm.json').open('a') as f:f.write('\n')
        run('changed-manifest-selects-new-version',False)
report={'passed':True,'cases':len(results),'results':results}
if args.report: args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':True,'cases':len(results)}))
