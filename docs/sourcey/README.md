# Sourcey API reference

The project's guidebook (mdBook, in `docs/src/`) has no integrated API
reference — per-type detail currently lives only on docs.rs. This directory
adds one, generated from the crate source with
[Sourcey](https://sourcey.com) 3.9.5, and published by the `Deploy MdBook`
workflow under `/api/` on the project's own Pages site
(`stevepryde.github.io/thirtyfour/api/`).

## Contents

- `sourcey.config.ts` — Sourcey config: a Documentation tab (the two
  guides in this directory) and a Rust API tab generated from the
  `thirtyfour` and `thirtyfour-macros` crates with the rustdoc adapter.
- `index.md`, `quickstart.md` — the guides shown on the reference site.

## Building locally

```sh
rustup toolchain install nightly-2026-05-15
npx -y sourcey@3.9.5 build -c docs/sourcey/sourcey.config.ts -o /tmp/thirtyfour-api
```

Two pins matter:

- **Sourcey 3.9.5** with **nightly-2026-05-15**: Sourcey 3.9.5's bundled
  rustdoc helper reads rustdoc JSON format v57, which is what that
  nightly produces. A current nightly produces v61 and is rejected.
- **Run Sourcey from outside this repo** (for example via `npx`, as
  above): the rustdoc helper is itself a Cargo package, and if the
  Sourcey install sits inside the thirtyfour workspace, Cargo picks it
  up as a workspace member and the build fails.
