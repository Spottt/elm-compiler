#!/usr/bin/env python3
"""Compare JSON syntax diagnostics and acceptance with Elm 0.19.1.

Fixtures include a leading blank line because the reference's Word16 snippet
bounds underflow for several first-line errors. Four control-character fixtures
need tolerant decoding of the reference's malformed JSON serialization; their
names are retained in reference_nonstandard_json. Terminal text is compared too.
These cases do not demonstrate byte-for-byte JSON serialization parity.
"""
import argparse, copy, hashlib, json, os, random, shutil, subprocess, tempfile
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
for key in ['elm', 'rust', 'report']: p.add_argument('--'+key, type=Path, required=True)
a = p.parse_args()
sha = {name: hashlib.sha256(binary.read_bytes()).hexdigest() for name, binary in [('elm', a.elm), ('rust', a.rust)]}
configs = {'app': {'type': 'application', 'elm-version': '0.19.1', 'source-directories': ['src'], 'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3'}, 'indirect': {}}, 'test-dependencies': {'direct': {}, 'indirect': {}}}, 'package': {'type': 'package', 'name': 'author/fixture', 'summary': 'Fixture.', 'license': 'BSD-3-Clause', 'version': '1.0.0', 'exposed-modules': ['Main'], 'elm-version': '0.19.0 <= v < 0.20.0', 'dependencies': {'elm/core': '1.0.0 <= v < 2.0.0'}, 'test-dependencies': {}}}
cases = [
 ('empty',''),('spaces',' \n  '),('object-open','{'),('object-key','{foo:1}'),
 ('object-colon','{"type" "application"}'),('object-value','{"type":}'),
 ('object-end','{"type":"application"'),('object-comma','{"type":"application",}'),
 ('object-missing-comma','{"a":1 "b":2}'),('array-open','['),('array-end','[1'),
 ('array-comma','[1,]'),('array-missing-comma','[1 2]'),
 ('string-end','{"type":"application}'),('string-newline','{"type":"app\nlication"}'),
 ('string-tab','{"type":"app\tlication"}'),('string-control','{"type":"app\x01lication"}'),
 ('escape-unknown',r'{"type":"app\q"}'),('escape-short',r'{"type":"app\u12"}'),
 ('escape-hex',r'{"type":"app\uGGGG"}'),('leading-zero','{"n":01}'),
 ('fraction','{"n":1.5}'),('zero-fraction','{"n":0.5}'),('exponent','{"n":1e2}'),
 ('zero-exponent','{"n":0e2}'),('negative','{"n":-1}'),
 ('keyword-partial','{"n":tru}'),('keyword-suffix','{"n":truex}'),
 ('extra','{} extra'),('extra-value','{} []'),
 ('multiline-comma','{\n "a":1,\n "b":2,\n}'),
 ('multiline-missing-colon','{\n "a":1,\n "b":2,\n "c" 3\n}'),
 ('unicode-error','{\n "é":"😀",\n "a":1,\n "b":2\n "c":3\n}'),
 ('crlf-error','{\r\n "a":1,\r\n}'),
]
for name,value in [('float','1.5'),('negative','-1'),('huge','9'*40),('positive','42'),('surrogate',r'"\ud800"')]:
 source=json.dumps(configs['app'],indent=2)
 source=source[:-1]+',"unused":'+value+'}'
 cases.append((('valid-' if name in ['huge','positive','surrogate'] else '')+name,source))
def diagnostic(stderr):
 if not stderr:return None
 try:return json.loads(stderr)
 except json.JSONDecodeError:
  try:return json.loads(stderr,strict=False)
  except json.JSONDecodeError:return {'unparsed':stderr}
cases.extend([
 ('escape-eof', '{"a":"abc'+chr(92)),
 ('hex-eof', r'{"a":"\u12'),
 ('object-key-eof', '{"abc'),
 ('nested-array-open', '{"a":['),
 ('nested-object-open', '[{'),
 ('keyword-underscore', '{"a":true_}'),
 ('keyword-unicode', '{"a":trueé}'),
 ('keyword-false', '{"a":falsee}'),
 ('keyword-null', '{"a":nullx}'),
 ('number-exponent-sign', '{"a":1E-2}'),
 ('number-plus', '{"a":+1}'),
 ('number-dot', '{"a":.1}'),
 ('zero-leading', '{"a":00}'),
 ('tab-whitespace', '{\n\t"a"\t1}'),
 ('carriage-whitespace', '{\r"a"\r1}'),
 ('vertical-whitespace', '{\v"a":1}'),
 ('bom', '\ufeff{}'),
])
# Deterministic single-character mutations exercise neighboring parser states.
# Multiline object bounds avoid the separately documented reference reporter loop.
mutation_rng = random.Random(20260925)
mutation_base = '{\n "unused": [true, null, 42, {"key":"é"}]\n}'
mutations = set()
for attempt in range(80):
 pos = mutation_rng.randrange(1, len(mutation_base))
 operation = mutation_rng.randrange(3)
 character = mutation_rng.choice('[]{}:,"\\ abc012\né')
 source = mutation_base[:pos] + (character if operation != 0 else '') + mutation_base[pos + (operation != 2):]
 if source not in mutations:
  mutations.add(source)
  cases.append((f'mutation-{attempt}', source))
nonstandard_reference_json=[]
results=[]
with tempfile.TemporaryDirectory(prefix='outline-json-') as temp:
 root=Path(temp); home=root/'home'; shutil.copytree(Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages',home/'0.19.1/packages'); (root/'src').mkdir(); (root/'src/Main.elm').write_text('module Main exposing (x)\nx = 1\n')
 for name,source in cases:
  # Elm's snippet renderer crashes on these first-line errors; retain a leading blank line.
  (root/'elm.json').write_text('\n'+source); rows=[]
  for binary in [a.elm,a.rust]:
   streams=[]
   for flags in [['--report=json'],[]]:
    run=subprocess.run([str(binary.resolve()),'make','src/Main.elm','--output=/dev/null',*flags],cwd=root,env={**os.environ,'GHCRTS':'-N1','ELM_HOME':str(home)},capture_output=True,text=True,timeout=10)
    if binary==a.elm and flags and run.stderr:
     try:json.loads(run.stderr)
     except json.JSONDecodeError:nonstandard_reference_json.append(name)
    streams.append({'code':run.returncode,'stderr':diagnostic(run.stderr) if flags else run.stderr})
   rows.append(streams)
  passed=rows[0]==rows[1] and rows[0][0]['code']==(0 if name.startswith('valid-') else 1)
  results.append({'case':name,'passed':passed,'official':rows[0],'rust':rows[1]});print(name,'PASS' if passed else 'FAIL',flush=True)
assert sha=={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in [('elm',a.elm),('rust',a.rust)]}
report={'scope':__doc__,'reference_nonstandard_json':nonstandard_reference_json,'sha256':sha,'cases':len(results),'passed':all(r['passed'] for r in results),'results':results};a.report.write_text(json.dumps(report,indent=2)+'\n');raise SystemExit(not report['passed'])
