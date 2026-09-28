#!/usr/bin/env python3
"""Compare bad-flags diagnostics against Elm 0.19.1 using isolated caches.

Each case checks ordinary compilation, incremental compilation and a warm
incremental repeat, including local/imported aliases and source annotations."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
from diff_test_process import run_command
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
worker = 'main = Platform.worker { init = \\_ -> ((), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }\n'
cases=[('inferred', worker)]
for label,tipe,extra in [
 ('variable','flags',''), ('named-variable','payload',''),
 ('function','Int -> Int',''), ('custom','Choice','type Choice = A | B\n'),
 ('set','Set.Set Int',''), ('result','Result String Int',''),
 ('list-variable','List payload',''), ('tuple-function','(Int, Int -> Int)',''),
 ('record-variable','{ value : payload }',''), ('open-record','{ row | value : Int }',''),
 ('alias','Flags','type alias Flags = { value : Int -> Int }\n'),
 ('tuple-priority','(Int -> Int, Set.Set Int)',''),
 ('record-priority','{ z : Int -> Int, a : Set.Set Int }',''),
 ('open-record-priority','{ row | bad : Int -> Int }',''),
 ('nested-open-record','{ outer : { row | value : Int } }',''),
 ('generic-alias','Flags payload','type alias Flags item = { value : item }\n'),
 ('alias-open-record','Flags row','type alias Flags row = { row | value : Int }\n'),
 ('closed-row-alias','Flags { bad : Int -> Int }','type alias Flags row = { row | value : Int }\n'),
 ('maybe-alias','Maybe Flags','type alias Flags = { value : Set.Set Int }\n'),
 ('long-variable','aVeryLongPayloadTypeVariableWhoseNameMustWrapInTheDiagnostic',''),
 ('tuple-variable-priority','(first, second)',''),
 ('nested-record-priority','{ a : { z : Int -> Int }, b : Set.Set Int }','')]:
 cases.append((label, 'import Set\n'+extra+'main : Program ('+tipe+') () Never\n'+worker))

cases.append(('program-alias', 'type alias App flags = Program flags () Never\nmain : App payload\n'+worker))

results=[]
with tempfile.TemporaryDirectory(prefix='bad-flags-parity-') as temp:
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
   run=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=output.js','--report=json',*options],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   try:report=json.loads(run.stderr)
   except ValueError:report={'unexpected_stderr':run.stderr}
   runs.append({'code':run.returncode,'report':report})
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')=='BAD FLAGS'
  passed=expected and all(run==runs[0] for run in runs[1:]) and runs[0]['code']==1
  terminal=[]
  for binary in [a.elm,a.rust]:
   run=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=output.js'],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   terminal.append({'code':run.returncode,'stderr':run.stderr})
  passed=passed and terminal[0]==terminal[1]
  colored=[]
  for binary in [a.elm,a.rust]:
   run=run_command([str(binary.resolve()),'make','src/Shadow.elm','--output=output.js'],root,env,True,'stderr')
   colored.append({'code':run['code'],'stderr':run['stderr']})
  passed=passed and colored[0]==colored[1]
  results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1],'incremental_cold':runs[2],'incremental_warm':runs[3],'terminal':terminal,'colored_terminal':colored});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'cases':len(results),'comparisons':3*len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
