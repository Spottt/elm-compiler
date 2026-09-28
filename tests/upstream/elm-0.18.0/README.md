# Historical Elm compiler tests

Source: https://github.com/elm/compiler/tree/0.18.0/tests
Commit: `eb97f2a5dd5421c708a91b71442e69d02453cc80`.
BSD 3-Clause license: [LICENSE](LICENSE).

The 67 files listed in `provenance.json` are copied without modification:
33 Elm programs, 28 JavaScript snapshots, the Haskell runners/properties, an
arguments file and the license. The manifest records their SHA-256.

The suite was removed before 0.19 by https://github.com/elm/compiler/commit/e2008b5,
whose message says it had to be redone for 0.19. This does not imply that a
replacement suite or exhaustive coverage exists.

From the repository root, with an official Elm 0.19.1 binary and a release build:

```sh
cargo build --locked --release
python3 scripts/check_upstream_parity.py \
  --elm "$(command -v elm)" \
  --report /tmp/elm-upstream-results.json
```

The runner verifies the hashes, then only adds a module header, in a temporary
project with temporary copies of the Elm caches. It compares acceptance by Elm
**0.19.1** and by the Rust `check` command. Elm 0.19.1 is the reference: the
0.18 expectations are not applied blindly, since ten former "good" cases are
now rejected (obsolete syntax, shadowing, old `Debug.crash`, etc.).

With `--diagnostics`, it also compares the complete JSON objects reported by
`make` and fails on any difference.

Result (24 September 2026): **33/33 identical decisions and 33/33 identical
complete JSON diagnostics** compared with Elm 0.19.1. This concerns these
archived fixtures, not universal parity. The runner keeps every case and fails
on any new difference; sources are not rewritten and errors are not ignored.

The 0.18 JavaScript snapshots are kept as a historical reference, **not executed
nor claimed as validated**: 0.19 output differs from 0.18. The four groups of
Haskell properties (types, patterns, literals, long regression pattern) are
archived unchanged and adapted in `tests/upstream_properties.rs`, run by
`cargo test`. Their generators are deterministic and adapted to 0.19 syntax;
this is not an execution of the historical QuickCheck engine.
