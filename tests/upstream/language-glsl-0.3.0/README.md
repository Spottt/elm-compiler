# language-glsl 0.3.0

Immutable source, tests, sample and license from the exact Hackage release used
by Elm 0.19.1. `provenance.json` pins the URL and SHA-256 of each archived file.
The Rust port is in `src/shader/parser.rs`; distribution includes `LICENSE-GLSL`.

`cases.json` contains 217 active string fixtures and the complete sample shader.
Regenerate with `python3 scripts/generate_glsl_cases.py`, or verify with `--check`.
No original tests are removed or rewritten. Commented-out upstream tests remain
in their original source but are not described as active tests.

Expressions are embedded in a parenthesized initializer. Invalid declarations
are embedded in a function body because some upstream negative cases are valid
function prototypes at top level. Other cases remain top-level shaders.
`tests/shader.rs` checks the upstream acceptance expectations. The differential
runner also compares these 218 shader programs to the actual Elm binary.

These checks cover acceptance and Elm's shader interface extraction, not equality
of the original GLSL AST or pretty-printer. The Haskell HUnit runner and its
AST pretty-print round trips are not executed by this Rust suite.
