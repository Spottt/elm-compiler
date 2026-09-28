# Elm compiler in Rust

An independent, **unofficial** reimplementation of the [Elm](https://elm-lang.org)
0.19.1 compiler, written in Rust. It parses, type-checks and generates
JavaScript by itself — it never calls the official Haskell compiler.

It was built to compile a large production Elm codebase (ten applications,
several hundred modules) faster and with far less memory:

| Cold build of the largest application | Time  | Peak memory |
| ------------------------------------- | ----- | ----------- |
| Official Elm 0.19.1 (8 threads)       | 160 s | 7.7 GiB     |
| This compiler                         | 19 s  | 336 MiB     |

_Single measurement on one codebase, excluding bundling/minification. Your
numbers will differ; please share them in an issue._

> **Status: experimental.** It compiles real applications that pass their test
> suites, but it is not a drop-in replacement for every Elm project yet. Read
> [Compatibility](#compatibility) before relying on it.

This project is not affiliated with or endorsed by the Elm project.

---

## Install

Prebuilt binaries are shipped inside the npm package for:

| OS                   | x64 | ARM64 |
| -------------------- | --- | ----- |
| Linux (glibc)        | ✅  | ✅    |
| macOS                | ✅  | ✅    |
| Windows, Alpine/musl | ❌ — [build from source](#build-from-source) | ❌ |

```sh
# In an Elm project
npm install --save-dev @spottt/elm-compiler
npx planexpo-elm make src/Main.elm --output=main.js

# Or without installing
npx @spottt/elm-compiler make src/Main.elm --output=main.js
```

The package has **no install scripts**: nothing is downloaded or compiled at
`npm install` time. The binary for your platform is already inside the tarball
and its SHA-256 is recorded in `checksums.json`. Releases are published from
CI with [npm provenance](https://docs.npmjs.com/generating-provenance-statements).

Pre-releases use the `next` tag: `npm install --save-dev @spottt/elm-compiler@next`.

Standalone binaries (`planexpo-elm-<version>-<platform>.tar.gz`) and a
`SHA256SUMS` file are attached to every
[GitHub Release](https://github.com/Spottt/elm-compiler/releases).

## Usage

The command line mirrors `elm`:

```sh
planexpo-elm init                                   # create elm.json and src/
planexpo-elm install elm/http                       # add a dependency
planexpo-elm make src/Main.elm --output=main.js
planexpo-elm make src/Main.elm --output=index.html
planexpo-elm make src/Main.elm --optimize --output=main.js
planexpo-elm make src/A.elm src/B.elm --output=bundle.js   # several entry points
planexpo-elm make src/Main.elm --output=/dev/null          # type-check only
planexpo-elm make --docs=docs.json                  # in a package: check + docs
planexpo-elm repl
```

Additional `make` flags:

| Flag            | Effect |
| --------------- | ------ |
| `--report=json` | Machine-readable errors, like `elm make --report=json`. |
| `--incremental` | Reuse cached per-module results between runs (large win in watch mode). |
| `--no-cache`    | Neither read nor write the generated-code and type caches. |

Environment variables:

| Variable                   | Effect |
| -------------------------- | ------ |
| `ELM_HOME`                 | Package cache location, as with official Elm (default `~/.elm`). |
| `PLANEXPO_ELM_RUST_BINARY` | Absolute path of a binary the npm launcher runs instead of the bundled one (e.g. your own build). |

Missing packages are downloaded from `package.elm-lang.org` into `ELM_HOME`
and their archive hashes are verified. Builds work offline once the cache is
populated.

### With existing tooling

Most tools accept a path to the compiler; point them at
`node_modules/.bin/planexpo-elm`:

```sh
npx elm-test --compiler ./node_modules/.bin/planexpo-elm
```

```js
// webpack + elm-webpack-loader
{ loader: 'elm-webpack-loader', options: { pathToElm: 'node_modules/.bin/planexpo-elm' } }
```

From JavaScript, `require('@spottt/elm-compiler').resolveBinary()` returns the
absolute path of the native executable.

## Compatibility

Target language: **Elm 0.19.1** (reference: elm/compiler tag `0.19.1`, commit
`c9aefb6`).

Supported:

- Applications and packages, dependency resolution with backtracking, package
  download and hash verification.
- The whole pipeline: parsing, operators, name resolution, type inference,
  pattern exhaustiveness, JavaScript generation, runtime linking, `--optimize`.
- Ports, effect managers, core kernel code, WebGL shaders.
- HTML output, several entry points in one bundle, `--report=json`.
- `init`, `install`, `repl`, `make --docs`.

Known differences:

- **No debugger**: `--debug` is not supported.
- **Error messages are less detailed** than official Elm's. Errors carry their
  location, but wording and hints often differ.
- GLSL shaders are tested differentially against official Elm, but full grammar
  parity is not yet claimed.
- `make --docs` output and exhaustive `elm.json` validation are incomplete.
- `bump`, `diff` and `publish` are not implemented.

Compatibility is checked with differential tests against the official binary
and with the historical Elm test programs kept in `tests/upstream/`.

**If a program compiles with official Elm but not with this compiler — or
behaves differently at runtime — that is a bug.** Please
[open an issue](https://github.com/Spottt/elm-compiler/issues) with a minimal
`elm.json` and module.

## Build from source

Requirements: Rust **1.98.1** (pinned in `rust-toolchain.toml`, installed
automatically by rustup), Node ≥ 18 for the npm tooling.

```sh
git clone https://github.com/Spottt/elm-compiler.git
cd elm-compiler
cargo build --locked --release
./target/release/planexpo-elm make path/to/src/Main.elm --output=main.js
```

To try your build through npm exactly as users receive it:

```sh
node distribution/pack-local.mjs --out dist
# → dist/spottt-elm-compiler-<version>.tgz, current platform only
cd /path/to/an/elm/project
npm install --save-dev /path/to/elm-compiler/dist/spottt-elm-compiler-<version>.tgz
npx planexpo-elm --version
```

Or keep the published package and swap only the binary:

```sh
PLANEXPO_ELM_RUST_BINARY=/path/to/elm-compiler/target/release/planexpo-elm npx planexpo-elm make src/Main.elm
```

## Repository layout

| Path            | Content |
| --------------- | ------- |
| `src/`          | The compiler (library `planexpo_elm` + binary `planexpo-elm`). |
| `tests/`        | Rust integration tests. `tests/upstream/` holds unmodified upstream fixtures under their own licenses. |
| `examples/`     | Small probes used by differential tests (REPL, registry, docs). |
| `npm/`          | Source of the npm package: launcher, platform resolution, checksum check. It contains no binaries. |
| `distribution/` | Release assembly (`stage-release.mjs`), local packing (`pack-local.mjs`) and their tests. |
| `.github/workflows/release.yml` | CI on every push/PR; npm + GitHub release on version tags. |

## Contributing and releasing

- [CONTRIBUTING.md](CONTRIBUTING.md) — development workflow, tests, reporting compatibility bugs.
- [RELEASING.md](RELEASING.md) — publishing a version, and publishing a fork under your own npm scope.
- [CHANGELOG.md](CHANGELOG.md)

## License

[BSD 3-Clause](LICENSE), Copyright (c) 2026 Spottt.

This compiler is derived from the Elm compiler and includes code or fixtures
from GHC's base library and language-glsl; their licenses are in
`LICENSE-ELM`, `LICENSE-GHC` and `LICENSE-GLSL`. See [NOTICE](NOTICE).
