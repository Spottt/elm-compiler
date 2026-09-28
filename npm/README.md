# @spottt/elm-compiler

A fast, **unofficial** Elm 0.19.1 compiler written in Rust, with prebuilt
binaries for Linux (glibc) and macOS, x64 and ARM64.

```sh
npm install --save-dev @spottt/elm-compiler
npx planexpo-elm make src/Main.elm --output=main.js
```

- The command line mirrors `elm`: `init`, `install`, `make` (`--output`,
  `--optimize`, `--report=json`, `--docs`), `repl`.
- Extra `make` flags: `--incremental` (reuse per-module cache), `--no-cache`.
- No install scripts: nothing is downloaded or compiled at install time; the
  binaries' SHA-256 are listed in `checksums.json`.
- `PLANEXPO_ELM_RUST_BINARY=/abs/path` makes the launcher run another binary
  (e.g. your own build).
- `require('@spottt/elm-compiler').resolveBinary()` returns the native
  executable path, for tools such as `elm-test --compiler` or
  `elm-webpack-loader`'s `pathToElm`.

Not supported yet: the debugger (`--debug`), `bump`/`diff`/`publish`, Windows
and Alpine/musl binaries. Error messages are less detailed than official Elm's.

Documentation, compatibility notes and bug reports:
https://github.com/Spottt/elm-compiler

Not affiliated with the Elm project. BSD 3-Clause, Copyright (c) 2026 Spottt;
upstream notices in `NOTICE` and `LICENSE-*`.
