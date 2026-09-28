"""Compare Unicode literal emission at BMP/surrogate/astral boundaries.

Includes observed Elm quirks: high unpaired surrogates give NaN character codes
(serialized as null), and escaped U+FFFF emits U+D7FF followed by U+DFFF.
"""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for key in ['elm','rust','report']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args()
crate=Path(__file__).resolve().parents[1]
binaries={'elm':a.elm.resolve(),'rust':a.rust.resolve()}
hashes={k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in binaries.items()}
points=[0,7,9,10,13,31,32,34,39,92,127,128,255,256,0xD7FF,0xD800,0xDBFF,0xDC00,0xDFFF,0xE000,0xFFFE,0xFFFF,0x10000,0x1F600,0x10FFFF]
values=[]
for cp in points:
 esc='\\u{'+f'{cp:04X}'+'}'
 values.append('{ text = "'+esc+'", code = Char.toCode \''+esc+'\', length = String.length "'+esc+'", equal = String.fromChar \''+esc+'\' == "'+esc+'" }')
source='''port module Main exposing (main)
import Platform
port outgoing : List {text:String, code:Int, length:Int, equal:Bool} -> Cmd msg
main : Program () () Never
main = Platform.worker { init = \\_ -> ((), outgoing ['''+', '.join(values)+''']), update = \\msg _ -> never msg, subscriptions = \\_ -> Sub.none }
'''
rows=[]
with tempfile.TemporaryDirectory(prefix='unicode-literals-') as d:
 root=Path(d);(root/'elm.json').write_bytes((crate/'tests/programs/worker/elm.json').read_bytes());(root/'Main.elm').write_text(source)
 packages=root/'home/0.19.1/packages';packages.mkdir(parents=True)
 original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
 shutil.copy2(original/'registry.dat',packages/'registry.dat')
 for name in ['elm/core/1.0.5','elm/json/1.1.3']:shutil.copytree(original/name,packages/name)
 for optimize in [False,True]:
  for label,key,flags in [('elm','elm',[]),('rust','rust',[]),('incremental','rust',['--incremental']),('warm','rust',['--incremental'])]:
   b=str(binaries[key])
   out=root/(label+'.js')
   r=subprocess.run([b,'make','Main.elm','--output='+str(out),'--report=json',*flags,*(['--optimize'] if optimize else [])],cwd=root,env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'},capture_output=True,text=True,timeout=60)
   assert r.returncode==0,r.stderr
   js="require(process.argv[1]).Elm.Main.init({flags:null}).ports.outgoing.subscribe(x=>console.log(JSON.stringify(x)))"
   executed=subprocess.run(['node','-e',js,str(out)],capture_output=True,text=True,timeout=10,check=True)
   rows.append({'mode':optimize,'compiler':label,'values':json.loads(executed.stdout)})
expected=[]
for cp in points:
 expected.append({'text':'\ud7ff\udfff' if cp==0xffff else chr(cp),'code':None if cp in [0xd800,0xdbff] else 0xd7ff if cp==0xffff else cp,'length':2 if cp>=0xffff else 1,'equal':True})
for row in rows:
 for cp,actual,wanted in zip(points,row['values'],expected):
  assert actual==wanted,(row['mode'],row['compiler'],hex(cp),actual,wanted)
assert hashes=={k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in binaries.items()}
a.report.write_text(json.dumps({'scope':__doc__,'binary_sha256':hashes,'points':points,'source':source,'runs':rows,'expected':expected,'passed':True},indent=2)+'\n')
print(json.dumps({'literal_cases':len(points),'builds':len(rows),'passed':True}))
