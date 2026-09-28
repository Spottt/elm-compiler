#!/usr/bin/env python3
"""Compare import suggestions after prior builds, edits, deletion and manifest changes."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();binaries=[a.elm.resolve(),a.rust.resolve()];hashes=[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries];rows=[]
with tempfile.TemporaryDirectory(prefix='elm-import-history-') as td:
 root=Path(td);home=root/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for pkg in ['elm/core/1.0.5','elm/json/1.1.3']:shutil.copytree(original/pkg,cache/pkg)
 env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'}
 for label in ['cold','warm','deleted','manifest-change','manifest-touch','failed-warm','partial-failure']:
  runs=[]
  for index,binary in enumerate(binaries):
   project=root/(label+str(index));project.mkdir();(project/'extra').mkdir()
   manifest=json.loads((Path(__file__).resolve().parents[1]/'tests/programs/worker/elm.json').read_text())
   (project/'elm.json').write_text(json.dumps(manifest))
   (project/'Widget.elm').write_text('module Widget exposing (value)\nvalue = 1\n' if label!='failed-warm' else 'module Widget exposing (value)\nvalue : Int\nvalue = "bad"\n')
   (project/'Main.elm').write_text('module Main exposing (..)\nimport Widget\nx = 1\n')
   if label=='partial-failure':
    (project/'Broken.elm').write_text('module Broken exposing (value)\nvalue : Int\nvalue = "bad"\n')
    (project/'Main.elm').write_text('module Main exposing (..)\nimport Widget\nimport Broken\nx = 1\n')
   command=[str(binary),'make','Main.elm','--output=/dev/null','--report=json']
   if label!='cold':
    warm=subprocess.run(command,cwd=project,env=env,text=True,capture_output=True,timeout=30)
    assert warm.returncode==(1 if label in ['failed-warm','partial-failure'] else 0),(label,index,warm.stderr)
   if label=='manifest-touch':
    stat=(project/'elm.json').stat();os.utime(project/'elm.json',ns=(stat.st_atime_ns,stat.st_mtime_ns+2_000_000_000))
   if label=='deleted':(project/'Widget.elm').unlink()
   if label=='manifest-change':
    manifest['source-directories'].append('extra');(project/'elm.json').write_text(json.dumps(manifest))
   (project/'Main.elm').write_text('module Main exposing (..)\nimport Widgte\nx = 1\n')
   r=subprocess.run(command,cwd=project,env=env,text=True,capture_output=True,timeout=30)
   report=json.loads(r.stderr)
   for error in report.get('errors',[]):error['path']=str(Path(error['path']).relative_to(project))
   runs.append({'code':r.returncode,'report':report})
  rows.append({'case':label,'passed':runs[0]==runs[1] and runs[0]['code']==1,'elm':runs[0],'rust':runs[1]})
 # Keep Main unchanged while another entry point teaches the compiler a name.
 # Rust must invalidate any cached diagnostic, not only its module/type caches.
 runs=[]
 for index,binary in enumerate(binaries):
  project=root/('cached-diagnostic'+str(index));project.mkdir()
  manifest=json.loads((Path(__file__).resolve().parents[1]/'tests/programs/worker/elm.json').read_text())
  (project/'elm.json').write_text(json.dumps(manifest))
  worker='\nmain = Platform.worker { init = \\() -> ( (), Cmd.none ), update = \\_ model -> ( model, Cmd.none ), subscriptions = \\_ -> Sub.none }\n'
  (project/'Main.elm').write_text('module Main exposing (main)\nimport Widgte\n'+worker)
  (project/'Helper.elm').write_text('module Helper exposing (main)\nimport Widget\n'+worker)
  (project/'Widget.elm').write_text('module Widget exposing (value)\nvalue = 1\n')
  def compile_entry(entry):
   command=[str(binary),'make',entry+'.elm','--output='+str(project/(entry+'.js')),'--report=json']
   if index==1:command.append('--incremental')
   return subprocess.run(command,cwd=project,env=env,text=True,capture_output=True,timeout=30)
  first=compile_entry('Main');assert first.returncode==1,first.stderr
  helper=compile_entry('Helper');assert helper.returncode==0,helper.stderr
  last=compile_entry('Main');assert last.returncode==1,last.stderr
  reports=[]
  for result in [first,last]:
   report=json.loads(result.stderr)
   for error in report.get('errors',[]):error['path']=str(Path(error['path']).relative_to(project))
   reports.append(report)
  runs.append(reports)
 rows.append({'case':'cached-diagnostic-new-history','passed':runs[0]==runs[1] and runs[0][0]!=runs[0][1],'elm':runs[0],'rust':runs[1]})
assert hashes==[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'scope':__doc__,'binary_sha256':hashes,'passed':all(r['passed'] for r in rows),'results':rows},indent=2)+'\n')
print(json.dumps({'checks':len(rows),'failures':[r['case'] for r in rows if not r['passed']]}))
raise SystemExit(not all(r['passed'] for r in rows))
