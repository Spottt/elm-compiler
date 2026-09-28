#!/usr/bin/env python3
"""Compare full declaration-type-order diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[('alphabetic', 'a : Int\na = "a"\nb : Int\nb = "b"\n'), ('reverse-source', 'b : Int\nb = "b"\na : Int\na = "a"\n'), ('three', 'z : Int\nz = "z"\na : Int\na = "a"\nm : Int\nm = "m"\n'), ('functions', 'f n = [1,"f"]\ng n = [1,"g"]\n'), ('constructor-pair', 'type Box = Box Int\nf (Box True) = 1\ng (Box False) = 1\n'), ('dependency', 'f n = [1,"f"]\ng n = let ignored = f n in [1,"g"]\n'), ('reverse-dependency', 'g n = [1,"g"]\nf n = let ignored = g n in [1,"f"]\n'), ('local', 'x =\n    let\n        a : Int\n        a = "a"\n        b : Int\n        b = "b"\n    in\n    (a,b)\n'), ('local-reverse', 'x =\n    let\n        b : Int\n        b = "b"\n        a : Int\n        a = "a"\n    in\n    (a,b)\n'), ('body-and-local', 'x : Int\nx =\n    let\n        a : Int\n        a = "a"\n    in\n    "body"\n'), ('multiple-per-definition', 'a : Int\na = if True then "a" else "aa"\nb : Int\nb = if True then "b" else "bb"\n'), ('recursive-annotated', 'f : Int -> Int\nf n = if True then g n else "f"\ng : Int -> Int\ng n = if True then f n else "g"\n'), ('recursive-reverse-source', 'g : Int -> Int\ng n = if True then f n else "g"\nf : Int -> Int\nf n = if True then g n else "f"\n'), ('recursive-three', 'c : Int -> Int\nc n = if True then a n else "c"\na : Int -> Int\na n = if True then b n else "a"\nb : Int -> Int\nb n = if True then c n else "b"\n'), ('local-recursive', 'x =\n    let\n        f : Int -> Int\n        f n = if True then g n else "f"\n        g : Int -> Int\n        g n = if True then f n else "g"\n    in\n    f 1\n'), ('local-destructuring', 'x =\n    let\n        (a,b) = (1,2)\n        c : Int\n        c = "c"\n        d : Int\n        d = "d"\n    in\n    (a,c,d)\n')]

results=[]
with tempfile.TemporaryDirectory(prefix='declaration-type-order-parity-') as temp:
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
