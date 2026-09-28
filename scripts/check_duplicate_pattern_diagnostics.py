#!/usr/bin/env python3
"""Compare full duplicate-pattern diagnostics against Elm 0.19.1 using isolated caches."""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
cases=[
 ('function','f x x = 42\n'),
 ('nested-tuple','f x (x, y) = 42\n'),
 ('tuple','f (x, x) = 42\n'),
 ('record','f { x, x } = 42\n'),
 ('alias','f (x as x) = 42\n'),
 ('lambda','f = \\x x -> 42\n'),
 ('case','f pair =\n    case pair of\n        (x, x) -> 42\n'),
 ('local-function','f =\n    let\n        g x x = 42\n    in\n    g 1 2\n'),
 ('local-destructure','f =\n    let\n        (x, x) = (1, 2)\n    in\n    x\n'),
 ('let-definitions','f =\n    let\n        x = 1\n        x = 2\n    in\n    x\n'),
 ('multiple-functions','foo x x = 42\nbar x (x, y) = 42\n'),
 ('scope-after-failure','foo x x = 42\nbar x = (\\y y -> 42) x x\n'),
 ('valid-between-errors','foo x x = 42\nvalid x = x\nbar x x = 42\n'),
 ('multiline-lambda','f =\n    \\x\n     x -> 42\n'),
 ('unicode','f café café = 42\n'),
]

results=[]
with tempfile.TemporaryDirectory(prefix='duplicate-pattern-parity-') as temp:
 root=Path(temp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for name,version in [('elm/core','1.0.5'),('elm/json','1.1.3')]:shutil.copytree(original/name/version,cache/name/version)
 (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}));(root/'src').mkdir()
 env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
 for label,source in cases:
  (root/'src/Shadow.elm').write_text('module Shadow exposing (..)\n'+source)
  runs=[];terminal_runs=[]
  for binary in [a.elm,a.rust]:
   run=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=/dev/null','--report=json'],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   try:report=json.loads(run.stderr)
   except ValueError:report={'unexpected_stderr':run.stderr}
   runs.append({'code':run.returncode,'report':report})
   terminal=subprocess.run([str(binary.resolve()),'make','src/Shadow.elm','--output=/dev/null'],cwd=root,env=env,text=True,capture_output=True,timeout=30)
   terminal_runs.append({'code':terminal.returncode,'stderr':re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', terminal.stderr)})
  expected=runs[0]['report'].get('errors',[{}])[0].get('problems',[{}])[0].get('title')=='NAME CLASH'
  passed=expected and runs[0]==runs[1] and runs[0]['code']==1 and terminal_runs[0]==terminal_runs[1]
  results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1],'terminal_official':terminal_runs[0],'terminal_rust':terminal_runs[1]});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
