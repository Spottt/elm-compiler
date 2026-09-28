#!/usr/bin/env python3
"""Compare full missing-pattern diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('union', 'type Choice = A | B | C\nf x =\n    case x of\n        A -> 1\n'), ('nested-tuple', 'type Bit = Zero | One\nf x =\n    case x of\n        (Zero, _) -> 1\n        (One, One) -> 2\n'), ('list-empty', 'f x =\n    case x of\n        [] -> 1\n'), ('list-cons', 'f x =\n    case x of\n        _ :: _ -> 1\n'), ('list-length', 'f x =\n    case x of\n        [] -> 1\n        [_] -> 2\n'), ('integer', 'f x =\n    case x of\n        1 -> 1\n'), ('nested-constructor', 'type Choice = A | B\ntype Box = Box Choice\nf x =\n    case x of\n        Box A -> 1\n'), ('function-argument', 'type Choice = A | B\nf A = 1\n'), ('lambda', 'type Choice = A | B\nf = \\A -> 1\n'), ('destructure', 'type Choice = A | B\nf x =\n    let\n        A = x\n    in\n    1\n'), ('destructure-list', 'f x =\n    let\n        [a] = x\n    in\n    a\n'), ('same-line', 'type Choice = A | B\nf x = case x of A -> 1\n'), ('maybe', 'f x =\n    case x of\n        Just _ -> 1\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='missing-pattern-parity-') as temp:
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
  # A real output uses the developer's project/type cache path. Warm it with
  # a valid program, then verify failed edits preserve the last good bundle.
  main=root/'src/Main.elm'
  main.write_text('module Main exposing (main)\nimport Shadow\nmain : Program () () Never\nmain = Platform.worker { init = \\_ -> ((), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }\n')
  shadow=root/'src/Shadow.elm'
  invalid_source=shadow.read_text()
  shadow.write_text('module Shadow exposing (..)\nf = ()\n')
  command=[str(a.rust.resolve()),'make','src/Main.elm','--output=result.js','--report=json','--incremental']
  valid=subprocess.run(command,cwd=root,env=env,text=True,capture_output=True,timeout=30)
  assert valid.returncode==0,valid.stderr
  bundle=(root/'result.js').read_bytes()
  assert any(path.is_file() for path in (root/'elm-stuff/planexpo-rust/types-v1').rglob('*')), 'Valid build did not populate the type cache'
  shadow.write_text(invalid_source)
  for temperature in ['after-valid-build','repeated-error']:
   run=subprocess.run(command,cwd=root,env=env,text=True,capture_output=True,timeout=30)
   try: report=json.loads(run.stderr)
   except ValueError: report={'unexpected_stderr':run.stderr}
   incremental.append({'code':run.returncode,'report':report})
   assert (root/'result.js').read_bytes()==bundle, 'Failed build replaced the valid bundle'
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
