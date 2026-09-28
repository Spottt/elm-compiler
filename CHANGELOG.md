# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org).

## [Unreleased]

First public source release.

### Added

- Elm 0.19.1 compiler written in Rust: parsing, name resolution, type
  inference, exhaustiveness checking, JavaScript generation, `--optimize`.
- `init`, `install`, `make` (JavaScript and HTML output, several entry points,
  `--report=json`, `--docs`), `repl`.
- Package resolution with backtracking, download and hash verification.
- Incremental compilation cache (`--incremental`).
- npm package `@spottt/elm-compiler` with prebuilt binaries for Linux and
  macOS (x64, ARM64).

### Known limitations

- No debugger (`--debug`), less detailed error messages than official Elm.
- `bump`, `diff`, `publish` are not implemented.
