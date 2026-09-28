"""Shared isolated pseudo-terminal capture for compiler differential tests."""
import errno
import os
import pty
import select
import subprocess
import termios
import time

def run_terminal(command, project, env):
    master, slave = pty.openpty()
    attributes = termios.tcgetattr(slave)
    attributes[1] &= ~termios.OPOST
    termios.tcsetattr(slave, termios.TCSANOW, attributes)
    process = subprocess.Popen(command, cwd=project, env=env, stdin=subprocess.DEVNULL,
                               stdout=subprocess.DEVNULL, stderr=slave)
    os.close(slave)
    output = bytearray()
    deadline = time.monotonic() + 30
    try:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(command, 30)
            if not select.select([master], [], [], remaining)[0]:
                raise subprocess.TimeoutExpired(command, 30)
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            output.extend(chunk)
        return process.wait(timeout=max(0.1, deadline - time.monotonic())), output.decode('utf-8')
    finally:
        os.close(master)
        if process.poll() is None:
            process.kill()
            process.wait()
