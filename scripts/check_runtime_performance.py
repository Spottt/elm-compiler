#!/usr/bin/env python3
"""Time the generated JavaScript against official Elm's on the same program.

Each test of tests/programs/runtime-performance runs in Node with both
compilers' output, in development and in --optimize builds. The two outputs
must compute the same result, and this compiler's median time must stay within
--threshold times official Elm's. Only the ratio is judged, never an absolute
time, and the two bundles are timed in alternation so that machine load
affects both.

Timings are noisy on shared machines: the default threshold leaves room for
that. check_generated_shapes.py is the deterministic counterpart.
"""
import json
import statistics
import subprocess
import tempfile

import generated_code_support as support

# Iteration counts keep every test around a tenth of a second with official Elm.
TESTS = {'cases': 5_000_000, 'records': 5_000_000, 'calls': 2_000_000,
         'comparisons': 5_000_000, 'locals': 3_000_000, 'folds': 1_000_000,
         'nested': 3_000_000, 'dict': 250_000, 'strings': 1_500_000,
         'decoding': 200_000}
RUNNER = r'''
const { Elm } = require(process.argv[1]);
const tests = JSON.parse(process.argv[2]);
const runs = Number(process.argv[3]);
const json = Array.from({ length: tests.decoding }, (_, i) => ({ a: i, b: i, c: i, d: i, e: i % 7 }));
function once(test) {
  return new Promise((resolve) => {
    const start = performance.now();
    const app = Elm.Main.init({ flags: { test, n: tests[test], json } });
    app.ports.done.subscribe((result) => resolve({ ms: performance.now() - start, result }));
  });
}
(async () => {
  const output = {};
  for (const test of Object.keys(tests)) {
    const warmup = await once(test);
    const ms = [];
    for (let i = 0; i < runs; i++) {
      const run = await once(test);
      if (run.result !== warmup.result) throw new Error(test + ': unstable result');
      ms.push(run.ms);
    }
    output[test] = { ms, result: warmup.result };
  }
  console.log(JSON.stringify(output));
})();
'''


def measure(bundle, runs):
    run = subprocess.run(['node', '-e', RUNNER, str(bundle), json.dumps(TESTS), str(runs)],
                         capture_output=True, text=True, timeout=600)
    if run.returncode != 0:
        raise SystemExit(f'{bundle} failed to run:\n{run.stderr[-3000:]}')
    return json.loads(run.stdout)


def compare(bundles, rounds, runs, threshold):
    samples = {name: {test: [] for test in TESTS} for name in support.COMPILERS}
    results = {name: {} for name in support.COMPILERS}
    for index in range(rounds):
        # Alternate which bundle goes first, so neither always runs on a warmer machine.
        order = support.COMPILERS if index % 2 == 0 else support.COMPILERS[::-1]
        for name in order:
            for test, row in measure(bundles[name], runs).items():
                samples[name][test].extend(row['ms'])
                results[name][test] = row['result']
    tests = {}
    for test in TESTS:
        elm, rust = (statistics.median(samples[name][test]) for name in support.COMPILERS)
        same = results['elm'][test] == results['rust'][test]
        tests[test] = {'elm_ms': round(elm, 1), 'rust_ms': round(rust, 1), 'ratio': round(rust/elm, 2),
                       'result': results['elm'][test], 'same_result': same,
                       'match': same and rust <= elm*threshold}
    return tests


def main():
    parser = support.arguments(__doc__.splitlines()[0])
    parser.add_argument('--threshold', type=float, default=1.25,
                        help='Largest accepted ratio of this compiler to official Elm (default: 1.25)')
    parser.add_argument('--rounds', type=int, default=3, help='Alternating Node processes per bundle (default: 3)')
    parser.add_argument('--runs', type=int, default=3, help='Timed runs per test in each process (default: 3)')
    args = parser.parse_args()
    compilers = support.binaries(args)
    with tempfile.TemporaryDirectory(prefix='elm-rust-performance-') as directory:
        bundles = support.compile_bundles(compilers, directory)
        modes = {mode: compare(bundles[mode], args.rounds, args.runs, args.threshold) for mode in support.MODES}
    passed = all(test['match'] for mode in modes.values() for test in mode.values())
    node = subprocess.run(['node', '--version'], capture_output=True, text=True).stdout.strip()
    report = {'compilers': {name: support.sha256(path) for name, path in compilers.items()},
              'node': node, 'threshold': args.threshold, 'samples_per_test': args.rounds*args.runs,
              'modes': modes, 'match': passed}
    if args.report:
        args.report.write_text(json.dumps(report, indent=2)+'\n')
    for mode, tests in modes.items():
        print(f'{mode} (median of {args.rounds*args.runs}, ms)')
        for test, row in tests.items():
            verdict = ('ok' if row['match'] else
                       'SLOWER' if row['same_result'] else 'DIFFERENT RESULT')
            print(f'  {test:<12} official {row["elm_ms"]:>7.1f}  this compiler {row["rust_ms"]:>7.1f}'
                  f'  x{row["ratio"]:<5} {verdict}')
    raise SystemExit(0 if passed else 1)


if __name__ == '__main__':
    main()
