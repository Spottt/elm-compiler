#!/usr/bin/env python3
"""Seeded typed expressions: compare port values to an independent integer oracle."""
import argparse
import hashlib
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

def expression(depth):
    if depth == 0:
        value = rng.randrange(-9, 10)
        return '('+str(value)+')', value
    left, x = expression(depth-1)
    right, y = expression(depth-1)
    divisor = rng.randrange(1, 10)
    return rng.choice([
        (f'({left} + {right})', x+y),
        (f'({left} - {right})', x-y),
        (f'({left} // {divisor})', int(x/divisor)),
        (f'(if {left} < {right} then {left} else {right})', min(x,y)),
        (f'(List.sum [{left}, {right}])', x+y),
        (f'(List.foldl (-) 0 [{left}, {right}])', y-x),
        (f'(List.foldr (-) 0 [{left}, {right}])', x-y),
        (f'(List.sum (List.map (\\n -> n + 1) [{left}, {right}]))', x+y+2),
        (f'((\\record -> record.value) {{ value = {left}, other = {right} }})', x),
    ])

cases = [expression(3) for _ in range(100)]
binaries = [a.elm.resolve(), a.rust.resolve()]
identities = [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
rows = []
with tempfile.TemporaryDirectory(prefix='elm-generated-integers-') as temp:
    root = Path(temp)
    cache = root/'home/0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat', cache/'registry.dat')
    for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(original/package, cache/package)
    (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    source = r'''port module Main exposing (main)
import List
port outgoing : List Int -> Cmd msg
main : Program () () Never
main = Platform.worker
    { init = \_ -> ((), outgoing values)
    , update = \_ model -> (model, Cmd.none)
    , subscriptions = \_ -> Sub.none
    }
values : List Int
values = [ EXPRESSIONS ]
'''.replace('EXPRESSIONS', ', '.join(expr for expr, _ in cases))
    env = {**os.environ, 'ELM_HOME':str(root/'home'), 'GHCRTS':'-N1'}
    runner = "const assert=require('node:assert/strict');const values=[];require(process.argv[1]).Elm.Main.init({flags:null}).ports.outgoing.subscribe(x=>values.push(x));setTimeout(()=>{assert.equal(values.length,1);console.log(JSON.stringify(values[0]));},20);"
    for mode, options in [('development', []), ('production', ['--optimize'])]:
        for label, binary, flags in [('elm',binaries[0],[]),('rust',binaries[1],[]),('rust-incremental',binaries[1],['--incremental']),('rust-warm',binaries[1],['--incremental'])]:
            # Change a comment to exercise module caches rather than whole-output reuse.
            (root/'Main.elm').write_text(source+'\n-- '+mode+' '+label+'\n')
            output = root/(label+'.js')
            compiled = subprocess.run([str(binary),'make','Main.elm','--report=json','--output='+str(output),*options,*flags],cwd=root,env=env,capture_output=True,text=True,timeout=60)
            assert compiled.returncode == 0, (mode,label,compiled.stderr)
            result = subprocess.run(['node','-e',runner,str(output)],capture_output=True,text=True,timeout=10)
            assert result.returncode == 0, (mode,label,result.stderr)
            values = json.loads(result.stdout)
            assert values == [value for _,value in cases], (mode,label,values,cases)
            rows.append({'mode':mode,'compiler':label,'passed':True,'values':values})
assert identities == [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'seed':a.seed,'compiler_sha256':identities,'passed':True,'expressions':cases,'runs':rows},indent=2)+'\n')
print('PASS: 100 expressions, 8 builds/executions, independent expected values')
