#!/usr/bin/env python3
"""Compare what loading the generated bundle costs with official Elm's.

Two things make a bundle heavier to load without changing what it computes:
dead code that official Elm leaves out, and functions the JavaScript engine
compiles before they are ever called. V8 compiles a parenthesised function
expression, `(function(){...})`, as soon as it parses it, on the assumption
that it is called on the spot; official Elm only writes plain ones.

For a worker and a browser program, in development and --optimize builds, this
compiler's bundle must not be more than 10% larger than official Elm's, must
not contain the time-travelling debugger, and must not make V8 compile many
more functions at load time (counted by V8 itself, with --log-function-events).
"""
import json
from pathlib import Path
import subprocess
import tempfile

import generated_code_support as support

PROGRAMS = {'worker': support.PROGRAM, 'browser': support.CRATE/'tests/programs/hot-reload'}
SIZE_RATIO = 1.10
# Official Elm's immediately-invoked functions are compiled lazily and ours
# eagerly: both are called at once, so the allowance covers that difference.
EAGER_RATIO, EAGER_SLACK = 1.2, 10


def eagerly_compiled(bundle, directory):
    """Functions V8 fully parses while the bundle loads, before any call."""
    log = Path(directory)/(bundle.parent.name+'-'+bundle.stem+'.v8.log')
    run = subprocess.run(['node', '--log-function-events', '--logfile='+str(log), '--no-logfile-per-isolate',
                          '-e', 'require(process.argv[1])', str(bundle)], capture_output=True, text=True, timeout=120)
    if run.returncode != 0:
        raise SystemExit(f'{bundle} failed to load:\n{run.stderr[-3000:]}')
    return sum(line.startswith('function,full-parse,') for line in log.read_text().splitlines())


def compare(bundles, directory):
    rows = {}
    for name in support.COMPILERS:
        source = bundles[name].read_text()
        rows[name] = {'bytes': len(source.encode()), 'debugger_definitions': source.count('$elm$browser$Debugger$'),
                      'eagerly_compiled': eagerly_compiled(bundles[name], directory)}
    elm, rust = rows['elm'], rows['rust']
    checks = {'size': rust['bytes'] <= elm['bytes']*SIZE_RATIO,
              'debugger': rust['debugger_definitions'] <= elm['debugger_definitions'],
              'eager compilation': rust['eagerly_compiled'] <= elm['eagerly_compiled']*EAGER_RATIO+EAGER_SLACK}
    return {**rows, 'checks': checks, 'match': all(checks.values())}


def main():
    args = support.arguments(__doc__.splitlines()[0]).parse_args()
    compilers = support.binaries(args)
    results = {}
    with tempfile.TemporaryDirectory(prefix='elm-rust-bundle-load-') as directory:
        for program, path in PROGRAMS.items():
            bundles = support.compile_bundles(compilers, Path(directory)/program, path)
            for mode in support.MODES:
                results[f'{program}, {mode}'] = compare(bundles[mode], directory)
    passed = all(row['match'] for row in results.values())
    report = {'compilers': {name: support.sha256(path) for name, path in compilers.items()},
              'results': results, 'match': passed}
    if args.report:
        args.report.write_text(json.dumps(report, indent=2)+'\n')
    for label, row in results.items():
        failed = [check for check, ok in row['checks'].items() if not ok]
        print(f'{label:<22} size {row["elm"]["bytes"]:>7} -> {row["rust"]["bytes"]:>7}'
              f'  compiled at load {row["elm"]["eagerly_compiled"]:>4} -> {row["rust"]["eagerly_compiled"]:>4}'
              f'  debugger {row["rust"]["debugger_definitions"]:>3}  '+('ok' if not failed else 'FAILED: '+', '.join(failed)))
    raise SystemExit(0 if passed else 1)


if __name__ == '__main__':
    main()
