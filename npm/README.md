# @spottt/elm-compiler

A fast, **unofficial** Elm 0.19.1 compiler written in Rust, with prebuilt
binaries for Linux (glibc) and macOS (x64, ARM64) and Windows (x64).

```sh
npm install --save-dev @spottt/elm-compiler
npx planexpo-elm make src/Main.elm --output=main.js
```

On a 677-module application, a cold development build takes 12 s and 256 MiB
instead of 171 s and 8.7 GiB with official Elm (single machine, median of 3).

- The command line mirrors `elm`: `init`, `install`, `make` (`--output`,
  `--optimize`, `--debug`, `--report=json`, `--docs`), `repl`, `reactor`,
  `diff`, `bump`, `publish`.
- Extra `make` flags: `--incremental` (reuse per-module cache), `--no-cache`.
- No install scripts: nothing is downloaded or compiled at install time; the
  binaries' SHA-256 are listed in `checksums.json`.
- `PLANEXPO_ELM_RUST_BINARY=/abs/path` makes the launcher run another binary
  (e.g. your own build).
- `require('@spottt/elm-compiler').resolveBinary()` returns the native
  executable path, for tools such as `elm-test --compiler` or
  `elm-webpack-loader`'s `pathToElm`.

Status: alpha, aiming at full Elm 0.19.1 compatibility with a few deliberate
fixes of known Elm defects. Documentation, compatibility notes and bug reports:
https://github.com/Spottt/elm-compiler

Not affiliated with the Elm project. BSD 3-Clause, Copyright (c) 2026 Spottt;
third-party notices in `NOTICE` and `LICENSE-*`.
