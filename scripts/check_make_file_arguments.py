#!/usr/bin/env python3
"""Compare make positional file validation and hidden output filenames."""
import argparse,hashlib,json,os,pty,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ('elm','rust','report'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
cases=[]
for name in ['bad.txt','','Main.ELM','src/Main','foo.elm/','écran.txt']:
 for prefix in [[],['A.elm'],['--report=json']]:cases.append([*prefix,name])
cases += [['bad.txt','--wat'],['bad.txt','--output=bad.txt'],['first.txt','second.txt'],['.elm'],['src/.elm'],['./.elm'],['A.elm','B.elm']]
for flag in ['--output','--docs']:
 for value in (['.js','./.js','dir/.js','.html','dir/.html'] if flag=='--output' else ['.json','./.json','dir/.json']):cases.append([flag+'='+value])
with tempfile.TemporaryDirectory(prefix='make-files-') as directory:
 for args,tty in [(args,tty) for tty in [False,True] for args in cases]:
  runs=[]
  for binary in (a.elm,a.rust):
   if tty:
    master,slave=pty.openpty()
    try:
     proc=subprocess.run([str(binary.resolve()),'make',*args],cwd=directory,env={**os.environ,'GHCRTS':'-N1'},stdout=subprocess.PIPE,stderr=slave,timeout=10)
    finally:os.close(slave)
    stderr=b''
    try:
     while chunk:=os.read(master,65536):stderr+=chunk
    except OSError:pass
    finally:os.close(master)
   else:
    proc=subprocess.run([str(binary.resolve()),'make',*args],cwd=directory,env={**os.environ,'GHCRTS':'-N1'},capture_output=True,timeout=10)
    stderr=proc.stderr
   runs.append({'code':proc.returncode,'stdout':proc.stdout.decode(),'stderr':stderr.decode()})
  passed=runs[0]==runs[1]
  results.append({'args':args,'tty':tty,'passed':passed,'official':runs[0],'rust':runs[1]})
  if not passed:print('FAIL',args,repr(runs[0]['stderr']),repr(runs[1]['stderr']))
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n');print(sum(r['passed'] for r in results),'/',len(results))
raise SystemExit(not report['passed'])
