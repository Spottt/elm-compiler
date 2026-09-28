#!/usr/bin/env node
// Use the frontend's installed minifier; this does not imply Elm --optimize.
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { createHash } = require('crypto');
const front = path.resolve(__dirname, '../../front');
const terser = require(require.resolve('terser', { paths: [front] }));
const version = require(require.resolve('terser/package.json', { paths: [front] })).version;

async function main() {
    const [input, output, diagnosticMode] = process.argv.slice(2);
    const compressionModes = {
        '--mangle-only': false,
        '--no-inline': { inline: false, reduce_vars: false, collapse_vars: false },
        '--no-reduce-vars': { reduce_vars: false },
        '--no-collapse-vars': { collapse_vars: false },
        '--no-inline-only': { inline: false },
    };
    if (!input || !output || process.argv.length > 5 || (diagnosticMode && !Object.hasOwn(compressionModes, diagnosticMode))) {
        throw new Error(`usage: node minify_bundle.cjs input.js output.min.js [${Object.keys(compressionModes).join('|')}]`);
    }
    const source = fs.readFileSync(input, 'utf8');
    const start = performance.now();
    // Terser defaults, as used by the project's TerserWebpackPlugin.
    const options = {
        compress: diagnosticMode ? compressionModes[diagnosticMode] : true,
        mangle: true,
    };
    // Terser 4 mutates its options with internal AST/scope objects.
    const reportedOptions = JSON.parse(JSON.stringify(options));
    const result = await terser.minify(source, options);
    if (result.error) throw result.error;
    if (typeof result.code !== 'string') throw new Error('Terser produced no JavaScript');
    fs.writeFileSync(output, result.code);
    const sizes = content => ({
        bytes: Buffer.byteLength(content),
        gzip_bytes: zlib.gzipSync(content, { level: 9 }).length,
        sha256: createHash('sha256').update(content).digest('hex'),
    });
    console.log(JSON.stringify({
        minifier: `terser@${version}`, options: reportedOptions,
        elapsed_ms: performance.now() - start,
        input: { path: input, ...sizes(source) }, output: { path: output, ...sizes(result.code) },
    }, null, 2));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
