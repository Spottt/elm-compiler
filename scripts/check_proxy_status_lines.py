#!/usr/bin/env python3
"""Opt-in comparison of proxy status parsing against the official Elm binary.

Uses the public package registry through the existing scoped CONNECT proxy.
Every child creates private package caches; no system proxy/trust changes.
"""
import argparse
import concurrent.futures
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
for key in ('rust', 'elm', 'report'):
    parser.add_argument('--' + key, type=Path, required=True)
args = parser.parse_args()
rust = args.rust.resolve()
elm = args.elm.resolve()
sha = hashlib.sha256(rust.read_bytes()).hexdigest()
cases = [
    ('99-headers', 'HTTP/1.1 502 Bad Gateway\r\n' + 'X: value\r\n' * 99 + '\r\n'),
    ('100-headers', 'HTTP/1.1 502 Bad Gateway\r\n' + 'X: value\r\n' * 100 + '\r\n'),
    ('large-total-headers', 'HTTP/1.1 502 Bad Gateway\r\n' + ('X: ' + 'v' * 200 + '\r\n') * 80 + '\r\n'),
    ('ignored-header-lines', 'HTTP/1.1 502 Bad Gateway\r\n' + 'invalid line\r\n' * 110 + '\r\n'),
    ('continue-many-headers', 'HTTP/1.1 100 Continue\r\n' + 'X: value\r\n' * 110 + '\r\nHTTP/1.1 502 Bad Gateway'),
    ('long-line-fragmented', 'HTTP/1.1 502 Bad Gateway\r\nX: ' + 'v' * 4500 + '\r\n\r\n'),
    ('shorter-line-fragmented', 'HTTP/1.1 502 Bad Gateway\r\nX: ' + 'v' * 4000 + '\r\n\r\n'),
    ('lf-endings', 'HTTP/1.1 502 Bad Gateway\nHeader: value\n\n'),
    ('mixed-endings', 'HTTP/1.1 502 Bad Gateway\nHeader: value\r\n\n'),
    ('three-blank-lines', '\r\n' * 3 + 'HTTP/1.1 502 Bad Gateway'),
    ('four-blank-lines', '\r\n' * 4 + 'HTTP/1.1 502 Bad Gateway'),
    ('blank-line-eof', '\r\n'),
    ('continue-lf', 'HTTP/1.1 100 Continue\n\nHTTP/1.1 502 Bad Gateway\n\n'),
    ('blank-lines-reset-after-continue', '\r\n' * 3 + 'HTTP/1.1 100 Continue\r\n\r\n' + '\r\n' * 3 + 'HTTP/1.1 502 Bad Gateway'),
    ('continue-reject', 'HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 502 Bad Gateway'),
    ('continue-reject-fragmented', 'HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 502 Bad Gateway'),
    ('repeated-continue', 'HTTP/1.1 100 Continue\r\nHeader: value\r\n\r\nHTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 502 Bad Gateway'),
    ('continue-eof', 'HTTP/1.1 100 Continue'),
    ('early-hints', 'HTTP/1.1 103 Early Hints\r\n\r\nHTTP/1.1 502 Bad Gateway'),
    ('invalid-text', 'NOT HTTP'),
    ('invalid-code', 'HTTP/1.1 xyz Broken'),
    ('numeric-version', 'HTTP/7.9 502 Bad Gateway'),
    ('extra-spaces', 'HTTP/1.1   502   Bad Gateway'),
    ('empty-reason', 'HTTP/1.1 502'),
    ('long-code', 'HTTP/1.1 1234 Custom'),
    ('signed-numbers', 'HTTP/+1.01 +502 Custom'),
    ('escaped-invalid-line', 'BAD "quote" \\ byte\xff1'),
]
helper = Path(__file__).with_name('check_download_failure_progress.py')
with tempfile.TemporaryDirectory(prefix='proxy-status-lines-') as directory:
    root = Path(directory)

    def run(case):
        name, line = case
        report = root / (name + '.json')
        completed = subprocess.run(
            [sys.executable, str(helper), '--rust', str(rust), '--elm', str(elm),
             '--disconnect', '--partial-response', line if line.endswith('\n') else line + '\r\n\r\n',
             '--report', str(report), '--registry-continue', '--registry-lf',
             *(['--fragmented'] if name.endswith('-fragmented') else [])], capture_output=True, text=True, timeout=200,
        )
        detail = json.loads(report.read_text()) if report.exists() else None
        return {'case': name, 'response_line': line, 'exit_code': completed.returncode,
                'passed': completed.returncode == 0 and detail is not None and detail['passed'],
                'detail': detail, 'harness_stderr': completed.stderr}

    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        results = list(pool.map(run, cases))
unchanged = hashlib.sha256(rust.read_bytes()).hexdigest() == sha
report = {'scope': __doc__, 'rust_sha256': sha, 'binary_unchanged': unchanged,
          'passed': unchanged and all(result['passed'] for result in results), 'results': results}
args.report.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'passed': report['passed'], 'cases': len(results), 'report': str(args.report)}))
raise SystemExit(not report['passed'])
