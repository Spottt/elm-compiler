#!/usr/bin/env python3
"""Compare full arithmetic diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('subtract-left-bool', 'x = True - 1\n'), ('subtract-left-string', 'x = "bad" - 1\n'), ('subtract-left-list', 'x = [] - 1\n'), ('subtract-right-bool', 'x = 1 - True\n'), ('subtract-right-string', 'x = 1 - "bad"\n'), ('subtract-right-list', 'x = 1 - []\n'), ('power-left-bool', 'x = True ^ 1\n'), ('power-left-string', 'x = "bad" ^ 1\n'), ('power-left-list', 'x = [] ^ 1\n'), ('power-right-bool', 'x = 1 ^ True\n'), ('power-right-string', 'x = 1 ^ "bad"\n'), ('power-right-list', 'x = 1 ^ []\n'), ('subtract-both', 'x = True - "bad"\n'), ('power-both', 'x = True ^ "bad"\n'), ('subtract-result', 'x : String\nx = 1 - 2\n'), ('power-result', 'x : String\nx = 1 ^ 2\n'), ('subtract-nested', 'x = 1 - (2 ^ True)\n'), ('power-multiline', 'x =\n    1\n        ^ True\n'), ('subtract-Int-Float', 'a : Int\na = 1\nb : Float\nb = 2\nx = a - b\n'), ('subtract-Float-Int', 'a : Float\na = 1\nb : Int\nb = 2\nx = a - b\n'), ('power-Int-Float', 'a : Int\na = 1\nb : Float\nb = 2\nx = a ^ b\n'), ('power-Float-Int', 'a : Float\na = 1\nb : Int\nb = 2\nx = a ^ b\n'), ('cast-multiline', 'a : Int\na = 1\nx =\n    a\n        - 2.5\n'), ('add-left-bool', 'x = True + 1\n'), ('add-left-string', 'x = "bad" + 1\n'), ('add-left-list', 'x = [] + 1\n'), ('add-right-bool', 'x = 1 + True\n'), ('add-right-string', 'x = 1 + "bad"\n'), ('add-right-list', 'x = 1 + []\n'), ('add-Int-Float', 'a : Int\na = 1\nb : Float\nb = 2\nx = a + b\n'), ('add-Float-Int', 'a : Float\na = 1\nb : Int\nb = 2\nx = a + b\n'), ('add-both', 'x = "bad" + []\n'), ('multiply-left-bool', 'x = True * 1\n'), ('multiply-left-string', 'x = "bad" * 1\n'), ('multiply-left-list', 'x = [] * 1\n'), ('multiply-right-bool', 'x = 1 * True\n'), ('multiply-right-string', 'x = 1 * "bad"\n'), ('multiply-right-list', 'x = 1 * []\n'), ('multiply-Int-Float', 'a : Int\na = 1\nb : Float\nb = 2\nx = a * b\n'), ('multiply-Float-Int', 'a : Float\na = 1\nb : Int\nb = 2\nx = a * b\n'), ('multiply-both', 'x = "bad" * []\n'), ('add-list-alias', 'type alias Items = List Int\na : Items\na = []\nx = a + 1\n'), ('add-string-alias', 'type alias Text = String\na : Text\na = "hello"\nx = a + 1\n'), ('multiply-list-alias', 'type alias Items = List Int\na : Items\na = []\nx = a * 2\n'), ('add-string-multiline', 'x =\n    1\n        + "bad"\n'), ('multiply-nested', 'x = 1 * (2 + "bad")\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='arithmetic-parity-') as temp:
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
