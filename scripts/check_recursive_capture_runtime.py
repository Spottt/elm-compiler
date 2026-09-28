#!/usr/bin/env python3
"""Validate terminating recursive closures against Elm and independent port values.

Covers declaration/name orders, nested helpers and four captured value shapes in
one worker, in development/production and full/incremental/warm Rust builds.
Does not resolve the separately documented historical captured-parameter bug.
"""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:
    p.add_argument('--'+key,type=Path,required=True)
p.add_argument('--terser',type=Path,help='Optional installed Terser module; minify before executing with Rust production options')
a=p.parse_args()
crate=Path(__file__).resolve().parents[1]
binaries={'elm':a.elm.resolve(),'rust':a.rust.resolve()}
hashes={k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in binaries.items()}
captures=[('list','[2,3,5]','List.sum z',10),('string','"hello"','String.length z',5),('tuple','(4,8)','Tuple.first z + Tuple.second z',12),('record','{count=7,label="abc"}','z.count + String.length z.label',10)]
definitions=[]
fixtures=[]
for (kind,value,base,total), names, reverse, helper in itertools.product(captures,[('f','g'),('zulu','alpha')],[False,True],[False,True]):
    index=len(fixtures)
    f,g=names
    read='let read ignored = '+base+' in read ()' if helper else base
    lines=[f'{f} n = if n <= 0 then ({read}) else {g} (n - 1)',f'{g} n = if n <= 0 then ({read}) + 1 else {f} (n - 1)']
    if reverse: lines.reverse()
    body=f'capture{index} z =\n    let\n'+''.join('        '+line+'\n' for line in lines)+f'    in\n    {f} 7\n'
    definitions.append(body)
    fixtures.append({'capture':kind,'names':names,'reversed':reverse,'helper':helper,'expression':f'capture{index} {value}','expected':total+1,'source':body})
expected=[x['expected'] for x in fixtures]
source=(crate/'tests/programs/worker/Main.elm').read_text().replace('value + model','Maybe.withDefault -999 (List.head (List.drop value capturedResults))')+'\n'+'\n'.join(definitions)+'\ncapturedResults = ['+', '.join(x['expression'] for x in fixtures)+']\n'
runner="const assert=require('node:assert/strict');const expected=JSON.parse(process.argv[2]);const values=[];const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});app.ports.outgoing.subscribe(x=>values.push(x));for(let i=0;i<expected.length;i++)app.ports.incoming.send(i);setTimeout(()=>{assert.deepEqual(values,expected);console.log(JSON.stringify(values));},20);"
rows=[]
with tempfile.TemporaryDirectory(prefix='elm-capture-runtime-') as directory:
    root=Path(directory); packages=root/'home/0.19.1/packages';packages.mkdir(parents=True)
    original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat',packages/'registry.dat')
    for package in ['elm/core/1.0.5','elm/json/1.1.3']: shutil.copytree(original/package,packages/package)
    env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1','PLANEXPO_ELM_STATS':''}
    for production in [False,True]:
        for label,compiler,flags in [('elm','elm',[]),('rust','rust',[]),('incremental','rust',['--incremental'])]:
            project=root/f'{production}-{label}';project.mkdir()
            shutil.copy2(crate/'tests/programs/worker/elm.json',project/'elm.json')
            (project/'Main.elm').write_text(source)
            command=[str(binaries[compiler]),'make','Main.elm','--output=result.js','--report=json',*flags,*(['--optimize'] if production else [])]
            for run in range(2 if label=='incremental' else 1):
                result=subprocess.run(command,cwd=project,env=env,capture_output=True,text=True,timeout=60)
                assert result.returncode==0 and result.stderr=='',result.stderr
                output=project/'result.js'
                if a.terser:
                    minimized=project/'result.min.js'
                    minifier="const fs=require('fs');Promise.resolve(require(process.argv[1]).minify(fs.readFileSync(process.argv[2],'utf8'),{compress:{inline:false},mangle:true})).then(r=>{if(r.error)throw r.error;if(typeof r.code!=='string')throw Error('Missing minified code');fs.writeFileSync(process.argv[3],r.code)}).catch(e=>{console.error(e);process.exitCode=1});"
                    subprocess.run(['node','-e',minifier,str(a.terser.resolve()),str(output),str(minimized)],capture_output=True,text=True,timeout=30,check=True)
                    output=minimized
                execution=subprocess.run(['node','-e',runner,str(output),json.dumps(expected)],capture_output=True,text=True,timeout=10,check=True)
                values=json.loads(execution.stdout)
                rows.append({'mode':'production' if production else 'development','compiler':label,'warm':run==1,'values':values})
                print(rows[-1]['mode'],label,run,'PASS',flush=True)
assert hashes=={k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in binaries.items()}
a.report.write_text(json.dumps({'scope':__doc__,'binary_sha256':hashes,'minification':{'module':str(a.terser.resolve()),'compress':{'inline':False},'mangle':True} if a.terser else None,'fixtures':fixtures,'results':rows,'passed':True},indent=2)+'\n')
