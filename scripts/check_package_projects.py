#!/usr/bin/env python3
"""Compare cached package projects with Elm 0.19.1, including default exposed roots."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
args = p.parse_args()
base = {'type':'package','name':'author/fixture','summary':'Package compiler compatibility fixture.',
        'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Library','Other'],
        'elm-version':'0.19.0 <= v < 0.20.0',
        'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
files = {'Library':'module Library exposing (value)\nimport Internal\nvalue = Internal.value + 1\n',
         'Other':'module Other exposing (value)\nimport Library\nvalue = Library.value + 1\n',
         'Internal':'module Internal exposing (value)\nvalue = 40\n',
         'Main':'module Main exposing (main)\nimport Html\nmain = Html.text "Package works"\n'}
cases = [
    ('explicit', {}, {}, ['src/Library.elm','--output=/dev/null'], True),
    ('application-unregistered-version', {'type':'application','elm-version':'0.19.1','source-directories':['src'],
     'dependencies':{'direct':{'elm/core':'9.9.9','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}, {}, ['src/Library.elm','--output=/dev/null'], False),
    ('package-unregistered-version', {'dependencies':{'elm/core':'9.0.0 <= v < 10.0.0'}}, {}, [], False),
    ('application-needs-json', {'type':'application','elm-version':'0.19.1','source-directories':['src'],
     'dependencies':{'direct':{'elm/core':'1.0.5'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}, {}, ['src/Library.elm','--output=/dev/null'], False),
    ('application-needs-core', {'type':'application','elm-version':'0.19.1','source-directories':['src'],
     'dependencies':{'direct':{'elm/json':'1.1.3'},'indirect':{'elm/core':'1.0.5'}},'test-dependencies':{'direct':{},'indirect':{}}}, {}, ['src/Library.elm','--output=/dev/null'], False),
    ('application-json-indirect', {'type':'application','elm-version':'0.19.1','source-directories':['src'],
     'dependencies':{'direct':{'elm/core':'1.0.5'},'indirect':{'elm/json':'1.1.3'}},'test-dependencies':{'direct':{},'indirect':{}}}, {}, ['src/Library.elm','--output=/dev/null'], True),
    ('exposed-default', {}, {}, [], True),
    ('exposed-categories', {'exposed-modules':{'Public':['Library','Other']}}, {}, [], True),
    ('unused-broken-module', {}, {'Main':'module Main exposing (..)\nbroken = missing\n'}, [], True),
    ('broken-exposed-module', {}, {'Other':'module Other exposing (..)\nbroken = missing\n'}, [], False),
    ('no-exposed-modules', {'exposed-modules':[]}, {}, [], False),
    ('no-exposed-explicit', {'exposed-modules':[]}, {}, ['src/Library.elm','--output=/dev/null'], True),
    ('wrong-elm-version', {'elm-version':'0.20.0 <= v < 0.21.0'}, {}, [], False),
    ('unsatisfiable', {'dependencies':{'elm/core':'2.0.0 <= v < 3.0.0'}}, {}, [], False),
    ('duplicate-test-dependency', {'test-dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'}}, {}, [], False),
    ('test-dependency-invisible', {'test-dependencies':{'elm/html':'1.0.0 <= v < 2.0.0'}},
     {'Library':'module Library exposing (value)\nimport Html\nvalue = Html.text "test"\n'}, [], False),
    ('default-ignores-output', {}, {}, ['--output=ignored.js'], True),
]
for mode in [[], ['--optimize']]:
    for output in ['out.js','out.html']:
        cases.append(('main-'+output+str(mode), {'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0','elm/html':'1.0.0 <= v < 2.0.0'}}, {}, ['src/Main.elm','--output='+output,*mode], True))
results = []
with tempfile.TemporaryDirectory(prefix='elm-package-parity-') as directory:
    root = Path(directory)
    home = root/'cache'
    original_home = Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
    cache = home/'0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copy(original_home/'registry.dat', cache/'registry.dat')
    for name, version in [('elm/core','1.0.5'),('elm/json','1.1.3'),('elm/html','1.0.0'),('elm/virtual-dom','1.0.3')]:
        shutil.copytree(original_home/name/version, cache/name/version)
    # A locally fabricated version must not override the official registry.
    shutil.copytree(cache/'elm/core/1.0.5', cache/'elm/core/9.9.9')
    fake = cache/'elm/core/9.9.9/elm.json'
    fake_config = json.loads(fake.read_text()); fake_config['version'] = '9.9.9'
    fake.write_text(json.dumps(fake_config))
    for label, updates, edits, arguments, expected in cases:
        row = {'case':label, 'arguments':arguments, 'expected':expected}
        for compiler, binary in [('official',args.elm.resolve()),('rust',crate/'target/release/planexpo-elm')]:
            project = root/str(len(results))/compiler
            (project/'src').mkdir(parents=True)
            config = copy.deepcopy(base); config.update(updates)
            (project/'elm.json').write_text(json.dumps(config))
            for name, source in {**files,**edits}.items():
                (project/'src'/(name+'.elm')).write_text(source)
            run = subprocess.run([str(binary),'make',*arguments,'--report=json'],cwd=project,
                                 env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'},capture_output=True,text=True,timeout=90)
            assert run.returncode in (0,1), (label, compiler, run.stderr)
            row[compiler] = {'accepted':run.returncode==0,'stderr':run.stderr if run.returncode else ''}
            if run.returncode == 0:
                assert not (project/'ignored.js').exists()
                if (project/'out.js').exists():
                    subprocess.run(['node','-e',"const e=require(process.argv[1]); if(typeof e.Elm?.Main?.init !== 'function') throw Error('missing init');",str(project/'out.js')],check=True,capture_output=True,text=True)
                if any(arg == '--output=out.html' for arg in arguments):
                    assert (project/'out.html').is_file()
        row['matches'] = row['official']['accepted'] == row['rust']['accepted'] == expected
        results.append(row)
    # Exercise actual upstream packages, not only a synthetic project.
    for package, version in [('elm/core','1.0.5'), ('elm/json','1.1.3')]:
        row = {'case':'upstream-'+package, 'arguments':[], 'expected':True}
        for compiler, binary in [('official',args.elm.resolve()),('rust',crate/'target/release/planexpo-elm')]:
            project = root/('upstream-'+package.replace('/','-'))/compiler
            shutil.copytree(cache/package/version, project)
            # Dependency cache artifacts belong to a different root.
            if (project/'elm-stuff').exists(): shutil.rmtree(project/'elm-stuff')
            run = subprocess.run([str(binary),'make','--report=json'],cwd=project,
                                 env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1'},capture_output=True,text=True,timeout=90)
            assert run.returncode in (0,1), (package, compiler, run.stderr)
            row[compiler] = {'accepted':run.returncode==0,'stderr':run.stderr if run.returncode else ''}
        row['matches'] = row['official']['accepted'] and row['rust']['accepted']
        results.append(row)
report = {'cases':len(results),'failures':[r for r in results if not r['matches']],'results':results}
if args.report: args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k!='results'},indent=2))
raise SystemExit(bool(report['failures']))
