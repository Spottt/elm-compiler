// Builds an installable npm tarball for the current platform only, so anyone can
// try the package without the release CI. The tarball cannot be published:
// verify.cjs (prepublishOnly) requires every release binary.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const {platforms} = createRequire(import.meta.url)(path.join(root, 'npm/resolve.cjs'));
const args = process.argv.slice(2);
const option = name => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const output = path.resolve(option('--out') ?? '.');
const platform = `${process.platform}-${process.arch}`;
const filename = process.platform === 'win32' ? 'planexpo-elm.exe' : 'planexpo-elm';
if (!platforms.includes(platform)) throw new Error(`Unsupported platform ${platform}; supported: ${platforms.join(', ')}`);
const run = (command, commandArgs, options = {}) => {
    const result = spawnSync(command, commandArgs, {stdio: 'inherit', ...options});
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`${command} ${commandArgs.join(' ')} failed (${result.status})`);
    return result;
};
let binary = option('--binary');
if (!binary) {
    run('cargo', ['build', '--locked', '--release'], {cwd: root});
    binary = path.join(root, 'target/release', filename);
}
const bytes = fs.readFileSync(binary);
const stage = fs.mkdtempSync(path.join(os.tmpdir(), 'elm-compiler-pack-'));
try {
    for (const file of ['package.json', 'resolve.cjs', 'README.md', 'LICENSE', 'NOTICE', 'LICENSE-ELM', 'LICENSE-GHC', 'LICENSE-GLSL', 'LICENSE-FONTS', 'verify.cjs']) {
        fs.copyFileSync(path.join(root, 'npm', file), path.join(stage, file));
    }
    fs.cpSync(path.join(root, 'npm/bin'), path.join(stage, 'bin'), {recursive: true});
    fs.mkdirSync(path.join(stage, 'platforms', platform), {recursive: true});
    fs.writeFileSync(path.join(stage, 'platforms', platform, filename), bytes, {mode: 0o755});
    const sha256 = crypto.createHash('sha256').update(bytes).digest('hex');
    fs.writeFileSync(path.join(stage, 'checksums.json'), JSON.stringify({[platform]: sha256}, null, 2) + '\n');
    fs.mkdirSync(output, {recursive: true});
    const packed = spawnSync('npm', ['pack', stage, '--pack-destination', output, '--json'], {encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], shell: process.platform === 'win32'});
    if (packed.status !== 0) throw new Error('npm pack failed');
    const [{filename}] = JSON.parse(packed.stdout);
    console.log(`Local package for ${platform}: ${path.join(output, filename)}`);
    console.log(`Try it: npm install --save-dev ${path.join(output, filename)} && npx planexpo-elm --version`);
} finally {
    fs.rmSync(stage, {recursive: true, force: true});
}
