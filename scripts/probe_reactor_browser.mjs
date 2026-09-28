// Isolated browser check of the actual reactor pages.
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';

const [chromium, profile, url] = process.argv.slice(2);
const browser = spawn(chromium, [
  '--headless', '--no-sandbox', '--disable-popup-blocking', '--disable-dev-shm-usage',
  '--no-first-run', '--no-default-browser-check', `--user-data-dir=${profile}`,
  '--remote-debugging-port=0', url,
], { stdio: ['ignore', 'ignore', 'pipe'], detached: true });
let diagnostics = '';
browser.stderr.on('data', chunk => { diagnostics = (diagnostics + chunk).slice(-4000); });
browser.on('error', error => { diagnostics += String(error); });
let socket;
try {
  const deadline = Date.now() + 45000;
  let target;
  while (Date.now() < deadline && !target) {
    if (browser.exitCode !== null) throw new Error(`Chromium exited: ${diagnostics}`);
    try {
      const port = diagnostics.match(/DevTools listening on ws:\/\/127\.0\.0\.1:(\d+)\//)?.[1];
      if (!port) { await delay(100); continue; }
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      target = targets.find(item => item.type === 'page' && item.url === url);
    } catch { /* Chromium is still starting. */ }
    if (!target) await delay(100);
  }
  if (!target) throw new Error(`Chromium startup timed out: ${diagnostics}`);
  socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  let serial = 0;
  const pending = new Map();
  socket.addEventListener('message', event => {
    const message = JSON.parse(event.data);
    const callback = pending.get(message.id);
    if (callback) { pending.delete(message.id); callback(message); }
  });
  async function evaluate(expression) {
    const id = ++serial;
    return await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => { pending.delete(id); reject(new Error('CDP evaluation timed out')); }, 5000);
      pending.set(id, message => {
        clearTimeout(timeout);
        if (message.error) reject(new Error(JSON.stringify(message.error)));
        else resolve(message.result.result.value);
      });
      socket.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true } }));
    });
  }
  let result;
  while (Date.now() < deadline) {
    result = await evaluate("({ready:document.readyState,title:document.title,text:document.body?.innerText,links:Array.from(document.links).map(a=>({text:a.innerText,path:a.getAttribute('href')}))})");
    if (result?.ready === 'complete' && result.text?.trim()) break;
    await delay(100);
  }
  if (!result?.text?.trim()) throw new Error(`Reactor produced no page: ${diagnostics}`);
  process.stdout.write(JSON.stringify(result));
} finally {
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ id: 1000000, method: 'Browser.close' }));
    await delay(500);
  }
  socket?.close();
  try { process.kill(-browser.pid, 'SIGTERM'); } catch (error) { if (!['ESRCH', 'EACCES', 'EPERM'].includes(error.code)) throw error; }
  await Promise.race([new Promise(resolve => browser.once('exit', resolve)), delay(2000)]);
  try { process.kill(-browser.pid, 'SIGKILL'); } catch (error) { if (!['ESRCH', 'EACCES', 'EPERM'].includes(error.code)) throw error; }
}
