import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
parser=argparse.ArgumentParser()
for key in ['elm','rust','report']:parser.add_argument('--'+key,type=Path,required=True)
args=parser.parse_args()
sha=hashlib.sha256(args.rust.read_bytes()).hexdigest()
results=[]
temporary=tempfile.TemporaryDirectory(prefix='unused-dependency-')
root=Path(temporary.name)
original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
for label,relative,extra in [('valid',None,None),('unused-exposed-type','Process.elm','\nbroken = True + 1\n'),('unused-private-type','UnusedPrivate.elm','module UnusedPrivate exposing (..)\nbroken = True + 1\n')]:
 project=root/label;project.mkdir()
 home=project/'home';cache=home/'0.19.1/packages';cache.mkdir(parents=True)
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for package in ['elm/core/1.0.5','elm/json/1.1.3']:
  shutil.copytree(original/package,cache/package)
  for artifact in (cache/package).glob('*.dat'): artifact.unlink()
 if relative:
  path=cache/'elm/core/1.0.5/src'/relative
  if path.exists():path.write_text(path.read_text()+extra)
  else:path.write_text(extra)
 (project/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
 (project/'Main.elm').write_text('module Main exposing (..)\nx = 1\n')
 runs=[]
 env={**os.environ,'ELM_HOME':str(home),'GHCRTS':'-N1 -A16m -c'}
 for binary in [str(args.elm.resolve()),str(args.rust.resolve())]:
  run=subprocess.run([binary,'make','Main.elm','--output=/dev/null','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=60)
  runs.append({'code':run.returncode,'stdout':run.stdout,'stderr':run.stderr})
 expected=1 if label=='unused-exposed-type' else 0
 diagnostics=[json.loads(run['stderr']) if run['stderr'] else None for run in runs]
 passed=all(run['code']==expected and not run['stdout'] for run in runs) and diagnostics[0]==diagnostics[1]
 stamp=project/'elm-stuff/planexpo-rust/dependencies-built-v1.json'
 if expected == 0:
  assert stamp.is_file(), 'successful dependency verification was not cached'
  before=(stamp.read_bytes(),stamp.stat().st_mtime_ns)
  warm=subprocess.run([str(args.rust.resolve()),'make','Main.elm','--output=/dev/null','--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=60)
  passed &= warm.returncode == 0 and not warm.stdout and not warm.stderr
  passed &= before == (stamp.read_bytes(),stamp.stat().st_mtime_ns)
 else:
  passed &= not stamp.exists()
 results.append({'case':label,'passed':passed,'official':runs[0],'rust':runs[1]})
 print(label,[r['code'] for r in runs],flush=True)
assert hashlib.sha256(args.rust.read_bytes()).hexdigest()==sha
args.report.write_text(json.dumps({'scope':'All exposed dependency modules are checked; unreachable private modules remain ignored. Private cold package caches, JSON diagnostics compared after decoding.','compiler_sha256':sha,'passed':all(r['passed'] for r in results),'results':results},indent=2)+'\n')
temporary.cleanup()
raise SystemExit(not all(r['passed'] for r in results))
