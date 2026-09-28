#!/usr/bin/env python3
"""Compare package names, summaries and license diagnostics with Elm 0.19.1."""
import argparse, copy, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
package = {'type': 'package', 'name': 'author/fixture', 'summary': 'Fixture.', 'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': ['Main'], 'elm-version': '0.19.0 <= v < 0.20.0', 'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}
cases = []
for label,key,value in [('summary-ascii', 'summary', 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'), ('summary-unicode', 'summary', 'éééééééééééééééééééééééééééééééééééééééé'), ('summary-escaped', 'summary', 'a\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\ba\\b'), ('license-case', 'license', 'mit'), ('license-typo', 'license', 'BSD-3-Clausee'), ('license-full', 'license', 'Apache License 2.0'), ('license-unknown', 'license', 'UNLICENSED'), ('license-empty', 'license', ''), ('license-unicode', 'license', 'é'), ('name-empty', 'name', ''), ('name-noslash', 'name', 'author'), ('name-uppercase', 'name', 'author/Project'), ('name-digit', 'name', 'author/2project'), ('name-underscore', 'name', 'author/elm_ui'), ('name-double-hyphen', 'name', 'author/elm--ui'), ('name-trailing-hyphen', 'name', 'author/elm-'), ('name-unicode', 'name', 'author/élm'), ('name-leading-author', 'name', '-author/project'), ('name-double-author', 'name', 'a--b/project'), ('name-long-author', 'name', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/project'), ('name-long-project', 'name', 'author/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'), ('valid-author-dash', 'name', 'author-/project'), ('valid-author-digit', 'name', '123/project'), ('valid-summary-boundary', 'summary', 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx')]:
 config = copy.deepcopy(package);config[key]=value
 cases.append((label,json.dumps(config,indent=2,ensure_ascii=False)))
for label,key,value in [('license-quoted-full', 'license', 'BSD 3-clause "New" or "Revised" License'), ('license-french', 'license', 'Licence Libre du Québec – Permissive version 1.1'), ('license-transposed', 'license', 'Mti'), ('name-extra-slash', 'name', 'author/project/extra'), ('name-empty-project', 'name', 'author/'), ('name-empty-author', 'name', '/project'), ('name-author-underscore', 'name', 'auth_or/project'), ('valid-name-boundaries', 'name', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')]:
 config = copy.deepcopy(package);config[key]=value
 cases.append((label,json.dumps(config,indent=2,ensure_ascii=False)))
results=[]
with tempfile.TemporaryDirectory(prefix='outline-missing-fields-') as temp:
 root=Path(temp); home=root/'home'; shutil.copytree(Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages',home/'0.19.1/packages'); (root/'src').mkdir(); (root/'src/Main.elm').write_text('module Main exposing (x)\nx = 1\n')
 for name,source in cases:
  (root/'elm.json').write_text(source+'\n'); rows=[]
  for binary in [a.elm,a.rust]:
   streams=[]
   for flags in [['--report=json'],[]]:
    run=subprocess.run([str(binary.resolve()),'make','src/Main.elm','--output=/dev/null',*flags],cwd=root,env={**os.environ,'GHCRTS':'-N1','ELM_HOME':str(home)},capture_output=True,text=True,timeout=10)
    streams.append({'code':run.returncode,'stderr':(json.loads(run.stderr) if run.stderr else None) if flags else run.stderr})
   rows.append(streams)
  passed=rows[0]==rows[1] and rows[0][0]['code']==(0 if name.startswith('valid-') else 1)
  results.append({'case':name,'passed':passed,'official':rows[0],'rust':rows[1]});print(name,'PASS' if passed else 'FAIL',flush=True)
assert sha=={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in [('elm',a.elm),('rust',a.rust)]}
report={'scope':__doc__,'sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
