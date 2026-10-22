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

Status: under construction. The design is in [ADR 0001](docs/adr/0001-report-engine.md) and
[spec 0001](docs/specs/0001-report-engine.md).

## Licence

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). The project names and logos are covered by
[TRADEMARKS.md](TRADEMARKS.md). The fonts under `website/fonts/` (SIL OFL 1.1) have their licences beside them.

Hosted by African River Corridors.
