#!/usr/bin/env python3
"""Compare declaration-order parsing, duplicate flags and spaced values."""
import argparse,hashlib,itertools,json,os,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ('elm','rust','report'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
invalid=['--debug=x','--optimize=x','--output=x','--report=x','--docs=x']
cases=[]
for left,right in itertools.permutations(invalid,2):
 for spaced in [False,True]:
  args=[]
  for flag in [left,right]:args.extend(flag.split('=',1) if spaced and flag.split('=')[0] in ['--output','--report','--docs'] else [flag])
  cases.append(args)
for first in ['--debug','--optimize','--output=a.js','--report=json','--docs=a.json']:
 for invalid_other in invalid:
  cases.append([first,first,invalid_other])
cases += [['--output','--debug','a.js'],['--docs','--debug','a.json'],['--report','--optimize','json'],['--wat','--output','a.js'],['--debug','Main.elm','--output','out.js']]
cases=[list(args) for args in dict.fromkeys(map(tuple,cases))]
with tempfile.TemporaryDirectory(prefix='flag-priority-') as directory:
 for args in cases:
  runs=[]
  for binary in (a.elm,a.rust):
   proc=subprocess.run([str(binary.resolve()),'make',*args],cwd=directory,env={**os.environ,'GHCRTS':'-N1'},capture_output=True,timeout=10)
   runs.append({'code':proc.returncode,'stdout':proc.stdout.decode(),'stderr':proc.stderr.decode()})
  comparison='text'
  try:
   reports=[json.loads(run['stderr']) for run in runs]
  except ValueError:
   passed=runs[0]==runs[1]
  else:
   comparison='decoded-json'
   passed=reports[0]==reports[1] and runs[0]['code']==runs[1]['code'] and runs[0]['stdout']==runs[1]['stdout']
  results.append({'args':args,'comparison':comparison,'passed':passed,'official':runs[0],'rust':runs[1]})
  if not passed:print('FAIL',args,repr(runs[0]['stderr']),repr(runs[1]['stderr']))
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n');print(sum(r['passed'] for r in results),'/',len(results))
raise SystemExit(not report['passed'])
