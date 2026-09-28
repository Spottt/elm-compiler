#!/usr/bin/env python3
"""Compare optimized Debug rejection, ordering and /dev/null behavior with Elm."""
import argparse, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
from diff_test_process import run_command
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest()
worker='main : Program () () Never\nmain = Platform.worker { init = \\_ -> ((), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }\n'
cases=[('log','value = Debug.log "hi" 1\n',{},False),('todo','value = Debug.todo "todo"\n',{},False),('toString','value = Debug.toString 1\n',{},False),('null-output','value = Debug.todo "todo"\n',{},True),('unused-import','value = 1\n',{},False),('type-before-debug','value : Int\nvalue = Debug.log "hi" True\n',{},False),('multiple-modules','value = Debug.log "main" 1\n',{'Alpha':'value = Debug.log "a" 1\n','Zebra':'value = Debug.todo "z"\n'},False)]
cases += [
 ('alias-debug-call', 'value : Int\nvalue = Trace.log "hi" True\n', {}, False),
 ('exposed-debug-call', 'value : Int\nvalue = log "hi" True\n', {}, False),
 ('local-log-call', 'log label x = x\nvalue : Int\nvalue = log "hi" True\n', {}, False),
 ('foreign-log-call', 'value : Int\nvalue = Helper.log "hi" True\n', {'Helper':'log label x = x\n'}, False),
]
cases += [(label+'-development',body,modules,null) for label,body,modules,null in cases if 'call' in label or label=='type-before-debug']
results=[]
with tempfile.TemporaryDirectory(prefix='debug-remnants-') as temp:
 root=Path(temp);original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 cache=root/'home/0.19.1/packages';cache.mkdir(parents=True);shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for name in ['elm/core/1.0.5','elm/json/1.1.3']:shutil.copytree(original/name,cache/name)
 env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
 for label,body,modules,null in cases:
  project=root/label;project.mkdir()
  (project/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
  for name,text in modules.items():(project/(name+'.elm')).write_text('module '+name+' exposing (..)\nimport Debug\n'+text)
  (project/'Main.elm').write_text('module Main exposing (..)\n'+('import Debug as Trace\n' if label.startswith('alias-debug-call') else 'import Debug exposing (log)\n' if label.startswith('exposed-debug-call') else 'import Debug\n')+''.join('import '+n+'\n' for n in modules)+worker+body)
  outputs=[]
  for binary in [a.elm,a.rust]:
   modes=[]
   for kind in ['json','plain','ansi']:
    command=[str(binary.resolve()),'make','Main.elm',* ([] if label.endswith('-development') else ['--optimize']),'--output='+('/dev/null' if null else 'result.js')]
    if kind=='json':command+=['--report=json']
    (project/'result.js').write_text('previous bundle')
    run=run_command(command,project,env,kind=='ansi')
    value=json.loads(run['stderr']) if kind=='json' and run['stderr'].strip() else run['stderr']
    modes.append({'mode':kind,'code':run['code'],'stderr':value})
    if run['code']!=0:assert (project/'result.js').read_text()=='previous bundle'
   outputs.append(modes)
  passed=outputs[0]==outputs[1]
  results.append({'case':label,'passed':passed,'official':outputs[0],'rust':outputs[1]});print(label,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
report={'compiler_sha256':sha,'cases':len(results),'passed':all(x['passed'] for x in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
