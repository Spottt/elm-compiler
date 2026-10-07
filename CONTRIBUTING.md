# Contributing

Thanks for helping! The most valuable contributions right now are
**compatibility reports**: real Elm code that official Elm (0.19.1 or 0.19.2) handles
differently from this compiler.

## Reporting a bug

Open an issue with:

1. The command you ran and its full output (`--report=json` output helps).
2. A minimal `elm.json` and module(s) that reproduce the problem.
3. What official `elm` does with the same input, and which version (0.19.1 or 0.19.2).
4. `elm --compiler-help` (first line shows the version), OS and architecture.

For runtime differences, include the JavaScript behaviour you observe and the
expected one.

## Development

Requirements: Rust 1.98.1 (selected automatically through `rust-toolchain.toml`)
and Node ≥ 18.

```sh
cargo build --locked --release
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
node --test distribution/package.test.mjs
```

`scripts/` contains differential checks that compare this compiler with the
official `elm` binary (diagnostics, runtime values, REPL, docs, package
commands...). They are Python 3 / Node scripts; most take the official binary
with `--elm /path/to/elm`. Run the ones related to your change.

Their fixtures are written for Elm 0.19.1. To compare with another release,
run them through the suite runner, which wraps both compilers so the same
fixtures are compiled as applications of that release (Linux and macOS):

```sh
cargo build --release
python3 scripts/check_compatibility.py --elm /path/to/elm-0.19.3 --release 0.19.3 \
    --output /tmp/elm-0.19.3 --suite module_errors --suite names_parity
```

Without `--suite` every registered suite runs. Suites that edit or inspect the
0.19.1 package cache themselves (`install`, `diff`, `publish`, downloads) or
drive a terminal cannot be judged this way yet; see
`scripts/elm_release_adapter.py`. Packages follow Elm 0.19.3, so the suites
built on package fixtures are expected to differ from the 0.19.1 binary.

CI runs exactly these commands on Linux and macOS (x64 and ARM64) and Windows
x64 for every pull request. Some tests download Elm packages from `package.elm-lang.org`
and need network access.

Guidelines:

- Add a test for every fix. Rust integration tests live in `tests/`; name the
  file after the compiler phase (`parser.rs`, `infer.rs`, `codegen.rs`, …).
- **Never modify files under `tests/upstream/`**: they are verbatim copies of
  upstream fixtures, verified by SHA-256 (`provenance.json`).
- When behaviour intentionally follows official Elm, reference the relevant
  file of the Haskell compiler (tag `0.19.1`) in a comment.
- Keep `Cargo.lock` committed and builds `--locked`.

## Pull requests

- One logical change per PR, with its tests.
- The CI must be green on the five platforms.
- By submitting a contribution you agree to license it under the BSD 3-Clause
  License of this project.
