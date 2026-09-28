// Opt-in benchmark preload. Set PLANEXPO_HMR_WORKER_PROFILE to a JSON path and
// use NODE_OPTIONS=--require=<this file> with benchmark_admin_hmr.mjs.
// PLANEXPO_HMR_PROFILE_PHASES=1 also enables buffered, detailed phase traces.
// Keep native wall time separate from the JS observer's request/response time.
const output = process.env.PLANEXPO_HMR_WORKER_PROFILE;
if (output && process.argv[1]?.endsWith('benchmark_admin_hmr.mjs')) {
    const fs = require('node:fs');
    const path = require('node:path');
    const assert = require('node:assert/strict');
    const { performance } = require('node:perf_hooks');
    process.env.PLANEXPO_ELM_PROFILE_WORKER = '1';
    const profilePhases = process.env.PLANEXPO_HMR_PROFILE_PHASES === '1' || process.env.PLANEXPO_ELM_PROFILE_MAKE === '1';
    if (profilePhases) process.env.PLANEXPO_ELM_PROFILE_MAKE = '1';
    const Worker = require(path.resolve(process.argv[2], 'scripts/elm-rust-worker.cjs'));
    const records = new Map();
    const compile = Worker.prototype.compile;
    const receive = Worker.prototype.receive;
    Worker.prototype.compile = function (...args) {
        let record = records.get(this);
        if (!record) {
            record = { rows: [], stderr: '' };
            records.set(this, record);
            if (profilePhases) this.child.stderr.on('data', text => { record.stderr += text; });
        }
        assert.equal(record.pending, undefined, 'Profiler expects serialized worker calls');
        record.pending = { entry: args[0][0], started: performance.now() };
        return compile.apply(this, args);
    };
    Worker.prototype.receive = function (message) {
        if (!message.ready) {
            const record = records.get(this);
            assert(record?.pending, 'Response must belong to a profiled request');
            assert(Number.isFinite(message.timing?.request_ms), 'Use a compiler with optional worker timing support');
            const { entry, started } = record.pending;
            const observed_ms = performance.now() - started;
            const native_ms = message.timing.request_ms;
            assert(observed_ms + 2 >= native_ms, 'Native work cannot exceed the observer envelope');
            record.rows.push({ entry, started, observed_ms, native_ms,
                delivery_and_transport_ms: observed_ms - native_ms, ok: message.ok });
            delete record.pending;
        }
        return receive.call(this, message);
    };
    process.on('exit', () => {
        const rows = [];
        for (const record of records.values()) {
            assert.equal(record.pending, undefined, 'Incomplete worker request');
            if (!profilePhases) { rows.push(...record.rows); continue; }
            const groups = [];
            for (const line of record.stderr.split('\n')) {
                if (!line.startsWith('ELM_MAKE_PROFILE ')) continue;
                const trace = JSON.parse(line.slice('ELM_MAKE_PROFILE '.length));
                if (trace.phase === 'package_resolution') groups.push([]);
                assert(groups.length, 'Missing request phase boundary');
                groups.at(-1).push(trace);
            }
            assert.equal(groups.length, record.rows.length, 'Each response must have exactly one phase group');
            for (const [index, row] of record.rows.entries()) {
                const phases = groups[index];
                assert(phases.every(trace => trace.entries.length === 1 && trace.entries[0] === row.entry), 'Phase entry mismatch');
                const phase_ms = phases.reduce((sum, trace) => sum + trace.ms, 0);
                assert(row.native_ms + 2 >= phase_ms, 'Phase timings must fit inside native request');
                rows.push({ ...row, phases, native_untraced_ms: row.native_ms - phase_ms });
            }
        }
        rows.sort((a, b) => a.started - b.started);
        fs.writeFileSync(output, JSON.stringify({
            scope: 'Native request includes parsing, make, output I/O, diagnostics and statistics; excludes response JSON serialization. Observed time additionally includes handshake, pipe transport and JS delivery delays. Untraced native work includes operations outside named make phases.',
            phase_traces: profilePhases,
            rows,
        }, null, 2) + '\n');
    });
}
