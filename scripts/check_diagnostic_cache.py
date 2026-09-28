#!/usr/bin/env python3
"""Verify repeated failure caching, source invalidation and official diagnostics."""
import argparse, hashlib, json, os, shutil, tempfile
from pathlib import Path
from diff_test_process import run_command
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']:
    p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = hashlib.sha256(a.rust.read_bytes()).hexdigest()
rows = []
with tempfile.TemporaryDirectory(prefix='diagnostic-cache-') as temp:
    root = Path(temp)
    home = root/'home/0.19.1/packages'
    home.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', home/'registry.dat')
    for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(original/package, home/package)
    env = {**os.environ, 'ELM_HOME': str(root/'home'), 'GHCRTS': '-N1'}
    (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    worker = 'module Main exposing (..)\nimport Choice exposing (Choice(..))\nmain : Program () () Never\nmain = Platform.worker { init = \\_ -> ((), Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }\nf x =\n    case x of\n        A -> 1\n'
    (root/'Choice.elm').write_text('module Choice exposing (..)\ntype Choice = A | B\n')
    (root/'Main.elm').write_text(worker)
    output = root/'result.js'
    output.write_text('previous valid bundle')
    def invoke(binary, kind='json', flags=()):
        command = [str(binary.resolve()), 'make', 'Main.elm', '--output=result.js', *flags]
        if kind == 'json': command += ['--report=json']
        r = run_command(command, root, env, kind == 'ansi')
        return {'code':r['code'], 'stderr':json.loads(r['stderr']) if kind=='json' and r['stderr'].strip() else r['stderr']}
    for scenario in ['first', 'repeat', 'move-region', 'dependency-change', 'corrupt-cache', 'no-cache']:
        if scenario == 'move-region': (root/'Main.elm').write_text(worker.replace('f x =', '\n\nf x ='))
        if scenario == 'dependency-change': (root/'Choice.elm').write_text('module Choice exposing (..)\ntype Choice = A | B | C\n')
        if scenario == 'corrupt-cache':
            for f in root.glob('elm-stuff/planexpo-rust/v1/*.diagnostic'): f.write_bytes(b'broken')
        prior_cache = {f: f.stat().st_mtime_ns for f in root.glob('elm-stuff/planexpo-rust/v1/*.diagnostic')}
        for kind in ['json', 'plain', 'ansi']:
            official = invoke(a.elm, kind)
            actual = invoke(a.rust, kind, ['--incremental'] + (['--no-cache'] if scenario=='no-cache' else []))
            assert actual == official and actual['code']==1, (scenario, kind, actual, official)
            assert output.read_text() == 'previous valid bundle'
            rows.append({'scenario':scenario, 'mode':kind, 'passed':True})
        if scenario in ['repeat', 'no-cache']:
            assert prior_cache == {f: f.stat().st_mtime_ns for f in root.glob('elm-stuff/planexpo-rust/v1/*.diagnostic')}, 'Cache hit/disabled cache rewrote the diagnostic'
        assert list(root.glob('elm-stuff/planexpo-rust/v1/*.diagnostic')), 'No cached diagnostic'
    (root/'Main.elm').write_text(worker+'        _ -> 2\n')
    repaired = invoke(a.rust, flags=['--incremental'])
    assert repaired['code']==0, repaired
    assert output.read_text()!='previous valid bundle'
    rows.append({'scenario':'repair', 'passed':True})
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
a.report.write_text(json.dumps({'compiler_sha256':sha,'passed':True,'checks':rows},indent=2)+'\n')
print('PASS', len(rows), 'diagnostic-cache checks')
