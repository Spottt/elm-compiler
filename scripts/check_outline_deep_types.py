#!/usr/bin/env python3
"""Compare deeply nested wrong-type manifest diagnostics with Elm."""
import argparse, copy, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
configs = {'app': {'type': 'application', 'elm-version': '0.19.1', 'source-directories': ['src'], 'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}}, 'test-dependencies': {'direct': {}, 'indirect': {}}}, 'package': {'type': 'package', 'name': 'author/fixture', 'summary': 'Fixture.', 'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': ['Main'], 'elm-version': '0.19.0 <= v < 0.20.0', 'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}}
cases = []
for kind,path in [('app',['type']),('app',['elm-version']),('app',['source-directories']),('app',['dependencies']),('app',['dependencies','direct']),('app',['dependencies','direct','elm/core']),('package',['summary']),('package',['exposed-modules']),('package',['exposed-modules','Public']),('package',['dependencies','elm/core'])]:
 for depth in [150,1000]:
  config=copy.deepcopy(configs[kind]);cursor=config
  for key in path[:-1]:
   if key=='exposed-modules':cursor[key]={}
   cursor=cursor.setdefault(key,{})
  cursor[path[-1]]='PLACEHOLDER'
  # The wrong outer type spans lines so Elm's reporter terminates.
  value=('{"nested":\n'+'['*depth+'\n0\n'+']'*depth+'\n}') if path[-1] in ['source-directories','exposed-modules','Public'] else ('[\n'+'['*depth+'\n0\n'+']'*depth+'\n]')
  source=json.dumps(config,indent=2).replace('"PLACEHOLDER"',value)
  cases.append((kind+'-'+'.'.join(path)+'-'+str(depth),source))
results=[]
with tempfile.TemporaryDirectory(prefix='outline-deep-types-') as temp:
 root=Path(temp); home=root/'home'; shutil.copytree(Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages',home/'0.19.1/packages'); (root/'src').mkdir(); (root/'src/Main.elm').write_text('module Main exposing (x)\nx = 1\n')
 for name,source in cases:
  (root/'elm.json').write_text(source+'\n'); rows=[]
  for binary in [a.elm,a.rust]:
   streams=[]
   for flags in [['--report=json'],[]]:
    run=subprocess.run([str(binary.resolve()),'make','src/Main.elm','--output=/dev/null',*flags],cwd=root,env={**os.environ,'GHCRTS':'-N1','ELM_HOME':str(home)},capture_output=True,text=True,timeout=10)
    streams.append({'code':run.returncode,'stderr':(json.loads(run.stderr) if run.stderr.lstrip().startswith('{') else {'unparsed':run.stderr}) if flags else run.stderr})
   rows.append(streams)
  passed=rows[0]==rows[1] and rows[0][0]['code']==(0 if name.startswith('valid-') else 1)
  results.append({'case':name,'passed':passed,'official':rows[0],'rust':rows[1]});print(name,'PASS' if passed else 'FAIL',flush=True)
assert sha=={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in [('elm',a.elm),('rust',a.rust)]}
report={'scope':__doc__,'sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
