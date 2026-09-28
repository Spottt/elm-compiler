#!/usr/bin/env python3
"""Compare bad-main-type diagnostics against Elm 0.19.1 using isolated caches.

Each case checks ordinary compilation, incremental compilation and a warm
incremental repeat, including local/imported aliases and source annotations."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[
 ('number','main = 42\n'),
 ('int-annotation','main : Int\nmain = 42\n'),
 ('string','main = "hello"\n'),
 ('unit','main = ()\n'),
 ('bool','main = True\n'),
 ('list','main = [1,2,3]\n'),
 ('tuple','main = ("hi", True)\n'),
 ('record','main = { count = 1, label = "hi" }\n'),
 ('function','main x = x\n'),
 ('named-variable','main : item -> item\nmain x = x\n'),
 ('alias','type alias Model = { count : Int }\nmain : Model\nmain = { count = 1 }\n'),
 ('alias-constructor','type alias Model = { count : Int }\nmain = Model 1\n'),
 ('generic-alias','type alias Box item = { value : item }\nmain : Box String\nmain = { value = "hi" }\n'),
 ('custom-type','type Message = First | Second\nmain = First\n'),
 ('qualified-type','import Set as S\nmain = S.empty\n'),
 ('imported-alias-constructor','import Models\nmain = Models.Model 1\n'),
 ('imported-alias-value','import Models\nmain = Models.empty\n'),
 ('alias-function-result','type alias Box item = { value : item }\nmain value = Box value\n'),
 ('wide-record','main = { firstLongField = 1, secondLongField = 2, thirdLongField = 3, fourthLongField = 4, fifthLongField = 5 }\n'),
 ('comment-before-definition','-- main is below, not here\nmain = 42\n'),
]

results=[]
with tempfile.TemporaryDirectory(prefix='bad-main-parity-') as temp:
 root=Path(temp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for name,version in [('elm/core','1.0.5'),('elm/json','1.1.3')]:shutil.copytree(original/name/version,cache/name/version)
 (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}));(root/'src').mkdir()
 (root/'src/Models.elm').write_text('module Models exposing (Model, empty)\ntype alias Model = { count : Int }\nempty : Model\nempty = Model 0\n')
 env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
 for label,source in cases:
  (root/'src/Shadow.elm').write_text('module Shadow exposing (..)\n'+source)
  runs=[]
  for binary,options in [(a.elm,[]),(a.rust,[]),(a.rust,['--incremental']),(a.rust,['--incremental'])]:
   run=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=/dev/null','--report=json',*options],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   try:report=json.loads(run.stderr)
   except ValueError:report={'unexpected_stderr':run.stderr}
   runs.append({'code':run.returncode,'report':report})
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')=='BAD MAIN TYPE'
  passed=expected and all(run==runs[0] for run in runs[1:]) and runs[0]['code']==1
  results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1],'incremental_cold':runs[2],'incremental_warm':runs[3]});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'cases':len(results),'comparisons':3*len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
