#!/usr/bin/env python3
"""Compare interactive elm bump with Elm 0.19.1 in disposable package projects.

Compares complete streams, exit status and manifest bytes.
--terminal-stream attaches the selected output stream to a pseudo-terminal. No project
outside temporary fixtures is modified. Uses the online package registry.
"""
import argparse,copy,hashlib,json,os,shutil,tempfile
from diff_test_process import run_command
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ['elm','rust','report']:p.add_argument('--'+name,type=Path,required=True)
p.add_argument('--terminal-stream',choices=['stdout','stderr'])
p.add_argument('--case',action='append',help='Run selected named cases; default runs all cases')
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
base={'type':'package','name':'elm/core','summary':'Isolated package version fixture','license':'BSD-3-Clause','version':'1.0.5','exposed-modules':['Example'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{},'test-dependencies':{}}
def docs(values):return [{'name':'Example','comment':'','unions':[],'aliases':[],'binops':[],'values':[{'name':name,'comment':'','type':tipe} for name,tipe in values]}]
identity=docs([('identity','a -> a')])
cases=[]
for label,old,answer in [('patch-eof',identity,''),('retry-eof',identity,'wrong\n'),('patch-accept',identity,'y\n'),('patch-refuse',identity,'n\n'),('minor-accept',docs([]),'\n'),('major-accept',docs([('identity','Int')]),'Y\n'),('major-refuse',docs([('identity','Int')]),'n\n'),('retry-answer',identity,'wrong\nN\nn\n')]:cases.append((label,base,old,answer,[]))
for label,version,answer in [('new-eof','4.3.2',''),('new-correct','1.0.0',''),('new-accept','4.3.2','y\n'),('new-refuse','4.3.2','n\n')]:
 config=copy.deepcopy(base);config.update(name='codex-bump-fixture/unpublished-package',version=version,dependencies={'elm/core':'1.0.0 <= v < 2.0.0'});cases.append((label,config,identity,answer,[]))
config=copy.deepcopy(base);config['version']='1.0.0';cases.append(('unexpected-version',config,identity,'',[]))
config=copy.deepcopy(base);config['exposed-modules']=[];cases.append(('empty-exposed',config,identity,'',[]))
app={'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
cases += [('application',app,identity,'',[]),('no-outline',None,identity,'',[]),('help',None,identity,'',['--help']),('extra-argument',None,identity,'',['1.0.0']),('unknown-flag',None,identity,'',['--report=json'])]
for label,changes in [
 ('ordered-dependencies', {'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0','aa/zz':'1.0.0 <= v < 2.0.0','aa-bb/yy':'1.0.0 <= v < 2.0.0'}}),
 ('ordered-test-dependencies', {'test-dependencies':{'aa/zz':'1.0.0 <= v < 2.0.0','aa-bb/yy':'1.0.0 <= v < 2.0.0'}}),
 ('grouped-exposed', {'exposed-modules':{'Zed':['Zed.Second','Zed.First'],'Alpha':['Alpha']}}),
 ('unicode-summary', {'summary':'Résumé avec café et symbole λ'}),
 ('ignored-extra-field', {'custom-tool':{'preserved-by-user':True}}),
 ('wrapped-dependency-constraint', {'dependencies':{'elm/core':'65537.0.0 <= v < 65538.0.0'}}),
 ('wrapped-test-constraint', {'test-dependencies':{'aa/bb':'65537.0.0 < v <= 65538.0.0'}}),
 ('wrapped-elm-constraint', {'elm-version':'65536.19.0 <= v < 65536.20.0'}),
]:
 config=copy.deepcopy(base);config.update(name='codex-bump-fixture/unpublished-package',version='3.2.1',dependencies={'elm/core':'1.0.0 <= v < 2.0.0'});config.update(changes)
 cases.append((label,config,identity,'y\n',[]))
if a.case:
 unknown=set(a.case)-{c[0] for c in cases}
 if unknown:p.error('Unknown cases: '+', '.join(sorted(unknown)))
 cases=[c for c in cases if c[0] in a.case]
results=[]
with tempfile.TemporaryDirectory(prefix='elm-bump-cli-') as tmp:
 root=Path(tmp)
 for label,config,old,answer,args in cases:
  project=root/label;(project/'src').mkdir(parents=True)
  manifest=project/'elm.json';initial=json.dumps(config,indent=2)+'\n' if config is not None else None
  (project/'src/Example.elm').write_text('module Example exposing (identity)\n{-| Example\n@docs identity\n-}\n{-| Identity -}\nidentity : a -> a\nidentity x = x\n')
  runs=[]
  for slot,binary in enumerate([a.elm,a.rust]):
   if initial is not None:manifest.write_text(initial)
   home=project/f'home-{slot}';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
   original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages';shutil.copy2(original/'registry.dat',cache/'registry.dat')
   path=cache/'elm/core/1.0.5/docs.json';path.parent.mkdir(parents=True);path.write_text(json.dumps(old))
   env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'}
   run=run_command([str(binary.resolve()),'bump',*args],project,env,bool(a.terminal_stream),a.terminal_stream or 'stderr',answer)
   runs.append({**run,'manifest':manifest.read_text() if manifest.exists() else None})
  expected_code=1 if label.endswith('eof') or label in ['unexpected-version','empty-exposed','application','no-outline','extra-argument','unknown-flag'] else 0
  passed=runs[0]==runs[1] and runs[0]['code']==expected_code
  if 'refuse' in label or label=='retry-answer' or expected_code==1:passed=passed and all(r['manifest']==initial for r in runs)
  results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1]});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha,'Compiler changed during validation'
r={'scope':__doc__,'terminal_stream':a.terminal_stream,'cases':len(results),'compiler_sha256':sha,'passed':all(x['passed'] for x in results),'results':results};a.report.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit(not r['passed'])
