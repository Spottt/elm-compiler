#!/usr/bin/env python3
"""Compare import diagnostics across multiple entry graphs, in both argument orders."""
import argparse, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for name in ['elm', 'rust', 'report']:
    p.add_argument('--' + name, type=Path, required=True)
a = p.parse_args()
binaries = [a.elm.resolve(), a.rust.resolve()]
hashes = [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
rows = []
with tempfile.TemporaryDirectory(prefix='elm-multi-import-') as td:
    root = Path(td)
    home = root / 'home'
    original = Path(os.environ.get('ELM_HOME', Path.home() / '.elm')) / '0.19.1/packages'
    cache = home / '0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copy2(original / 'registry.dat', cache / 'registry.dat')
    for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(original / package, cache / package)
    env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1'}
    for case in ['other-entry', 'other-dependency', 'other-missing', 'shared-failure']:
        sources = {'Main': 'import Widgte\nx = 1', 'Widget': 'x = 1'}
        other = 'Widget'
        if case == 'other-dependency':
            sources['Helper'] = 'import Widget\nx = 1'
            other = 'Helper'
        elif case == 'other-missing':
            sources = {'Main': 'import Widgte\nx = 1', 'Helper': 'import Widget\nx = 1'}
            other = 'Helper'
        elif case == 'shared-failure':
            sources = {'Main': 'import Broken\nx = 1', 'Helper': 'import Broken\nimport Widget\nx = 1', 'Broken': 'import Widgte\nx = 1', 'Widget': 'x = 1'}
            other = 'Helper'
        for reverse in [False, True]:
            for mode in ['json', 'plain']:
                runs = []
                for index, binary in enumerate(binaries):
                    project = root / f'{case}-{reverse}-{mode}-{index}'
                    project.mkdir()
                    shutil.copy2(Path(__file__).resolve().parents[1] / 'tests/programs/worker/elm.json', project / 'elm.json')
                    for name, body in sources.items():
                        (project / (name + '.elm')).write_text('module ' + name + ' exposing (..)\n' + body + '\n')
                    entries = ['Main.elm', other + '.elm']
                    if reverse:
                        entries.reverse()
                    command = [str(binary), 'make', *entries, '--output=/dev/null']
                    if mode == 'json':
                        command.append('--report=json')
                    r = subprocess.run(command, cwd=project, env=env, text=True, capture_output=True, timeout=30)
                    stderr = r.stderr.replace(str(project), '<project>')
                    if mode == 'json':
                        stderr = json.loads(stderr)
                    runs.append({'code': r.returncode, 'stderr': stderr})
                rows.append({'case': case, 'reverse': reverse, 'mode': mode, 'passed': runs[0] == runs[1] and runs[0]['code'] == 1, 'elm': runs[0], 'rust': runs[1]})
assert hashes == [hashlib.sha256(b.read_bytes()).hexdigest() for b in binaries]
a.report.write_text(json.dumps({'scope': __doc__, 'binary_sha256': hashes, 'passed': all(r['passed'] for r in rows), 'results': rows}, indent=2) + '\n')
print(json.dumps({'checks': len(rows), 'failures': [{k:r[k] for k in ['case','reverse','mode']} for r in rows if not r['passed']]}))
raise SystemExit(not all(r['passed'] for r in rows))
