#!/usr/bin/env python3
"""Compare full case-branch diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('second', 'x = case True of\n    True -> 1\n    False -> "bad"\n'), ('reverse', 'x = case True of\n    True -> "bad"\n    False -> 1\n'), ('third', 'type Choice = A | B | C\nx = case A of\n    A -> 1\n    B -> 2\n    C -> "bad"\n'), ('several', 'type Choice = A | B | C\nx = case A of\n    A -> 1\n    B -> "bad"\n    C -> ()\n'), ('bound-variable', 'x = case Just "bad" of\n    Nothing -> 1\n    Just value -> value\n'), ('tuple-pattern', 'x = case (1,"bad") of\n    (0, _) -> 1\n    (_, value) -> value\n'), ('list-pattern', 'x = case ["bad"] of\n    [] -> 1\n    value :: _ -> value\n'), ('list-result', 'x = case True of\n    True -> [1]\n    False -> ["bad"]\n'), ('tuple-result', 'x = case True of\n    True -> (1,True)\n    False -> (1,"bad")\n'), ('call-context', 'f : Int -> Int\nf a = a\nx = f (case True of\n    True -> "bad"\n    False -> "also bad"\n    )\n'), ('call-bad-case', 'f : Int -> Int\nf a = a\nx = f (case True of\n    True -> 1\n    False -> "bad"\n    )\n'), ('if-condition', 'x = if (case True of\n    True -> 1\n    False -> "bad"\n    ) then 1 else 2\n'), ('nested-case', 'x = case True of\n    True ->\n        case False of\n            True -> 1\n            False -> "bad"\n    False -> 2\n'), ('nested-if', 'x = case True of\n    True -> 1\n    False -> if True then "bad" else "also bad"\n'), ('annotated-bound', 'x : Int\nx = case Just "bad" of\n    Nothing -> 1\n    Just value -> value\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='case-branch-parity-') as temp:
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
