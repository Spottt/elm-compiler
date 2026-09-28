#!/usr/bin/env python3
"""Replay the original upstream 0.18 fixtures against Elm 0.19.1 and Rust.

Source fixtures and JS snapshots remain byte-for-byte upstream. The runner only
adds a module header for standalone application compilation. Elm 0.19.1 is the
acceptance oracle: some original 'good' fixtures use syntax now forbidden.
Old JS snapshots are retained for provenance, not compared with 0.19 output.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--elm', type=Path, required=True)
parser.add_argument('--report', type=Path, help='Write the full per-fixture report')
args = parser.parse_args()
elm = args.elm.resolve()
crate = Path(__file__).resolve().parents[1]
binary = crate/'target/release/planexpo-elm'
upstream = crate/'tests/upstream/elm-0.18.0'
provenance = json.loads((upstream/'provenance.json').read_text())
for entry in provenance['files']:
    path = upstream/entry['path']
    if hashlib.sha256(path.read_bytes()).hexdigest() != entry['sha256']:
        raise SystemExit(f'Upstream fixture modified: {path}')
results=[]
with tempfile.TemporaryDirectory(prefix='elm-rust-upstream-parity-') as directory:
    root=Path(directory)
    (root/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
    fixtures=sorted((upstream/'tests/test-files').rglob('*.elm'))
    for index,fixture in enumerate(fixtures):
        relative=fixture.relative_to(upstream).as_posix()
        module=f'Upstream{index}'
        source=root/f'{module}.elm'
        source.write_bytes(f'module {module} exposing (..)\n'.encode()+fixture.read_bytes())
        reference=subprocess.run([str(elm),'make',source.name,'--output=/dev/null','--report=json'],cwd=root,text=True,capture_output=True,timeout=30,env={**os.environ,'GHCRTS':'-N1 -A16m -c'})
        if reference.returncode not in (0,1):
            raise SystemExit(f'Reference failed unexpectedly: {relative}\n{reference.stderr}')
        titles=[]
        if reference.returncode:
            try: report=json.loads(reference.stderr)
            except ValueError: raise SystemExit(reference.stderr)
            if report.get('type') != 'compile-errors':
                raise SystemExit(f'Reference environment error: {reference.stderr}')
            titles=[p['title'] for e in report['errors'] for p in e['problems']]
        native=subprocess.run([str(binary),'check',str(root/'elm.json'),source.name],capture_output=True,text=True,timeout=30)
        if native.returncode not in (0,1):
            raise SystemExit(f'Rust failed unexpectedly: {relative}\n{native.stderr}')
        results.append({'fixture':relative,'historical_expected':'/good/' in relative,'elm_0_19_1_accepts':reference.returncode==0,'rust_accepts':native.returncode==0,'matches':reference.returncode==native.returncode,'elm_errors':titles,'rust_error':native.stderr.strip()})
output={'source_commit':provenance['commit'],'fixtures':len(results),'matches':sum(r['matches'] for r in results),'legacy_good_now_rejected':sum(r['historical_expected'] and not r['elm_0_19_1_accepts'] for r in results),'failures':[r for r in results if not r['matches']],'results':results}
if args.report:
    args.report.write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps({k:v for k,v in output.items() if k!='results'},indent=2))
raise SystemExit(any(not r['matches'] for r in results))
