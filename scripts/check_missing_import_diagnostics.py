#!/usr/bin/env python3
"""Compare exact JSON and plain terminal diagnostics for unresolved imports."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
p.add_argument('--incremental',action='store_true')
a=p.parse_args();rows=[]
binaries=[a.elm.resolve(),a.rust.resolve()];hashes=[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
cases=[('simple',{'Main':'import Missing\nx = 1'}),('alias',{'Main':'import Missing.Nested as M exposing (..)\nx = 1'}),('multiline',{'Main':'import\n    Missing\nx = 1'}),('several',{'Main':'import Zebra\nimport Absent\nx = 1'}),('hint-html',{'Main':'import Html\nx = 1'}),('hint-http',{'Main':'import Http\nx = 1'}),('transitive',{'Main':'import Child\nx = 1','Child':'import Missing\nx = 1'}),('siblings',{'Main':'import Alpha\nimport Beta\nx = 1','Alpha':'import Missing\nx = 1','Beta':'import Absent\nx = 1'}),('independent-type',{'Main':'import Alpha\nimport Beta\nx = 1','Alpha':'import Missing\nx = 1','Beta':'x : Int\nx = \"bad\"'}),('independent-syntax',{'Main':'import Alpha\nimport Beta\nx = 1','Alpha':'import Missing\nx = 1','Beta':'x ='})]
with tempfile.TemporaryDirectory(prefix='elm-import-diagnostics-') as td:
 root=Path(td);home=root/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for pkg in ['elm/core/1.0.5','elm/json/1.1.3']:shutil.copytree(original/pkg,cache/pkg)
 env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'}
 for label,sources in cases:
  project=root/label;project.mkdir()
  shutil.copy2(Path(__file__).resolve().parents[1]/'tests/programs/worker/elm.json',project/'elm.json')
  for name,body in sources.items():
   path=project/(name.replace('.','/')+'.elm');path.parent.mkdir(parents=True,exist_ok=True);path.write_text('module '+name+' exposing (..)\n'+body+'\n')
  for mode in ['json','plain']:
   runs=[]
   for binary in binaries:
    r=subprocess.run([str(binary),'make','Main.elm','--output=/dev/null',*(['--incremental'] if a.incremental and binary==binaries[1] else []),*(['--report=json'] if mode=='json' else [])],cwd=project,env=env,text=True,capture_output=True,timeout=30)
    runs.append({'code':r.returncode,'stderr':json.loads(r.stderr) if mode=='json' else r.stderr})
   passed=runs[0]==runs[1] and runs[0]['code']==1
   rows.append({'case':label,'mode':mode,'passed':passed,'elm':runs[0],'rust':runs[1]})
  if label=='simple':
   (project/'Missing.elm').write_text('module Missing exposing (value)\nvalue = 1\n')
   runs=[]
   for binary in binaries:
    r=subprocess.run([str(binary),'make','Main.elm','--output=/dev/null','--report=json',*(['--incremental'] if a.incremental and binary==binaries[1] else [])],cwd=project,env=env,text=True,capture_output=True,timeout=30)
    runs.append({'code':r.returncode,'stderr':r.stderr})
   rows.append({'case':'new-module-recovery','mode':'json','passed':runs[0]==runs[1] and runs[0]['code']==0,'elm':runs[0],'rust':runs[1]})
assert hashes==[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'scope':__doc__,'incremental':a.incremental,'binary_sha256':hashes,'passed':all(r['passed'] for r in rows),'results':rows},indent=2)+'\n')
print(json.dumps({'checks':len(rows),'failures':[(r['case'],r['mode']) for r in rows if not r['passed']]}))
raise SystemExit(not all(r['passed'] for r in rows))
