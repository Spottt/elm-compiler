# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org).

## [Unreleased]

First public source release, from the 0.3.0-alpha.4 compiler used in production
at Planexpo.

### Added

- Elm 0.19.1 compiler written in Rust: parsing, name resolution, type
  inference, exhaustiveness checking, JavaScript generation, `--optimize`,
  `--debug`, diagnostics compared with the official compiler.
- Commands `init`, `install`, `make` (JavaScript and HTML output, several entry
  points, `--report=json`, `--docs`), `repl`, `reactor`, `diff`, `bump`,
  `publish`.
- Package resolution with backtracking, download and hash verification.
- Incremental compilation cache (`--incremental`).
- npm package `@spottt/elm-compiler` with prebuilt binaries for Linux and
  macOS (x64, ARM64) and Windows (x64).

### Deliberate differences from Elm 0.19.1

- Rejects an unsound recursive-capture program accepted by official Elm.
- Correct JavaScript for the negation of overflowing integer literals.
- Stricter TLS certificate checks when downloading packages.
