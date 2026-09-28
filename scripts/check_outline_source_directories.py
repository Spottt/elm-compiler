#!/usr/bin/env python3
"""Compare complete source-directory outline diagnostics and duplicate selection."""
import argparse, copy, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
app = {'type':'application','elm-version':'0.19.1','source-directories':['src'],'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
cases = []
for name, dirs in [('empty', []), ('missing', ['missing']), ('two-missing', ['z-missing', 'a-missing']), ('mixed-missing', ['src', 'a-missing', 'z-missing']), ('same', ['src', 'src']), ('relative', ['src', './src']), ('relative-reversed', ['./src', 'src']), ('symlink', ['src', 'alias']), ('three-aliases', ['src', 'alias', './src']), ('two-groups', ['src', './src', 'other', './other']), ('two-groups-reversed', ['other', './other', 'src', './src']), ('absolute', ['src', '@ROOT@/src']), ('missing-priority', ['src', './src', 'missing']), ('unicode', ['é-inexistant', 'autre']), ('nested-path-order', ['a/b', './a/b', 'a-b', './a-b']), ('nested-path-order-reversed', ['a-b', './a-b', 'a/b', './a/b'])]:
 value = copy.deepcopy(app); value["source-directories"] = dirs
 cases.append((name,json.dumps(value,indent=2)))
cases.append(("empty-multiline", cases[0][1].replace("[]", "[\n  ]",1)))
results=[]
with tempfile.TemporaryDirectory(prefix='outline-missing-fields-') as temp:
 root=Path(temp); (root/'src').mkdir(); (root/'other').mkdir(); (root/'a/b').mkdir(parents=True); (root/'a-b').mkdir(); (root/'alias').symlink_to(root/'src',target_is_directory=True); (root/'src/Main.elm').write_text('module Main exposing (x)\nx = 1\n')
 for name,source in cases:
  source=source.replace('@ROOT@',str(root)); (root/'elm.json').write_text(source+'\n'); rows=[]
  for binary in [a.elm,a.rust]:
   streams=[]
   for flags in [['--report=json'],[]]:
    run=subprocess.run([str(binary.resolve()),'make','src/Main.elm','--output=/dev/null',*flags],cwd=root,env={**os.environ,'GHCRTS':'-N1'},capture_output=True,text=True,timeout=10)
    streams.append({'code':run.returncode,'stderr':json.loads(run.stderr) if flags else run.stderr})
   rows.append(streams)
  passed=rows[0]==rows[1] and rows[0][0]['code']==1 and 'SOURCE DIRECTOR' in rows[0][0]['stderr']['title']
  results.append({'case':name,'passed':passed,'official':rows[0],'rust':rows[1]});print(name,'PASS' if passed else 'FAIL',flush=True)
assert sha=={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in [('elm',a.elm),('rust',a.rust)]}
report={'scope':__doc__,'sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
