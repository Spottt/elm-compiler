#!/usr/bin/env python3
"""Compare repeated exposed-module groups in default and explicit package builds."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm',type=Path,required=True)
p.add_argument('--report',type=Path)
args=p.parse_args()
crate=Path(__file__).resolve().parents[1]
binary=crate/'target/release/planexpo-elm'
original=Path(os.environ.get('ELM_HOME',str(Path.home()/'.elm')))/'0.19.1/packages'
base={'type':'package','name':'author/fixture','summary':'Exposed groups fixture.',
      'license':'BSD-3-Clause','version':'1.0.0','elm-version':'0.19.0 <= v < 0.20.0',
      'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
cases=[
    ('valid', '{"Public":["Main"],"Public":["Other"]}', False, True, True),
    ('first-broken', '{"Public":["Other"],"Public":["Main"]}', True, False, True),
    ('last-broken', '{"Public":["Main"],"Public":["Other"]}', True, False, True),
    ('invalid-name-hidden', '{"Public":["bad"],"Public":["Main"]}', False, False, False),
    ('missing-module-hidden', '{"Public":["Absent"],"Public":["Main"]}', False, False, True),
    ('same-module-twice', '{"Public":["Main"],"Public":["Main"]}', False, True, True),
    ('invalid-name-distinct-group', '{"Z":["bad"],"A":["Main"]}', False, False, False),
]
results,failures=[],[]
with tempfile.TemporaryDirectory(prefix='elm-exposed-groups-') as tmp:
    root=Path(tmp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat',cache/'registry.dat')
    shutil.copytree(original/'elm/core/1.0.5/src',cache/'elm/core/1.0.5/src')
    shutil.copyfile(original/'elm/core/1.0.5/elm.json',cache/'elm/core/1.0.5/elm.json')
    env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1 -A16m -c'}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:env[key]='http://127.0.0.1:9'
    env.update(no_proxy='',NO_PROXY='')
    for index,(name,groups,broken,default_ok,explicit_ok) in enumerate(cases):
        for explicit in [False,True]:
            expected=explicit_ok if explicit else default_ok
            row={'case':name,'explicit_entry':explicit,'expected':expected}
            for label,executable in [('official',args.elm.resolve()),('rust',binary)]:
                project=root/str(index)/str(explicit)/label;(project/'src').mkdir(parents=True)
                (project/'src/Main.elm').write_text('module Main exposing (answer)\nanswer = 42\n')
                (project/'src/Other.elm').write_text('module Other exposing (answer)\nanswer = '+('unknownName' if broken else '43')+'\n')
                source=json.dumps(base,indent=2)
                (project/'elm.json').write_text(source[:-1]+',\n "exposed-modules":'+groups+'\n}\n')
                command=[str(executable),'make']+(['src/Main.elm'] if explicit else [])+['--output=/dev/null','--report=json']
                run=subprocess.run(command,cwd=project,env=env,capture_output=True,text=True,timeout=20)
                row[label]={'accepted':run.returncode==0,'stderr':run.stderr}
                if row[label]['accepted']!=expected:failures.append(f'{name}/{explicit}/{label}')
            results.append(row)
report={'cases':len(results),'failures':failures,'results':results,'compiler_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
