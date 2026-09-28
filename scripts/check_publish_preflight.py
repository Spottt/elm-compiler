#!/usr/bin/env python3
"""Compare pre-build publish rejection categories with Elm 0.19.1.

This compares decision priority and complete stderr diagnostics. Every fixture fails before
building; a fake git also rejects every command before any registration is possible.
"""
import argparse, copy, json, os, shutil, tempfile
from diff_test_process import run_command
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','probe','report']: p.add_argument('--'+key,type=Path,required=True)
p.add_argument('--ansi',action='store_true')
p.add_argument('--terminal-stream',choices=['stdout','stderr'],default='stderr')
p.add_argument('--public',action='store_true')
a=p.parse_args()
base={'type':'package','name':'elm/core','summary':'Preflight fixture','license':'BSD-3-Clause','version':'1.0.5','exposed-modules':['Example'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{},'test-dependencies':{}}
cases=[('application',{'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}},None)]
for name,change,readme in [
 ('empty-exposed',{'exposed-modules':[],'summary':''},None),
 ('empty-groups',{'exposed-modules':{'Empty':[]}},None),
 ('empty-summary',{'summary':''},None),
 ('default-summary',{'summary':'helpful summary of your project, less than 80 characters'},None),
 ('near-default-summary',{'summary':'A helpful summary of your project, less than 80 characters'},None),
 ('spaces-summary',{'summary':' '},None),
 ('grouped-modules',{'exposed-modules':{'Empty':[],'API':['Example']}},None),
 ('missing-readme',{},None), ('short-readme',{},'x'*299),
 ('unicode-readme',{},'é'*150), ('boundary-readme',{},'x'*300),
]:
 config=copy.deepcopy(base);config.update(change);cases.append((name,config,readme))
results=[]
with tempfile.TemporaryDirectory(prefix='publish-preflight-') as temp:
 root=Path(temp);guard=root/'bin';guard.mkdir();git=guard/'git';git.write_text('#!/bin/sh\nexit 1\n');git.chmod(0o755)
 home=root/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
 source=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages/registry.dat'
 shutil.copy2(source,cache/'registry.dat')
 env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1','PATH':str(guard)+os.pathsep+os.environ['PATH']}
 for name,config,readme in cases:
  project=root/name;project.mkdir();(project/'src').mkdir();(project/'elm.json').write_text(json.dumps(config))
  if readme is not None:(project/'README.md').write_text(readme)
  official=run_command([str(a.elm.resolve()),'publish'],project,env,a.ansi,a.terminal_stream,'')
  probe=run_command([str(a.probe.resolve()),*(['publish'] if a.public else [])],project,env,a.ansi,a.terminal_stream,'')
  category=probe['stdout'].strip();passed=probe['code']==0 and official['code']==1 and ('-- '+category+' ') in official['stderr'] and probe['stderr'] == official['stderr']
  if a.public:passed=official==probe;category='public command'
  results.append({'case':name,'passed':passed,'category':category,'rust_stderr':probe['stderr'],'official':{'code':official['code'],'stdout':official['stdout'],'stderr':official['stderr']}})
  print(name,'PASS' if passed else 'FAIL',flush=True)
report={'scope':__doc__,'ansi':a.ansi,'terminal_stream':a.terminal_stream,'public_command':a.public,'passed':all(r['passed'] for r in results),'cases':len(results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
