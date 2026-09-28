#!/usr/bin/env python3
"""Differential recursion ordering probe; known discrepancies remain failures."""
import argparse,json,os,subprocess,tempfile,hashlib
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--elm',type=Path,required=True)
parser.add_argument('--report',type=Path,required=True)
args=parser.parse_args()
repo=Path(__file__).resolve().parents[2];elm=args.elm.resolve();rust=repo/'elm-rs/target/release/planexpo-elm'
rows=[]
with tempfile.TemporaryDirectory(prefix='elm-recursive-order-') as directory:
 p=Path(directory);(p/'elm.json').write_bytes((repo/'elm-rs/tests/programs/worker/elm.json').read_bytes())
 for first,second in [('f','g'),('g','f'),('a','z'),('z','a'),('aa','a'),('a','aa')]:
  for reverse in [False,True]:
   for growth in [0,1]:
    bodies=[f'{first} x = {second} '+('[x]' if growth==0 else 'x'),f'{second} y = {first} '+('[y]' if growth==1 else 'y')]
    if reverse:bodies.reverse()
    source='module Fixture exposing (..)\n'+'\n'.join(bodies)+'\n'
    (p/'Fixture.elm').write_text(source)
    row={'first':first,'second':second,'reverse_declarations':reverse,'growing_function':[first,second][growth],'source':source}
    for name,binary in [('rust',rust),('elm',elm)]:
     r=subprocess.run([str(binary),'make','Fixture.elm','--output=/dev/null','--report=json'],cwd=p,env={**os.environ,'GHCRTS':'-N1 -A16m -c'},capture_output=True,text=True,timeout=30)
     row[name]={'accepted':r.returncode==0,'diagnostic':json.loads(r.stderr) if r.returncode else None}
    expected=row['growing_function']==max(first,second)
    assert row['elm']['accepted']==expected, ('historical ordering changed',row)
    if not expected:
     titles=[p['title'] for e in row['elm']['diagnostic'].get('errors',[]) for p in e['problems']]
     assert titles and all(title=='INFINITE TYPE' for title in titles), (row,titles)
    row['expected']=expected
    rows.append(row)
report={'scope':'24 two-function cycles: six name pairs, both declaration orders and both positions of list growth. Strict differential acceptance; lexical sensitivity is verified against the historical binary, not implemented as a compiler rule.','compiler_sha256':hashlib.sha256(rust.read_bytes()).hexdigest(),'elm_sha256':hashlib.sha256(elm.read_bytes()).hexdigest(),'results':rows,'failures':[{'first':r['first'],'second':r['second'],'reverse_declarations':r['reverse_declarations'],'growing_function':r['growing_function']} for r in rows if r['rust']['accepted']!=r['expected']]}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(rows),'failures':len(report['failures'])}))
raise SystemExit(bool(report['failures']))
