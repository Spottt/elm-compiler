#!/usr/bin/env python3
"""Compare exact JSON/plain diagnostics for conflicting local and package imports.

Synthetic package exposures are added only to private copies of package sources.
Also checks recovery after deleting an owned conflicting source fixture."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
p.add_argument('--incremental',action='store_true')
a=p.parse_args();rows=[];binaries=[a.elm.resolve(),a.rust.resolve()];hashes=[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
cases=[('two-roots',['src','other'],'Shared'),('three-roots',['src','third','other'],'Shared'),('nested-name',['src','other'],'Widgets.Shared'),('local-package',['src'],'Array'),('two-packages',['src'],'Clash'),('local-two-packages',['src'],'Clash')]
with tempfile.TemporaryDirectory(prefix='elm-ambiguous-import-') as td:
 root=Path(td);home=root/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for pkg in ['elm/core/1.0.5','elm/json/1.1.3','elm/url/1.0.0']:shutil.copytree(original/pkg,cache/pkg)
 env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'}
 for label,roots,name in cases:
  if label.endswith('packages'):
   for pkg in ['elm/json/1.1.3','elm/url/1.0.0']:
    package=cache/pkg;outline=json.loads((package/'elm.json').read_text())
    exposed=outline['exposed-modules']
    if isinstance(exposed,dict):exposed=exposed.setdefault('Test fixture',[])
    if name not in exposed:exposed.append(name)
    (package/'elm.json').write_text(json.dumps(outline))
    (package/'src/Clash.elm').write_text('module Clash exposing (value)\nvalue = 1\n')
    for artifact in package.glob('*.dat'):artifact.unlink()
  project=root/label;project.mkdir()
  manifest=json.loads((Path(__file__).resolve().parents[1]/'tests/programs/worker/elm.json').read_text());manifest['source-directories']=roots
  if label.endswith('packages'):manifest['dependencies']['direct']['elm/url']='1.0.0'
  (project/'elm.json').write_text(json.dumps(manifest))
  for directory in roots:
   source=project/directory/(name.replace('.','/')+'.elm');source.parent.mkdir(parents=True,exist_ok=True)
   if label!='two-packages':source.write_text('module '+name+' exposing (value)\nvalue = 1\n')
  (project/'src/Main.elm').write_text('module Main exposing (..)\nimport '+name+'\nx = 1\n')
  for mode in ['json','plain']:
   runs=[]
   for binary in binaries:
    r=subprocess.run([str(binary),'make','src/Main.elm','--output=/dev/null',*(['--incremental'] if a.incremental and binary==binaries[1] else []),*(['--report=json'] if mode=='json' else [])],cwd=project,env=env,text=True,capture_output=True,timeout=30)
    runs.append({'code':r.returncode,'stderr':json.loads(r.stderr) if mode=='json' else r.stderr})
   passed=runs[0]==runs[1] and runs[0]['code']==1
   rows.append({'case':label,'mode':mode,'passed':passed,'elm':runs[0],'rust':runs[1]})
  if label=='two-roots':
   (project/'other/Shared.elm').unlink()
   runs=[]
   for binary in binaries:
    r=subprocess.run([str(binary),'make','src/Main.elm','--output=/dev/null','--report=json',*(['--incremental'] if a.incremental and binary==binaries[1] else [])],cwd=project,env=env,text=True,capture_output=True,timeout=30)
    runs.append({'code':r.returncode,'stderr':r.stderr})
   rows.append({'case':'removed-conflict-recovery','mode':'json','passed':runs[0]==runs[1] and runs[0]['code']==0,'elm':runs[0],'rust':runs[1]})
assert hashes==[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'scope':__doc__,'incremental':a.incremental,'binary_sha256':hashes,'passed':all(r['passed'] for r in rows),'results':rows},indent=2)+'\n')
print(json.dumps({'checks':len(rows),'failures':[(r['case'],r['mode']) for r in rows if not r['passed']]}))
raise SystemExit(not all(r['passed'] for r in rows))
