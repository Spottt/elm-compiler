#!/usr/bin/env python3
"""Compare GLSL literal acceptance and inferred-interface compatibility."""
import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path
import shutil
import subprocess
import tempfile

crate = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
args = p.parse_args()
subprocess.run([sys.executable, str(crate/'scripts/generate_glsl_cases.py'), '--check'], check=True)
cases = [
    '', 'void main() {}', 'void main() { gl_Position = vec4(0.0); }',
    'attribute vec3 position; uniform mat4 transform; varying vec2 uv; void main() { gl_Position = transform * vec4(position, 1.0); }',
    'precision mediump float; uniform sampler2D texture; varying vec2 uv; void main() { gl_FragColor = texture2D(texture, uv); }',
    '// comment\nvoid main() { /* comment */ }',
    'uniform float time;', 'uniform float time',
    'void main() {} garbage', 'void main() { float a = ; }',
    'void main( {', 'void main() {} /* unfinished',
    '#version 100\nvoid main() {}', '#define X 1.0\nvoid main() { gl_Position = vec4(X); }',
    'uniform highp float time; void main() {}',
    'uniform float first, second; void main() {}',
    'uniform float values[4]; void main() {}',
    'void main() { for (int i=0; i<4; i++) { if (i==2) continue; } }',
    'struct Light { vec3 position; }; uniform Light light; void main() {}',
    'layout(location=0) in vec3 position; void main() {}',
]
# Reserved by the language-glsl 0.3.0 parser embedded in Elm 0.19.1,
# even where modern GLSL gives these words a meaning.
for word in """common partition active asm class union enum typedef template this packed goto inline noinline volatile public static extern external interface long short double half fixed unsigned superp input output hvec2 hvec3 hvec4 dvec2 dvec3 dvec4 fvec2 fvec3 fvec4 sampler3DRect filter image1D image2D image3D imageCube iimage1D iimage2D iimage3D iimageCube uimage1D uimage2D uimage3D uimageCube image1DArray image2DArray iimage1DArray iimage2DArray uimage1DArray uimage2DArray image1DShadow image2DShadow image1DArrayShadow image2DArrayShadow imageBuffer iimageBuffer uimageBuffer sizeof cast namespace using row_major""".split():
    cases.extend([f'{word} value;', f'float {word};', f'void main() {{ float value = 1.0; /* {word} */ }}'])
cases.extend(['float doubleValue;', 'float _double;', '// double\nvoid main() {}'])
# language-glsl reserves consecutive underscores anywhere in identifiers.
for name in ['__value', 'value__', 'a__b', '_value', 'value_', 'a_b']:
    cases.extend([
        f'float {name};',
        f'void main() {{ float value = {name}; }}',
        f'struct Light {{ float {name}; }};',
        f'void {name}() {{}}',
        f'void main() {{ /* {name} */ }}',
    ])
# Confirmed differences with the former modern GLSL parser.
for name in ['buffer', 'shared', 'precise', 'coherent', 'restrict', 'readonly',
             'writeonly', 'atomic_uint', 'dmat2', 'café']:
    cases.extend([f'float {name};', f'void main() {{ {name} = 1.0; }}'])
cases.extend(['Foo value;', 'float value=1.0lf;', 'float value=1.0LF;',
              'float value=1.0f;', 'void main() { float x = 1e2; }'])
# Exercise complete expressions, declarations, control flow and lexical edges.
for expression in [
    '1.0', '.1', '1.', '1e2', '1e-2', '0e1', '00.1', '08', '0xF',
    '0xffu', '0b10', '1u', '1f', '1.0f', '1.0lf', '1.0LF', '1e2 f',
    '1e2 /* gap */ f', 'a.b', 'a. b', 'a .b', 'a . b', 'a.length()',
    'foo(void)', 'foo()', 'foo(1,2)', 'vec3(1.0)', 'float[2](1,2)',
    'a[1,2]', 'a[]', 'a[1][2]', 'a ? b : c', 'a ? b,c : d = e',
    'a ^^ b', 'a && b || c', 'a << b', 'a >>= b', 'a += b *= c',
    '++a--', 'a++--', 'a+++b', 'a-- - b', '~a', '!a',
]:
    cases.append(f'void main() {{ value = ({expression}); }}')
for statement in [
    'if (true) return;', 'if (true) { } else discard;',
    'while (true) break;', 'while (bool x = true) break;',
    'while (Foo x = value) break;', 'do {} while (true);',
    'for (;;) {}', 'for (int i=0; i<10; i++) {}',
    'for (Foo i=value; true; i++) {}',
    'switch (x) { case 1: break; default: return; }',
    'case 1: break;', 'return 1,2;', 'int f();', 'struct A { float x; };',
    'struct { float x; } value;', 'struct A { };',
    'float;', 'float value[2][3];', 'float value = {1,2};',
]:
    cases.append(f'void main() {{ {statement} }}')
for qualifier in ['uniform', 'attribute', 'varying', 'centroid varying',
                  'layout()', 'layout(location=1) uniform', 'smooth uniform',
                  'invariant varying', 'invariant flat uniform',
                  'uniform invariant', 'uniform uniform', 'precise', 'buffer']:
    for declaration in ['float value;', 'float value, other;', 'Block { float value; };']:
        cases.append(qualifier+' '+declaration)
for name in ['a²', 'aⅫ', 'a١', 'a\U0001e950', 'a\u0345', 'ǅvalue', 'gl_Position']:
    cases.append(f'float {name};')
for space in ['\t', '\r', '\n', '\v', '\f', '\u0085', '\u00a0', '\u2003', '\u2028']:
    cases.append(f'float{space}value;')
# Deterministic malformed variants exercise parser backtracking and EOF handling.
for source in [
    'uniform highp vec3 light;',
    'struct Light { vec3 position; float intensity; };',
    'layout(location=1) uniform float value;',
    'float f(in vec3 p, out float x) { x=p.x; return x; }',
    'void main() { for (int i=0; i<4; i++) { value += float(i); } }',
    'void main() { if (a && b) x = y ? z : w; else return; }',
    'void main() { while (bool active = true) { break; } }',
    'void main() { switch(x) { case 1: break; default: discard; } }',
    'uniform Block { mat4 transform; vec3 color; } instance[2];',
]:
    cases.extend(source[:i] for i in range(0, len(source), 3))
    cases.extend(source[:m.start()]+source[m.end():]
                 for m in re.finditer(r'[{}()\[\];,=?:]', source))
archive = crate/'tests/upstream/language-glsl-0.3.0'
for file, sha in json.loads((archive/'provenance.json').read_text())['files'].items():
    assert hashlib.sha256((archive/file).read_bytes()).hexdigest() == sha, file
historical = json.loads((archive/'cases.json').read_text())
cases.extend(row['shader'] for row in historical)
results = []
with tempfile.TemporaryDirectory(prefix='elm-shader-parity-') as directory:
    root = Path(directory)
    shutil.copy(crate/'tests/programs/worker/elm.json', root/'elm.json')
    bodies = [('syntax', 'shader = [glsl|'+source+'|]\n') for source in cases]
    for qualifier in ['attribute', 'uniform', 'varying']:
        for first, second in [('float','float'),('float','int'),('vec2','vec3')]:
            bodies.append(('interface', f'shaders = [ [glsl|{qualifier} {first} value;|], [glsl|{qualifier} {second} value;|] ]\n'))
    for qualifier in ['attribute', 'uniform', 'varying']:
        for second in [f'{qualifier} float other;', 'void main() {}', f'{qualifier} float value, other;']:
            bodies.append(('record-row', f'shaders = [ [glsl|{qualifier} float value;|], [glsl|{second}|] ]\n'))
    bodies.append(('qualified-interface', 'shaders = [ [glsl|invariant varying float value;|], [glsl|varying int value;|] ]\n'))
    for declaration in ['invariant varying float value;', 'layout(location=0) uniform float value;']:
        bodies.append(('qualified-interface-empty', f'shaders = [ [glsl|{declaration}|], [glsl|void main() {{}}|] ]\n'))
    for label, body in bodies:
        source = 'module Main exposing (..)\n'+body
        (root/'Main.elm').write_text(source)
        outcomes=[]
        for compiler in [args.elm.resolve(), crate/'target/release/planexpo-elm']:
            r = subprocess.run([str(compiler),'make','Main.elm','--output=/dev/null','--report=json'],cwd=root,env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=30)
            assert r.returncode in (0,1), r.stderr
            outcomes.append({'accepted':r.returncode == 0,'error':r.stderr if r.returncode else ''})
        if not outcomes[0]['accepted'] and not outcomes[1]['accepted']:
            reference = json.loads(outcomes[0]['error'])
            if any(problem['title'] == 'SHADER PROBLEM' for error in reference.get('errors', []) for problem in error['problems']):
                native = json.loads(outcomes[1]['error'])
                assert native['type'] == 'compile-errors', native
                assert native['errors'][0]['problems'][0]['title'] == 'SHADER PROBLEM', native
        results.append({'kind':label,'source':source,'reference':outcomes[0],'rust':outcomes[1],'matches':outcomes[0]['accepted']==outcomes[1]['accepted']})
report={'cases':len(results),'failures':[r for r in results if not r['matches']],'results':results}
if args.report: args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k!='results'},indent=2))
raise SystemExit(bool(report['failures']))
