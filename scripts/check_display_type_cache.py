#!/usr/bin/env python3
import os,sys,json,shutil,subprocess,tempfile,hashlib
from pathlib import Path
import argparse
parser = argparse.ArgumentParser(description="Compare cached type diagnostics, source edits and corrupted artifacts with Elm 0.19.1.")
parser.add_argument('--rust', type=Path, required=True)
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
binary=args.rust.resolve();oracle=args.elm.resolve();report=args.report.resolve()
report.parent.mkdir(parents=True, exist_ok=True)
home=Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))
expected_hashes={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [binary,oracle]}
manifest={'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
rows=[]
with tempfile.TemporaryDirectory(prefix='display-cache-',dir=report.parent) as temporary:
 root=Path(temporary);env=dict(os.environ,ELM_HOME=str(root/'home'),GHCRTS='-N1')
 packages=root/'home/0.19.1/packages';packages.mkdir(parents=True);shutil.copy2(home/'0.19.1/packages/registry.dat',packages/'registry.dat')
 for package in ['elm/core/1.0.5','elm/json/1.1.3']:shutil.copytree(home/'0.19.1/packages'/package,packages/package)
 for label in ['elm','rust']:
  p=root/label;(p/'src').mkdir(parents=True);(p/'elm.json').write_text(json.dumps(manifest))
 for case,(alias,bad) in enumerate([
  ('Box','wrong : Int\nwrong = Helper.box "text"'),
  ('Box','wrong : Bool\nwrong = Helper.box "text"'),
  ('Parcel','wrong : Int\nwrong = Helper.box "text"'),
  ('Parcel','wrong : Helper.Parcel String\nwrong = Helper.box 123'),
  ('Parcel','wrong : a -> b\nwrong = Helper.identity'),
  ('Parcel','wrong : Helper.Parcel Int\nwrong = Helper.box 123'),
 ]):
  outputs={}
  for label,compiler in [('elm',oracle),('rust',binary)]:
   p=root/label
   (p/'src/Helper.elm').write_text(f'module Helper exposing (..)\ntype alias {alias} item = {{ item : item }}\nbox : item -> {alias} item\nbox value = {{ item = value }}\nidentity : value -> value\nidentity value = value\n')
   (p/'src/Main.elm').write_text('module Main exposing (main)\nimport Helper\nmain : Program () () Never\nmain = Platform.worker { init = \\() -> ((), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }\n'+bad+'\n')
   for fmt in ['json','plain']:
    r=subprocess.run([str(compiler),'make','src/Main.elm','--output=out.js',*(['--incremental'] if label=='rust' else []),*(['--report=json'] if fmt=='json' else [])],cwd=p,env=env,capture_output=True,text=True,timeout=60)
    stderr=r.stderr.replace(str(p),'<project>');outputs[label,fmt]=(r.returncode,json.loads(stderr) if fmt=='json' and stderr else stderr)
  for fmt in ['json','plain']:
   assert outputs['elm',fmt]==outputs['rust',fmt],(case,fmt,outputs)
   rows.append({'case':case,'format':fmt,'exit':outputs['rust',fmt][0],'equal':True})
  if case==1:
   files=list((root/'rust/elm-stuff/planexpo-rust/types-v1/diagnostics-v1').glob('*.cache'));assert files,'diagnostic cache was not populated'
   # A damaged cached dependency must be safely rebuilt on the next type error.
   for f in files:f.write_bytes(b'truncated')
 report.write_text(json.dumps({'compiler':str(binary),'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'rows':rows},indent=2)+'\n')
 print(f'{len(rows)} comparisons passed')

assert expected_hashes=={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [binary,oracle]}, "Compiler changed during validation"
