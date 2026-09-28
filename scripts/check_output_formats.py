#!/usr/bin/env python3
"""Compare HTML/multiple-entry output contracts with the official compiler."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
rust = crate / 'target/release/planexpo-elm'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--elm', type=Path, default=crate.parent/'front/node_modules/elm/bin/elm')
parser.add_argument('--rust', type=Path, default=rust)
args = parser.parse_args()
official = args.elm.resolve()
rust = args.rust.resolve()
with tempfile.TemporaryDirectory(prefix='elm-output-parity-') as directory:
    root = Path(directory)
    shutil.copytree(crate / 'tests/programs/worker', root, dirs_exist_ok=True)
    source = (root / 'Main.elm').read_text()
    (root / 'Other.elm').write_text(source.replace('port module Main', 'port module Other').replace('incoming', 'incomingOther').replace('outgoing', 'outgoingOther'))
    for compiler in [official, rust]:
        for previous in root.glob('*.js'):
            previous.unlink()
        for previous in root.glob('*.html'):
            previous.unlink()
        def compile(*args):
            return subprocess.run([str(compiler), 'make', *args], cwd=root,
                env={**os.environ, 'GHCRTS': '-N2'}, capture_output=True, text=True, timeout=60)
        result = compile('Main.elm', 'Other.elm')
        assert result.returncode == 0 and (root / 'elm.js').is_file(), (compiler, result.stderr)
        for mode in [[], ['--optimize']]:
            out = root / 'both.js'
            result = compile('Main.elm', 'Other.elm', '--output=both.js', *mode)
            assert result.returncode == 0, (compiler, result.stderr)
            result = subprocess.run(['node', '-e', '''
const Elm = require(process.argv[1]).Elm;
for (const name of ['Main','Other']) {
 const app = Elm[name].init({flags:{start:5}});
 app.ports[name === 'Main' ? 'outgoing' : 'outgoingOther'].subscribe(v=>console.log(name+':'+v));
 app.ports[name === 'Main' ? 'incoming' : 'incomingOther'].send(1);
}
''', str(out)], capture_output=True, text=True, timeout=10)
            assert result.returncode == 0, result.stderr
            assert sorted(result.stdout.splitlines()) == ['Main:6', 'Other:6'], result.stdout
            previous = out.read_bytes()
            result = compile('Main.elm', 'Missing.elm', '--output=both.js', *mode)
            assert result.returncode != 0 and out.read_bytes() == previous
        for args in [[], ['--output=index.html'], ['--output=index.html', '--optimize'], ['--output=./.html']]:
            html_path=root/('.html' if '--output=./.html' in args else 'index.html')
            html_path.unlink(missing_ok=True)
            result = compile('Main.elm', *args)
            assert result.returncode == 0, (compiler, result.stderr)
            html = html_path.read_text()
            assert '<html>' in html and '.init(' in html and '</html>' in html
        # Execute the HTML script with the DOM operations used by its shell.
        html_runner = r"""
const fs=require('fs'),vm=require('vm');
const html=fs.readFileSync(process.argv[1],'utf8');
const code=html.slice(html.indexOf('<script>')+8,html.lastIndexOf('</script>'));
const pre={}, headers=[];
const document={getElementById:()=>pre,createElement:()=>({style:{}}),body:{insertBefore:h=>headers.push(h)}};
const context={document,console:{warn(){}},setTimeout,clearTimeout};
let error;
try{vm.runInNewContext(code,context)}catch(e){error=e}
if(process.argv[2]==='error'){
 if(!error || headers[0]?.innerText!=='Initialization Error' || !pre.innerText)process.exit(1);
}else{
 if(error)throw error;
 if(!context.app?.ports?.incoming)process.exit(2);
}
"""
        result = subprocess.run(['node','-e',html_runner,str(root/'index.html'),'error'],capture_output=True,text=True)
        assert result.returncode == 0, result.stderr
        (root/'Main.elm').write_text(source.replace('Program { start : Int }','Program ()').replace('flags.start','0'))
        assert compile('Main.elm','--output=unit.html').returncode == 0
        result = subprocess.run(['node','-e',html_runner,str(root/'unit.html'),'success'],capture_output=True,text=True)
        assert result.returncode == 0, result.stderr
        (root/'Main.elm').write_text(source)
        (root / 'Library.elm').write_text('module Library exposing (value)\nvalue = 42\n')
        assert compile('Library.elm').returncode == 0
        for output in ['library.js', 'library.html']:
            assert compile('Library.elm', '--output='+output).returncode != 0
            assert not (root / output).exists()
        (root/'index.html').unlink()
        assert compile('Library.elm', 'Main.elm').returncode == 0
        assert '.init(' in (root/'index.html').read_text()
        assert compile('Main.elm', 'Library.elm', '--output=mixed.js').returncode != 0
        # Duplicate source arguments are rejected by the official compiler.
        result = compile('Main.elm', 'Main.elm', '--output=duplicate.js')
        assert result.returncode != 0
        # A collision in two distinct modules is visible in the shared runtime.
        other = root / 'Other.elm'
        original = other.read_text()
        other.write_text(source.replace('port module Main', 'port module Other'))
        result = compile('Main.elm', 'Other.elm', '--output=collision.js')
        assert result.returncode == 0, (compiler, result.stderr)
        result = subprocess.run(['node', '-e', 'require(process.argv[1])', str(root/'collision.js')], capture_output=True, text=True)
        assert result.returncode != 0 and 'There can only be one port named' in result.stderr
        other.write_text(original)
        result = compile('Main.elm', 'Other.elm', '--output=both.html')
        assert result.returncode != 0
    print('PASS: official/Rust parity for default/explicit HTML, multi-entry JS in both modes, independent ports and failure preservation')
