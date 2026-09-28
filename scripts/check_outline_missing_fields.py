#!/usr/bin/env python3
"""Compare complete missing-field outline diagnostics, including source snippets."""
import argparse, copy, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
app = {'type':'application','elm-version':'0.19.1','source-directories':['src'],'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
pkg = {'type':'package','name':'author/fixture','summary':'Fixture.','license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
cases = []
for kind, config in [('app',app),('package',pkg)]:
 for key in config:
  value = copy.deepcopy(config); del value[key]
  cases.append((kind+'-'+key, json.dumps(value,indent=2)))
for section in ['dependencies','test-dependencies']:
 for key in ['direct','indirect']:
  value = copy.deepcopy(app); del value[section][key]
  cases.append((section+'-'+key, json.dumps(value,indent=2).replace('{}','{\n    }')))
value = copy.deepcopy(app); del value['dependencies']['direct']
source = json.dumps(value,indent=2).replace('{}','{\n    }')
cases += [('escaped-key',source.replace('"dependencies"','"dependenc\\u0069es"',1)), ('unicode-before',source.replace('{','{\n  "é☃": "text",',1)), ('duplicate-key',source.replace('"dependencies": {','"dependencies": {',1).replace('  "test-dependencies":', '  "dependencies": { "direct": {}, "indirect": {} },\n  "test-dependencies":',1)), ('leading-whitespace','\n\t'+source)]
results=[]
with tempfile.TemporaryDirectory(prefix='outline-missing-fields-') as temp:
 root=Path(temp); (root/'src').mkdir(); (root/'src/Main.elm').write_text('module Main exposing (x)\nx = 1\n')
 for name,source in cases:
  (root/'elm.json').write_text(source+'\n'); rows=[]
  for binary in [a.elm,a.rust]:
   streams=[]
   for flags in [['--report=json'],[]]:
    run=subprocess.run([str(binary.resolve()),'make','src/Main.elm','--output=/dev/null',*flags],cwd=root,env={**os.environ,'GHCRTS':'-N1'},capture_output=True,text=True,timeout=10)
    streams.append({'code':run.returncode,'stderr':json.loads(run.stderr) if flags else run.stderr})
   rows.append(streams)
  passed=rows[0]==rows[1] and rows[0][0]['code']==1 and rows[0][0]['stderr']['title']=='MISSING FIELD'
  results.append({'case':name,'passed':passed,'official':rows[0],'rust':rows[1]});print(name,'PASS' if passed else 'FAIL',flush=True)
assert sha=={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in [('elm',a.elm),('rust',a.rust)]}
report={'scope':__doc__,'sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
