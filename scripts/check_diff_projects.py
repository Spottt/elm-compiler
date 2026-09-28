#!/usr/bin/env python3
"""Compare diff project/package errors against Elm 0.19.1 (online registry).

Each case gets an isolated cache; compares stdout, stderr and exit code.
With --ansi also compares exact terminal escape sequences on stderr.
Does not cover offline transport errors.
"""
import argparse, copy, hashlib, json, os, shutil, tempfile
from diff_test_process import run_command
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ['elm','rust','report']: p.add_argument('--'+name,type=Path,required=True)
p.add_argument('--ansi',action='store_true')
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
package={'type':'package','name':'elm/core','summary':'Isolated package comparison fixture','license':'BSD-3-Clause','version':'1.0.5','exposed-modules':[],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{},'test-dependencies':{}}
application={'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
unpublished=copy.deepcopy(package);unpublished['name']='codex-diff-fixture/unpublished-fixture'
grouped=copy.deepcopy(package);grouped['exposed-modules']={'Empty':[]}
cases=[('no-outline',None,[]),('no-outline-exact',None,['1.0.0']),('application',application,[]),('application-versions',application,['1.0.0','2.0.0']),('unpublished',unpublished,[]),('unknown-package',None,['elm/corre','1.0.0','2.0.0']),('unknown-version',None,['elm/http','9.0.0','9.0.0']),('unknown-versions-reversed',None,['elm/core','9.0.0','8.0.0']),('local-unknown-version',package,['9.0.0']),('no-exposed',package,[]),('no-exposed-grouped',grouped,[])]
results=[]
with tempfile.TemporaryDirectory(prefix='elm-diff-projects-') as temp:
 root=Path(temp)
 for label,config,args in cases:
  project=root/label;(project/'src').mkdir(parents=True)
  if config is not None: (project/'elm.json').write_text(json.dumps(config))
  home=project/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
  original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
  shutil.copy2(original/'registry.dat',cache/'registry.dat')
  docs=cache/'elm/core/1.0.5/docs.json';docs.parent.mkdir(parents=True);docs.write_text('[]')
  env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'};runs=[]
  for binary in [a.elm,a.rust]:
   runs.append(run_command([str(binary.resolve()),'diff',*args],project,env,a.ansi))
  passed=runs[0]==runs[1] and runs[0]['code']==1
  results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1]});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha,'Compiler changed during validation'
r={'scope':__doc__,'ansi':a.ansi,'compiler_sha256':sha,'cases':len(results),'passed':all(x['passed'] for x in results),'results':results}
a.report.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit(not r['passed'])
