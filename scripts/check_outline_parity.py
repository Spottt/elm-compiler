#!/usr/bin/env python3
"""Compare elm.json structural and source-directory validation with Elm 0.19.1."""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile

p=argparse.ArgumentParser(description=__doc__);p.add_argument('--elm',type=Path,required=True);p.add_argument('--report',type=Path);args=p.parse_args()
crate=Path(__file__).resolve().parents[1]
app={'type':'application','elm-version':'0.19.1','source-directories':['src'],'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
pkg={'type':'package','name':'author/fixture','summary':'Fixture.','license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
cases=[]
def add(name,base,field=None,value=None,remove=False,ascii=False):
 c=copy.deepcopy(base)
 if field:
  if remove:c.pop(field)
  else:c[field]=value
 cases.append((name,c,ascii))
for kind,base in [('app',app),('pkg',pkg)]:
 add(kind+'-valid',base)
 for field in base:
  add(kind+'-missing-'+field,base,field,remove=True)
  add(kind+'-null-'+field,base,field,None)
for n in [0,79,80]:add('summary-'+str(n),pkg,'summary','x'*n)
for n in [39,40]:add('summary-utf8-'+str(n),pkg,'summary','é'*n)
add('summary-escaped-utf8',pkg,'summary','é'*39,ascii=True)
for license in ['MIT','Apache-2.0','0BSD','UNLICENSED','mit','MIT OR Apache-2.0','not-a-license']:add('license-'+license,pkg,'license',license)
for name in ['', 'Main.lower','Main..Other','Main_', 'Main.A1',"Main'",'Éclair','A'*255,'A'*256]:add('module-'+name[:30]+'-'+str(len(name)),pkg,'exposed-modules',[name])
for n in [0,19,20]:add('heading-'+str(n),pkg,'exposed-modules',{'x'*n:['Main']})
for value in [[],{}, {'Public':['Main']},{'Public':[42]},[42]]:add('exposed-'+str(value),pkg,'exposed-modules',value)
for value in [[],['missing'],['src','src'],['src','./src'],['src','alias'],['src',42],'src']:
 add('dirs-'+str(value),app,'source-directories',value)
for version in ['1.0','01.0.0','1.0.0-beta','65536.0.0',42]:add('version-'+str(version),pkg,'version',version)
add('package-without-core',pkg,'dependencies',{})
add('application-wrapped-compiler-version',app,'elm-version','65536.19.1')
# Multiline wrong values avoid the official same-line recursive region bug.
add('oracle-expecting-string',pkg,'summary',[0,1])
add('oracle-expecting-object',pkg,'dependencies',[0,1])
add('oracle-expecting-array',app,'source-directories',{'probe':0})
results=[];failures=[];timeouts=[];diagnostic_failures=[]
with tempfile.TemporaryDirectory(prefix='elm-outline-parity-') as tmp:
 root=Path(tmp)
 for index,(name,config,ascii) in enumerate(cases):
  project=root/str(index);(project/'src').mkdir(parents=True)
  (project/'alias').symlink_to(project/'src',target_is_directory=True)
  (project/'src/Main.elm').write_text('module Main exposing (value)\nvalue : Int\nvalue = 42\n')
  (project/'elm.json').write_text(json.dumps(config,ensure_ascii=ascii,indent=2)+'\n')
  row={'case':name}
  for compiler,binary in [('official',args.elm.resolve()),('rust',crate/'target/release/planexpo-elm')]:
   try:
    r=subprocess.run([str(binary),'make','src/Main.elm','--output=/dev/null','--report=json'],cwd=project,env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=3)
    row[compiler]={'accepted':r.returncode==0,'stderr':r.stderr}
   except subprocess.TimeoutExpired:
    row[compiler]={'accepted':None,'timed_out':True};timeouts.append({'case':name,'compiler':compiler})
  if row['official']['accepted'] is not None and row['rust']['accepted'] is not None and row['official']['accepted']!=row['rust']['accepted']:failures.append(name)
  if row['official']['accepted'] is False and row['rust']['accepted'] is False:
   official=json.loads(row['official']['stderr']); native=json.loads(row['rust']['stderr'])
   for key in ['type','path','title']:
    if official.get(key)!=native.get(key):diagnostic_failures.append({'case':name,'field':key,'official':official.get(key),'rust':native.get(key)})
  results.append(row)
report={'cases':len(results),'timeout_seconds':3,'comparison_complete':not timeouts,'compared':len(results)-len({t['case'] for t in timeouts}),'failures':failures,'diagnostic_failures':diagnostic_failures,'timeouts':timeouts,'results':results}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures,'diagnostic_failures':diagnostic_failures,'timeouts':timeouts}));raise SystemExit(bool(failures or diagnostic_failures or timeouts))
