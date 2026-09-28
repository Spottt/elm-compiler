#!/usr/bin/env python3
"""Generate every frontend entry and load its JavaScript with Node's default stack.

This checks linking and module initialization, not browser startup or behavior.
"""
import argparse
import json
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--front', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--entry', action='append', help='application directory, e.g. Admin or PO')
parser.add_argument('--optimize', action='store_true', help='exercise optimized make without the output cache')
args = parser.parse_args()
binary = Path(__file__).resolve().parents[1]/'target/release/planexpo-elm'
args.output.mkdir(parents=True, exist_ok=True)
applications = args.entry or ['Admin','Back','Bill','Contract','Exhibitor','Password','Plan','PO','Public','Superadmin']
results = []
for app in applications:
    start = time.monotonic()
    output = (args.output/f'{app}.js').resolve()
    result = {'application': app}
    try:
        if args.optimize:
            command = [str(binary), 'make', f'src/{app}/Main{app}.elm', '--optimize', '--no-cache', '--report=json', '--output='+str(output)]
        else:
            command = [str(binary), 'link-js', str(args.front.resolve()/'elm.json'), f'src/{app}/Main{app}.elm', str(output)]
        run = subprocess.run(command, cwd=args.front.resolve(), capture_output=True, text=True, timeout=180)
        if run.returncode:
            raise RuntimeError(run.stdout + run.stderr)
        if args.optimize:
            result.update(mode='Production', bytes=output.stat().st_size)
        else:
            result.update(json.loads(run.stdout))
        run = subprocess.run(['node', '-e', '''
const loaded = require(process.argv[1]);
const elm = loaded.Elm;
const name = process.argv[2];
if (!elm || typeof elm[name]?.init !== 'function') throw new Error('Missing init export for ' + name);
''', str(output), f'Main{app}'], capture_output=True, text=True, timeout=30)
        if run.returncode:
            raise RuntimeError(run.stdout + run.stderr)
        result['loaded'] = True
    except (RuntimeError, subprocess.TimeoutExpired) as error:
        result['error'] = str(error)
    result['elapsed_seconds'] = round(time.monotonic()-start, 2)
    results.append(result)
    print(json.dumps(result), flush=True)
(args.output/'report.json').write_text(json.dumps(results, indent=2))
raise SystemExit(any('error' in result for result in results))
