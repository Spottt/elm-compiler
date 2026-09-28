#!/usr/bin/env python3
"""Compare full open-record diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('close-rigid', 'f : { a | x : Int } -> { a | x : Int }\nf r = { x = 1 }\n'), ('drop-field', 'f : { a | x : Int, y : Bool } -> { a | x : Int }\nf r = r\n'), ('add-field', 'f : { a | x : Int } -> { a | x : Int, y : Bool }\nf r = r\n'), ('distinct-rows', 'f : { a | x : Int } -> { b | x : Int }\nf r = r\n'), ('bare-row', 'f : { a | x : Int } -> a\nf r = r\n'), ('row-as-record', 'f : a -> { a | x : Int }\nf r = { r | x = 1 }\n'), ('update-rigid-field', 'f : { a | x : b } -> { a | x : b }\nf r = { r | x = 1 }\n'), ('closed-to-open-call', 'f : { a | x : Int } -> Int\nf r = r.x\nx = f { y = 1 }\n'), ('open-to-closed-call', 'g : { x : Int } -> Int\ng r = r.x\nf : { a | x : Int } -> Int\nf r = g r\n'), ('nested-rows', 'f : { a | child : { b | x : Int } } -> { a | child : { b | x : Int } }\nf r = { r | child = { x = 1 } }\n'), ('valid-preserve', 'f : { a | x : Int } -> { a | x : Int }\nf r = { r | x = 2 }\nx = f { x = 1, y = True }\n'), ('valid-alias', 'type alias WithX a = { a | x : Int }\nf : WithX a -> WithX a\nf r = { r | x = r.x + 1 }\nx = f { x = 1, y = "a" }\n'), ('valid-inferred', 'f r = { r | x = r.x + 1 }\nx = (f { x = 1 }, f { x = 1, y = True })\n'), ('valid-row-access', 'f : { a | x : Int, y : Bool } -> Int\nf r = r.x\nx = f { x = 1, y = True, z = "a" }\n'), ('different-fields-rows', 'f : { a | x : Int } -> { b | y : Bool }\nf r = r\n'), ('two-common-errors', 'f : { a | x : Int, y : String } -> { a | x : String, y : Bool }\nf r = r\n')]

cases += [('valid-nested-alias', 'type alias WithX a = { a | x : Int }\ntype alias WithY a = WithX { a | y : Bool }\nf : WithY a -> Int\nf r = r.x\nx = f { x = 1, y = True, z = "ok" }\n'), ('nested-alias-missing', 'type alias WithX a = { a | x : Int }\ntype alias WithY a = WithX { a | y : Bool }\nf : WithY a -> Int\nf r = r.x\nx = f { x = 1, z = "ok" }\n'), ('valid-row-independent-instantiations', 'get r = r.x\nx = (get { x = 1 }, get { x = True, y = 2 })\n'), ('row-shared-instantiation', 'f : { a | x : Int } -> { a | y : Bool } -> Int\nf r s = r.x\nx = f { x = 1, z = "ok" } { y = True, z = False }\n'), ('valid-row-shared-instantiation', 'f : { a | x : Int } -> { a | y : Bool } -> Int\nf r s = r.x\nx = f { x = 1, z = "ok" } { y = True, z = "yes" }\n'), ('row-higher-order', 'f : ({ a | x : Int } -> Int) -> { a | x : Int } -> Int\nf g r = g { x = r.x }\n'), ('valid-row-composition', 'f = .child >> .x\nx = (f { child = { x = 1 } }, f { child = { x = True, y = 2 }, z = 3 })\n'), ('row-nested-field-mismatch', 'f : { a | child : { b | x : Int } } -> Int\nf r = r.child.x\nx = f { child = { x = True, y = 2 }, z = 3 }\n')]

cases += [('alias-actual-missing', 'type alias A = { x : Int, z : String }\nf : { x : Int, y : Bool } -> Int\nf r = r.x\na : A\na = { x = 1, z = "ok" }\nx = f a\n'), ('different-alias-fields', 'type alias A = { x : Int, z : String }\ntype alias B = { x : Int, y : Bool }\nf : B -> Int\nf r = r.x\na : A\na = { x = 1, z = "ok" }\nx = f a\n'), ('alias-inner-type-mismatch', 'type alias A = { x : Int }\nf : A -> Int\nf r = r.x\nx = f { x = True }\n'), ('alias-actual-inner-type-mismatch', 'type alias A = { x : Int }\nf : { x : Bool } -> Bool\nf r = r.x\na : A\na = { x = 1 }\nx = f a\n')]

cases += [('nested-different-alias-fields', 'type alias A = { x : Int, z : String }\ntype alias B = { x : Int, y : Bool }\nf : ( B, B ) -> Int\nf r = 1\na : A\na = { x = 1, z = "ok" }\nb : B\nb = { x = 1, y = True }\nx = f ( a, b )\n'), ('nested-different-alias-fields-reversed', 'type alias A = { x : Int, z : String }\ntype alias B = { x : Int, y : Bool }\nf : ( B, B ) -> Int\nf r = 1\na : A\na = { x = 1, z = "ok" }\nb : B\nb = { x = 1, y = True }\nx = f ( b, a )\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='open-record-parity-') as temp:
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
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')=='TYPE MISMATCH'
  expected=(runs[0]['code']==0) if label.startswith('valid-') else (expected and runs[0]['code']==1)
  passed=expected and runs[0]==runs[1]
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
