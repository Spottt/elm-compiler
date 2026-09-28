#!/usr/bin/env python3
"""Compare make value-flag diagnostics before project loading, including JSON override."""
import argparse, hashlib, json, os, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ('elm','rust','report'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
with tempfile.TemporaryDirectory(prefix='make-arguments-') as directory:
 for flag,bad in [('--output','bad.txt'),('--docs','bad.txt'),('--report','xml')]:
  for args in [[flag],[flag+'='],[flag,bad],[flag+'='+bad],[flag,'--debug'],[flag,'-x'],[flag+'=--debug']]:
   for prefix in dict.fromkeys([(),('--report=json',),({'--output':'--output=ok.js','--docs':'--docs=ok.json','--report':'--report=json'}[flag],)]):
    arguments=['make',*prefix,*args]
    runs=[]
    for binary in [a.elm,a.rust]:
     run=subprocess.run([str(binary.resolve()),*arguments],cwd=directory,env={**os.environ,'GHCRTS':'-N1'},capture_output=True,timeout=10)
     runs.append({'code':run.returncode,'stdout':run.stdout.decode(),'stderr':run.stderr.decode()})
    passed=runs[0]==runs[1]
    results.append({'args':arguments,'passed':passed,'official':runs[0],'rust':runs[1]})
    if not passed:print('FAIL',arguments,repr(runs[0]['stderr']),repr(runs[1]['stderr']))
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n');print(sum(r['passed'] for r in results),'/',len(results))
raise SystemExit(not report['passed'])
