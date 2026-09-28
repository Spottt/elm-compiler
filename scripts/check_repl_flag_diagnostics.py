#!/usr/bin/env python3
"""Compare REPL argument diagnostics with exact terminal colors and plain output."""
import argparse,hashlib,json,os,pty,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ('elm','rust','report'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
cases=[['--bogus'],['--interprter=x'],['--no-cloors'],['--interpreter'],['--interpreter','--no-colors'],['--no-colors=yes'],['--no-colors='],['--no-colors','--no-colors'],['--interpreter=node','--interpreter=nodejs'],['foo'],['foo','bar'],['foo','--bad'],['--no-colors=yes','--interpreter'],['--INTERPRETER=x'],['--🐢'],[''],['--report=json']]
with tempfile.TemporaryDirectory(prefix='repl-flags-') as directory:
 for args,tty in [(args,tty) for tty in [False,True] for args in cases]:
  runs=[]
  for binary in (a.elm,a.rust):
   if tty:
    master,slave=pty.openpty()
    try:
     proc=subprocess.run([str(binary.resolve()),'repl',*args],cwd=directory,env={**os.environ,'GHCRTS':'-N1'},stdout=subprocess.PIPE,stderr=slave,timeout=10)
    finally:os.close(slave)
    stderr=b''
    try:
     while chunk:=os.read(master,65536):stderr+=chunk
    except OSError:pass
    finally:os.close(master)
   else:
    proc=subprocess.run([str(binary.resolve()),'repl',*args],cwd=directory,env={**os.environ,'GHCRTS':'-N1'},capture_output=True,timeout=10)
    stderr=proc.stderr
   runs.append({'code':proc.returncode,'stdout':proc.stdout.decode(),'stderr':stderr.decode()})
  passed=runs[0]==runs[1]
  results.append({'args':args,'tty':tty,'passed':passed,'official':runs[0],'rust':runs[1]})
  if not passed:print('FAIL',args,repr(runs[0]['stderr']),repr(runs[1]['stderr']))
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n');print(sum(r['passed'] for r in results),'/',len(results))
raise SystemExit(not report['passed'])
