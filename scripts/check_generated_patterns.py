#!/usr/bin/env python3
"""Seeded recursive constructor matches: compiler acceptance and runtime oracle."""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import random
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']:
    p.add_argument('--'+key, type=Path, required=True)
p.add_argument('--seed', type=int, default=20260925)
a = p.parse_args()
rng = random.Random(a.seed)
binaries = [a.elm.resolve(), a.rust.resolve()]
hashes = [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
# Pattern data doubles as an independent, ordered matching oracle.
patterns = ['_', 'A', 'B _', 'B 0', 'B 1', 'C _ _', 'C A _', 'C _ A',
            'C (B _) _', 'C _ (B _)', 'C (C _ _) _', 'C _ (C _ _)',
            'C A A', 'C (B _) (B _)', 'C (B 0) A']
def matches(pattern, value):
    if pattern == '_': return True
    if pattern == 'A': return value == 'A'
    if pattern.startswith('B '):
        return isinstance(value, tuple) and value[0] == 'B' and (pattern == 'B _' or value[1] == int(pattern[2:]))
    if not isinstance(value, tuple) or value[0] != 'C': return False
    children = {'C _ _': ('_', '_'), 'C A _': ('A', '_'), 'C _ A': ('_', 'A'),
        'C (B _) _': ('B _', '_'), 'C _ (B _)': ('_', 'B _'),
        'C (C _ _) _': ('C _ _', '_'), 'C _ (C _ _)': ('_', 'C _ _'),
        'C A A': ('A', 'A'), 'C (B _) (B _)': ('B _', 'B _'), 'C (B 0) A': ('B 0', 'A')}
    left, right = children[pattern]
    return matches(left, value[1]) and matches(right, value[2])
def elm(value):
    if value == 'A': return 'A'
    if value[0] == 'B': return f'(B {value[1]})'
    return f'(C {elm(value[1])} {elm(value[2])})'
leaves = ['A', ('B', 0), ('B', 1), ('B', 2)]
first = [('C', l, r) for l, r in itertools.product(leaves, repeat=2)]
values = leaves + first + [('C', rng.choice(leaves+first), rng.choice(leaves+first)) for _ in range(50)]
cases = [['_'], ['A', 'B _', 'C _ _'], ['A'], ['_', 'A']]
for _ in range(100):
    case = rng.sample(patterns, rng.randrange(1, 8))
    if rng.randrange(2): case.append('_')
    if case not in cases: cases.append(case)
rows = []
with tempfile.TemporaryDirectory(prefix='elm-generated-patterns-') as temp:
    root = Path(temp)
    cache = root/'home/0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(original/package, cache/package)
    (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    env = {**os.environ, 'ELM_HOME':str(root/'home'), 'GHCRTS':'-N1'}
    for patterns_for_case in cases:
        source = 'module Main exposing (..)\ntype Tree = A | B Int | C Tree Tree\nf : Tree -> Int\nf tree =\n    case tree of\n' + ''.join(f'        {pat} -> {i}\n' for i, pat in enumerate(patterns_for_case))
        (root/'Main.elm').write_text(source)
        runs = []
        for binary in binaries:
            run = subprocess.run([str(binary),'make','Main.elm','--output=/dev/null','--report=json'],cwd=root,env=env,capture_output=True,text=True,timeout=30)
            assert run.returncode == 0 or (run.returncode == 1 and json.loads(run.stderr)['type'] == 'compile-errors'), run.stderr
            runs.append({'accepted':run.returncode == 0,'diagnostic':json.loads(run.stderr) if run.returncode else None})
        runtime = []
        if all(r['accepted'] for r in runs):
            expected = [next(i for i, pat in enumerate(patterns_for_case) if matches(pat, v)) for v in values]
            worker = source.replace('module Main', 'port module Main') + '\nport outgoing : List Int -> Cmd msg\nmain : Program () () Never\nmain = Platform.worker { init = \\_ -> ((), outgoing (List.map f [' + ','.join(elm(v) for v in values) + '])), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }\n'
            (root/'Main.elm').write_text(worker)
            for label, binary, flags in [(label+suffix,binary,flags+mode) for suffix,mode in [('',[]),('-production',['--optimize'])] for label,binary,flags in [('elm',binaries[0],[]),('rust',binaries[1],[]),('rust-incremental',binaries[1],['--incremental'])]]:
                output = root/(label+'.js')
                compiled = subprocess.run([str(binary),'make','Main.elm','--output='+str(output),*flags],cwd=root,env=env,capture_output=True,text=True,timeout=30)
                assert compiled.returncode == 0, compiled.stderr
                runner = "const assert=require('node:assert/strict');const values=[];require(process.argv[1]).Elm.Main.init({flags:null}).ports.outgoing.subscribe(x=>values.push(x));setTimeout(()=>{assert.equal(values.length,1);console.log(JSON.stringify(values[0]));},20);"
                executed = subprocess.run(['node','-e',runner,str(output)],capture_output=True,text=True,timeout=10,check=True)
                actual = json.loads(executed.stdout)
                runtime.append({'compiler':label,'passed':actual == expected,'actual':actual,'expected':expected})
        row = {'patterns':patterns_for_case,'runs':runs,'runtime':runtime,'equal_diagnostics':runs[0]['diagnostic']==runs[1]['diagnostic'],'passed':runs[0]==runs[1] and all(r['passed'] for r in runtime)}
        rows.append(row)
        if not row['passed']: print(json.dumps(row), flush=True)
assert all(r['accepted'] for r in rows[0]['runs'])
assert all(not r['accepted'] for r in rows[2]['runs'])
assert all(not r['accepted'] for r in rows[3]['runs'])
assert hashes == [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'seed':a.seed,'binary_sha256':hashes,'passed':all(r['passed'] for r in rows),'values':values,'cases':rows},indent=2)+'\n')
print('cases',len(rows),'accepted',sum(r['runs'][0]['accepted'] for r in rows),'mismatches',sum(not r['passed'] for r in rows))
raise SystemExit(not all(r['passed'] for r in rows))
