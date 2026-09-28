#!/usr/bin/env python3
"""Compare complete dependency-name and project-type diagnostics in elm.json."""
import argparse, copy, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
configs = {'app': {'type': 'application', 'elm-version': '0.19.1', 'source-directories': ['src'], 'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}}, 'test-dependencies': {'direct': {}, 'indirect': {}}}, 'package': {'type': 'package', 'name': 'author/fixture', 'summary': 'Fixture.', 'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': ['Main'], 'elm-version': '0.19.0 <= v < 0.20.0', 'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}}
cases = []
for kind, paths in [('app', [['dependencies', 'direct'], ['dependencies', 'indirect'], ['test-dependencies', 'direct'], ['test-dependencies', 'indirect']]), ('package', [['dependencies'], ['test-dependencies']])]:
 for path in paths:
  for name in ['', 'elm', 'Author/Bad', 'a--b/core', 'author/pkg-', 'é/core', 'author/foo_bar', 'author/foo/bar']:
   config=copy.deepcopy(configs[kind]);cursor=config
   for key in path:cursor=cursor[key]
   cursor[name]='1.0.0' if kind=='app' else '1.0.0 <= v < 2.0.0'
   cases.append((kind+'-'+'.'.join(path)+'-'+name,json.dumps(config,indent=2,ensure_ascii=False)))
for value in ['', 'Application', 'library', 'é', 'app\\lication']:
 config=copy.deepcopy(configs['app']);config['type']=value
 cases.append(('type-'+value,json.dumps(config,indent=2,ensure_ascii=False)))
# Preserve raw spellings, repeated fields, and source order.
source=cases[0][1]
for label, text in [
 ('escaped-name',source.replace('"": "1.0.0"', '"author/fo\\u006f": "1.0.0"')),
 ('escaped-quote-name',source.replace('"": "1.0.0"', '"author/foo\\\"bar": "1.0.0"')),
 ('two-invalid-source-order',source.replace('"": "1.0.0"', '"z/Bad": "1.0.0", "a/Bad": "1.0.0"')),
 ('duplicate-invalid-name',source.replace('"": "1.0.0"', '"bad": "1.0.0", "bad": "2.0.0"')),
 ('type-compact', '{"type":"library"}'),
 ('type-leading-space', '  {"type":"library"}'),
 ('type-duplicate', '{"type":"library", "type":"application"}'),
]: cases.append((label,text))
results=[]
with tempfile.TemporaryDirectory(prefix='outline-names-') as temp:
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
