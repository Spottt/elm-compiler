#!/usr/bin/env python3
"""Compare repeated named fields and valid repeated dependency entries with Elm."""
import argparse
import copy
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
app={'type':'application','elm-version':'0.19.1','source-directories':['src'],
     'dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},
     'test-dependencies':{'direct':{},'indirect':{}}}
pkg={'type':'package','name':'author/fixture','summary':'Duplicate field fixture.',
     'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
     'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
def raw_object(pairs):
    return '{\n'+',\n'.join('  '+json.dumps(key)+': '+value for key,value in pairs)+'\n}'
def duplicate(base,key,first,last):
    return raw_object([(k,json.dumps(v)) for k,v in base.items() if k!=key]+[(key,json.dumps(first)),(key,json.dumps(last))])
def replace_raw(base,key,value):
    return raw_object([(k,value if k==key else json.dumps(v)) for k,v in base.items()])
cases=[]
for label,base,key,good,bad in [
    ('app-version',app,'elm-version','0.19.1','0.18.0'),
    ('package-version',pkg,'elm-version','0.19.0 <= v < 0.20.0','0.20.0 <= v < 0.21.0'),
    ('app-type',app,'type','application','package'),
    ('package-type',pkg,'type','package','application'),
    ('directories',app,'source-directories',['src'],['absent']),
    ('license',pkg,'license','BSD-3-Clause','Unknown-License'),
    ('summary',pkg,'summary','Short','x'*80),
    ('dependency-field',app,'dependencies',app['dependencies'],{'direct':{'elm/core':'9.9.9','elm/json':'1.1.3'},'indirect':{}}),
]:
    cases.append((label+'-first-good',duplicate(base,key,good,bad),True))
    cases.append((label+'-first-bad',duplicate(base,key,bad,good),False))
for reverse in [False,True]:
    good=json.dumps(app['dependencies']['direct'])
    bad=json.dumps({'elm/core':'9.9.9','elm/json':'1.1.3'})
    sections=raw_object([('direct',bad if reverse else good),('direct',good if reverse else bad),('indirect','{}')])
    cases.append(('direct-section-'+str(reverse),replace_raw(app,'dependencies',sections),not reverse))
    pairs=[('elm/core',json.dumps('1.0.5' if reverse else '9.9.9')),('elm/core',json.dumps('9.9.9' if reverse else '1.0.5')),('elm/json',json.dumps('1.1.3'))]
    sections=raw_object([('direct',raw_object(pairs)),('indirect','{}')])
    cases.append(('dependency-entry-'+str(reverse),replace_raw(app,'dependencies',sections),not reverse))
for kind,base,valid,bad in [('app',app,'1.0.5','latest'),('package',pkg,'1.0.0 <= v < 2.0.0','1.0.0  <= v < 2.0.0')]:
    for section in ['dependencies','test-dependencies']:
        for placement in ['first','middle']:
            values=[bad,valid] if placement=='first' else [valid,bad,valid]
            entries=raw_object([('elm/core',json.dumps(value)) for value in values])
            if kind=='app':
                if section=='dependencies':
                    entries=entries[:-2]+',\n  "elm/json": "1.1.3"\n}'
                entries=raw_object([('direct',entries),('indirect','{}')])
            cases.append((kind+'-'+section+'-invalid-'+placement,replace_raw(base,section,entries),False))
sections=raw_object([('direct',json.dumps(app['dependencies']['direct'])),('direct','{"elm/core":"latest"}'),('indirect','{}')])
cases.append(('ignored-later-named-section',replace_raw(app,'dependencies',sections),True))
for kind,base,bad,valid in [('app',app,'latest','1.0.5'),('package',pkg,'latest','1.0.0 <= v < 2.0.0')]:
    for reverse in [False,True]:
        pairs=[('z/pkg',json.dumps(bad)),('broken',json.dumps(valid))]
        if reverse:pairs.reverse()
        entries=raw_object(pairs)
        if kind=='app':entries=raw_object([('direct',entries),('indirect','{}')])
        cases.append((kind+'-first-error-'+str(reverse),replace_raw(base,'dependencies',entries),False))
results,failures=[],[]
with tempfile.TemporaryDirectory(prefix='elm-duplicate-json-') as tmp:
    root=Path(tmp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat',cache/'registry.dat')
    for name in ['elm/core/1.0.5','elm/json/1.1.3']:
        shutil.copytree(original/name/'src',cache/name/'src')
        shutil.copyfile(original/name/'elm.json',cache/name/'elm.json')
    env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1 -A16m -c'}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:env[key]='http://127.0.0.1:9'
    env.update(no_proxy='',NO_PROXY='')
    for index,(name,source,expected) in enumerate(cases):
        project=root/str(index);(project/'src').mkdir(parents=True)
        (project/'src/Main.elm').write_text('module Main exposing (answer)\nanswer = 42\n')
        (project/'elm.json').write_text(source+'\n')
        row={'case':name,'expected':expected}
        for label,executable in [('official',args.elm.resolve()),('rust',binary)]:
            run=subprocess.run([str(executable),'make','src/Main.elm','--output=/dev/null','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=20)
            row[label]={'accepted':run.returncode==0,'stderr':run.stderr}
            if row[label]['accepted']!=expected:failures.append(name+'/'+label)
        if not expected:
            a=json.loads(row['official']['stderr']);b=json.loads(row['rust']['stderr'])
            if any(a[k]!=b[k] for k in ['type','title','path']):failures.append(name+'/diagnostic-category')
        results.append(row)
report={'cases':len(results),'failures':failures,'results':results,'compiler_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
