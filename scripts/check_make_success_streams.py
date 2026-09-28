#!/usr/bin/env python3
"""Compare make status, streams and final summaries; retain progress-text differences."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
for name in ('elm', 'rust', 'report'):
    p.add_argument('--' + name, type=Path, required=True)
a = p.parse_args()
sha = hashlib.sha256(a.rust.read_bytes()).hexdigest()
results = []
crate = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='make-success-streams-') as directory:
    root = Path(directory)
    cache = root / 'home/0.19.1/packages'
    cache.mkdir(parents=True)
    original = Path(os.environ.get('ELM_HOME', Path.home() / '.elm')) / '0.19.1/packages'
    shutil.copy2(original / 'registry.dat', cache / 'registry.dat')
    for package, version in [('elm/core', '1.0.5'), ('elm/json', '1.1.3')]:
        shutil.copytree(original / package / version, cache / package / version)
    env = {**os.environ, 'ELM_HOME': str(root / 'home'), 'GHCRTS': '-N1', 'PLANEXPO_ELM_STATS': ''}
    project = root / 'project'
    shutil.copytree(crate / 'tests/programs/worker', project)
    source = (project / 'Main.elm').read_text().replace('import Platform', 'import Offset\nimport Platform', 1)
    (project / 'Main.elm').write_text(source)
    (project / 'Offset.elm').write_text('module Offset exposing (value)\nvalue = 1\n')
    (project / 'Second.elm').write_text(source.replace('module Main', 'module Second', 1))
    for mode, entries in (([], ['Main.elm']), (['--debug'], ['Main.elm']), (['--optimize'], ['Main.elm']), ([], ['Second.elm', 'Main.elm'])):
        for json_report in (False, True):
            for temperature in ('cold', 'warm'):
                if temperature == 'cold':
                    shutil.rmtree(project / 'elm-stuff', ignore_errors=True)
                args = ['make', *entries, '--output=main.js', *mode]
                if json_report:
                    args.append('--report=json')
                runs = []
                for compiler in (a.elm, a.rust):
                    output = project / 'main.js'
                    output.unlink(missing_ok=True)
                    run = subprocess.run([str(compiler.resolve()), *args], cwd=project, env=env,
                                         capture_output=True, timeout=60)
                    # Preserve carriage returns: progress redraws are part of the CLI contract.
                    runs.append({'code': run.returncode, 'stdout': run.stdout.decode(),
                                 'stderr': run.stderr.decode(), 'generated': output.is_file() and output.stat().st_size > 0})
                passed = all(r['code'] == 0 and r['generated'] and not r['stderr'] for r in runs)
                passed &= all(not r['stdout'] if json_report else bool(r['stdout']) for r in runs)
                summary_match = json_report or runs[0]['stdout'].split('Success!', 1)[1] == runs[1]['stdout'].split('Success!', 1)[1]
                passed &= summary_match
                compilation_progress_match = json_report or (
                    'Compiling ...' in runs[0]['stdout'] and
                    'Compiling ...' in runs[1]['stdout'] and
                    runs[0]['stdout'][runs[0]['stdout'].index('Compiling ...'):] == runs[1]['stdout'][runs[1]['stdout'].index('Compiling ...'):]
                )
                passed &= compilation_progress_match
                results.append({'compilation_progress_match': compilation_progress_match, 'summary_match': summary_match, 'mode': mode, 'entries': entries, 'json': json_report, 'temperature': temperature,
                                'passed': passed, 'progress_text_match': runs[0]['stdout'] == runs[1]['stdout'],
                                'official': runs[0], 'rust': runs[1]})
                print(mode, json_report, temperature, 'PASS' if passed else 'FAIL', flush=True)
assert hashlib.sha256(a.rust.read_bytes()).hexdigest() == sha
report = {'scope': __doc__, 'compiler_sha256': sha, 'cases': len(results),
          'passed': all(r['passed'] for r in results), 'results': results}
a.report.write_text(json.dumps(report, indent=2) + '\n')
raise SystemExit(not report['passed'])
