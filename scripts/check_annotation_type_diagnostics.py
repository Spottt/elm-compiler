#!/usr/bin/env python3
"""Compare full annotation-type diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('string-int', 'x : Int\nx = "hello"\n'), ('bool-int', 'x : Int\nx = True\n'), ('int-bool', 'x : Bool\nx = 1\n'), ('float-int', 'x : Int\nx = 1.5\n'), ('char-string', "x : String\nx = 'a'\n"), ('unit-int', 'x : Int\nx = ()\n'), ('list-int', 'x : Int\nx = [1,2]\n'), ('tuple-int', 'x : Int\nx = (1,True)\n'), ('function-result', 'f : Int -> Int\nf x = "hello"\n'), ('list-items', 'x : List Int\nx = ["hello"]\n'), ('record-field', 'x : { name : Int }\nx = { name = "hello" }\n'), ('alias', 'type alias Count = Int\nx : Count\nx = "hello"\n'), ('variable', 'x : a\nx = "hello"\n'), ('if-branch', 'x : Int\nx = if True then 1 else "hello"\n'), ('hexadecimal', 'x : Bool\nx = 0xdead\n'), ('named-variable', 'x : custom\nx = "hello"\n'), ('shared-expected-type', 'x : { a : Int, b : Int }\nx = { a = 1, b = "hello" }\n'), ('shared-actual-type', 'x : { a : String, b : Int }\nx = { a = "yes", b = "no" }\n'), ('nested-list', 'x : List (List Int)\nx = [["hello"]]\n'), ('nested-record', 'x : { r : { name : Int }, ok : Bool }\nx = { r = { name = "hello" }, ok = True }\n'), ('tuple-items', 'x : (Int, Bool)\nx = ("hello", True)\n'), ('maybe-bool', 'x : Bool\nx = Just True\n'), ('maybe-in-list', 'x : List Bool\nx = [Just True]\n'), ('maybe-wrong-payload', 'x : Bool\nx = Just "bad"\n'), ('concrete-int-string', 'x : String\nx = String.length "test"\n'), ('value-to-list', 'x : List Int\nx = 1\n'), ('nested-value-to-list', 'x : (List Int, Bool)\nx = (1, True)\n'), ('incompatible-value-to-list', 'x : List Int\nx = True\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='annotation-type-parity-') as temp:
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
