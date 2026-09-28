#!/usr/bin/env python3
"""Compare invalid init/install invocations with and without a cached registry."""
import argparse,hashlib,json,os,pty,subprocess,tempfile,shutil
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ('elm','rust','report'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
cases=[]
for args in [['extra'],['a','b'],[''],['--wat'],['a','--report=json'],['--yes']]:cases.append(['init',*args])
for args in [['bad'],['elm/HTTP'],['elm/http','extra'],['bad','extra'],['--wat'],['bad','--report=json'],['elm//http'],[''],['/']]:cases.append(['install',*args])
with tempfile.TemporaryDirectory(prefix='global-cli-') as directory:
 for args,tty,cached in [(args,tty,cached) for cached in [False,True] for tty in [False,True] for args in cases]:
  runs=[]
  for index,binary in enumerate((a.elm,a.rust)):
   home=Path(directory)/f'home-{index}-{cached}'
   if cached and not home.exists():
    (home/'0.19.1/packages').mkdir(parents=True)
    original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages/registry.dat'
    shutil.copy2(original,home/'0.19.1/packages/registry.dat')
   if tty:
    master,slave=pty.openpty()
    try:
     proc=subprocess.run([str(binary.resolve()),*args],cwd=directory,env={**os.environ,'GHCRTS':'-N1','ELM_HOME':str(home)},stdout=subprocess.PIPE,stderr=slave,timeout=10)
    finally:os.close(slave)
    stderr=b''
    try:
     while chunk:=os.read(master,65536):stderr+=chunk
    except OSError:pass
    finally:os.close(master)
   else:
    proc=subprocess.run([str(binary.resolve()),*args],cwd=directory,env={**os.environ,'GHCRTS':'-N1','ELM_HOME':str(home)},capture_output=True,timeout=10)
    stderr=proc.stderr
   runs.append({'code':proc.returncode,'stdout':proc.stdout.decode(),'stderr':stderr.decode()})
  passed=runs[0]==runs[1]
  results.append({'args':args,'tty':tty,'cached_registry':cached,'passed':passed,'official':runs[0],'rust':runs[1]})
  if not passed:print('FAIL',args,repr(runs[0]['stderr']),repr(runs[1]['stderr']))
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n');print(sum(r['passed'] for r in results),'/',len(results))
raise SystemExit(not report['passed'])
