#!/usr/bin/env python3
"""Check diff registry-refresh failures and diagnostic envelopes offline.

Only the HTTP library's exception-detail line is normalized: reqwest and
Haskell http-client report different implementation details. All other output,
exit status and preservation of registry.dat must match Elm 0.19.1.
"""
import argparse,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ['elm','rust','report']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
with tempfile.TemporaryDirectory(prefix='elm-diff-offline-') as temp:
 root=Path(temp)
 for cached in [False,True]:
  for malformed in [False,True]:
   runs=[]
   for slot,binary in enumerate([a.elm,a.rust]):
    project=root/f'{cached}-{malformed}-{slot}';project.mkdir();home=project/'home';registry=home/'0.19.1/packages/registry.dat'
    if malformed:(project/'elm.json').write_text('broken json')
    if cached:
     registry.parent.mkdir(parents=True)
     original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages/registry.dat';shutil.copy2(original,registry)
    before=registry.read_bytes() if registry.exists() else None
    env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1','NO_PROXY':'','no_proxy':''}
    for key in ['http_proxy','https_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','all_proxy']:env[key]='http://127.0.0.1:1'
    run=subprocess.run([str(binary.resolve()),'diff'],cwd=project,env=env,text=True,capture_output=True,timeout=45)
    stderr,count=re.subn(r'(?<=error message:\n\n)    [^\n]+','    <transport exception>',run.stderr)
    assert count==1,run.stderr
    after=registry.read_bytes() if registry.exists() else None
    runs.append({'code':run.returncode,'stdout':run.stdout,'stderr':stderr,'actual_stderr':run.stderr,'cache_preserved':before==after})
   comparable=lambda run:{k:v for k,v in run.items() if k!='actual_stderr'}
   passed=comparable(runs[0])==comparable(runs[1]) and runs[0]['code']==1 and all(r['cache_preserved'] for r in runs)
   results.append({'cached':cached,'malformed_project':malformed,'passed':passed,'official':runs[0],'rust':runs[1]});print(cached,malformed,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha,'Compiler changed during validation'
r={'scope':__doc__,'cases':len(results),'compiler_sha256':sha,'passed':all(x['passed'] for x in results),'results':results};a.report.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit(not r['passed'])
