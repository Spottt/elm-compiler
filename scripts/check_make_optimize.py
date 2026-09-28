#!/usr/bin/env python3
"""Exercise optimized make output, mode-separated caches and failure preservation."""
import os
os.environ["PLANEXPO_ELM_STATS"] = "1"
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
binary = crate / 'target/release/planexpo-elm'
runner = r'''
const app=require(process.argv[1]).Elm.Main.init({flags:{start:5}});
app.ports.outgoing.subscribe(value=>console.log(JSON.stringify(value)));
app.ports.incoming.send(1);
'''
with tempfile.TemporaryDirectory(prefix='elm-rust-make-optimize-') as directory:
    project = Path(directory)
    shutil.copytree(crate/'tests/programs/worker', project, dirs_exist_ok=True)
    output = project/'main.js'

    def compile(mode, cached, *extra):
        args = ['--optimize'] if mode == 'production' else []
        result = subprocess.run([str(binary), 'make', 'Main.elm', '--output='+str(output), *args, *extra], cwd=project, capture_output=True, text=True, timeout=60)
        assert result.returncode == 0, result.stderr
        assert f'({int(cached)} cached)' in result.stdout, result.stdout
        execution = subprocess.run(['node', '-e', runner, str(output)], capture_output=True, text=True, check=True, timeout=10)
        assert json.loads(execution.stdout) == 6, execution.stdout
        return output.read_bytes()

    dev = compile('development', False)
    prod = compile('production', False)
    assert dev != prod
    assert compile('development', True) == dev
    output.unlink()
    assert compile('production', True) == prod
    assert compile('production', False, '--no-cache') == prod
    assert len(list((project/'elm-stuff/planexpo-rust/v1').glob('*.cache'))) == 2
    linked = project/'linked.js'
    subprocess.run([str(binary), 'link-js-prod', str(project/'elm.json'), 'Main.elm', str(linked)], capture_output=True, text=True, check=True, timeout=60)
    assert linked.read_bytes() == prod
    quiet = subprocess.run([str(binary), 'make', 'Main.elm', '--optimize', '--output='+str(output), '--report=json'], cwd=project, capture_output=True, text=True, timeout=60)
    assert quiet.returncode == 0 and not quiet.stdout and not quiet.stderr, quiet
    source = project/'Main.elm'
    source.write_text(source.read_text().replace('import Platform', 'import Platform\nimport Debug').replace('value + model', 'Debug.log "result" (value + model)'))
    debug_dev = subprocess.run([str(binary), 'make', 'Main.elm', '--output='+str(output)], cwd=project, capture_output=True, text=True, timeout=60)
    assert debug_dev.returncode == 0, debug_dev.stderr
    previous = output.read_bytes()
    failed = subprocess.run([str(binary), 'make', 'Main.elm', '--optimize', '--output='+str(output), '--report=json'], cwd=project, capture_output=True, text=True, timeout=60)
    assert failed.returncode != 0 and json.loads(failed.stderr)['type'] == 'error', failed
    assert 'Debug' in failed.stderr
    assert output.read_bytes() == previous
    print('PASS: optimized make equals link-js-prod, independent reusable caches, bypass, quiet JSON success and Debug rejection preserving existing output')
