#!/usr/bin/env python3
"""Reject an invalid unused exposed dependency before building docs in diff and bump."""
import argparse,hashlib,json,os,shutil,tempfile
from pathlib import Path
from diff_test_process import run_command
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
with tempfile.TemporaryDirectory(prefix='package-docs-core-only-') as temp:
 root=Path(temp);original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 for command in ['diff','bump']:
  runs=[]
  for slot,binary in enumerate([a.elm,a.rust]):
   project=root/f'{command}-{slot}';(project/'src').mkdir(parents=True)
   (project/'elm.json').write_text(json.dumps({'type':'package','name':'elm/time','summary':'Core-only documentation regression','license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Example'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}))
   (project/'src/Example.elm').write_text('module Example exposing (identity)\n{-| Example\n@docs identity\n-}\n{-| Identity -}\nidentity : a -> a\nidentity x = x\n')
   home=project/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True);shutil.copy2(original/'registry.dat',cache/'registry.dat');shutil.copytree(original/'elm/core/1.0.5',cache/'elm/core/1.0.5')
   for artifact in (cache/'elm/core/1.0.5').glob('*.dat'):artifact.unlink()
   invalid=cache/'elm/core/1.0.5/src/Process.elm';invalid.write_text(invalid.read_text()+'\nbroken = True + 1\n')
   published=cache/'elm/time/1.0.0';published.mkdir(parents=True);(published/'docs.json').write_text(json.dumps([{'name':'Example','comment':'','unions':[],'aliases':[],'binops':[],'values':[{'name':'identity','comment':'','type':'a -> a'}]}]))
   env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'}
   before=(project/'elm.json').read_bytes()
   result=run_command([str(binary.resolve()),command],project,env,False,'stderr','n\n' if command=='bump' else '')
   result['manifest_unchanged']=(project/'elm.json').read_bytes()==before;runs.append(result)
  passed=runs[0]==runs[1] and runs[0]['code']==1 and 'PROBLEM BUILDING DEPENDENCIES' in runs[0]['stderr'] and runs[0]['manifest_unchanged'];results.append({'case':command,'passed':passed,'official':runs[0],'rust':runs[1]});print(command,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'scope':__doc__,'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
