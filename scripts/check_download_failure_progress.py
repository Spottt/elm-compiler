#!/usr/bin/env python3
"""Opt-in network check for dependency progress after failed archive download."""
import argparse,re,time
import socket,socketserver,select,threading,json,os,subprocess,shutil,tempfile,concurrent.futures,hashlib
from pathlib import Path
p=argparse.ArgumentParser(description="Network integration: block archive CONNECT after allowing the package registry")
p.add_argument('--rust',type=Path,required=True);p.add_argument('--elm',type=Path,required=True);p.add_argument('--report',type=Path,required=True)
p.add_argument('--status',type=int,default=502)
p.add_argument('--reason',default='Bad Gateway')
p.add_argument('--fragmented',action='store_true')
p.add_argument('--disconnect',action='store_true',help='Close the rejected CONNECT connection without sending a response')
p.add_argument('--registry-lf',action='store_true',help='Use LF-only lines in successful registry tunnel responses')
p.add_argument('--registry-continue',action='store_true',help='Send 100 Continue before successful registry tunnels')
p.add_argument('--partial-response',default='',help='Bytes sent before --disconnect closes the proxy')
a=p.parse_args()
if not 100 <= a.status <= 599 or a.status == 200 or any(c in a.reason for c in '\r\n'):
 p.error('expected a rejection status and a single-line reason phrase')
requests=[]
class Proxy(socketserver.BaseRequestHandler):
 def handle(self):
  self.request.settimeout(15);data=b''
  while b'\r\n\r\n' not in data and len(data)<16384:
   part=self.request.recv(4096)
   if not part:return
   data+=part
  line=data.split(b'\r\n',1)[0].decode();method,target,_=line.split();requests.append(target)
  if method!='CONNECT' or target!='package.elm-lang.org:443':
   if a.disconnect:
    response=a.partial_response.encode('latin-1')
    if a.fragmented:
     for start in range(0,len(response),9):
      try:self.request.sendall(response[start:start+9])
      except (BrokenPipeError,ConnectionResetError):return
      time.sleep(0.005)
    else:self.request.sendall(response)
    return
   response=f'HTTP/1.1 {a.status} {a.reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n'.encode('latin-1')
   if a.fragmented:
    self.request.sendall(response[:9]);time.sleep(0.02);self.request.sendall(response[9:])
   else:self.request.sendall(response)
   return
  try:
   with socket.create_connection(('package.elm-lang.org',443),timeout=15) as upstream:
    response=(b'HTTP/1.1 100 Continue\r\n\r\n' if a.registry_continue else b'') + b'HTTP/1.1 200 Connection established\r\n\r\n'
    self.request.sendall(response.replace(b'\r\n',b'\n') if a.registry_lf else response)
    while True:
     ready,_,_=select.select([self.request,upstream],[],[],20)
     if not ready:return
     for src in ready:
      chunk=src.recv(65536)
      if not chunk:return
      (upstream if src is self.request else self.request).sendall(chunk)
  except (OSError,TimeoutError):return
class Server(socketserver.ThreadingTCPServer):
 daemon_threads=True
server=Server(('127.0.0.1',0),Proxy);threading.Thread(target=server.serve_forever,daemon=True).start()
proxy=f'http://127.0.0.1:{server.server_address[1]}'
root=Path(tempfile.mkdtemp(prefix='download-proxy-'))
original=Path.home()/'.elm/0.19.1/packages'
def run(item):
 label,binary,json_mode=item
 project=root/(label+('-json' if json_mode else '-terminal'));project.mkdir();cache=project/'home/0.19.1/packages';cache.mkdir(parents=True)
 shutil.copy2(original/'registry.dat',cache/'registry.dat')
 for package in ['elm/core/1.0.5','elm/json/1.1.3']:shutil.copytree(original/package,cache/package)
 (cache/'elm/url/1.0.0').mkdir(parents=True);shutil.copy2(original/'elm/url/1.0.0/elm.json',cache/'elm/url/1.0.0/elm.json')
 (project/'elm.json').write_text(json.dumps({'type':'application','source-directories':['.'],'elm-version':'0.19.1','dependencies':{'direct':{'elm/core':'1.0.5','elm/json':'1.1.3','elm/url':'1.0.0'},'indirect':{}},'test-dependencies':{'direct':{},'indirect':{}}}))
 (project/'Main.elm').write_text('module Main exposing (..)\nx = 1\n')
 env={**os.environ,'ELM_HOME':str(project/'home'),'GHCRTS':'-N1 -A16m -c','NO_PROXY':'','no_proxy':'',**{key:proxy for key in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy']}}
 p=subprocess.run([binary,'make','Main.elm','--output=/dev/null',*(['--report=json'] if json_mode else [])],cwd=project,env=env,capture_output=True,timeout=90)
 return {'compiler':label,'json':json_mode,'sha256':hashlib.sha256(Path(binary).read_bytes()).hexdigest(),'code':p.returncode,'stdout':p.stdout.decode(),'stderr':p.stderr.decode(),'sources_published':(cache/'elm/url/1.0.0/src').exists(),'verification_cached':(project/'elm-stuff/planexpo-rust/dependencies-built-v1.json').exists()}
try:
 with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:results=list(pool.map(run,[(label,str(binary.resolve()),mode) for mode in (False,True) for label,binary in [('elm',a.elm),('rust',a.rust)]]))
 checks=[]
 diagnostics=[]
 for result in results:
  output=result['stdout']
  if result['json']:
   try:
    diagnostic=json.loads(result['stderr'])
    chunks=diagnostic['message']
    body=''.join(chunk if isinstance(chunk,str) else chunk['string'] for chunk in chunks)
    diagnostics.append(diagnostic['type']=='error' and diagnostic['title']=='PROBLEM DOWNLOADING PACKAGE' and diagnostic['path'] is None and 'I was trying to download the source code for elm/url 1.0.0' in body and 'https://github.com/elm/url/zipball/1.0.0/' in body and 'But my HTTP library is giving me the following error message:' in body)
   except (ValueError,KeyError,TypeError):diagnostics.append(False)
  else:
   diagnostics.append(result['stderr'].startswith('-- PROBLEM DOWNLOADING PACKAGE ') and 'I was trying to download the source code for elm/url 1.0.0' in result['stderr'] and 'https://github.com/elm/url/zipball/1.0.0/' in result['stderr'] and 'But my HTTP library is giving me the following error message:' in result['stderr'])
  counters=[(int(n),int(t)) for n,t in re.findall(r'Verifying dependencies \((\d+)/(\d+)\)',output)]
  if result['json']:
   checks.append(result['code']==1 and not result['sources_published'] and not result['verification_cached'] and not output)
   continue
  checks.append(result['code']==1 and not result['sources_published'] and not result['verification_cached'] and output.startswith('Starting downloads...\n\n  ✗ elm/url 1.0.0\n\nVerifying dependencies (') and bool(counters) and counters[-1]==(3,3) and all(t==3 and 0<=n<=3 for n,t in counters) and all(x[0]<=y[0] for x,y in zip(counters,counters[1:])) and output.endswith('\r'+' '*len('Verifying dependencies (3/3)')+'\rDependency problem!\n'))
 for mode in (False, True):
  pair=[result for result in results if result['json']==mode]
  if mode:
   normalized=[]
   for result in pair:
    try:
     diagnostic=json.loads(result['stderr'])
     normalized.append(diagnostic)
    except (ValueError,KeyError,TypeError):normalized.append(None)
   diagnostics.append(normalized[0] is not None and normalized[0]==normalized[1])
  else:
   diagnostics.append(pair[0]['stderr']==pair[1]['stderr'])
 report={'scope':'Download failure progress and exact proxy failure diagnostics, including transport exception','proxy_status':a.status,'proxy_reason':a.reason,'fragmented':a.fragmented,'disconnect':a.disconnect,'registry_continue':a.registry_continue,'registry_lf':a.registry_lf,'partial_response':a.partial_response,'root':str(root),'requests':requests,'passed':all(checks) and all(diagnostics),'progress_passed':all(checks),'diagnostic_context_passed':all(diagnostics),'results':results}
 a.report.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
finally:server.shutdown();server.server_close();shutil.rmtree(root)

raise SystemExit(not report['passed'])
