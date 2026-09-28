#!/usr/bin/env python3
"""Compare full shadowing diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[
 ('lambda-same-line','f x = (\\x -> x) x\n'),
 ('argument-and-let','f x =\n    let\n        x = 1\n    in\n    x\n'),
 ('argument-and-case','f x =\n    case Just x of\n        Just x -> x\n        Nothing -> 0\n'),
 ('record-argument','f { x } =\n    let\n        x = 1\n    in\n    x\n'),
 ('global-argument','x = 1\nf x = x\n'),
 ('global-defined-later','f x = x\nx = 1\n'),
 ('global-let','x = 1\nf =\n    let\n        x = 2\n    in\n    x\n'),
 ('let-and-lambda','f =\n    let\n        x = 1\n    in\n    (\\x -> x) 2\n'),
 ('pattern-alias','f original =\n    case Just 1 of\n        (Just value) as original -> value\n        Nothing -> 0\n'),
 ('repeated-name-in-comment','-- x x x\nf x =\n    (\\x -> x) x\n'),
 ('unicode-name','f café =\n    (\\café -> café) café\n'),
]
results=[]
with tempfile.TemporaryDirectory(prefix='shadowing-parity-') as temp:
 root=Path(temp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for name,version in [('elm/core','1.0.5'),('elm/json','1.1.3')]:shutil.copytree(original/name/version,cache/name/version)
 (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}));(root/'src').mkdir()
 env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
 for label,source in cases:
  (root/'src/Shadow.elm').write_text('module Shadow exposing (..)\n'+source)
  runs=[]
  for binary in [a.elm,a.rust]:
   run=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=/dev/null','--report=json'],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   try:report=json.loads(run.stderr)
   except ValueError:report={'unexpected_stderr':run.stderr}
   runs.append({'code':run.returncode,'report':report})
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')=='SHADOWING'
  passed=expected and runs[0]==runs[1] and runs[0]['code']==1
  results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1]});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
