#!/usr/bin/env python3
"""Compare constraint syntax, whitespace, operators and Word16 bounds with Elm."""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile

p=argparse.ArgumentParser(description=__doc__);p.add_argument('--elm',type=Path,required=True);p.add_argument('--report',type=Path);args=p.parse_args()
crate=Path(__file__).resolve().parents[1]
base={'type':'package','name':'author/fixture','summary':'Constraint grammar fixture.','license':'BSD-3-Clause','version':'1.0.0',
      'exposed-modules':['Main'],'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
cases=[]
for target,lower,upper in [('dependency','1.0.0','2.0.0'),('compiler','0.19.0','0.20.0')]:
 canonical=f'{lower} <= v < {upper}'
 variants=[]
 for lo in ['<','<=']:
  for hi in ['<','<=']:variants.append((f'ops-{lo}-{hi}',f'{lower} {lo} v {hi} {upper}',False))
 for index in range(4):
  for whitespace in ['', '  ', '\t', '\n', '\r', '\u00a0', '\u2003']:
   parts=[lower,'<=','v','<',upper];separators=[' ']*4;separators[index]=whitespace
   text=''.join(part+(separators[i] if i<4 else '') for i,part in enumerate(parts))
   variants.append((f'gap-{index}-{whitespace!r}',text,False))
 for text in [' '+canonical,canonical+' ',canonical.replace(' v ',' V '),canonical.replace('<=','>='),
              canonical.replace('<=','='),canonical.replace('<=','< ='),f'{lower} <= v <= {lower}',
              f'{upper} <= v < {lower}',canonical.replace(lower,'0'+lower,1)]:
  variants.append((text,text,False))
 variants.append(('escaped-space',canonical,True))
 for label,text,escaped in variants:cases.append((target,label,text,escaped))
cases.append(('dependency','wrapped-lower','65537.0.0 <= v < 2.0.0',False))
cases.append(('dependency','wrapped-upper','1.0.0 <= v < 65538.0.0',False))
results=[];failures=[]
with tempfile.TemporaryDirectory(prefix='elm-constraint-parity-') as temp:
 root=Path(temp)
 for index,(target,label,text,escaped) in enumerate(cases):
  project=root/str(index);(project/'src').mkdir(parents=True)
  (project/'src/Main.elm').write_text('module Main exposing (answer)\nanswer : Int\nanswer = 42\n')
  config=copy.deepcopy(base)
  if target=='dependency':config['dependencies']['elm/core']=text
  else:config['elm-version']=text
  source=json.dumps(config,ensure_ascii=False,indent=2)+'\n'
  if escaped:source=source.replace(json.dumps(text),json.dumps(text).replace(' ',r'\u0020',1))
  (project/'elm.json').write_text(source)
  row={'target':target,'case':label,'constraint':text,'escaped_space':escaped}
  for compiler,binary in [('official',args.elm.resolve()),('rust',crate/'target/release/planexpo-elm')]:
   run=subprocess.run([str(binary),'make','src/Main.elm','--output=/dev/null','--report=json'],cwd=project,env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=30)
   row[compiler]={'accepted':run.returncode==0,'stderr':run.stderr}
  equal=row['official']['accepted']==row['rust']['accepted']
  if not row['official']['accepted'] and not row['rust']['accepted']:
   a=json.loads(row['official']['stderr']);b=json.loads(row['rust']['stderr'])
   equal=equal and all(a.get(key)==b.get(key) for key in ['type','path','title'])
  if not equal:failures.append({'target':target,'case':label})
  results.append(row)
report={'cases':len(results),'failures':failures,'results':results}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}));raise SystemExit(bool(failures))
