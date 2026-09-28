#!/usr/bin/env python3
"""Compare make --docs CLI selection, output paths and repeated builds with Elm.

Compares acceptance and output artifacts, not exact diagnostic prose/regions.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
p.add_argument('--rust', type=Path, help='Compiler under test; defaults to the release binary')
a = p.parse_args()
rust = (a.rust or Path(__file__).resolve().parents[1]/'target/release/planexpo-elm').resolve()
original = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm')))/'0.19.1/packages'
source = 'module Main exposing (value)\n{-| @docs value -}\n{-| Value. -}\nvalue : Int\nvalue = 1\n'
config = {'type':'package','name':'author/fixture','summary':'Documentation CLI fixture.',
          'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
          'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
cases = [
    ('equals', ['--docs=docs.json'], '.', source),
    ('separate', ['--docs', 'docs.json'], '.', source),
    ('optimize', ['--optimize', '--docs=docs.json'], '.', source),
    ('output-ignored', ['--docs=docs.json', '--output=output.js'], '.', source),
    ('explicit-ignored', ['src/Main.elm', '--docs=docs.json'], '.', source.replace('{-| @docs value -}\n','').replace('{-| Value. -}\n','')),
    ('explicit-devnull-ignored', ['src/Main.elm', '--docs=docs.json', '--output=/dev/null'], '.', source),
    ('nested-output', ['--docs=src/docs.json'], '.', source),
    ('subdirectory', ['--docs=docs.json'], 'src', source),
    ('missing-parent', ['--docs=missing/docs.json'], '.', source),
    ('bad-extension', ['--docs=docs.txt'], '.', source),
    ('missing-path', ['--docs'], '.', source),
    ('empty-path', ['--docs='], '.', source),
    ('invalid-documentation', ['--docs=docs.json'], '.', source.replace('value : Int\n','')),
]
results, failures = [], []
with tempfile.TemporaryDirectory(prefix='elm-doc-cli-') as tmp:
    root = Path(tmp)
    cache = root/'home/0.19.1/packages'; cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat', cache/'registry.dat')
    shutil.copytree(original/'elm/core/1.0.5/src', cache/'elm/core/1.0.5/src')
    shutil.copyfile(original/'elm/core/1.0.5/elm.json', cache/'elm/core/1.0.5/elm.json')
    env = {**os.environ, 'ELM_HOME':str(root/'home'), 'GHCRTS':'-N1 -A16m -c'}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    env.update(no_proxy='', NO_PROXY='')
    def artifacts(project):
        result = {}
        for name in ['docs.json','src/docs.json','output.js','docs.txt']:
            path = project/name
            if path.exists():
                text = path.read_text()
                try: result[name] = json.loads(text)
                except ValueError: result[name] = text
        return result
    def invoke(binary, project, args, cwd='.'):
        run = subprocess.run([str(binary), 'make', *args, '--report=json'], cwd=project/cwd, env=env, capture_output=True, text=True, timeout=30)
        return {'accepted':run.returncode == 0, 'artifacts':artifacts(project), 'stderr':run.stderr}
    def project_at(path, text):
        (path/'src').mkdir(parents=True)
        (path/'elm.json').write_text(json.dumps(config))
        (path/'src/Main.elm').write_text(text)
        return path
    for index, (name, args, cwd, text) in enumerate(cases):
        outcomes = []
        for label, binary in [('official',a.elm.resolve()),('rust',rust)]:
            project = project_at(root/f'{index}-{label}', text)
            (project/'docs.json').write_text('sentinel')
            outcomes.append(invoke(binary, project, args, cwd))
        if any(outcomes[0][key] != outcomes[1][key] for key in ['accepted','artifacts']): failures.append(name)
        results.append({'case':name,'official':outcomes[0],'rust':outcomes[1]})
    sequences = []
    for label, binary in [('official',a.elm.resolve()),('rust',rust)]:
        project = project_at(root/f'rebuild-{label}', source)
        steps = []
        for action in ['initial','warm','delete-output','edit-comment','invalid-after-success']:
            if action == 'delete-output': (project/'docs.json').unlink()
            if action == 'edit-comment': (project/'src/Main.elm').write_text(source.replace('Value.', 'Updated.'))
            if action == 'invalid-after-success': (project/'src/Main.elm').write_text(source.replace('value : Int\n',''))
            steps.append(invoke(binary,project,['--docs=docs.json']))
        sequences.append(steps)
    for index, action in enumerate(['initial','warm','delete-output','edit-comment','invalid-after-success']):
        official, actual = sequences[0][index], sequences[1][index]
        if any(official[key] != actual[key] for key in ['accepted','artifacts']): failures.append('rebuild-'+action)
        results.append({'case':'rebuild-'+action,'official':official,'rust':actual})
report = {'scope':__doc__,'cases':len(results),'failures':failures,'results':results,'compiler_sha256':hashlib.sha256(rust.read_bytes()).hexdigest()}
if a.report: a.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
