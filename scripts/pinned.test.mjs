import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const repository = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../.."
);
const hash = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
test("Make defaults to the immutable compiler and preserves explicit selections", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "pinned-elm-"));
    try {
        for (const folder of [
            "distribution",
            "releases/0.3.0-alpha.4",
            "target/pinned/0.3.0-alpha.4",
        ])
            fs.mkdirSync(path.join(root, "elm-rs", folder), {
                recursive: true,
            });
        fs.copyFileSync(
            path.join(repository, "Makefile"),
            path.join(root, "Makefile")
        );
        fs.copyFileSync(
            path.join(repository, "elm-rs/distribution/prepare-pinned.cjs"),
            path.join(root, "elm-rs/distribution/prepare-pinned.cjs")
        );
        const release = path.join(root, "elm-rs/releases/0.3.0-alpha.4");
        const installed = path.join(root, "elm-rs/target/pinned/0.3.0-alpha.4");
        const binary = path.join(installed, "planexpo-elm");
        fs.writeFileSync(path.join(release, "source.tar.gz"), "fixed source");
        fs.writeFileSync(
            path.join(release, "manifest.json"),
            JSON.stringify({
                version: "0.3.0-alpha.4",
                archive: "source.tar.gz",
                sourceSha256: hash("fixed source"),
            })
        );
        fs.writeFileSync(binary, "#!/bin/sh\nexit 0\n", { mode: 0o755 });
        fs.writeFileSync(
            path.join(installed, "receipt.json"),
            JSON.stringify({
                sourceSha256: hash("fixed source"),
                platform: `${process.platform}-${process.arch}`,
                binarySha256: hash(fs.readFileSync(binary)),
            })
        );
        fs.writeFileSync(
            path.join(root, "probe.cjs"),
            "console.log(JSON.stringify([process.env.PLANEXPO_ELM_COMPILER,process.env.PLANEXPO_ELM_RUST_BINARY]));"
        );
        fs.appendFileSync(
            path.join(root, "Makefile"),
            "\nprobe: prepareElmCompiler\n\t@node probe.cjs\n"
        );
        const env = { ...process.env };
        for (const key of [
            "PLANEXPO_ELM_COMPILER",
            "PLANEXPO_ELM_RUST_BINARY",
            "MAKEFLAGS",
            "MFLAGS",
            "MAKELEVEL",
        ])
            delete env[key];
        const run = (...args) =>
            spawnSync("make", ["--no-print-directory", "probe", ...args], {
                cwd: root,
                env,
                encoding: "utf8",
            });
        let result = run();
        assert.equal(result.status, 0, result.stderr);
        assert.deepEqual(JSON.parse(result.stdout), ["rust", binary]);
        fs.writeFileSync(
            path.join(root, "elm-rs/Cargo.toml"),
            "experimental source changed"
        );
        assert.equal(run().status, 0);
        const custom = path.join(root, "custom");
        fs.copyFileSync(binary, custom);
        assert.deepEqual(
            JSON.parse(run(`PLANEXPO_ELM_RUST_BINARY=${custom}`).stdout),
            ["rust", custom]
        );
        fs.appendFileSync(binary, "# modified");
        result = run();
        assert.notEqual(result.status, 0);
        assert.match(result.stderr, /Installation figée invalide/);
        assert.equal(run("PLANEXPO_ELM_COMPILER=elm").status, 0);
        fs.writeFileSync(
            path.join(release, "source.tar.gz"),
            "modified source"
        );
        assert.match(run().stderr, /checksum incorrect/);
    } finally {
        fs.rmSync(root, { recursive: true, force: true });
    }
});
