#!/usr/bin/env python3
"""Compare complete version and constraint diagnostics in elm.json."""
import argparse, copy, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
configs = {'app': {'type': 'application', 'elm-version': '0.19.1', 'source-directories': ['src'], 'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}}, 'test-dependencies': {'direct': {}, 'indirect': {}}}, 'package': {'type': 'package', 'name': 'author/fixture', 'summary': 'Fixture.', 'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': ['Main'], 'elm-version': '0.19.0 <= v < 0.20.0', 'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}}
cases = []
for label,kind,path,value in [('version-0', 'package', ['version'], ''), ('version-1', 'package', ['version'], '1'), ('version-2', 'package', ['version'], '1.2'), ('version-3', 'package', ['version'], '01.2.3'), ('version-4', 'package', ['version'], '1.02.3'), ('version-5', 'package', ['version'], '1.2.x'), ('version-6', 'package', ['version'], '1.2.3-beta'), ('version-7', 'package', ['version'], '1.2.3.4'), ('version-8', 'package', ['version'], 'v1.2.3'), ('app-elm-version', 'app', ['elm-version'], '0.19'), ('dependency-version', 'app', ['dependencies', 'direct', 'elm/core'], '1.0'), ('test-dependency-version', 'app', ['test-dependencies', 'indirect', 'author/pkg'], '1.x.0'), ('constraint-0', 'package', ['dependencies', 'elm/core'], ''), ('constraint-1', 'package', ['dependencies', 'elm/core'], '1.0.0'), ('constraint-2', 'package', ['dependencies', 'elm/core'], '1.0.0<=v<2.0.0'), ('constraint-3', 'package', ['dependencies', 'elm/core'], '1.0.0  <= v < 2.0.0'), ('constraint-4', 'package', ['dependencies', 'elm/core'], '1.0.0 > v < 2.0.0'), ('constraint-5', 'package', ['dependencies', 'elm/core'], '1.0.0 <= V < 2.0.0'), ('constraint-6', 'package', ['dependencies', 'elm/core'], '1.0.0 <= v < 2.x.0'), ('constraint-7', 'package', ['dependencies', 'elm/core'], '01.0.0 <= v < 2.0.0'), ('constraint-8', 'package', ['dependencies', 'elm/core'], '1.0.0 <= v < 2.0.0 suffix'), ('constraint-9', 'package', ['dependencies', 'elm/core'], '1.0.0 <= v < 1.0.0'), ('constraint-10', 'package', ['dependencies', 'elm/core'], '2.0.0 <= v < 1.0.0'), ('constraint-11', 'package', ['dependencies', 'elm/core'], '65535.0.0 <= v < 65535.0.0'), ('constraint-12', 'package', ['dependencies', 'elm/core'], '1.0.0 <= v < 1.0.0 suffix'), ('elm-constraint', 'package', ['elm-version'], '0.19'), ('test-constraint', 'package', ['test-dependencies', 'author/pkg'], '1.0.0 < v < 1.0.0')]:
 config=copy.deepcopy(configs[kind]); cursor=config
 for key in path[:-1]:cursor=cursor.setdefault(key,{})
 cursor[path[-1]]=value
 cases.append((label,json.dumps(config,indent=2)))
for kind,path,good,bad in [('app',['dependencies','direct'],'1.0.5','1.x.0'),('package',['dependencies'],'1.0.0 <= v < 2.0.0','1.0.0 <= v < 1.0.0')]:
 source=json.dumps(configs[kind],indent=2)
 replacement='"elm/core": '+json.dumps(good)
 cases.append(('duplicate-'+kind,source.replace(replacement,replacement+', "elm/core": '+json.dumps(bad))))
results=[]
with tempfile.TemporaryDirectory(prefix='outline-versions-') as temp:
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
