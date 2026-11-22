#!/usr/bin/env python3
"""Compare the shape of the generated JavaScript with official Elm's.

Counts, definition by definition, the constructs that make generated code
slower than it needs to be: function expressions (curried closures and
immediately-invoked functions add some), structural helper calls where a
direct comparison would do, and operators called as functions. This compiler
must not emit more of any of them than official Elm for the same program, in
development and in --optimize builds.

The check is deterministic: it reads the output, it does not time it. See
check_runtime_performance.py for timings.
"""
import json
import re
import tempfile

import generated_code_support as support

OPERATORS = 'add|sub|mul|fdiv|idiv|eq|neq|lt|gt|le|ge|and|or|xor|append|apL|apR'
METRICS = {
    'function expressions': re.compile(r'\bfunction\b'),
    'structural helper calls': re.compile(r'\b_Utils_(?:eq|cmp)\('),
    'operator calls': re.compile(r'\bA2\(\s*\$elm\$core\$Basics\$(?:'+OPERATORS+r')\b'),
}
DEFINITION = re.compile(r'^var (\$[\w$]+)\s*=', re.MULTILINE)


def definitions(bundle):
    """Generated top-level definitions by name; the kernel preamble is left out."""
    parts = DEFINITION.split(bundle.read_text())
    return dict(zip(parts[1::2], parts[2::2]))


def compare(bundles):
    generated = {name: definitions(bundles[name]) for name in support.COMPILERS}
    shared = sorted(set(generated['elm']) & set(generated['rust']))
    result = {'definitions': len(shared), 'metrics': {}}
    for metric, pattern in METRICS.items():
        counts = {name: {key: len(pattern.findall(generated[name][key])) for key in shared}
                  for name in support.COMPILERS}
        excess = {key: [counts['elm'][key], counts['rust'][key]] for key in shared
                  if counts['rust'][key] > counts['elm'][key]}
        totals = {name: sum(counts[name].values()) for name in support.COMPILERS}
        result['metrics'][metric] = {
            **totals,
            'match': totals['rust'] <= totals['elm'],
            'worst': dict(sorted(excess.items(), key=lambda item: item[1][0]-item[1][1])[:10]),
        }
    return result


def main():
    args = support.arguments(__doc__.splitlines()[0]).parse_args()
    compilers = support.binaries(args)
    with tempfile.TemporaryDirectory(prefix='elm-rust-shapes-') as directory:
        bundles = support.compile_bundles(compilers, directory)
        modes = {mode: compare(bundles[mode]) for mode in support.MODES}
    passed = all(metric['match'] for mode in modes.values() for metric in mode['metrics'].values())
    report = {'compilers': {name: support.sha256(path) for name, path in compilers.items()},
              'modes': modes, 'match': passed}
    if args.report:
        args.report.write_text(json.dumps(report, indent=2)+'\n')
    for mode, result in modes.items():
        print(f'{mode} ({result["definitions"]} shared definitions)')
        for metric, row in result['metrics'].items():
            verdict = 'ok' if row['match'] else 'MORE THAN OFFICIAL'
            print(f'  {metric:<24} official {row["elm"]:>5}  this compiler {row["rust"]:>5}  {verdict}')
            if not row['match']:
                for name, (official, ours) in list(row['worst'].items())[:3]:
                    print(f'      {name}: {official} -> {ours}')
    raise SystemExit(0 if passed else 1)


if __name__ == '__main__':
    main()
