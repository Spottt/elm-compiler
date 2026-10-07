# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org).

## [Unreleased]

### Added

- `elm --make-worker`: a persistent compiler process driven by JSON lines on
  stdin/stdout, for bundler plugins and dev servers. Documented in
  `docs/worker.md`. The former `--internal-make-worker` name still works.

### Changed

- Applications whose `elm.json` declares `"elm-version": "0.19.2"` or
  `"0.19.3"` now follow the observable behaviour of that official binary
  where it differs from 0.19.1: Unicode 15.1 identifiers, decoded character
  literals, long hexadecimal literals, the `UNEXPECTED ENCODING` report and
  corrected messages since 0.19.2; the name order of 0.19.3 in generated
  record fields and in error messages. Applications declaring `0.19.1` are
  unchanged. See the Compatibility section of the README.
- Packages, which only declare a range of Elm versions, now follow the
  observable behaviour of Elm 0.19.3 instead of 0.19.1. This changes the
  order of names in their error messages and generated record fields, and
  version numbers in their `elm.json` no longer wrap around at 65536.

### Fixed

- Applications whose `elm.json` declares `"elm-version": "0.19.3"` are
  accepted, like those declaring `0.19.1` or `0.19.2`.

## [0.3.0-alpha.7] - 2026-09-28

### Fixed

- Applications whose `elm.json` declares `"elm-version": "0.19.2"` are
  accepted, like those declaring `0.19.1`. Other versions are still rejected
  with the official error message.

### Changed

- Releases are published under the npm `latest` tag by default, pre-releases
  included; only `<version>-next.N` builds go to `next`.

## [0.3.0-alpha.6] - 2026-09-28

### Changed

- The npm package now provides the `elm` command, a drop-in replacement for the
  official compiler (`planexpo-elm` stays as an alias). Install it instead of
  the official `elm` package, not alongside it.
- Standalone release archives are named `elm-compiler-<version>-<platform>.tar.gz`
  and contain a single `elm` executable.

## [0.3.0-alpha.5] - 2026-09-28

First public release, published under the `next` npm tag. Same compiler as
0.3.0-alpha.4, used daily in development at Planexpo, with public packaging.

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
