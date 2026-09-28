#!/usr/bin/env python3
"""Compare real elm diff local-package invocation modes (network registry refresh)."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--elm',type=Path,required=True);p.add_argument('--rust',type=Path,required=True);p.add_argument('--report',type=Path,required=True);a=p.parse_args()
compiler_sha256=hashlib.sha256(a.rust.read_bytes()).hexdigest()
def value(name):return {'name':name,'comment':'','type':'a -> a'}
def docs(names):return [{'name':'Example','comment':'','unions':[],'aliases':[],'binops':[],'values':[value(n) for n in names]}]
results=[]
with tempfile.TemporaryDirectory(prefix='elm-diff-local-') as tmp:
 root=Path(tmp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages';shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for version,names in [('1.0.4',['identity']),('1.0.5',['identity','other'])]:
  path=cache/'elm/core'/version;path.mkdir(parents=True);(path/'docs.json').write_text(json.dumps(docs(names)))
 project=root/'project';(project/'src').mkdir(parents=True)
 (project/'elm.json').write_text(json.dumps({'type':'package','name':'elm/core','summary':'Isolated public API comparison fixture','license':'BSD-3-Clause','version':'1.0.5','exposed-modules':['Example'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{},'test-dependencies':{}}))
 (project/'src/Example.elm').write_text('module Example exposing (identity, local)\n{-| Example\n@docs identity, local\n-}\n{-| Identity -}\nidentity : a -> a\nidentity x = x\n{-| Local addition -}\nlocal : a -> a\nlocal x = x\n')
 env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
 for label,args,cwd in [('latest',[],project),('exact',['1.0.4'],project),('local-versions',['1.0.5','1.0.4'],project),('subdirectory',[],project/'src'),('global-reversed',['elm/core','1.0.5','1.0.4'],root)]:
  runs=[]
  for binary in [a.elm,a.rust]:
   run=subprocess.run([str(binary.resolve()),'diff',*args],cwd=cwd,env=env,capture_output=True,text=True,timeout=60)
   runs.append({'code':run.returncode,'stdout':run.stdout,'stderr':run.stderr})
  results.append({'case':label,'passed':runs[0]==runs[1] and runs[0]['code']==0,'official':runs[0],'rust':runs[1]})
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==compiler_sha256, 'Compiler changed during validation'
r={'scope':__doc__,'cases':len(results),'passed':all(x['passed'] for x in results),'compiler_sha256':compiler_sha256,'results':results};a.report.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'cases':len(results),'failures':[x['case'] for x in results if not x['passed']]}));raise SystemExit(not r['passed'])
