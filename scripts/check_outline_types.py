#!/usr/bin/env python3
"""Compare complete structural type diagnostics in elm.json."""
import argparse, copy, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
configs = {'app': {'type': 'application', 'elm-version': '0.19.1', 'source-directories': ['src'], 'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}}, 'test-dependencies': {'direct': {}, 'indirect': {}}}, 'package': {'type': 'package', 'name': 'author/fixture', 'summary': 'Fixture.', 'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': ['Main'], 'elm-version': '0.19.0 <= v < 0.20.0', 'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}}
cases = []
for kind in ['app','package']:
 for field in configs[kind]:
  config=copy.deepcopy(configs[kind]);config[field]={'probe':0} if field=='source-directories' else [0,1]
  # exposed-modules is itself a list, so test a wrong entry instead.
  if field=='exposed-modules':config[field]=[[0,1]]
  cases.append((kind+'-'+field,json.dumps(config,indent=2)))
for kind,path,value in [
 ('app',['dependencies','direct'],[0,1]),
 ('app',['test-dependencies','indirect'],[0,1]),
 ('app',['dependencies','direct','elm/core'],[0,1]),
 ('app',['test-dependencies','direct','author/pkg'],[0,1]),
 ('package',['dependencies','elm/core'],[0,1]),
 ('package',['test-dependencies','author/pkg'],[0,1]),
 ('package',['exposed-modules'],{'Public':{'probe':0}}),
 ('package',['exposed-modules'],{'Public':['Main',[0,1]]}),
 ('package',['exposed-modules'],{'x'*20:['Main']}),
 ('app',['source-directories'],['src',[0,1]]),
]:
 config=copy.deepcopy(configs[kind]);cursor=config
 for key in path[:-1]:cursor=cursor.setdefault(key,{})
 cursor[path[-1]]=value
 cases.append((kind+'-nested-'+'.'.join(path)+'-'+str(len(cases)),json.dumps(config,indent=2)))
cases.append(('root-array', '[\n  0,\n  1\n]'))
config=copy.deepcopy(configs['app']); source=json.dumps(config,indent=2)
cases.append(('duplicate-dependency',source.replace('"elm/core": "1.0.5"', '"elm/core": "1.0.5", "elm/core": [\n 0,\n 1\n]')))
config=copy.deepcopy(configs['package']);source=json.dumps(config,indent=2)
cases.append(('duplicate-group',source.replace('[\n    "Main"\n  ]', '{"Public":["Main"],"Public":["Main",[\n0,\n1\n]]}')))
# The same numeric spelling is a field in one context and an index in another.
cases.append(('numeric-group',source.replace('[\n    "Main"\n  ]', '{"0":[[\n0,\n1\n]]}')))
results=[]
with tempfile.TemporaryDirectory(prefix='outline-types-') as temp:
 root=Path(temp); home=root/'home'; shutil.copytree(Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages',home/'0.19.1/packages'); (root/'src').mkdir(); (root/'src/Main.elm').write_text('module Main exposing (x)\nx = 1\n')
 for name,source in cases:
  (root/'elm.json').write_text(source+'\n'); rows=[]
  for binary in [a.elm,a.rust]:
   streams=[]
   for flags in [['--report=json'],[]]:
    run=subprocess.run([str(binary.resolve()),'make','src/Main.elm','--output=/dev/null',*flags],cwd=root,env={**os.environ,'GHCRTS':'-N1','ELM_HOME':str(home)},capture_output=True,text=True,timeout=10)
    streams.append({'code':run.returncode,'stderr':(json.loads(run.stderr) if run.stderr else None) if flags else run.stderr})
   rows.append(streams)
  passed=rows[0]==rows[1] and rows[0][0]['code']==(0 if name.startswith('valid-') else 1)
  results.append({'case':name,'passed':passed,'official':rows[0],'rust':rows[1]});print(name,'PASS' if passed else 'FAIL',flush=True)
assert sha=={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in [('elm',a.elm),('rust',a.rust)]}
report={'scope':__doc__,'sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
