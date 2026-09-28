#!/usr/bin/env python3
"""Compare full coverage-aggregation diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('top-level', 'a x = case x of Just _ -> 1\nz x = case x of Just _ -> 2\n'), ('top-level-reversed', 'z x = case x of Just _ -> 2\na x = case x of Just _ -> 1\n'), ('nested', 'f x y =\n    case x of\n        Just _ ->\n            case y of\n                Just _ -> 1\n'), ('arguments', 'f (Just a) (Just b) = a + b\n'), ('argument-body', 'f (Just a) b = case b of Just _ -> a\n'), ('tuple', 'f x y = (case x of Just _ -> 1, case y of Just _ -> 2)\n'), ('record', 'f x y = { z = case x of Just _ -> 1, a = case y of Just _ -> 2 }\n'), ('let', 'f x y =\n    let\n        z = case x of Just _ -> 1\n        a = case y of Just _ -> 2\n    in\n    z + a\n'), ('dependency', 'a x = case x of Just v -> z v\nz x = case x of Just _ -> 2\n'), ('mixed', 'f x y = (case x of Just _ -> 1, case y of\n    _ -> 1\n    Nothing -> 2\n    )\n'), ('dependency-reverse', 'z x = case x of Just v -> a v\na x = case x of Just _ -> 2\n'), ('mutual-functions', 'a x = case x of Just v -> z (Just v)\nz x = case x of Just v -> a (Just v)\n'), ('recursive-value-function', 'a x = case x of Just _ -> z\nz = a Nothing\n'), ('subject', 'f x =\n    case (case x of Just y -> y) of\n        Just _ -> 1\n'), ('if', 'f x y = if True then (case x of Just _ -> 1) else (case y of Just _ -> 2)\n'), ('let-destruct', 'f x y =\n    let\n        (Just a) = x\n        (Just b) = y\n    in\n    a + b\n'), ('let-functions', 'f x y =\n    let\n        a p = case p of Just _ -> 1\n        z p = case p of Just _ -> 2\n    in\n    a x + z y\n'), ('let-destruct-reversed', 'f x y =\n    let\n        (Just b) = y\n        (Just a) = x\n    in\n    a + b\n'), ('let-mutual', 'f x =\n    let\n        a p = case p of Just v -> z (Just v)\n        z p = case p of Just v -> a (Just v)\n    in\n    a x\n'), ('let-destruct-dependency', 'f x =\n    let\n        (Just z) = x\n        (Just a) = z\n    in\n    a\n'), ('let-destruct-tuple', 'f x y =\n    let\n        (Just a, Just z) = x\n        (Just b) = y\n    in\n    a + z + b\n'), ('let-destruct-value', 'f x y =\n    let\n        (Just a) = x\n        z = case y of Just _ -> 1\n    in\n    a + z\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='coverage-aggregation-parity-') as temp:
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
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')in ('MISSING PATTERNS','UNSAFE PATTERN')
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
