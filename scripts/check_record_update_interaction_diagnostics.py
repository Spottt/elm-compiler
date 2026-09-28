#!/usr/bin/env python3
"""Compare record update constraint interactions against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases = [('rigid-base', 'f : a -> a\nf r = { r | x = 1 }\n'), ('rigid-extension', 'f : { a | x : Int } -> { b | x : Int }\nf r = { r | x = 1 }\n'), ('rigid-field', 'f : { a | x : b } -> { a | x : b }\nf r = { r | x = 1 }\n'), ('update-cycle', 'f r = { r | x = r }\n'), ('two-updates', 'f r = ( { r | x = 1 }, { r | x = True } )\n'), ('nested-updates', 'f r = { r | x = { r | x = True } }\n'), ('row-swap', 'f : { a | x : Int, y : Bool } -> { a | x : Bool, y : Int }\nf r = { r | x = r.y, y = r.x }\n'), ('extra-result-field', 'f : { a | x : Int } -> { a | x : Int, y : Bool }\nf r = { r | x = 2 }\n'), ('alias-result', 'type alias A = { x : Int, y : Bool }\nf : { x : Int } -> A\nf r = { r | x = 2 }\n'), ('lambda-update', 'f : { x : Int } -> { x : Int }\nf r = (\\s -> { s | x = True }) r\n')]
cases += [('independent-same-type-fields', 'r = { x = "a", y = "b" }\nf = { r | x = 1, y = True }\n'), ('same-record-two-updates', 'r = { x = "a" }\nf = ( { r | x = 1 }, { r | x = True } )\n'), ('independent-records', 'r = { x = "a" }\ns = { x = "b" }\nf = ( { r | x = 1 }, { s | x = True } )\n'), ('row-swap-reversed', 'f : { a | x : Int, y : Bool } -> { a | x : Bool, y : Int }\nf r = { r | y = r.x, x = r.y }\n'), ('row-swap-closed', 'f : { x : Int, y : Bool } -> { x : Bool, y : Int }\nf r = { r | x = r.y, y = r.x }\n')]
results=[]
with tempfile.TemporaryDirectory(prefix='record_update_value-parity-') as temp:
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
  title = 'INFINITE TYPE' if label == 'update-cycle' else 'TYPE MISMATCH'
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')==title
  passed=expected and runs[0]==runs[1] and runs[0]['code']==1
  incremental=[]
  for temperature in ['cold','warm']:
   run=subprocess.run([str(a.rust.resolve()),'make','src/Shadow.elm','--output=/dev/null','--report=json','--incremental'],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   try: report=json.loads(run.stderr)
   except ValueError: report={'unexpected_stderr':run.stderr}
   incremental.append({'code':run.returncode,'report':report})
  passed=passed and all(item==runs[0] for item in incremental)
  terminal=[]
  for binary in [a.elm,a.rust]:
   run=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=/dev/null'],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   terminal.append({'code':run.returncode,'stderr':run.stderr})
  terminal_match=terminal[0]==terminal[1]
  passed=passed and terminal_match
  results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1],'incremental':incremental,'terminal_match':terminal_match,'terminal':terminal});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
