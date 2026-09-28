#!/usr/bin/env python3
"""Compare full infinite-type diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('self-application', 'f x = x x\n'), ('self-list', 'f x = x :: x\n'), ('self-tuple', 'f x = if True then x else (x,x)\n'), ('self-record', 'f x = if True then x else { child = x }\n'), ('recursive-return', 'f x = f\n'), ('nested-let', 'f x =\n    let\n        g y = y y\n    in\n    g x\n'), ('lambda', 'f = \\x -> x x\n'), ('record-pattern', 'f { x } = x x\n'), ('tuple-pattern', 'f (x,y) = x x\n'), ('alias-pattern', 'f (x as y) = y y\n'), ('local-recursive', 'f x =\n    let\n        g y = g\n    in\n    g x\n'), ('nested-list', 'f x = if True then x else [[x]]\n'), ('two-functions', 'f x = x x\ng y = y y\n'), ('two-arguments', 'f x y = (x x, y y)\n'), ('type-error-before', 'a : Int\na = "bad"\nf x = x x\n'), ('type-error-after', 'f x = x x\nz : Int\nz = "bad"\n'), ('mutual', 'f x = g\ng y = f\n'), ('local-two', 'f x =\n    let\n        a y = y y\n        b z = z z\n    in\n    (a x, b x)\n'), ('nested-value', 'f x = x [x]\n'), ('record-access-cycle', 'f r = if True then r else r.child\n'), ('record-update-cycle', 'f r = { r | child = r }\n'), ('valid-list-pattern', 'f xs =\n    case xs of\n        [] -> []\n        x :: _ -> x\n'), ('constructor-cycle', 'type Box a = Box a\nf x = if True then x else Box x\n'), ('alias-cycle', 'type alias Box a = { child : a }\nf x = if True then x else Box x\n'), ('tuple-cycle', 'f (x,y) = if True then x else (x,y)\n'), ('case-cycle', 'f x =\n    case x of\n        Just y -> if True then x else y\n        Nothing -> Nothing\n'), ('two-fields-cycle', 'f r = (r.first r.first, r.second r.second)\n'), ('two-fields-reversed', 'f r = (r.second r.second, r.first r.first)\n'), ('three-fields', 'f r = (r.a r.a, r.b r.b, r.c r.c)\n'), ('one-field-plus-value', 'f r = (r.a r.a, r.z)\n'), ('one-value-plus-field', 'f r = (r.z, r.a r.a)\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='infinite-type-parity-') as temp:
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
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')==('TYPE MISMATCH' if label=='type-error-before' else 'INFINITE TYPE')
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
