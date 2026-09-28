#!/usr/bin/env node
const { spawn } = require('node:child_process');
const { resolveBinary } = require('../resolve.cjs');
try {
    const child = spawn(resolveBinary(), process.argv.slice(2), { stdio: 'inherit' });
    for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
        process.on(signal, () => { if (!child.killed) child.kill(signal); });
    }
    child.on('error', error => { console.error(error.message); process.exitCode = 1; });
    child.on('exit', (code, signal) => {
        process.exitCode = code ?? ({ SIGINT: 130, SIGTERM: 143, SIGHUP: 129 }[signal] || 1);
    });
} catch (error) {
    console.error(error.message);
    process.exitCode = 1;
}
