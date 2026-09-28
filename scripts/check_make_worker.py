#!/usr/bin/env python3
"""Compare a persistent worker with fresh CLI processes after edits and errors."""
import argparse, hashlib, json, os, queue, shutil, subprocess, tempfile, threading
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rust', type=Path, required=True)
parser.add_argument('--oracle', type=Path, help='Independent reference binary, always invoked with --no-cache')
parser.add_argument('--report', type=Path, required=True)
parser.add_argument('--stream-batches', action='store_true')
args = parser.parse_args()
binary = args.rust.resolve()
oracle = args.oracle.resolve() if args.oracle else binary
digest = hashlib.sha256(binary.read_bytes()).hexdigest()
home = Path(os.environ.get('ELM_HOME', Path.home()/'.elm'))
args.report.parent.mkdir(parents=True, exist_ok=True)
manifest = {'type':'application','source-directories':['src'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}
main = '''module Main exposing (main)
import Helper
main : Program () Int Never
main = Platform.worker { init = \\() -> (Helper.answer, Cmd.none), update = \\_ model -> (model, Cmd.none), subscriptions = \\_ -> Sub.none }
'''
helper = 'module Helper exposing (answer)\nanswer : Int\nanswer = 42\n'
rows = []
with tempfile.TemporaryDirectory(prefix='worker-parity-', dir=args.report.parent) as temporary:
    root = Path(temporary)
    packages = root/'home/0.19.1/packages'
    packages.mkdir(parents=True)
    shutil.copy2(home/'0.19.1/packages/registry.dat', packages/'registry.dat')
    for package in ['elm/core/1.0.5', 'elm/json/1.1.3']:
        shutil.copytree(home/'0.19.1/packages'/package, packages/package)
    env = dict(os.environ, ELM_HOME=str(root/'home'))
    projects = {label:root/label for label in ['cli', 'worker']}
    for project in projects.values():
        (project/'src').mkdir(parents=True)
        (project/'elm.json').write_text(json.dumps(manifest))
    process = subprocess.Popen([str(binary), '--internal-make-worker'], cwd=projects['worker'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    messages = queue.Queue()
    def reader():
        for line in process.stdout:
            messages.put(line)
        messages.put(None)
    threading.Thread(target=reader, daemon=True).start()
    def receive():
        line = messages.get(timeout=60)
        if line is None:
            process.wait(timeout=5)
            raise AssertionError('Worker exited early: ' + process.stderr.read())
        return json.loads(line)
    def request(value):
        if args.stream_batches and 'batch' in value: value = dict(value, stream=True)
        process.stdin.write(json.dumps(value)+'\n'); process.stdin.flush()
        first = receive()
        if not value.get('stream') or 'index' not in first: return first
        results = [None] * len(value['batch']); message = first
        while 'index' in message:
            index = message['index']
            assert isinstance(index, int) and 0 <= index < len(results) and results[index] is None, message
            results[index] = message['result']; message = receive()
        assert message['batch_done'] and message['count'] == len(results) and all(r is not None for r in results), message
        return dict(message, results=results)
    try:
        handshake = receive()
        assert handshake['protocol'] == 1 and handshake['ready'] is True
        assert Path(handshake['executable']).resolve() == binary
        cases = [('cold-error',helper.replace('42','"cold failure"'),main,False), ('cold',helper,main,False), ('warm',helper,main,False), ('comment',helper+'-- edit\n',main,False), ('value',helper.replace('42','43'),main,False), ('error',helper.replace('42','"bad"'),main,False), ('cached-error',helper.replace('42','"bad"'),main,False), ('lexical-error',helper.replace('42','"unterminated'),main,False), ('blocked-importer-syntax',helper.replace('42','"bad"'),main+'\nbroken = )\n',False), ('repair',helper,main,False), ('no-cache',helper,main,True), ('missing-import',helper,main.replace('import Helper','import Extra\nimport Helper'),False), ('new-import',helper,main.replace('import Helper','import Extra\nimport Helper'),False)]
        missing_pattern = 'module Helper exposing (answer)\ntype Choice = First | Second\nanswer : Int\nanswer =\n    case First of\n        First -> 42\n'
        cases += [
            ('non-exhaustive',missing_pattern,main,False),
            ('non-exhaustive-and-type-error',missing_pattern.replace('42','"bad"'),main,False),
            ('coverage-repair',helper,main,False),
            ('nested-trailing-comment',helper+'\n{- outer {- inner -} -}\n',main,False),
            ('shifted-source-coordinates',helper.replace('answer : Int','\nanswer : Int'),main,False),
            ('comment-markers-in-string',helper.replace('42', 'if "--" == "--" then 42 else 0'),main,False),
            ('todo-source-location',helper.replace('42', 'Debug.todo "location"'),main,False),
            ('todo-shifted-location',helper.replace('answer : Int','\nanswer : Int').replace('42', 'Debug.todo "location"'),main,False),
            ('late-edit',helper,main+'\n-- late edit\n',False),
            ('late-edit-again',helper,main+'\n-- another late edit\n',False),
            ('earlier-symbol-change',helper.replace('answer : Int','unused = 1\nanswer : Int').replace('42','43'),main,False),
            ('manifest-change',helper,main,False),
            ('multi-entry',helper,main,False),
            ('multi-entry-valid',helper.replace('exposing (answer)','exposing (answer, main)')+main[main.index('main :'):].replace('Helper.answer','answer'),main,False),
            ('corrupt-disk-cache',helper,main+'\n-- force analysis\n',False),
        ]
        for alias in ['Box', 'Crate']:
            alias_helper = f'module Helper exposing (answer, box, {alias})\ntype alias {alias} item = {{ item : item }}\nanswer : Int\nanswer = 42\nbox : item -> {alias} item\nbox value = {{ item = value }}\n'
            cases.append(('alias-error-'+alias, alias_helper, main+'\nbad : Int\nbad = Helper.box "wrong"\n', False))
        independent_main = main.replace('import Helper', 'import Extra\nimport Helper')
        cases += [
            ('independent-valid', helper, independent_main, False),
            ('independent-two-errors', helper.replace('42','"bad"'), independent_main, False),
            ('independent-one-repaired', helper, independent_main, False),
            ('independent-repair', helper, independent_main, False),
        ]
        choice_helper = 'module Helper exposing (answer, Choice(..))\ntype Choice = First | Second\nanswer : Int\nanswer = 42\n'
        choice_main = main + '\nchoose choice =\n    case choice of\n        Helper.First -> 1\n        Helper.Second -> 2\n'
        cases += [
            ('coverage-dependency-valid', choice_helper, choice_main, False),
            ('coverage-dependency-body', choice_helper.replace('42', '43'), choice_main, False),
            ('coverage-dependency-added-constructor', choice_helper.replace('First | Second', 'First | Second | Third'), choice_main, False),
            ('coverage-dependency-repair', choice_helper, choice_main, False),
            ('coverage-source-removed-pattern', choice_helper, choice_main.replace('        Helper.Second -> 2\n', ''), False),
            ('coverage-source-repair', choice_helper, choice_main, False),
        ]
        text_helper = 'module Helper exposing (answer)\nimport String\nlabel = "first"\nchoose value =\n    case value of\n        "first" -> 1\n        _ -> 2\nanswer : Int\nanswer = String.length label + choose label\n'
        cases += [
            ('text-seed', text_helper, main, False),
            ('text-edit', text_helper.replace('label = "first"', 'label = "longer é 😀"'), main, False),
            ('text-multiline', text_helper.replace('label = "first"', 'label = """first\nsecond"""'), main, False),
            ('text-pattern-error', text_helper.replace('        _ -> 2', '        "first" -> 3\n        _ -> 2'), main, False),
            ('text-value-type-error', text_helper.replace('label = "first"', 'label = 42'), main, False),
            ('text-annotation-error', text_helper.replace('answer : Int', 'answer : String'), main, False),
            ('text-lexical-error', text_helper.replace('label = "first"', 'label = "\\q"'), main, False),
            ('text-repair', text_helper.replace('label = "first"', 'label = "repaired"'), main, False),
        ]
        for label, helper_source, main_source, no_cache in cases:
            for project in projects.values():
                (project/'src/Main.elm').write_text(main_source)
                (project/'src/Helper.elm').write_text(helper_source)
                if label == 'new-import': (project/'src/Extra.elm').write_text('module Extra exposing (extra)\nextra = 1\n')
                if label.startswith('independent-'):
                    extra = 'module Extra exposing (extra)\nextra : Int\nextra = 1\n'
                    if label in {'independent-two-errors', 'independent-one-repaired'}:
                        extra = extra.replace('= 1', '= "bad"')
                    (project/'src/Extra.elm').write_text(extra)
                config = dict(manifest)
                if label == 'manifest-change':
                    (project/'other').mkdir(exist_ok=True)
                    config['source-directories'] = ['src','other']
                (project/'elm.json').write_text(json.dumps(config))
                if label == 'corrupt-disk-cache':
                    for artifact in (project/'elm-stuff/planexpo-rust/types-v1').rglob('*.cache'):
                        artifact.write_bytes(b'corrupted')
            command = ['src/Main.elm', '--incremental', '--output=out.js'] + (['--no-cache'] if no_cache else [])
            if label.startswith('multi-entry'): command.insert(0, 'src/Helper.elm')
            cli = subprocess.run([str(oracle),'make',*command,'--report=json',*(['--no-cache'] if args.oracle and not no_cache else [])], cwd=projects['cli'], env=env, capture_output=True, text=True, timeout=60)
            dependencies = projects['worker']/'dependencies.json'
            dependencies.unlink(missing_ok=True)
            before = rows[-1]['cache'] if rows else None
            result = request({'arguments':command,'dependencies':str(dependencies),'changed':[str(projects['worker']/'src/Helper.elm')]})
            assert result['ok'] == (cli.returncode == 0), (label,cli.stderr,result)
            assert result['ok'] == (label not in {'cold-error','error','cached-error','lexical-error','blocked-importer-syntax','missing-import','multi-entry','non-exhaustive','non-exhaustive-and-type-error','independent-two-errors','independent-one-repaired','coverage-dependency-added-constructor','coverage-source-removed-pattern','text-pattern-error','text-value-type-error','text-annotation-error','text-lexical-error'} and not label.startswith('alias-error-')), (label, result)
            if cli.returncode == 0:
                assert (projects['cli']/'out.js').read_bytes() == (projects['worker']/'out.js').read_bytes(), label
                assert result['error'] is None and result['report'] is None
                files = json.loads(dependencies.read_text())['files']
                assert str(projects['worker']/'src/Helper.elm') in files
            else:
                normalize = lambda value, project: json.dumps(value,sort_keys=True).replace(str(project),'<project>')
                assert normalize(json.loads(cli.stderr),projects['cli']) == normalize(result['report'],projects['worker']), (label,cli.stderr,result)
                assert result['error']
                if label in {'independent-two-errors', 'independent-one-repaired'}:
                    expected = {'Extra', 'Helper'} if label == 'independent-two-errors' else {'Extra'}
                    assert {error['name'] for error in result['report']['errors']} == expected, (label, result)
                if label == 'cold-error':
                    assert result['analysis']['verified']['skipped_passes'] == 0
                if label == 'error':
                    assert result['analysis']['verified']['skipped_passes'] > rows[-1]['analysis']['verified']['skipped_passes'], 'Unchanged survivors were re-analyzed'
                if label in {'non-exhaustive', 'non-exhaustive-and-type-error'}:
                    title = result['report']['errors'][0]['problems'][0]['title']
                    assert title == ('MISSING PATTERNS' if label == 'non-exhaustive' else 'TYPE MISMATCH'), (label, title)
                if label.startswith('alias-error-'):
                    assert label.removeprefix('alias-error-') in json.dumps(result['report']), result['report']
                if label == 'blocked-importer-syntax':
                    paths = {Path(error['path']).name for error in result['report']['errors']}
                    assert paths == {'Main.elm', 'Helper.elm'}, result['report']
                    assert result['cache']['hits'][2] > 0, 'Unchanged syntax was not reused'
            if no_cache:
                assert result['cache'] == before, 'Explicit no-cache touched worker memoization'
                assert result['analysis'] == rows[-1]['analysis'], 'Explicit no-cache resumed an analysis checkpoint'
            if label == 'comment' and env.get('PLANEXPO_ELM_TRIVIA_CACHE') == '1':
                assert result['analysis'] == rows[-1]['analysis'], 'A trivia-equivalent hit repeated semantic analysis'
            rows.append({'case':label,'ok':result['ok'],'identical':True,'cache':result['cache'],'analysis':result['analysis']})
        # Shared modules are traversed in a different order in another app.
        # Keep the first app larger, with its checkpoint before an unrelated tail.
        sources = {
            'PLeft': 'module PLeft exposing (value)\nvalue = 3\n',
            'QRight': 'module QRight exposing (value)\nvalue = 4\n',
            'RShared': 'module RShared exposing (value)\nimport PLeft\nimport QRight\nvalue = PLeft.value + QRight.value\n',
            'TExtraOne': 'module TExtraOne exposing (value)\nvalue = 1\n',
            'TExtraTwo': 'module TExtraTwo exposing (value)\nvalue = 2\n',
            'STail': 'module STail exposing (value)\nimport TExtraOne\nimport TExtraTwo\nvalue = TExtraOne.value + TExtraTwo.value\n',
            'ABefore': 'module ABefore exposing (value)\nimport QRight\nvalue = QRight.value\n',
            'Large': main.replace('module Main', 'module Large').replace('import Helper', 'import RShared\nimport STail').replace('Helper.answer', 'RShared.value + STail.value'),
            'Other': main.replace('module Main', 'module Other').replace('import Helper', 'import ABefore\nimport RShared').replace('Helper.answer', 'RShared.value + ABefore.value'),
        }
        for project in projects.values():
            for name, source in sources.items(): (project/f'src/{name}.elm').write_text(source)
        for label, entry, changed in [
            ('reordered-prime', 'Large', 'TExtraOne'),
            ('reordered-other', 'Other', 'Other'),
            ('reordered-origin', 'Large', 'TExtraOne'),
            ('reordered-dependency-error', 'Other', 'PLeft'),
            ('reordered-dependency-repair', 'Other', 'PLeft'),
            ('shared-large-error', 'Large', 'RShared'),
            ('shared-other-error', 'Other', 'RShared'),
            ('shared-repair', 'Other', 'RShared'),
            ('shared-alias-large-error', 'Large', 'RShared'),
            ('shared-alias-other-error', 'Other', 'RShared'),
        ]:
            for project in projects.values():
                if label == 'reordered-origin':
                    (project/'src/TExtraOne.elm').write_text(sources['TExtraOne'].replace('= 1', '= 2'))
                source = sources['PLeft'].replace('3', '\"wrong\"') if label == 'reordered-dependency-error' else sources['PLeft']
                (project/'src/PLeft.elm').write_text(source)
                shared = sources['RShared']
                if label in {'shared-large-error', 'shared-other-error'}:
                    shared = shared.replace('value = PLeft.value + QRight.value', 'value : Int\nvalue = \"bad\"')
                if label in {'shared-alias-large-error', 'shared-alias-other-error'}:
                    shared = shared.replace('value = PLeft.value + QRight.value', 'type alias Box item = { item : item }\nbox : item -> Box item\nbox item = { item = item }\nvalue : Int\nvalue = box \"bad\"')
                (project/'src/RShared.elm').write_text(shared)
            command = [f'src/{entry}.elm', '--incremental', '--output=out.js']
            cli = subprocess.run([str(oracle),'make',*command,'--report=json','--no-cache'], cwd=projects['cli'], env=env, capture_output=True, text=True, timeout=60)
            result = request({'arguments':command,'changed':[str(projects['worker']/f'src/{changed}.elm')]})
            assert result['ok'] == (cli.returncode == 0) == (not label.endswith('-error')), (label, cli.stderr, result)
            if result['ok']:
                assert (projects['cli']/'out.js').read_bytes() == (projects['worker']/'out.js').read_bytes(), label
            else:
                normalize = lambda value, project: json.dumps(value,sort_keys=True).replace(str(project),'<project>')
                assert normalize(json.loads(cli.stderr),projects['cli']) == normalize(result['report'],projects['worker']), (label, cli.stderr, result)
            if label in {'shared-other-error', 'shared-alias-other-error'}:
                assert result['analysis']['verified']['reused_failures'] > rows[-1]['analysis']['verified']['reused_failures'], 'Shared diagnostic was not reused'
            if label == 'reordered-other':
                assert result['analysis']['resumed_modules'] > rows[-1]['analysis']['resumed_modules'], 'Shared analysis was not resumed'
            if label == 'reordered-dependency-error':
                assert result['analysis']['checkpoint_entry'] == ['application:Large']
                assert result['analysis']['resumed_modules'] == rows[-1]['analysis']['resumed_modules'], 'Changed checkpoint member was reused'
            rows.append({'case':label,'ok':result['ok'],'identical':True,'cache':result['cache'],'analysis':result['analysis']})
        for project in projects.values():
            (project/'src/Main.elm').write_text(main)
            (project/'src/Helper.elm').write_text(helper)
        assert not request({'arguments':['src/Main.elm'],'changed':[42]})['ok']
        assert not request({'arguments':[42]})['ok']
        assert request({'arguments':['src/Main.elm','--incremental','--output=out.js']})['ok']
        batch_checks = []
        if handshake.get('capabilities', {}).get('batch'):
            other = main.replace('module Main', 'module Other')
            for project in projects.values():
                (project/'src/Other.elm').write_text(other)
            for phase in ['cold', 'value', 'cached', 'independent-error', 'two-entry-errors', 'shared-error', 'shared-error-importer-syntax', 'repair', 'html']:
                for project in projects.values():
                    (project/'src/Helper.elm').write_text(helper.replace('42', '"shared bad"' if phase.startswith('shared-error') else '75' if phase == 'cold' else '76'))
                    (project/'src/Main.elm').write_text(main + ('\nbad : Int\nbad = "wrong"\n' if phase in {'independent-error', 'two-entry-errors'} else '\nbroken = )\n' if phase == 'shared-error-importer-syntax' else ''))
                    (project/'src/Other.elm').write_text(other + ('\nbad : Int\nbad = \"other wrong\"\n' if phase == 'two-entry-errors' else ''))
                commands = [['src/Main.elm', '--incremental', '--output=main.html' if phase == 'html' else '--output=main.js'], ['src/Other.elm', '--incremental', '--output=other.js']]
                references = [subprocess.run([str(oracle),'make',*command,'--report=json','--no-cache'], cwd=projects['cli'], env=env, capture_output=True, text=True, timeout=60) for command in commands]
                result = request({'batch':[{'arguments':command,'dependencies':str(projects['worker']/f'deps-{i}.json'),'changed':[str(projects['worker']/'src/Helper.elm')]} for i,command in enumerate(commands)]})
                assert len(result['results']) == 2, result
                if phase in {'cold', 'value'}: assert result['shared_analysis_entries'] == 2, result
                if phase in {'cached', 'independent-error', 'html'}: assert result['shared_analysis_entries'] == 0, result
                for i,(command,cli,response) in enumerate(zip(commands,references,result['results'])):
                    assert response['ok'] == (cli.returncode == 0), (phase,cli.stderr,response)
                    if response['ok']:
                        target = command[-1].split('=',1)[1]
                        assert (projects['worker']/target).read_bytes() == (projects['cli']/target).read_bytes(), (phase,target)
                    else:
                        normalize = lambda value, project: json.dumps(value,sort_keys=True).replace(str(project),'<project>')
                        assert normalize(json.loads(cli.stderr),projects['cli']) == normalize(response['report'],projects['worker']), (phase,cli.stderr,response)
                    dependencies = json.loads((projects['worker']/f'deps-{i}.json').read_text())['files']
                    assert str(projects['worker']/command[0]) in dependencies
                    assert str(projects['worker']/'src/Helper.elm') in dependencies
                    rows.append({'case':f'batch-{phase}-{i}','ok':response['ok'],'identical':True})
                batch_checks.append({'phase':phase,'shared_analysis_entries':result['shared_analysis_entries']})
            invalid = request({'batch':[{'arguments':[42]},{'arguments':['src/Other.elm','--incremental','--output=other.js']}]})
            assert [item['ok'] for item in invalid['results']] == [False, True]
            duplicate = request({'batch':[{'arguments':['src/Main.elm','--incremental','--output=duplicate.js']},{'arguments':['src/Other.elm','--incremental','--output=duplicate.js']}]})
            assert all(item['ok'] for item in duplicate['results']) and duplicate['shared_analysis_entries'] == 0
            assert (projects['worker']/'duplicate.js').read_bytes() == (projects['worker']/'other.js').read_bytes()
            for invalid_batch in [[], [None]*33, 'invalid']:
                assert not request({'batch':invalid_batch})['ok']
            single = request({'batch':[{'arguments':['src/Other.elm','--incremental','--output=other.js']}]})
            assert single['results'][0]['ok'] and single['shared_analysis_entries'] == 0
        value_edit = next(row for row in rows if row['case'] == 'value')
        assert value_edit['cache']['hits'][0] > 0
        assert value_edit['analysis']['resumed_modules'] > 0
        process.stdin.close()
        assert process.wait(timeout=10) == 0
        assert process.stderr.read() == ''
    finally:
        if process.poll() is None: process.kill(); process.wait()
assert hashlib.sha256(binary.read_bytes()).hexdigest() == digest
args.report.write_text(json.dumps({'compiler_sha256':digest,'oracle_sha256':hashlib.sha256(oracle.read_bytes()).hexdigest(),'oracle_uncached':bool(args.oracle),'stream_batches':args.stream_batches,'trivia_cache':os.environ.get('PLANEXPO_ELM_TRIVIA_CACHE') == '1','cases':rows,'batch_checks':batch_checks,'invalid_request_recovery':True,'eof_shutdown':True},indent=2)+'\n')
print(f'{len(rows)} worker/CLI comparisons passed')
