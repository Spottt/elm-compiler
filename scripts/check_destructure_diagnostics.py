#!/usr/bin/env python3
"""Compare full destructure diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('tuple-number', 'x =\n    let\n        (a, b) = 1\n    in\n    a\n'), ('tuple-string', 'x =\n    let\n        (a, b) = "bad"\n    in\n    b\n'), ('tuple-size', 'x =\n    let\n        (a, b) = (1, 2, 3)\n    in\n    a\n'), ('record-number', 'x =\n    let\n        { a } = 1\n    in\n    a\n'), ('record-missing', 'x =\n    let\n        { a } = { b = 1 }\n    in\n    a\n'), ('nested-tuple', 'x =\n    let\n        (a, (b, c)) = (1, True)\n    in\n    b\n'), ('inline', 'x = let (a,b) = True in a\n'), ('independent', 'x =\n    let\n        (a, b) = 1\n        { c } = False\n    in\n    (a, c)\n')]

cases += [('tuple-alias', 'type alias A = { x : Int }\nf : A -> Int\nf r =\n    let\n        (a,b) = r\n    in\n    a\n'), ('annotated-result', 'f : Int\nf =\n    let\n        (a,b) = True\n    in\n    a\n'), ('nested-let', 'f =\n    let\n        x =\n            let\n                (a,b) = "bad"\n            in\n            a\n    in\n    x\n'), ('dependent-errors', 'f =\n    let\n        (a,b) = 1\n        (c,d) = a\n    in\n    c\n'), ('typeerror-before', 'a : Int\na = True\nf =\n    let\n        (x,y) = 1\n    in\n    x\n'), ('typeerror-after', 'f =\n    let\n        (x,y) = 1\n    in\n    x\na : Int\na = True\n'), ('destructure-lambda', 'f =\n    let\n        (a,b) = \\x -> x\n    in\n    a\n'), ('destructure-if', 'f =\n    let\n        (a,b) = if True then 1 else 2\n    in\n    a\n')]
results=[]
with tempfile.TemporaryDirectory(prefix='destructure-parity-') as temp:
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
