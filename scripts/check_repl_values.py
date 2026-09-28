#!/usr/bin/env python3
"""Compare compiled REPL value evaluation and output layout with Elm's REPL.

By default the probe receives explicit fixture type text. --infer-structural
instead exercises experimental inferred type rendering without excluding known
gaps. Input classification, interactive history and CLI initialization are not
tested here.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    crate = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elm', type=Path, required=True)
    parser.add_argument('--probe', type=Path, default=crate/'target/debug/examples/repl_value')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--infer-structural', action='store_true', help='Exercise experimental inferred structural type rendering, including currently unsupported alias/name cases')
    args = parser.parse_args()
    cases = [
        ('integer', '1 + 2', 'number'),
        ('float', '1 / 2', 'Float'),
        ('boolean', 'True', 'Bool'),
        ('string', '"hello"', 'String'),
        ('escaped-string', '"a\\nb\\t\\\"c"', 'String'),
        ('unit', '()', '()'),
        ('tuple', '(1, "x")', '( number, String )'),
        ('record', '{ name = "Ada", age = 3 }', '{ age : number, name : String }'),
        ('list', 'List.sort [3,1,2]', 'List number'),
        ('maybe', 'Just (1, True)', 'Maybe ( number, Bool )'),
        ('result', 'Err "bad"', 'Result String value'),
        ('function', '\\x -> x', 'a -> a'),
        ('function-argument', '\\f x -> f (Just x)', '(Maybe a -> b) -> a -> b'),
        ('record-accessor', '.field', '{ b | field : a } -> a'),
        ('empty-record', '{}', '{}'),
        ('independent-numbers', '(1, 2)', '( number, number1 )'),
        ('nested-type', '[ Just () ]', 'List (Maybe ())'),
        ('list-function', '\\x -> [x]', 'a -> List a'),
        ('reserved-fields', '{ default = True, class = False }', '{ class : Bool, default : Bool }'),
        ('record-update', 'let r = { z = String.length "a", a = String.length "ab" } in { r | z = String.length "aaa", a = String.length "aaaa" }', '{ a : Int, z : Int }'),
        ('record-alias', 'Person 7 "Ada"', 'Person'),
        ('wide-record-alias', 'Wide 1 2 3 4 5 6 7 8 9 10', 'Wide'),
        ('generic-record-alias', 'Box ()', 'Box ()'),
        ('alias-constructor-function', 'Box', 'item -> Box item'),
        ('alias-inside-list', 'List.map Box [1,2]', 'List (Box number)'),
        ('phantom-record-alias', 'Phantom 42', 'Phantom a'),
        ('width-79', '"'+'x'*68+'"', 'String'),
        ('width-80', '"'+'x'*69+'"', 'String'),
    ]
    preludes = {'record-alias': 'type alias Person = { z : Int, a : String }',
                'wide-record-alias': 'type alias Wide = { '+', '.join(name+' : Int' for name in 'zyxwvutsrq')+' }',
                'generic-record-alias': 'type alias Box item = { item : item }',
                'alias-constructor-function': 'type alias Box item = { item : item }',
                'alias-inside-list': 'type alias Box item = { item : item }',
                'phantom-record-alias': 'type alias Phantom a = { value : Int }'}
    results = []
    if args.infer_structural:
        # Layout probes use actual inference; no hand-written type text is fed
        # to the compiler. Retain every comparison, including boundary failures.
        long_fields = ', '.join('field'+str(i)+'x'*16+' = ()' for i in range(5))
        cases.extend([
            ('long-record-layout', '{ '+long_fields+' }', None),
            ('long-tuple-layout', '('+', '.join('{ '+long_fields+' }' for _ in range(3))+')', None),
            ('long-function-layout', '\\f -> f { '+long_fields+' }', None),
        ])
        for width in [28, 29, 30, 31, 32, 65]:
            first, second = 'a'+'x'*width, 'b'+'x'*width
            record = '{ '+first+' = (), '+second+' = () }'
            cases.extend([
                ('record-width-'+str(width), record, None),
                ('application-width-'+str(width), 'Just '+record, None),
                ('open-record-width-'+str(width), '\\r -> (r.'+first+', r.'+second+')', None),
            ])
    with tempfile.TemporaryDirectory(prefix='elm-repl-values-') as temporary:
        root = Path(temporary)
        original_home = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))
        home = root/'home'
        cache = home/'0.19.1/packages'
        cache.mkdir(parents=True)
        shutil.copy2(original_home/'0.19.1/packages/registry.dat', cache/'registry.dat')
        for package in ['core/1.0.5', 'json/1.1.3']:
            shutil.copytree(original_home/'0.19.1/packages/elm'/package, cache/'elm'/package)
        (root/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes())
        env = {**os.environ, 'ELM_HOME': str(home), 'GHCRTS': '-N1'}
        for key in ['http_proxy', 'https_proxy', 'all_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY']:
            env[key] = 'http://127.0.0.1:9'
        env['NO_PROXY'] = env['no_proxy'] = ''
        for label, expression, annotation in cases:
            prelude = preludes.get(label, '')
            # An unused crashing top-level value must not execute while evaluating value.
            (root/'Repl.elm').write_text('module Repl exposing (..)\n'+prelude+'\nvalue = '+expression+'\nunused = Debug.todo "must stay unreachable"\n')
            for ansi in [False, True]:
                official = subprocess.run([str(args.elm.resolve()), 'repl']+([] if ansi else ['--no-colors']),
                                          cwd=root, env=env, input=(prelude+'\n' if prelude else '')+expression+'\n:exit\n', text=True,
                                          capture_output=True, timeout=45)
                body = official.stdout.split('\n', 3)[-1]
                prefix = '> > ' if prelude else '> '
                framed = body.startswith(prefix) and body.endswith('\n> ')
                expected = body[len(prefix):-2] if framed else None
                script = root/'repl.js'
                compilation = subprocess.run([str(args.probe.resolve()), str(home), str(root/'elm.json'),
                                              'Repl.elm', 'value', '--infer-structural' if args.infer_structural else annotation, str(script)]+(['--ansi'] if ansi else []),
                                             cwd=root, env=env, text=True, capture_output=True, timeout=45)
                actual = subprocess.run(['node', str(script)], text=True, capture_output=True, timeout=15) if compilation.returncode == 0 else None
                passed = (official.returncode == 0 and not official.stderr and framed
                          and actual is not None and actual.returncode == 0 and not actual.stderr and actual.stdout == expected)
                rejection_control = label == 'phantom-record-alias'
                if rejection_control:
                    # Elm forbids unused alias parameters. Preserve this case
                    # as a rejection control rather than comparing value text.
                    passed = (official.returncode == 0 and 'UNUSED TYPE VARIABLE' in official.stderr
                              and compilation.returncode != 0 and 'unused type parameter a' in compilation.stderr
                              and actual is None)
                results.append({'case': label, 'ansi': ansi, 'passed': passed, 'fixture_type': annotation,
                                'comparison': 'unused-alias-parameter rejection' if rejection_control else 'value and type output',
                                'official_stdout': official.stdout, 'official_stderr': official.stderr,
                                'compilation_stderr': compilation.stderr,
                                'rust_stdout': actual.stdout if actual else None, 'rust_stderr': actual.stderr if actual else None})
    report = {'scope': __doc__, 'probe_sha256': hashlib.sha256(args.probe.read_bytes()).hexdigest(),
              'inferred_structural_types': args.infer_structural,
              'passed': all(r['passed'] for r in results), 'cases': len(results), 'results': results}
    args.report.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({'cases': len(results), 'failures': [(r['case'],r['ansi']) for r in results if not r['passed']]}))
    raise SystemExit(not report['passed'])


if __name__ == '__main__':
    main()
