#!/usr/bin/env python3
"""Run the registered compiler parity/runtime/cache suites and retain each outcome.

Does not rebuild the compiler, run frontend tests, minify, launch browsers or
measure performance. Those validations remain separate. No failing suite is
silently skipped and all suites run even if an earlier suite fails.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def save_report(path, report):
    temporary = path.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(report, indent=2)+'\n')
    temporary.replace(path)


def resume_results(previous, current, commands):
    """Reuse only a verified prefix of this exact audit, retaining failures."""
    for key in ('compiler_sha256', 'elm_sha256', 'planned_suites', 'input_sha256'):
        if previous.get(key) != current[key]:
            raise ValueError('Cannot resume: changed '+key)
    rows = previous.get('results')
    if not isinstance(rows, list) or len(rows) > len(commands):
        raise ValueError('Cannot resume: invalid result prefix')
    for row, (label, command) in zip(rows, commands):
        if (not isinstance(row, dict) or row.get('suite') != label or row.get('command') != command
                or not isinstance(row.get('passed'), bool)
                or (row['passed'] and row.get('returncode') != 0)):
            raise ValueError('Cannot resume: changed or invalid suite '+label)
    return rows


def main():
    crate = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elm', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--resume', action='store_true', help='Resume a verified audit prefix, retaining failed outcomes')
    parser.add_argument('--list', action='store_true', help='List registered suites without executing them')
    parser.add_argument('--suite', action='append', help='Run only this named suite; repeat to select several (default: full audit)')
    parser.add_argument('--elm-hot', type=Path, help='Path to the elm-hot package used by the development environment')
    parser.add_argument('--release', default='0.19.1', choices=['0.19.1', '0.19.2', '0.19.3'],
                        help='Official release of --elm. The fixtures target 0.19.1; for another release both '
                             'compilers are wrapped by elm_release_adapter (see its limits)')
    args = parser.parse_args()
    elm = args.elm.resolve()
    binary = crate/'target/release/planexpo-elm'
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    scripts = crate/'scripts'
    working_directory = crate.parent
    environment = {**os.environ, 'GHCRTS': '-N1 -A16m -c'}
    if args.release != '0.19.1':
        import elm_release_adapter
        scripts, binary, elm = elm_release_adapter.mirror(crate, args.output, elm, binary, args.release)
        working_directory = scripts.parent.parent
        # Some releases crash where 0.19.1 reports an error; parity checks then skip that case.
        environment['ELM_REFERENCE_RELEASE'] = args.release
    suites = []
    for name in ['exposed_groups', 'duplicate_json', 'initialization_order', 'elm_version', 'metadata_cache', 'constraint_parity', 'application_resolution', 'package_resolution', 'application_dependencies', 'package_projects', 'shader_parity', 'shader_runtime', 'unicode_parity', 'output_formats', 'diagnostic_regions', 'header_diagnostics', 'pattern_layout', 'type_layout', 'expression_layout', 'control_layout', 'declaration_layout', 'module_errors', 'name_diagnostics', 'upstream_parity', 'parser_parity', 'names_parity',
                 'inference_parity', 'validation_parity', 'ports_parity',
                 'worker_runtime', 'production_runtime', 'constructor_runtime',
                 'arithmetic_runtime', 'integer_literal_runtime', 'debug_runtime', 'tail_runtime',
                 'recursive_order_parity', 'recursive_graph_parity',
                 'recursive_runtime_parity', 'recursive_capture_soundness',
                 'global_value_cycles', 'incremental_interfaces']:
        options = ['--elm', str(elm)]
        if name in ['exposed_groups', 'duplicate_json', 'initialization_order', 'elm_version', 'metadata_cache', 'constraint_parity', 'application_resolution', 'package_resolution', 'application_dependencies', 'package_projects', 'header_diagnostics', 'pattern_layout', 'type_layout', 'expression_layout', 'control_layout', 'declaration_layout', 'module_errors', 'name_diagnostics', 'shader_parity', 'upstream_parity', 'inference_parity', 'recursive_order_parity',
                    'recursive_graph_parity', 'recursive_runtime_parity',
                    'recursive_capture_soundness', 'global_value_cycles',
                    'incremental_interfaces']:
            options += ['--report', str(args.output/(name+'.json'))]
        if name == 'upstream_parity':
            options += ['--diagnostics', '--terminal']
        suites.append((name, name, options))
    suites.append(('init_cli', 'init_cli', ['--elm', str(elm), '--report', str(args.output/'init_cli.json')]))
    for stream in ['stderr', 'stdout']:
        label = 'init_cli_ansi_' + stream
        suites.append((label, 'init_cli', ['--elm', str(elm), '--rust', str(binary), '--ansi',
            '--terminal-stream', stream, '--report', str(args.output/(label+'.json'))]))
    suites.append(('install_diagnostics', 'install_diagnostics', ['--elm', str(elm), '--report', str(args.output/'install_diagnostics.json')]))
    for mode in ['declined', 'accepted']:
        options = ['--elm', str(elm), '--rust-cli', str(binary), '--report', str(args.output/('install_'+mode+'.json'))]
        if mode == 'accepted':
            options += ['--accept']
        suites.append(('install_'+mode, 'installation_plans', options))
    suites.append(('docs_diagnostics', 'docs_diagnostics', ['--elm', str(elm), '--report', str(args.output/'docs_diagnostics.json')]))
    suites.append(('docs_cli', 'docs_cli', ['--elm', str(elm), '--report', str(args.output/'docs_cli.json')]))
    suites.append(('docs_render', 'docs_render', ['--public-cli', '--elm', str(elm), '--report', str(args.output/'docs_render.json')]))
    suites.append(('tail_runtime_production', 'tail_runtime', ['--elm',str(elm),'--production']))
    suites.extend((name,name,[]) for name in ['cache_runtime','make_optimize','incremental_runtime','hot_reload'])
    suites.append(('incremental_runtime_development', 'incremental_runtime', ['--development']))
    if args.elm_hot:
        suites = [(label, name, options + ['--injector', str(args.elm_hot.resolve())]
                   if name == 'hot_reload' else options) for label, name, options in suites]
    diagnostics = 'destructure record_update_interaction open_record infinite_type record_update_keys record_update_value record_access record_shape composition pipe ordered_comparison equality append cons arithmetic division boolean_negate typed_argument_pattern list_pattern declaration_type_order constructor_pattern case_pattern list_entry case_branch call_arity call_argument inferred_branch if_condition annotation_type branch_annotation main_type main_flags port'.split()
    for name in [name + '_diagnostics' for name in diagnostics] + ['repl_flag_diagnostics', 'init_install_arguments', 'global_cli', 'make_file_arguments', 'make_flag_priority', 'make_unknown_flags', 'make_arguments', 'make_success_streams', 'make_failure_streams', 'make_output_diagnostics', 'unused_dependencies', 'null_output', 'repl_dependencies', 'package_dependency_failures', 'dependency_progress', 'download_body_failures', 'download_tls', 'download_connection_refused', 'no_proxy_routing', 'proxy_environment', 'proxy_tunnel_auth', 'invalid_proxy_environment']:
        suites.append((name, name, ['--elm', str(elm), '--rust', str(binary),
                                   '--report', str(args.output/(name+'.json'))]))
    for name in ['multi_entry_import_diagnostics', 'import_suggestion_history', 'missing_import_diagnostics', 'ambiguous_import_diagnostics', 'repl_cli', 'repl_raw_error_recovery', 'repl_diagnostics', 'repl_history', 'repl_interrupt',
                 'repl_name_diagnostics', 'repl_terminal', 'bump_cli',
                 'diff_arguments', 'diff_docs', 'diff_local', 'diff_offline',
                 'diff_projects', 'reactor', 'reactor_cli']:
        suites.append((name, name, ['--elm', str(elm), '--rust', str(binary),
                                   '--report', str(args.output/(name+'.json'))]))
    suites.append(('missing_import_diagnostics_incremental', 'missing_import_diagnostics', ['--elm', str(elm), '--rust', str(binary), '--incremental', '--report', str(args.output/'missing_import_diagnostics_incremental.json')]))
    suites.append(('ambiguous_import_diagnostics_incremental', 'ambiguous_import_diagnostics', ['--elm', str(elm), '--rust', str(binary), '--incremental', '--report', str(args.output/'ambiguous_import_diagnostics_incremental.json')]))
    for mode, flags in [('plain', []), ('ansi', ['--ansi'])]:
        label = 'api_diff_' + mode
        suites.append((label, 'api_diff', ['--elm', str(elm), '--rust-cli', str(binary),
            '--report', str(args.output/(label+'.json')), *flags]))
    suites.append(('publish_arguments', 'publish_arguments', [
        '--elm', str(elm), '--rust', str(binary),
        '--report', str(args.output/'publish_arguments.json')]))
    # Fake Git rejects publication before any remote registration is possible.
    for name in ['publish_preflight', 'publish_version']:
        for mode, terminal_options in [('plain', []), ('ansi', ['--ansi']),
                                       ('ansi_stdout', ['--ansi', '--terminal-stream', 'stdout'])]:
            label = name + '_' + mode
            suites.append((label, name, ['--elm', str(elm), '--probe', str(binary),
                '--public', '--report', str(args.output/(label+'.json')), *terminal_options]))
    # Public compiler checks that were previously run only as standalone suites.
    for name in ['production_optimizations', 'ambiguous_name_diagnostics', 'ambiguous_operator_diagnostics',
                 'arity_diagnostics', 'associativity_diagnostics',
                 'coverage_aggregation', 'cycle_diagnostics', 'debug_flags', 'debug_remnants', 'diagnostic_cache', 'generated_inference', 'generated_integer_runtime', 'generated_patterns', 'proxy_cli_errors', 'output_io_errors', 'docs_io_errors', 'recursive_capture_runtime', 'unicode_literal_runtime', 'outline_missing_fields', 'outline_source_directories', 'outline_package_metadata', 'outline_versions', 'outline_names', 'outline_modules', 'outline_types', 'outline_json', 'outline_depth', 'outline_deep_types',
                 'duplicate_pattern_diagnostics', 'local_cycle_diagnostics',
                 'operator_name_diagnostics', 'package_docs_core_only',
                 'pattern_diagnostics', 'record_pattern_diagnostics',
                 'redundant_pattern_diagnostics', 'shadowing_diagnostics',
                 'type_variable_diagnostics', 'unused_type_variable_diagnostics']:
        suites.append((name, name, ['--elm', str(elm), '--rust', str(binary),
                                   '--report', str(args.output/(name+'.json'))]))
    suites.append(('display_type_cache', 'display_type_cache', ['--elm', str(elm), '--rust', str(binary), '--report', str(args.output/'display_type_cache.json')]))
    suites.append(('output_io_errors_incremental', 'output_io_errors', [
        '--elm', str(elm), '--rust', str(binary), '--incremental',
        '--report', str(args.output/'output_io_errors_incremental.json')]))
    suites.append(('loader_dependencies', 'loader_dependencies', ['--rust', str(binary), '--report', str(args.output/'loader_dependencies.json')]))
    suites.append(('make_worker', 'make_worker', ['--rust', str(binary), '--report', str(args.output/'make_worker.json')]))
    assert len({label for label, _, _ in suites}) == len(suites), 'Duplicate suite labels'
    if args.suite:
        requested = set(args.suite)
        unknown = requested - {label for label, _, _ in suites}
        if unknown:
            parser.error('Unknown suites: ' + ', '.join(sorted(unknown)))
        suites = [suite for suite in suites if suite[0] in requested]

    assert all((scripts/('check_'+name+'.py')).is_file() for _, name, _ in suites)
    if args.list:
        print(json.dumps([{'suite': label, 'command': [sys.executable, str(scripts/('check_'+name+'.py')), *options]}
                          for label, name, options in suites], indent=2))
        return
    inputs = hashlib.sha256()
    for folder in [crate/'scripts', crate/'tests']:
        for path in sorted(folder.rglob('*')):
            if path.is_file() and '__pycache__' not in path.parts:
                inputs.update(str(path.relative_to(crate)).encode())
                inputs.update(hashlib.sha256(path.read_bytes()).digest())
    report = {'input_sha256': inputs.hexdigest(), 'scope': (__doc__ + ' Selected suites only; not a full audit.') if args.suite else __doc__,
              'selected_suites': sorted(set(args.suite)) if args.suite else None, 'compiler_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
              'elm_sha256':hashlib.sha256(elm.read_bytes()).hexdigest(),
              'planned_suites':len(suites), 'complete':False, 'results': []}
    commands = [(label, [sys.executable, str(scripts/('check_'+name+'.py')), *options])
                for label, name, options in suites]
    if args.resume:
        try:
            previous = json.loads((args.output/'report.json').read_text())
            report['results'] = resume_results(previous, report, commands)
        except (OSError, ValueError, KeyError, TypeError) as error:
            parser.error(str(error))
    for label, command in commands[len(report['results']):]:
        start = time.monotonic()
        print('RUN '+label, flush=True)
        with (args.output/(label+'.log')).open('w') as log:
            try:
                result = subprocess.run(command,cwd=working_directory,stdout=log,stderr=subprocess.STDOUT,
                                        env=environment,timeout=1800)
                outcome = {'returncode':result.returncode,'passed':result.returncode==0}
            except subprocess.TimeoutExpired:
                outcome = {'passed':False,'timeout_seconds':1800}
        row = {'suite':label,'command':command,'seconds':round(time.monotonic()-start,2),**outcome}
        report['results'].append(row)
        report['passed'] = all(r['passed'] for r in report['results'])
        save_report(args.output/'report.json', report)
        print(('PASS ' if row['passed'] else 'FAIL ')+label, flush=True)
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == report['compiler_sha256'], 'Compiler changed during validation'
    assert hashlib.sha256(elm.read_bytes()).hexdigest() == report['elm_sha256'], 'Reference compiler changed during validation'
    report['passed'] = all(r['passed'] for r in report['results'])
    report['complete'] = True
    save_report(args.output/'report.json', report)
    print(json.dumps({'suites':len(suites),'passed':sum(r['passed'] for r in report['results'])}),flush=True)
    raise SystemExit(not report['passed'])


if __name__ == '__main__':
    main()
