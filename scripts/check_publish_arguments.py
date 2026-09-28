#!/usr/bin/env python3
"""Compare publish help, argument precedence and missing-project diagnostics."""
import argparse,hashlib,json,os,tempfile
from pathlib import Path
from diff_test_process import run_command
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
with tempfile.TemporaryDirectory(prefix='publish-arguments-') as temp:
 for ansi in [False,True]:
  for args in [[],['--help'],['--help','--bad'],['thing','--help'],['--report=json'],['--bad'],['one'],['one','two'],['one','--bad'],['--','--help']]:
   runs=[run_command([str(binary.resolve()),'publish',*args],Path(temp),os.environ,ansi,'stderr','') for binary in [a.elm,a.rust]]
   passed=runs[0]==runs[1];results.append({'args':args,'ansi':ansi,'passed':passed,'official':runs[0],'rust':runs[1]});print(args,ansi,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'cases':len(results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
