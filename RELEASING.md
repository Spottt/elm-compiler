# Releasing

Everything is automated by `.github/workflows/release.yml`. A maintainer only
bumps the version and pushes a tag.

## How it works

| Event                    | What CI does |
| ------------------------ | ------------ |
| Push / pull request      | Builds and tests the five binaries (`linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`, `win32-x64`), assembles the npm tarball, installs it in an empty project and compiles an Elm program with it. The tarball and binaries are kept as workflow artifacts. |
| Push of tag `v<version>` | Same, then publishes **that exact tarball** to npm with provenance and creates a GitHub Release with the tarball, per-platform binaries and `SHA256SUMS`. |

`distribution/stage-release.mjs` refuses to assemble a release when:

- `Cargo.toml` and `npm/package.json` versions differ;
- the tag does not match the version;
- a binary is missing or is not a valid executable for its platform/architecture.

The npm package runs `verify.cjs` before publishing, which checks every binary
against `checksums.json`. The source directory `npm/` has no binaries and can
never be published directly.

## One-time setup

1. **npm**: the `@spottt` organisation must exist on npmjs.com. Create an
   automation (or granular) access token allowed to publish `@spottt/elm-compiler`.
2. **GitHub → Settings → Environments**: create an environment named
   `npm-publish` restricted to `v*` tags, add the secret `NPM_TOKEN` to it and,
   if your plan allows it, required reviewers so every publication needs an
   explicit approval.
3. Optionally protect `v*` tags so only maintainers can create them.

## Publishing a version

1. Choose the version, following [semver](https://semver.org). Use a
   pre-release suffix (`0.4.0-beta.1`) to publish under the `next` dist-tag
   instead of `latest`.
2. Set it in **both** `Cargo.toml` and `npm/package.json`, then run
   `cargo build` so `Cargo.lock` is updated.
3. Update `CHANGELOG.md`, open a pull request, wait for green CI, merge.
4. Tag the merge commit and push the tag:

   ```sh
   git tag v0.4.0
   git push origin v0.4.0
   ```

5. Approve the `npm-publish` deployment in the Actions tab (if reviewers are required).
6. Check the result:

   ```sh
   npm view @spottt/elm-compiler dist-tags
   npx @spottt/elm-compiler@0.4.0 --version
   ```

A published npm version can never be reused. If something is wrong, deprecate
it (`npm deprecate @spottt/elm-compiler@0.4.0 "reason"`) and release a new one.

## Testing a release without publishing

- Download the `npm-package` artifact of any CI run and
  `npm install ./spottt-elm-compiler-<version>.tgz` in a test project.
- Or build locally for your own platform: `node distribution/pack-local.mjs --out dist`.

## Publishing a fork

Forks can publish their own build under their own scope:

1. Change `name`, `repository`, `homepage` and `bugs` in `npm/package.json`
   (e.g. `@your-scope/elm-compiler`).
2. Do the one-time setup above in your fork (npm token, `npm-publish` environment).
3. Push a `v<version>` tag.

Keep `LICENSE`, `NOTICE` and the `LICENSE-*` files: the BSD licenses require
them in every redistribution, source or binary.
