# Contributing to open-report-designer

Thank you for helping. Bug reports, fixes and new writers or checks are all welcome.

## Set up and test

You need the Rust toolchain (`rustup` reads `rust-toolchain.toml`), `cargo-nextest`, and Python 3.12 or
later for the tools and the bindings.

```
git clone https://github.com/African-River-Corridors/open-report-designer
cd open-report-designer
bash tools/gates.sh
python3 tools/release_guard.py
```

Both must pass. CI runs them on every pull request.

## Rules for the engine

- The engine stays domain-free and pure (see `AGENTS.md`).
- Every change comes with tests. A change to what a writer outputs comes with a conformance case: the same
  document in PDF, HTML and Word has the same structure and text.
- A change to the document format updates the published schema and the specs in `docs/specs/`.

## Pull requests

1. Open an issue first for anything bigger than a small fix.
2. Fork, branch from `main`, keep the change focused.
3. Add a changelog fragment in `changelog.d/` (see its README). Do not edit `CHANGELOG.md`.
4. Make sure the tests and the release guard pass.
5. Say what changed and how you checked it.

Branch protection on `main` requires `test` and `release-guard` to pass. Pull requests are squash-merged.

## Licence of contributions

This project is licensed under Apache-2.0. Unless you say otherwise, any contribution you
intentionally submit is licensed under Apache-2.0, as set out in section 5 of the licence.

## Conduct

Please follow the [code of conduct](CODE_OF_CONDUCT.md).
