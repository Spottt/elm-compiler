#!/usr/bin/env python3
"""Seeded expression acceptance and observable worker values, not diagnostic parity."""
from pathlib import Path
import argparse, random, tempfile, subprocess, json, os, shutil, hashlib
parser=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']: parser.add_argument('--'+key,type=Path,required=True)
parser.add_argument('--seed',type=int,default=20260925)
args=parser.parse_args()
rng=random.Random(args.seed)
atoms=['0','True','"x"','[]','Nothing','(Just 1)','(\\x -> x)','{ field = 1 }','()']
def expression(depth):
    if depth==0 or rng.randrange(4)==0:return rng.choice(atoms)
    a=expression(depth-1);b=expression(depth-1)
    return rng.choice([f'({a}, {b})',f'[{a}, {b}]',f'({a} {b})',f'(if True then {a} else {b})',f'(let local x = x in (local {a}, local {b}))',f'(List.map {a} {b})',f'(Maybe.map {a} {b})',f'({a} == {b})',f'({a} ++ {b})',f'(\\arg -> ({a}, arg))',f'((\\r -> r.field) {a})'])
exprs=['1','(1 + True)']+list(dict.fromkeys(expression(3) for _ in range(180)))
binaries=[args.elm.resolve(),args.rust.resolve()]
identities=[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
rows=[]
def run_case(command, **kwargs):
    try:
        return subprocess.run(command, **kwargs)
    except subprocess.TimeoutExpired as error:
        raise AssertionError(
            f"seed={args.seed}, case={i}, expression={expr!r}, command={command!r}: timed out after {error.timeout}s"
        ) from error
with tempfile.TemporaryDirectory(prefix='elm-expression-acceptance-') as temp:
    root=Path(temp);(root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    cache=root/'home/0.19.1/packages';cache.mkdir(parents=True);original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages';shutil.copy2(original/'registry.dat',cache/'registry.dat');shutil.copytree(original/'elm/core',cache/'elm/core');shutil.copytree(original/'elm/json',cache/'elm/json');env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
    for i,expr in enumerate(exprs):
        source='module Main exposing (..)\nimport List\nimport Maybe\nvalue = '+expr+'\n';(root/'Main.elm').write_text(source);runs=[]
        for binary in binaries:
            r=run_case([str(binary),'make','Main.elm','--output=/dev/null','--report=json'],cwd=root,env=env,capture_output=True,text=True,timeout=30);runs.append({'code':r.returncode,'stderr':r.stderr});assert r.returncode==0 or (r.returncode==1 and json.loads(r.stderr)['type']=='compile-errors'), (expr, r.stderr)
        runtime=[]
        if all(run['code']==0 for run in runs):
            worker=source.replace('module Main', 'port module Main')+r"""
port outgoing : String -> Cmd msg
main : Program () () Never
main = Platform.worker
    { init = \_ -> ((), outgoing (Debug.toString value))
    , update = \_ model -> (model, Cmd.none)
    , subscriptions = \_ -> Sub.none
    }
"""
            (root/'Main.elm').write_text(worker)
            runner = "const assert=require('node:assert/strict');const app=require(process.argv[1]).Elm.Main.init({flags:null});const values=[];app.ports.outgoing.subscribe(value=>values.push(value));setTimeout(()=>{assert.equal(values.length,1);console.log(JSON.stringify(values));},20);"
            for label,binary,flags in [('elm',binaries[0],[]),('rust',binaries[1],[]),('rust-incremental',binaries[1],['--incremental'])]:
                output=root/(label+'.js')
                compiled=run_case([str(binary),'make','Main.elm','--output='+str(output),'--report=json',*flags],cwd=root,env=env,capture_output=True,text=True,timeout=30)
                assert compiled.returncode==0,(expr,label,compiled.stderr)
                result=run_case(['node','-e',runner,str(output)],cwd=root,capture_output=True,text=True,timeout=10)
                assert result.returncode==0,(expr,label,result.stderr)
                runtime.append({'compiler':label,'value':json.loads(result.stdout)})
        row={'index':i,'source':source,'equal_acceptance':(runs[0]['code']==0)==(runs[1]['code']==0),'runs':runs,'runtime':runtime,'equal_runtime':not runtime or all(item['value']==runtime[0]['value'] for item in runtime)};rows.append(row)
        if not (row['equal_acceptance'] and row['equal_runtime']):print(json.dumps(row),flush=True)
assert all(r['code']==0 for r in rows[0]['runs']), 'Positive control failed'
assert all(r['code']==1 and 'TYPE MISMATCH' in r['stderr'] for r in rows[1]['runs']), 'Negative control failed'
assert identities==[hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
args.report.write_text(json.dumps({'compiler_sha256':hashlib.sha256(binaries[1].read_bytes()).hexdigest(),'seed':args.seed,'passed':all(r['equal_acceptance'] and r['equal_runtime'] for r in rows),'cases':rows},indent=2))
print('cases',len(rows),'accepted',sum(r['runs'][0]['code']==0 for r in rows),'mismatches',sum(not r['equal_acceptance'] for r in rows),flush=True)

raise SystemExit(not all(r['equal_acceptance'] and r['equal_runtime'] for r in rows))
