#!/usr/bin/env python3
"""Network integration: download immutable elm/url 1.0.0 into private caches.

Requires the public Elm package server and archive host. Checks exact download
lines, valid verification counters/final redraw, compilation output, warm reuse
and silent JSON mode. Not registered in the offline compatibility suite.
"""
import argparse, concurrent.futures, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ('elm','rust','report'):
    p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();sha=hashlib.sha256(a.rust.read_bytes()).hexdigest();results=[]
crate=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='download-progress-') as directory:
    root=Path(directory)
    original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
    for json_mode in (False,True):
        def run(slot):
            label,binary=slot
            project=root/f'{json_mode}-{label}'
            shutil.copytree(crate/'tests/programs/worker',project)
            cache=project/'home/0.19.1/packages';cache.mkdir(parents=True)
            shutil.copy2(original/'registry.dat',cache/'registry.dat')
            for package in ('elm/core/1.0.5','elm/json/1.1.3'):
                shutil.copytree(original/package,cache/package)
            (cache/'elm/url/1.0.0').mkdir(parents=True)
            shutil.copy2(original/'elm/url/1.0.0/elm.json',cache/'elm/url/1.0.0/elm.json')
            config=json.loads((project/'elm.json').read_text());config['dependencies']['direct']['elm/url']='1.0.0'
            (project/'elm.json').write_text(json.dumps(config))
            env={**os.environ,'ELM_HOME':str(project/'home'),'GHCRTS':'-N1 -A16m -c'}
            runs=[]
            for temperature in ('cold','warm'):
                proc=subprocess.run([str(binary.resolve()),'make','Main.elm','--output=main.js',*(['--report=json'] if json_mode else [])],cwd=project,env=env,capture_output=True,timeout=90)
                runs.append({'temperature':temperature,'code':proc.returncode,'stdout':proc.stdout.decode(),'stderr':proc.stderr.decode(),'downloaded':(cache/'elm/url/1.0.0/src/Url.elm').is_file(),'generated':(project/'main.js').is_file()})
            return runs
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            official,rust=list(pool.map(run,[('elm',a.elm),('rust',a.rust)]))
        for reference,actual in zip(official,rust):
            passed=all(r['code']==0 and not r['stderr'] and r['downloaded'] and r['generated'] for r in (reference,actual))
            if json_mode:
                passed &= not reference['stdout'] and not actual['stdout']
            elif reference['temperature']=='cold':
                tails=[]
                for r in (reference,actual):
                    output=r['stdout']
                    passed &= output.startswith('Starting downloads...\n\n  ● elm/url 1.0.0\n\nVerifying dependencies (')
                    events=[(int(n),int(total)) for n,total in re.findall(r'Verifying dependencies \((\d+)/(\d+)\)',output)]
                    passed &= bool(events) and events[-1]==(3,3) and all(t==3 and 0<=n<=3 for n,t in events) and all(x[0]<=y[0] for x,y in zip(events,events[1:]))
                    final='\r'+' '*len('Verifying dependencies (3/3)')+'\rDependencies ready!\n'
                    passed &= final in output
                    tails.append(output.split(final,1)[-1])
                passed &= tails[0]==tails[1]
            else:
                passed &= reference['stdout']==actual['stdout'] and 'downloads' not in actual['stdout'] and 'Verifying dependencies' not in actual['stdout']
            results.append({'json':json_mode,'temperature':reference['temperature'],'passed':passed,'official':reference,'rust':actual})
            print(json_mode,reference['temperature'],'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest()==sha
a.report.write_text(json.dumps({'scope':__doc__,'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results},indent=2)+'\n')
raise SystemExit(not all(r['passed'] for r in results))
