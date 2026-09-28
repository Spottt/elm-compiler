#!/usr/bin/env python3
"""Compare all official null output spellings; no output file may be created."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
with tempfile.TemporaryDirectory(prefix='null-output-') as temp:
 root=Path(temp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for package in ['elm/core/1.0.5','elm/json/1.1.3']:shutil.copytree(original/package,cache/package)
 env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1 -A16m -c'}
 for output in ['/dev/null','NUL','$null']:
  for equal in [False,True]:
   for json_mode in [False,True]:
    project=root/f'case-{len(results)}';project.mkdir()
    (project/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    (project/'Main.elm').write_text('module Main exposing (..)\nx = 1\n')
    flags=['--output='+output] if equal else ['--output',output]
    if json_mode:flags.append('--report=json')
    runs=[]
    for compiler in [a.elm,a.rust]:
     run=subprocess.run([str(compiler.resolve()),'make','Main.elm',*flags],cwd=project,env=env,capture_output=True,timeout=60)
     runs.append({'code':run.returncode,'stdout':run.stdout.decode(),'stderr':run.stderr.decode(),'created_output':any((project/name).exists() for name in ['NUL','$null','elm.js','index.html'])})
    expected=runs[0]['stdout']
    if 'Compiling ...' in expected:expected=expected[expected.index('Compiling ...'):]
    actual=runs[1]['stdout']
    if 'Compiling ...' in actual:actual=actual[actual.index('Compiling ...'):]
    passed=all(r['code']==0 and not r['stderr'] and not r['created_output'] for r in runs) and expected==actual
    results.append({'output':output,'equals':equal,'json':json_mode,'passed':passed,'official':runs[0],'rust':runs[1]})
    print(output,equal,json_mode,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n')
raise SystemExit(not report['passed'])
