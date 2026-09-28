#!/usr/bin/env python3
"""Compare cached-package verification progress, including real failures and warm reuse.

Elm's concurrent workers may omit the initial zero update. Intermediate updates
are checked for monotonic counts; final redraw/status and subsequent output are exact.
Downloads and ANSI terminals are outside this suite.
"""
import argparse, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ('elm', 'rust', 'report'):
    p.add_argument('--' + key, type=Path, required=True)
a = p.parse_args()
sha = hashlib.sha256(a.rust.read_bytes()).hexdigest()
crate = Path(__file__).resolve().parents[1]
results = []
with tempfile.TemporaryDirectory(prefix='dependency-progress-') as directory:
    root = Path(directory)
    original = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))/'0.19.1/packages'
    for broken in (False, True):
        for json_mode in (False, True):
            project = root/f'{broken}-{json_mode}'
            shutil.copytree(crate/'tests/programs/worker', project)
            home = project/'home'; cache = home/'0.19.1/packages'; cache.mkdir(parents=True)
            shutil.copy2(original/'registry.dat', cache/'registry.dat')
            for package in ('elm/core/1.0.5', 'elm/json/1.1.3'):
                shutil.copytree(original/package, cache/package)
                for artifact in (cache/package).glob('*.dat'):
                    artifact.unlink()
            if broken:
                source = cache/'elm/core/1.0.5/src/Process.elm'
                source.write_text(source.read_text()+'\nbroken = True + 1\n')
            env = {**os.environ, 'ELM_HOME':str(home), 'GHCRTS':'-N1 -A16m -c'}
            for temperature in ('cold', 'warm'):
                runs = []
                for binary in (a.elm, a.rust):
                    run = subprocess.run([str(binary.resolve()), 'make', 'Main.elm', '--output=main.js', *(['--report=json'] if json_mode else [])], cwd=project, env=env, capture_output=True, timeout=60)
                    runs.append({'code':run.returncode, 'stdout':run.stdout.decode(), 'stderr':run.stderr.decode()})
                passed = all(run['code'] == int(broken) for run in runs)
                passed &= (json.loads(runs[0]['stderr']) == json.loads(runs[1]['stderr'])) if broken and json_mode else runs[0]['stderr'] == runs[1]['stderr']
                tails = []
                for run in runs:
                    output = run['stdout']
                    events = [(int(n), int(total)) for n, total in re.findall(r'\rVerifying dependencies \((\d+)/(\d+)\)', output)]
                    expected_progress = not json_mode and (temperature == 'cold' or broken)
                    if expected_progress:
                        passed &= bool(events) and events[-1] == (2,2) and all(total == 2 and 0 <= n <= 2 for n,total in events)
                        passed &= all(x[0] <= y[0] for x,y in zip(events,events[1:]))
                        final = '\r' + ' '*len('Verifying dependencies (2/2)') + '\r' + ('Dependency problem!' if broken else 'Dependencies ready!') + '\n'
                        passed &= final in output
                        tails.append(output.split(final,1)[-1])
                    else:
                        passed &= not events
                        tails.append(output)
                passed &= tails[0] == tails[1]
                if json_mode:
                    passed &= not any(run['stdout'] for run in runs)
                results.append({'broken':broken,'json':json_mode,'temperature':temperature,'passed':passed,'official':runs[0],'rust':runs[1]})
                print(broken,json_mode,temperature,'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest() == sha
a.report.write_text(json.dumps({'scope':__doc__,'compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results},indent=2)+'\n')
raise SystemExit(not all(r['passed'] for r in results))
