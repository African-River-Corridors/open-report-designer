# Changelog

All notable changes to open-report-designer. Entries arrive as fragments in `changelog.d/` and are moved here
at release time by `python3 tools/changelog.py release <version>`.

## Unreleased

## 0.1.0 - 2026-10-22

### Added

- **The document.** Rust types for the report document, `parse` with JSON-pointer findings (`ORD001`), the published JSON Schema (`schema/ord-document.schema.json`) and a fictional example screening report.
- **Register and lifecycle.** The register of document classes and the `standard` lifecycle as data, a tool's own register in its place, and the document-control checks `ORD002` to `ORD008`: class, status, sign-offs, revision form, number prefix, revision history and dates.
- **Resolution and prose.** The engine numbers sections, appendices, tables and figures once and resolves `[[#id]]` cross-references; prose is read into one model for all writers, with `$` always literal; structural checks `ORD009` to `ORD020` and image checks (`ORD011`).
- **The default template and bundled fonts.** `Template` with the values of spec 0001 §7 as one table (`template::DEFAULT`) and the document's overrides (colours checked as `#rrggbb`, `ORD001`; an unknown logo asset, `ORD011`); Public Sans, Libre Caslon Text and IBM Plex Mono bundled as static TrueType files with woff2 copies for HTML, under the SIL Open Font License 1.1.
- **HTML writer and `render`.** `render(doc, assets, register, format)` checks, refuses on any error and returns the bytes and the warnings. The HTML writer gives one self-contained, well-formed file with inline styles, fonts and images, and print styles for A4.
- **PDF writer.** The PDF writer through Typst (the `typst` and `typst-pdf` crates, compiled in memory with the bundled fonts and the document's assets only): cover on the template's paper, document-control page, contents page with page numbers, running head and foot with "Page n of N", PDF outline from the headings, tables with a repeated header row, SVG and PNG figures, data boxes, and title, author, language, creator and document-date metadata. Every piece of text enters the Typst source as a string literal, so `$`, `#` and the rest print as written. The same input gives the same bytes, and the engine still builds for `wasm32-unknown-unknown`.
- **The Word writer.** `render(..., Format::Docx)` writes a .docx package directly as Office Open XML: named styles in the template's fonts and colours, an A4 page with the template's margins, a cover on the cream paper, the document-control and revision-history tables, a table of contents field that Word fills on open, running heads and feet with "Page n of N" fields, tables with a repeated header row, data boxes, lists, code and links. PNG figures go in as they are; SVG figures go in with a PNG fallback drawn by the engine with the bundled fonts only. The bundled fonts are embedded, obfuscated as ECMA-376 requires. The zip has a fixed order and time, so two renders give equal bytes.
- **Conformance suite.** The example and a stress document (deep nesting, a long table across pages, every block type and special character) render to PDF, HTML and Word, and independent readers find the same headings, paragraphs, table cells, captions and document-control values, in the same order, in all three. One doctored text in any output fails the suite.
- **Assembly and the `ord` program.** `assemble` builds one document from a manifest and chapter files found by id (`ORD021`). The `ord` program renders, checks, assembles (rewriting asset paths for the new document), and prints the schema and the register.
- **Python bindings.** `ord_report` (PyO3, abi3, Python 3.10+): `render`, `render_file`, `check`, `assemble`, `schema`, `builtin_register` and `DocumentError`, which carries the findings. The gate builds the wheel and tests it against the native program.
- **WebAssembly package.** `ord-wasm` (wasm-bindgen, pinned) with `render`, `check`, `schema` and `version`, `tools/build_wasm.sh`, and a CI job that renders the example in all three formats in Node and compares each with the native output.
- **ADR 0001 and spec 0001.** The design of the report engine: one typed document, three writers (PDF through Typst, HTML, Word), document control, checks, and Python and WebAssembly bindings.
- **The ARC open library look.** The ORD mark and lockup at the top of the README, and a docs page in `website/`, served at https://www.africanrivercorridors.com/ord once GitHub Pages is on. The `website` workflow checks its links and deploys it. `TRADEMARKS.md` now lists the Drawing Format Standard (DFS).
