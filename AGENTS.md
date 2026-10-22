# Agent instructions

## Agent skills

### Issue tracker

GitHub Issues on this repo, via the `gh` CLI. The repo is public: no confidential names or data. See
`docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels, each named as its role. See `docs/agents/triage-labels.md`.

### Build process

How ticketed work is built and merged. See `docs/agents/build-process.md`.

## Rules for the engine

- **Domain-free.** No waterway, vessel or other tool vocabulary in the engine. A tool supplies its own
  document classes and words; the engine supplies structure, checks and writers.
- **Pure core.** The `ord` crate opens no file, reads no clock and uses no network. It takes a document and its
  assets as values and returns bytes. The CLI and the bindings do the input and output. This is what lets the
  same engine run in a desktop app, in Python and in the browser (wasm).
- **One document, three writers.** PDF (through Typst), HTML and Word render the same document. A change to
  one writer that changes structure or text needs the same change in the other two; the conformance suite
  (`crates/ord/tests/conformance.rs`) fails otherwise.
- **The document format is public.** A change to the document types changes `schema/ord-document.schema.json`
  (generated; a test fails if it is stale) and needs a changelog fragment.
- **No private content.** No house style, client, partner, person or project name of any organisation in code,
  tests, examples or templates. Examples use invented, plainly fictional content.
- Run `python3 tools/release_guard.py` and `--history` before every push.
