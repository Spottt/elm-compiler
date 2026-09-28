#!/usr/bin/env python3
"""Compare port-signature diagnostics against Elm 0.19.1 using isolated caches.

Each case checks ordinary compilation, incremental compilation and a warm
incremental repeat, including local/imported aliases and source annotations."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
from diff_test_process import run_command
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases = [('port-0', 'port send : Cmd msg\n'), ('port-1', 'port send : Int -> String -> Cmd msg\n'), ('port-2', 'port send : String -> Cmd Int\n'), ('port-3', 'port receive : (String -> a) -> Sub b\n'), ('port-4', 'port receive : String -> Sub msg\n'), ('port-5', 'port receive : (String -> Int) -> Sub Int\n'), ('port-6', 'port send : String -> msg\n'), ('port-7', 'port send : Char -> Cmd msg\n'), ('port-8', 'port send : a -> Cmd msg\n'), ('port-9', 'port send : (Int -> Int) -> Cmd msg\n'), ('port-10', 'port send : { r | value : Int } -> Cmd msg\n'), ('port-11', 'type Custom = Custom\nport send : Custom -> Cmd msg\n'), ('port-12', 'port send : List (Maybe Char) -> Cmd msg\n'), ('port-13', 'type alias Row a = { a | value : Int }\nport send : Row {} -> Cmd msg\n')]

cases.extend([
 ('multiple-ports','port zebra : Char -> Cmd msg\nport alpha : Cmd msg\n'),
 ('valid-between-errors','port zebra : Char -> Cmd msg\nport valid : Int -> Cmd msg\nport alpha : Cmd msg\n'),
 ('port-before-value-type','port send : Char -> Cmd msg\nvalue : Int\nvalue = True\n'),
 ('three-invalid-ports','port zebra : Char -> Cmd msg\nport middle : (Int -> a) -> Sub b\nport alpha : Cmd msg\n'),
 ('multiple-reversed','port alpha : Cmd msg\nport zebra : Char -> Cmd msg\n'),
 ('three-arguments','port send : Int -> Int -> Int -> Cmd msg\n'),
 ('four-arguments','port send : Int -> Int -> Int -> Int -> Cmd msg\n'),
 ('arity-before-message','port send : Int -> Int -> Cmd Int\n'),
 ('no-arg-before-message','port send : Cmd Int\n'),
 ('named-payload','port send : payload -> Cmd msg\n'),
 ('nested-payload','port send : { value : Maybe payload } -> Cmd msg\n'),
 ('incoming-payload','port receive : ((Int -> Int) -> msg) -> Sub msg\n'),
 ('port-and-unknown-value','port send : Char -> Cmd msg\nvalue = missing\n'),
 ('port-and-shadowing','port send : Char -> Cmd msg\nf x x = x\n'),
 ('imported-alias-payload','import Models\nport send : Models.Data payload -> Cmd msg\n'),
 ('multiline-port','port\n    send\n    : Char -> Cmd msg\n'),
 ('unicode-port','port envoyerÉtat : Char -> Cmd msg\n'),
 ('alias-payload','type alias Data item = { value : item }\nport send : Data payload -> Cmd msg\n'),
])

results=[]
with tempfile.TemporaryDirectory(prefix='port-signature-parity-') as temp:
 root=Path(temp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for name,version in [('elm/core','1.0.5'),('elm/json','1.1.3')]:shutil.copytree(original/name/version,cache/name/version)
 (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}));(root/'src').mkdir()
 (root/'src/Models.elm').write_text('module Models exposing (Model, empty, Data)\ntype alias Model = { count : Int }\nempty : Model\nempty = Model 0\ntype alias Data item = { value : item }\n')
 env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
 for label,source in cases:
  (root/'src/Shadow.elm').write_text('port module Shadow exposing (..)\n'+source)
  runs=[]
  for binary,options in [(a.elm,[]),(a.rust,[]),(a.rust,['--incremental']),(a.rust,['--incremental'])]:
   run=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=output.js','--report=json',*options],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   try:report=json.loads(run.stderr)
   except ValueError:report={'unexpected_stderr':run.stderr}
   runs.append({'code':run.returncode,'report':report})
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')in ('BAD PORT','PORT ERROR','NAMING ERROR','SHADOWING','NAME CLASH')
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
