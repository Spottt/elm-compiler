# Elm reactor interface

Vendored from elm/compiler tag 0.19.1, reactor directory (BSD-3-Clause;
see LICENSE). The Elm sources and original styles/fonts are preserved.
assets/elm.js is compiled by planexpo-elm, without a Haskell compiler at runtime.

Regenerate from this directory with a built Rust compiler:

    ../target/release/planexpo-elm make src/NotFound.elm src/Errors.elm src/Index.elm --optimize --output=assets/elm.js

The server embeds the assets, so installed binaries do not require this folder.

The compiled UI includes the Elm dependencies pinned in elm.json. Their
license files are retained in licenses/.
