#!/usr/bin/env python3
"""Compare publication build and version validation with Elm 0.19.1.

Valid versions must reach NO TAG in both Elm and the internal Rust probe. All rejection diagnostics are compared byte for byte. A fake Git
always fails before GitHub requests or package registration.
"""
import argparse,copy,hashlib,json,os,shutil,tempfile
from pathlib import Path
from diff_test_process import run_command
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','probe','report']:p.add_argument('--'+key,type=Path,required=True)
p.add_argument('--ansi',action='store_true')
p.add_argument('--terminal-stream',choices=['stdout','stderr'],default='stderr')
p.add_argument('--public',action='store_true')
a=p.parse_args();sha=hashlib.sha256(a.probe.read_bytes()).hexdigest()
base={'type':'package','name':'elm/core','summary':'Publication version fixture','license':'BSD-3-Clause','version':'1.0.6','exposed-modules':['Example'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{},'test-dependencies':{}}
def docs(values):return [{'name':'Example','comment':'','unions':[],'aliases':[],'binops':[],'values':[{'name':name,'comment':'','type':tipe} for name,tipe in values]}]
old_types=[('patch',docs([('identity','a -> a')])),('minor',docs([])),('major',docs([('identity','Int')]))]
cases=[]
for actual,old in old_types:
 for claimed,version in [('patch','1.0.6'),('minor','1.1.0'),('major','2.0.0')]:
  config=copy.deepcopy(base);config['version']=version;cases.append((actual+'-as-'+claimed,config,old,actual==claimed))
for label,version in [('already-published','1.0.5'),('skipped-version','1.0.8')]:
 config=copy.deepcopy(base);config['version']=version;cases.append((label,config,old_types[0][1],False))
for version in ['1.0.0','1.2.0']:
 config=copy.deepcopy(base);config.update(name='codex-publish-fixture/unpublished-package',version=version,dependencies={'elm/core':'1.0.0 <= v < 2.0.0'});cases.append(('initial-'+version,config,old_types[0][1],version=='1.0.0'))
config=copy.deepcopy(base);cases.append(('missing-git',config,old_types[0][1],True))
for label in ['missing-documentation','invalid-source']:
 config=copy.deepcopy(base);cases.append((label,config,old_types[0][1],False))
results=[]
with tempfile.TemporaryDirectory(prefix='publish-version-') as temp:
 root=Path(temp);guard=root/'bin';guard.mkdir();git=guard/'git';git.write_text('#!/bin/sh\nexit 1\n');git.chmod(0o755)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 for label,config,old,valid in cases:
  runs=[]
  for slot,binary in enumerate([a.elm,a.probe]):
   project=root/f'{label}-{slot}';(project/'src').mkdir(parents=True)
   (project/'elm.json').write_text(json.dumps(config));(project/'README.md').write_text('x'*300);(project/'LICENSE').write_text('')
   source='module Example exposing (identity)\n{-| Example\n@docs identity\n-}\n{-| Identity -}\nidentity : a -> a\nidentity x = x\n'
   if label=='missing-documentation':source='module Example exposing (identity)\nidentity : a -> a\nidentity x = x\n'
   if label=='invalid-source':source='module Example exposing (identity)\nidentity =\n'
   (project/'src/Example.elm').write_text(source)
   home=project/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True);shutil.copy2(original/'registry.dat',cache/'registry.dat')
   core=cache/'elm/core/1.0.5'
   if config['name']!='elm/core':shutil.copytree(original/'elm/core/1.0.5',core)
   core.mkdir(parents=True,exist_ok=True);(core/'docs.json').write_text(json.dumps(old))
   env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1','PATH':str(root/'no-git') if label=='missing-git' else str(guard)+os.pathsep+os.environ['PATH']}
   runs.append(run_command([str(binary.resolve()),*(['publish'] if slot==0 or a.public else [])],project,env,a.ansi,a.terminal_stream,''))
  official,rust=runs
  passed=official['code']==rust['code']==1 and official['stderr']==rust['stderr']
  if a.public:passed=passed and official['stdout']==rust['stdout']
  if valid:passed=passed and ('-- NO GIT ' if label=='missing-git' else '-- NO TAG ') in official['stderr']
  results.append({'case':label,'passed':passed,'official':official,'rust':rust});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.probe.read_bytes()).hexdigest()==sha,'Probe changed during comparisons'
report={'scope':__doc__,'ansi':a.ansi,'terminal_stream':a.terminal_stream,'public_command':a.public,'probe_sha256':sha,'passed':all(r['passed'] for r in results),'cases':len(results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
