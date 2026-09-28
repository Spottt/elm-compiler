#!/usr/bin/env python3
"""Compare public documentation diagnostics with Elm --docs output.

Tests public make --docs. Module, definition, name and selected syntax errors
compare complete JSON reports, uncolored stderr, and ANSI stderr in a pseudo-terminal.
Lexer/layout failures are not included in that claim.
"""
import argparse
from compiler_test_support import run_terminal
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tempfile

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm',type=Path,required=True)
p.add_argument('--report',type=Path)
args=p.parse_args()
crate=Path(__file__).resolve().parents[1]
extractor=crate/'target/release/planexpo-elm'
original=Path(os.environ.get('ELM_HOME',str(Path.home()/'.elm')))/'0.19.1/packages'
def module(exports, body, overview=None):
    if overview is None: overview = '@docs ' + exports.replace('(..)', '')
    return f"module Main exposing ({exports})\n{{-|{overview}-}}\nimport String\n" + body

cases = []
for annotation in [False, True]:
    for comment in [False, True]:
        body = ('{-| Value. -}\n' if comment else '') + ('value : Int\n' if annotation else '') + 'value = 1\n'
        cases.append((f'value-annotation-{annotation}-comment-{comment}', module('value', body)))
for kind, exports, declaration in [
    ('alias', 'Row', 'type alias Row = { x : Int }\n'),
    ('union-open', 'Choice(..)', 'type Choice = A | B\n'),
    ('union-closed', 'Choice', 'type Choice = A | B\n'),
]:
    for comment in [False, True]:
        cases.append((f'{kind}-comment-{comment}', module(exports, ('{-| Type. -}\n' if comment else '') + declaration)))
for annotation in [False, True]:
    for comment in [False, True]:
        body = 'infix left 5 (<+>) = combine\n' + ('{-| Adds. -}\n' if comment else '') + ('combine : Int -> Int -> Int\n' if annotation else '') + 'combine a b = a + b\n'
        cases.append((f'operator-annotation-{annotation}-comment-{comment}', module('(<+>)', body)))
valid = '{-| Value. -}\nvalue : Int\nvalue = 1\n'
cases.extend([
    ('empty-comment', module('value', valid.replace('{-| Value. -}', '{-|-}'))),
    ('private-value', module('value', valid + 'private = 2\n')),
    ('private-types', module('value', valid + 'type Hidden = Hidden\ntype alias Internal = Int\n')),
    ('private-documentation', module('value', valid + '{-| Private. -}\nprivate = 2\n')),
    ('multiple-errors', module('alpha, beta, gamma', 'alpha = 1\nbeta : Int\nbeta = 2\ngamma = 3\n')),
    ('names-before-definitions', module('value', 'value = 1\n', '@docs unknown')),
    ('operator-and-function', module('(<+>), combine', 'infix left 5 (<+>) = combine\n{-| Adds. -}\ncombine : Int -> Int -> Int\ncombine a b = a + b\n')),
    ('operator-and-function-no-annotation', module('(<+>), combine', 'infix left 5 (<+>) = combine\ncombine a b = a + b\n')),
    ('unused-private-operator', module('value', 'infix left 5 (<+>) = combine\ncombine a b = a + b\n' + valid)),
])
cases.extend([
    ('unicode-definition', module('café', 'café = 1\n')),
    ('unicode-export', module('café', 'café : Int\ncafé = 1\n')),
    ('multiline-header', module('\n    value\n    ', 'value : Int\nvalue = 1\n', '@docs value')),
    ('long-name', module('a' * 70, ('a' * 70) + ' = 1\n')),
    ('operator-and-function-no-comment', module('(<+>), combine', 'infix left 5 (<+>) = combine\ncombine : Int -> Int -> Int\ncombine a b = a + b\n')),
    ('line-number-width', module('value', '\n' * 12 + 'value = 1\n')),
])
cases.extend([
    ('implicit-exposing', 'module Main exposing (..)\n{-| @docs value -}\nvalue = 1\n'),
    ('implicit-before-no-docs', 'module Main exposing (..)\nvalue = 1\n'),
    ('implicit-multiline', 'module Main exposing\n    ( ..\n    )\nvalue = 1\n'),
    ('missing-module-docs', 'module Main exposing (value)\nimport String\nvalue = 1\n'),
    ('missing-module-docs-no-imports', 'module Main exposing (value)\nvalue = 1\n'),
    ('missing-module-docs-blank-lines', 'module Main exposing (value)\n\n\nimport String\nvalue = 1\n'),
    ('missing-module-docs-comment', 'module Main exposing (value)\n{- ordinary comment -}\nimport String\nvalue = 1\n'),
    ('missing-module-docs-line-comment', 'module Main exposing (value) -- ordinary\nimport String\nvalue = 1\n'),
    ('missing-module-docs-multiline-header', 'module Main exposing\n    ( value\n    )\nimport String\nvalue = 1\n'),
])
cases.extend([
    ('name-missing', module('value', valid, 'No names here.')),
    ('name-extra', module('value', valid, '@docs value, extra')),
    ('name-missing-and-extra', module('value', valid, '@docs other')),
    ('name-duplicate-inline', module('value', valid, '@docs value, value')),
    ('name-duplicate-lines', module('value', valid, '@docs value\n@docs value')),
    ('name-triplicate', module('value', valid, '@docs value, value, value')),
    ('name-triplicate-lines', module('value', valid, '@docs value\n@docs value\n@docs value')),
    ('name-unknown-duplicate', module('value', valid, '@docs value, extra, extra')),
    ('name-unicode-missing', module('café', 'café = 1\n', 'No names here.')),
    ('name-unicode-extra', module('value', valid, '@docs value, café')),
    ('name-unicode-duplicate', module('café', 'café = 1\n', '@docs café, café')),
    ('name-header-multiline', module('\n    value\n    ', valid, 'No names here.')),
    ('name-long-extra', module('value', valid, '@docs value, ' + 'a' * 70)),
    ('name-duplicate-distant', module('value', valid, '@docs value\n' + '\n' * 12 + '@docs value')),
    ('name-operator-missing', module('(<+>)', 'infix left 5 (<+>) = combine\ncombine a b = a + b\n', 'No names here.')),
    ('name-operator-extra', module('value', valid, '@docs value, (<+>)')),
    ('name-operator-duplicate', module('(<+>)', 'infix left 5 (<+>) = combine\ncombine a b = a + b\n', '@docs (<+>), (<+>)')),
    ('name-many-errors', module('alpha, beta', 'alpha = 1\nbeta = 2\n', '@docs alpha, alpha, extra')),
])
cases.extend((f'name-cr-{name}', module('value', valid, overview)) for name, overview in [
    ('before-first', '@docs\r value, extra'),
    ('before-extra', '@docs value,\r extra'),
    ('duplicate-first', '@docs\r value, value'),
    ('duplicate-second', '@docs value,\r value'),
    ('duplicate-lines', '@docs\r value\n@docs\r value'),
    ('prose', 'Prose\r @docs value, extra'),
    ('comment', '@docs value, {- ordinary\r comment -} extra'),
    ('crlf', '@docs value,\r\n extra'),
    ('unicode', '@docs value,\r café'),
    ('operator', '@docs value,\r (<+>)'),
])
cases.extend((f'syntax-{name}', module('value', valid, overview)) for name, overview in [
    ('empty', '@docs'), ('trailing-comma', '@docs value,'),
    ('cr-spacing', '@docs\r (|)'), ('cr-prose', 'text\r @docs if'),
    ('cr-reserved-name', '@docs\r if'),
    ('reserved-name', '@docs if'), ('underscore', '@docs _value'),
    ('empty-operator', '@docs ()'), ('operator-space', '@docs (+ )'),
    ('operator-no-close', '@docs (+'), ('operator-dot', '@docs (.)'),
    ('operator-pipe', '@docs (|)'), ('operator-arrow', '@docs (->)'),
    ('operator-equals', '@docs (=)'), ('operator-colon', '@docs (:)'),
    ('comma-before-name', '@docs ,'), ('reserved-after-comma', '@docs value, case'),
    ('multiline', '@docs value,\n    '), ('unicode-prose', 'Éléments 😀. @docs _value'),
])
def named(name, overview='@docs value', body='value = 1\n'):
    return module('value', body, overview).replace('module Main ', f'module {name} ', 1)
cases.extend([
    ('modules-independent', {'Alpha':named('Alpha'), 'Beta':named('Beta')}),
    ('modules-reverse-time', {'Beta':named('Beta'), 'Alpha':named('Alpha')}),
    ('modules-three', {'Zebra':named('Zebra'), 'Alpha':named('Alpha'), 'Middle':named('Middle')}),
    ('modules-blocked-importer', {'Alpha':named('Alpha'), 'Beta':named('Beta', body='import Alpha\nvalue = Alpha.value\n')}),
    ('modules-blocked-chain-independent', {'Alpha':named('Alpha'), 'Beta':named('Beta', body='import Alpha\nvalue = Alpha.value\n'), 'Gamma':named('Gamma', body='import Beta\nvalue = Beta.value\n'), 'Other':named('Other')}),
    ('modules-valid-dependency', {'Alpha':named('Alpha', body=valid), 'Beta':named('Beta', body='import Alpha\nvalue = Alpha.value\n'), 'Other':named('Other')}),
    ('modules-docs-and-syntax', {'Alpha':named('Alpha'), 'Beta':named('Beta', body='import Alpha\nvalue =\n'), 'Other':named('Other')}),
    ('modules-docs-and-types', {'Alpha':named('Alpha'), 'Beta':named('Beta', body='{-| Value. -}\nvalue : Int\nvalue = \"bad\"\n'), 'Other':named('Other')}),
    ('modules-mixed-doc-errors', {'Alpha':named('Alpha', '@docs unknown'), 'Beta':named('Beta', '@docs (|)'), 'Other':named('Other')}),
])
results,failures=[],[]
with tempfile.TemporaryDirectory(prefix='elm-doc-comments-') as tmp:
    root=Path(tmp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat',cache/'registry.dat')
    shutil.copytree(original/'elm/core/1.0.5/src',cache/'elm/core/1.0.5/src')
    shutil.copyfile(original/'elm/core/1.0.5/elm.json',cache/'elm/core/1.0.5/elm.json')
    env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1 -A16m -c'}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:env[key]='http://127.0.0.1:9'
    env.update(no_proxy='',NO_PROXY='')
    for index,(name,source) in enumerate(cases):
        project=root/str(index);(project/'src').mkdir(parents=True)
        config={'type':'package','name':'elm/fixture','summary':'Documentation comment fixture.',
                'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':list(source) if isinstance(source, dict) else ['Main'],
                'elm-version':'0.19.0 <= v < 0.20.0','dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
        (project/'elm.json').write_text(json.dumps(config))
        sources = source if isinstance(source, dict) else {'Main': source}
        for order, (module_name, module_source) in enumerate(sources.items()):
            path = project / 'src' / (module_name.replace('.', '/') + '.elm')
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(module_source)
            os.utime(path, (1700000000 + order, 1700000000 + order))
        run=subprocess.run([str(args.elm.resolve()),'make','--docs=docs.json','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
        inspected = subprocess.run([str(extractor),'make','--docs=rust-docs.json','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30)
        expected = actual = None
        if run.returncode == 0:
            docs=json.loads((project/'docs.json').read_text())[0]
            expected={'overview':docs['comment'], 'declarations':{
                entry['name']:entry['comment'] for category in ['values','aliases','unions','binops'] for entry in docs[category]
            }}
        if inspected.returncode == 0:
            document=json.loads((project/'rust-docs.json').read_text())[0]
            actual={'overview':document['comment'], 'declarations':{entry['name']:entry['comment'] for category in ['values','aliases','unions','binops'] for entry in document[category]}}
        if (run.returncode == 0) != (inspected.returncode == 0) or expected != actual:
            failures.append(name)
        official_titles = []
        if run.returncode:
            error = json.loads(run.stderr)
            official_titles = [problem['title'] for entry in error.get('errors', []) for problem in entry['problems']]
        rust_report = json.loads(inspected.stderr) if inspected.returncode else None
        rust_titles = [problem['title'] for entry in (rust_report or {}).get('errors', []) for problem in entry['problems']]
        if name in ['modules-docs-and-types', 'modules-docs-and-syntax']:
            expected_modules = [(entry['name'], [p['title'] for p in entry['problems']]) for entry in error['errors']]
            actual_modules = [(entry['name'], [p['title'] for p in entry['problems']]) for entry in rust_report.get('errors', [])]
            if name == 'modules-docs-and-syntax':
                expected_modules = [entry[0] for entry in expected_modules]
                actual_modules = [entry[0] for entry in actual_modules]
            if expected_modules != actual_modules:
                failures.append(name + ': module error categories or identities')
        compared_full = bool(official_titles) and all(title in ['NO DOCS','NO TYPE ANNOTATION','IMPLICIT EXPOSING','DOCS MISTAKE','DUPLICATE DOCS','PROBLEM IN DOCS'] for title in official_titles)
        if compared_full and rust_report != error:
            failures.append(name + ': full JSON diagnostic')
        official_terminal = rust_terminal = official_ansi = rust_ansi = None
        if compared_full:
            terminal_args = ['make', '--docs=terminal-docs.json']
            official_plain = subprocess.run([str(args.elm.resolve()), *terminal_args], cwd=project, env=env, capture_output=True, text=True, timeout=30)
            rust_plain = subprocess.run([str(extractor), *terminal_args], cwd=project, env=env, capture_output=True, text=True, timeout=30)
            official_terminal, rust_terminal = official_plain.stderr, rust_plain.stderr
            official_ansi_code, official_ansi = run_terminal([str(args.elm.resolve()), *terminal_args], project, env)
            rust_ansi_code, rust_ansi = run_terminal([str(extractor), *terminal_args], project, env)
            if official_ansi_code != rust_ansi_code or official_ansi != rust_ansi:
                failures.append(name + ': ANSI terminal diagnostic')
            if name in ['multiple-errors', 'modules-independent']:
                json_args = [*terminal_args, '--report=json']
                official_json_code, official_json_tty = run_terminal([str(args.elm.resolve()), *json_args], project, env)
                rust_json_code, rust_json_tty = run_terminal([str(extractor), *json_args], project, env)
                if (official_json_code != rust_json_code or '\x1b' in rust_json_tty
                        or json.loads(official_json_tty) != json.loads(rust_json_tty)):
                    failures.append(name + ': JSON in terminal')
            if name in ['multiple-errors', 'name-unicode-extra', 'syntax-empty']:
                nested = project / 'nested' / 'working'
                nested.mkdir(parents=True)
                official_nested = subprocess.run([str(args.elm.resolve()), *terminal_args], cwd=nested, env=env, capture_output=True, text=True, timeout=30)
                rust_nested = subprocess.run([str(extractor), *terminal_args], cwd=nested, env=env, capture_output=True, text=True, timeout=30)
                if official_nested.returncode != rust_nested.returncode or official_nested.stderr != rust_nested.stderr:
                    failures.append(name + ': nested plain terminal diagnostic')
            if official_plain.returncode != rust_plain.returncode or official_terminal != rust_terminal:
                failures.append(name + ': plain terminal diagnostic')
        results.append({'official_ansi':official_ansi,'rust_ansi':rust_ansi,'official_terminal':official_terminal,'rust_terminal':rust_terminal,'full_diagnostic_compared':compared_full,'official_titles':official_titles,'rust_definition_titles':rust_titles,'case':name,'official_accepted':run.returncode==0,'rust_accepted':inspected.returncode==0,
                        'expected':expected,'actual':actual,'official_stderr':run.stderr,'rust_stderr':inspected.stderr})
report={'scope':__doc__,'cases':len(results),'failures':failures,'results':results,
        'extractor_sha256':hashlib.sha256(extractor.read_bytes()).hexdigest()}
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(results),'failures':failures}))
raise SystemExit(bool(failures))
