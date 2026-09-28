#!/usr/bin/env python3
"""Compare full record_update_keys diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('missing', 'r = { name = "a" }\nx = { r | nme = "b" }\n'), ('empty', 'r = {}\nx = { r | name = 1 }\n'), ('multiple', 'r = { name = "a" }\nx = { r | z = 1, age = 2 }\n'), ('alias', 'type alias Person = { name : String }\nr : Person\nr = { name = "a" }\nx = { r | age = 1 }\n'), ('multiline', 'r = { name = "a" }\nx =\n    { r\n        | nme = "b"\n    }\n'), ('many', 'r = { a = 1, b = 2, c = 3, d = 4, e = 5 }\nx = { r | f = 1 }\n'), ('nonrecord-int', 'r = 1\nx = { r | name = 1 }\n'), ('nonrecord-string', 'r = "a"\nx = { r | name = 1 }\n'), ('nonrecord-bool', 'r = True\nx = { r | name = 1 }\n'), ('nonrecord-list', 'r = [1]\nx = { r | name = 1 }\n'), ('nonrecord-function', 'r = identity\nx = { r | name = 1 }\n'), ('nonrecord-unit', 'r = ()\nx = { r | name = 1 }\n'), ('rigid', 'f : a -> a\nf r = { r | name = 1 }\n'), ('open', 'f : { a | name : String } -> { a | name : String }\nf r = { r | age = 1 }\n'), ('same-record-twice', 'r = { name = "a" }\nx = ({ r | nme = 1 }, { r | age = 1 })\n'), ('independent-records', 'r = { name = "a" }\ns = { age = 1 }\nx = ({ r | nme = 1 }, { s | name = "a" })\n'), ('bad-value-too', 'r = { name = "a" }\nx = { r | nme = 1, name = True }\n'), ('nested-error', 'r = { name = "a" }\nx = { r | nme = 1 + True }\n'), ('comment', 'r = { name = "a" }\nx = { r {- r | nme -} | nme {- nme -} = "a" }\n'), ('unicode', 'récord = { prénom = "a" }\nx = { récord | préno = "b" }\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='record_update_keys-parity-') as temp:
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
