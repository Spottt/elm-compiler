const fs = require('node:fs');
const path = require('node:path');
const platforms = ['linux-x64', 'linux-arm64', 'darwin-x64', 'darwin-arm64', 'win32-x64'];
function resolveBinary(env = process.env, platform = process.platform, arch = process.arch) {
    const key = `${platform}-${arch}`;
    if (!env.PLANEXPO_ELM_RUST_BINARY && !platforms.includes(key)) {
        throw new Error(`Unsupported compiler platform: ${key}. Use PLANEXPO_ELM_RUST_BINARY to supply a native build.`);
    }
    const binary = env.PLANEXPO_ELM_RUST_BINARY || path.join(__dirname, 'platforms', key, platform === 'win32' ? 'planexpo-elm.exe' : 'planexpo-elm');
    try { fs.accessSync(binary, fs.constants.X_OK); }
    catch { throw new Error(`Compiler executable missing: ${binary}. Published packages contain every release binary; from a source checkout build with "cargo build --release" and set PLANEXPO_ELM_RUST_BINARY, or use "node distribution/pack-local.mjs".`); }
    return binary;
}
module.exports = { resolveBinary, platforms };
