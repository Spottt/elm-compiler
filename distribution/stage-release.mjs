import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const [input, output] = process.argv.slice(2);
if (!input || !output || fs.existsSync(output)) throw new Error('Usage: node stage-release.mjs BINARIES NEW_OUTPUT_DIRECTORY');
const platforms = ['linux-x64', 'linux-arm64', 'darwin-x64', 'darwin-arm64', 'win32-x64'];
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'npm/package.json')));
const cargoVersion = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8').match(/^version = "([^"]+)"/m)[1];
if (process.env.GITHUB_REF?.startsWith('refs/tags/') && process.env.GITHUB_REF !== `refs/tags/v${manifest.version}`) throw new Error('Release tag does not match package version');
if (manifest.version !== cargoVersion) throw new Error('Cargo and npm versions differ');
const binaries = platforms.map(platform => {
    const filename = platform.startsWith('win32-') ? 'planexpo-elm.exe' : 'planexpo-elm';
    const file = path.join(input, platform, filename);
    const bytes = fs.readFileSync(file);
    const linux = platform.startsWith('linux');
    const pe = bytes.length > 64 ? bytes.readUInt32LE(60) : 0;
    const windows = bytes.length > 64 && bytes.toString('ascii', 0, 2) === 'MZ' && pe >= 64 && pe + 26 <= bytes.length && bytes.readUInt32LE(pe) === 0x4550 && bytes.readUInt16LE(pe + 4) === 0x8664 && bytes.readUInt16LE(pe + 24) === 0x20b;
    const valid = platform.startsWith('win32-') ? windows : linux
        ? bytes.length > 64 && bytes.subarray(0,4).equals(Buffer.from([127,69,76,70])) && bytes[4] === 2 && bytes[5] === 1 && bytes.readUInt16LE(18) === (platform.endsWith('x64') ? 62 : 183)
        : bytes.length > 32 && bytes.readUInt32LE(0) === 0xfeedfacf && bytes.readUInt32LE(4) === (platform.endsWith('x64') ? 0x1000007 : 0x100000c);
    if (!valid) throw new Error(`Wrong or invalid executable for ${platform}`);
    return { platform, filename, bytes, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
});
fs.mkdirSync(output, { recursive: true });
for (const file of ['package.json','resolve.cjs','README.md','LICENSE','NOTICE','LICENSE-ELM','LICENSE-GHC','LICENSE-GLSL','LICENSE-UNICODE','LICENSE-FONTS','verify.cjs']) fs.copyFileSync(path.join(root,'npm',file),path.join(output,file));
fs.cpSync(path.join(root,'npm/bin'),path.join(output,'bin'),{recursive:true});
for (const {platform,filename,bytes} of binaries) {
    const dir = path.join(output,'platforms',platform); fs.mkdirSync(dir,{recursive:true});
    fs.writeFileSync(path.join(dir,filename),bytes,{mode:0o755});
}
fs.writeFileSync(path.join(output,'checksums.json'), JSON.stringify(Object.fromEntries(binaries.map(b=>[b.platform,b.sha256])),null,2)+'\n');
console.log(`Release ${manifest.version} assembled with ${platforms.length} binaries in ${output}`);
