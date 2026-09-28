#!/usr/bin/env python3
"""Compare corrupt documentation cache handling with Elm 0.19.1.

Real registry refresh; isolated caches. Compares complete non-TTY output,
exit status and which cached documents survive the failed comparison.
"""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ['elm','rust','report']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
bodies=['','not json','{}','[{}]',json.dumps([{'name':'Example','comment':'','unions':[],'aliases':[],'binops':[],'values':[{'name':'broken','comment':'','type':'List ('}]}])]
results=[]
with tempfile.TemporaryDirectory(prefix='elm-diff-docs-') as tmp:
 root=Path(tmp)
 for index,body in enumerate(bodies):
  for corrupt_version in ['1.0.4','1.0.5']:
   runs=[]
   for slot,binary in enumerate([a.elm,a.rust]):
    home=root/f'{index}-{corrupt_version}-{slot}';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
    original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat',cache/'registry.dat')
    paths=[]
    for version in ['1.0.4','1.0.5']:
     path=cache/f'elm/core/{version}/docs.json';path.parent.mkdir(parents=True);path.write_text(body if version==corrupt_version else '[]');paths.append(path)
    env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'}
    run=subprocess.run([str(binary.resolve()),'diff','elm/core','1.0.5','1.0.4'],cwd=root,env=env,capture_output=True,text=True,timeout=60)
    runs.append({'code':run.returncode,'stdout':run.stdout,'stderr':run.stderr,'cache_survives':[path.exists() for path in paths]})
   passed=runs[0]==runs[1] and runs[0]['code']==1 and runs[0]['cache_survives']==[corrupt_version!='1.0.4',corrupt_version!='1.0.5']
   results.append({'body':body,'corrupt_version':corrupt_version,'passed':passed,'official':runs[0],'rust':runs[1]});print(index,corrupt_version,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha,'Compiler changed during validation'
r={'scope':__doc__,'cases':len(results),'compiler_sha256':sha,'passed':all(x['passed'] for x in results),'results':results};a.report.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit(not r['passed'])
