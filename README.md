<picture>
  <source media="(prefers-color-scheme: dark)" srcset="website/assets/ord-lockup-dark.svg">
  <img alt="ORD, Open Report Designer: an ARC open library" src="website/assets/ord-lockup.svg" width="440">
</picture>

**One typed report document to PDF, HTML and Word.**
[Docs page](https://www.africanrivercorridors.com/ord) · [Design (ADR 0001)](docs/adr/0001-report-engine.md)

A report engine for open engineering tools. You give it one typed report document (JSON, with a published
schema); it writes the same report as **PDF**, **HTML** and **Word**.

- **Rust core.** Pure: it opens no file and uses no network, so it runs in a command-line tool, a desktop
  app, Python, and the browser (WebAssembly).
- **One document, three writers.** PDF is typeset with [Typst](https://typst.app). A conformance suite checks
  that the PDF, HTML and Word outputs of one document have the same structure and text.
- **Document control built in.** Revision, date, prepared, checked, approved and status, with checks that a
  document cannot claim a status its sign-offs do not support.
- **A default look.** The built-in template uses the look of Open Waterway Designer: cream paper, one ink,
  and open fonts (SIL OFL 1.1) bundled in the engine.

The design is in [ADR 0001](docs/adr/0001-report-engine.md) and [spec 0001](docs/specs/0001-report-engine.md).
Each [release](https://github.com/African-River-Corridors/open-report-designer/releases) attaches the
example report, [`examples/screening-report.json`](examples/screening-report.json), rendered in all three
formats, so you can see the output before you install anything.

## The document

A document is one JSON file ([schema](schema/ord-document.schema.json)): document control, sections, Markdown
prose, tables, figures (SVG or PNG), maps, "Data and limits" boxes and appendices. The engine numbers
sections, tables and figures, resolves `[[#id]]` cross-references, and refuses a document with an error
(each finding has a code, such as `ORD004`, and a JSON pointer to the problem).

```json
{
  "ord": "0.1",
  "class": "report",
  "title": "River screening",
  "control": { "number": "RPT-0001", "revision": "A", "date": "2026-10-22", "status": "draft" },
  "body": [
    { "type": "section", "title": "Introduction", "body": [
      { "type": "prose", "markdown": "Three options were screened. A rate of $5 to $10 is used." }
    ] }
  ]
}
```

## Use it

**Command line.** Build with `cargo install --git https://github.com/African-River-Corridors/open-report-designer --tag v0.1.0 ord-cli`.

```
ord render report.json --format all -o out/     # report.pdf, report.html, report.docx
ord check report.json                            # findings, exit 1 on any error
ord assemble manifest.json --chapters chapters/ -o report.json
ord schema                                       # the JSON Schema
ord register                                     # the document classes and their lifecycle
```

**Rust.** Add `ord = { git = "https://github.com/African-River-Corridors/open-report-designer", tag = "v0.1.0" }`.

```rust
let doc = ord::parse(&json)?;
let (assets, _) = ord::assets::collect(&doc, &supplied_bytes);
let pdf = ord::render(&doc, &assets, &ord::builtin_register(), ord::Format::Pdf)?.bytes;
```

**Python** (3.10 or later). Build the wheel with `maturin build -m crates/ord-py/Cargo.toml`, then:

```python
import ord_report
pdf = ord_report.render_file("report.json", "pdf")
findings = ord_report.check(document_dict)
```

**WebAssembly.** `bash tools/build_wasm.sh web` builds the package for a browser:

```js
import init, { render } from "./ord.js";
await init();
const pdf = render(documentJson, "pdf", new Map([["plan", pngBytes]]));
```

## Develop

`bash tools/gates.sh` runs every check: format, clippy, the wasm32 build, the Rust tests (cargo-nextest) and
the Python tests. See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).

## Licence

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). The project names and logos are covered by
[TRADEMARKS.md](TRADEMARKS.md). The fonts under `website/fonts/` (SIL OFL 1.1) have their licences beside them.

Hosted by African River Corridors.
