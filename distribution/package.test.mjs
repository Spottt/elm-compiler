import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawn, spawnSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(import.meta.url);
const {resolveBinary} = require('../npm/resolve.cjs');
test('launcher preserves arguments, streams and failure status', () => {
    const temp = fs.mkdtempSync(path.join(os.tmpdir(),'elm-npm-'));
    try {
        const binary = path.join(temp,'compiler with spaces');
        fs.writeFileSync(binary, '#!/usr/bin/env node\nconsole.log(JSON.stringify(process.argv.slice(2))); console.error("diagnostic"); process.exit(7);\n',{mode:0o755});
        const result = spawnSync(process.execPath,[path.join(root,'npm/bin/planexpo-elm.cjs'),'make','file with spaces.elm','--output=out.js'],{encoding:'utf8',env:{...process.env,PLANEXPO_ELM_RUST_BINARY:binary}});
        assert.equal(result.status,7); assert.equal(result.stderr.trim(),'diagnostic');
        assert.deepEqual(JSON.parse(result.stdout),['make','file with spaces.elm','--output=out.js']);
        assert.equal(resolveBinary({PLANEXPO_ELM_RUST_BINARY:binary},'unknown','unknown'),binary);
        assert.throws(()=>resolveBinary({},'win32','x64'),/Unsupported compiler platform/);
        assert.throws(()=>resolveBinary({PLANEXPO_ELM_RUST_BINARY:path.join(temp,'missing')}),/missing/);
    } finally {fs.rmSync(temp,{recursive:true,force:true});}
});
test('release cannot be assembled from missing or wrong architecture binaries', () => {
    const temp = fs.mkdtempSync(path.join(os.tmpdir(),'elm-release-'));
    try {
        const result = spawnSync(process.execPath,[path.join(root,'distribution/stage-release.mjs'),temp,path.join(temp,'out')],{encoding:'utf8'});
        assert.notEqual(result.status,0); assert.equal(fs.existsSync(path.join(temp,'out')),false);
        fs.mkdirSync(path.join(temp,'linux-x64')); fs.writeFileSync(path.join(temp,'linux-x64/planexpo-elm'),'not an executable');
        const invalid = spawnSync(process.execPath,[path.join(root,'distribution/stage-release.mjs'),temp,path.join(temp,'out')],{encoding:'utf8'});
        assert.match(invalid.stderr,/Wrong or invalid executable/);assert.equal(fs.existsSync(path.join(temp,'out')),false);
    } finally {fs.rmSync(temp,{recursive:true,force:true});}
});
// The export tool only exists in the Planexpo monorepo, not in the public repository it produces.
test('repository export excludes Planexpo reports and preserves all test sources', {skip: !fs.existsSync(path.join(root,'distribution/export-repository.mjs'))}, () => {
    const temp = fs.mkdtempSync(path.join(os.tmpdir(),'elm-export-'));const out=path.join(temp,'repo');
    try {
        const result=spawnSync(process.execPath,[path.join(root,'distribution/export-repository.mjs'),out],{encoding:'utf8'});
        assert.equal(result.status,0,result.stderr);
        for(const name of ['benchmarks','front','target','node_modules','releases','DELIVERY.md','COMPATIBILITY.md','distribution/prepare-pinned.cjs','distribution/pinned.test.mjs','distribution/standalone','scripts/check_admin_browser.py','distribution/export-repository.mjs'])assert.equal(fs.existsSync(path.join(out,name)),false,name);
        for(const name of ['LICENSE','NOTICE','LICENSE-ELM','README.md','CONTRIBUTING.md','RELEASING.md','CHANGELOG.md','rust-toolchain.toml','npm/LICENSE','npm/NOTICE'])assert.ok(fs.existsSync(path.join(out,name)),name);
        assert.doesNotMatch(fs.readFileSync(path.join(out,'README.md'),'utf8'),/make elmRust|exposalons/);
        const compare = (directory) => {for(const item of fs.readdirSync(path.join(root,directory),{withFileTypes:true})){const rel=path.join(directory,item.name);if(item.isDirectory())compare(rel);else assert.deepEqual(fs.readFileSync(path.join(root,rel)),fs.readFileSync(path.join(out,rel)),rel);}};
        compare('tests');compare('src');
        assert.ok(fs.existsSync(path.join(out,'.github/workflows/release.yml')));
    } finally {fs.rmSync(temp,{recursive:true,force:true});}
});

test('launcher forwards termination and waits for the child exit', {timeout: 5000}, async () => {
 const temp=fs.mkdtempSync(path.join(os.tmpdir(),'elm-signal-'));
 const binary=path.join(temp,'compiler');
 fs.writeFileSync(binary, '#!/usr/bin/env node\nprocess.on("SIGTERM",()=>process.exit(23));console.log("ready");setInterval(()=>{},1000);\n',{mode:0o755});
 const child=spawn(process.execPath,[path.join(root,'npm/bin/planexpo-elm.cjs')],{env:{...process.env,PLANEXPO_ELM_RUST_BINARY:binary},stdio:['ignore','pipe','pipe']});
 try {
  const exited=new Promise((resolve,reject)=>{child.on('close',resolve);child.on('error',reject);});
  await new Promise((resolve,reject)=>{child.stdout.once('data',resolve);child.once('error',reject);});
  child.kill('SIGTERM');assert.equal(await exited,23);
 }finally{if(child.exitCode===null)child.kill('SIGTERM');fs.rmSync(temp,{recursive:true,force:true});}
});

test('local pack produces an installable single-platform tarball that refuses publication', {skip: !['linux-x64','linux-arm64','darwin-x64','darwin-arm64'].includes(`${process.platform}-${process.arch}`)}, () => {
    const temp = fs.mkdtempSync(path.join(os.tmpdir(),'elm-pack-'));
    try {
        const binary = path.join(temp,'compiler');
        fs.writeFileSync(binary,'#!/bin/sh\necho local-build\n',{mode:0o755});
        const packed = spawnSync(process.execPath,[path.join(root,'distribution/pack-local.mjs'),'--binary',binary,'--out',path.join(temp,'dist')],{encoding:'utf8'});
        assert.equal(packed.status,0,packed.stderr);
        const [tarball] = fs.readdirSync(path.join(temp,'dist'));
        assert.match(tarball,/^spottt-elm-compiler-.*\.tgz$/);
        const extract = path.join(temp,'extract'); fs.mkdirSync(extract);
        assert.equal(spawnSync('tar',['-xzf',path.join(temp,'dist',tarball),'-C',extract]).status,0);
        const pkg = path.join(extract,'package');
        for (const file of ['LICENSE','NOTICE','LICENSE-ELM','checksums.json',`platforms/${process.platform}-${process.arch}/planexpo-elm`]) assert.ok(fs.existsSync(path.join(pkg,file)),file);
        const run = spawnSync(process.execPath,[path.join(pkg,'bin/planexpo-elm.cjs')],{encoding:'utf8',env:{...process.env,PLANEXPO_ELM_RUST_BINARY:''}});
        assert.equal(run.stdout.trim(),'local-build',run.stderr);
        assert.notEqual(spawnSync(process.execPath,[path.join(pkg,'verify.cjs')]).status,0);
    } finally {fs.rmSync(temp,{recursive:true,force:true});}
});
