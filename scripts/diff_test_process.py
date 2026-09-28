"""Capture independent stdout and terminal/non-terminal stderr for diff checks."""
import errno, os, pty, subprocess, termios, threading

def run_command(command, cwd, env, ansi=False, terminal_stream="stderr", input_text=None):
 if not ansi:
  run=subprocess.run(command,cwd=cwd,env=env,capture_output=True,text=True,input=input_text,timeout=60)
  return {'code':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
 master,slave=pty.openpty();attrs=termios.tcgetattr(slave);attrs[1] &= ~termios.ONLCR;termios.tcsetattr(slave,termios.TCSANOW,attrs)
 chunks=[]
 def drain():
  try:
   while True:
    try: data=os.read(master,65536)
    except OSError as error:
     if error.errno==errno.EIO:break
     raise
    if not data:break
    chunks.append(data)
  finally:os.close(master)
 process=subprocess.Popen(command,cwd=cwd,env=env,stdin=subprocess.PIPE if input_text is not None else None,stdout=slave if terminal_stream=="stdout" else subprocess.PIPE,stderr=slave if terminal_stream=="stderr" else subprocess.PIPE);os.close(slave)
 reader=threading.Thread(target=drain);reader.start()
 try: stdout,stderr=process.communicate(input=input_text.encode() if input_text is not None else None,timeout=60)
 except subprocess.TimeoutExpired:
  process.kill();process.communicate();raise
 finally:reader.join()
 terminal=b''.join(chunks).decode()
 return {'code':process.returncode,'stdout':terminal if terminal_stream=='stdout' else stdout.decode(),'stderr':terminal if terminal_stream=='stderr' else stderr.decode()}
