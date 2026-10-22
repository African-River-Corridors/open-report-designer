# Spec 0001: the report engine, v0.1

- Status: accepted
- Decision: [ADR 0001](../adr/0001-report-engine.md)
- Release: `v0.1.0`

## 1. Goal

One typed report document in; the same report out as PDF, HTML and Word. The engine runs as a Rust library,
a command-line program, a Python package and a WebAssembly package, with the same result in each.

## 2. Crates

| Crate | What |
|---|---|
| `ord` | The core: document types, schema, register and lifecycle, checks, resolution, assembly, the default template and fonts, and the three writers. Pure (no file, clock or network). Builds for `wasm32-unknown-unknown`. |
| `ord-cli` | The `ord` program: `render`, `check`, `schema`, `assemble`, `register`. Reads files, resolves asset paths, writes outputs. |
| `ord-py` | Python bindings (PyO3, abi3, Python 3.10+), import name `ord_report`. |
| `ord-wasm` | WebAssembly bindings (wasm-bindgen). |

## 3. The document

A document is one JSON object. The published schema is `schema/ord-document.schema.json` (JSON Schema
2020-12), generated from the Rust types. Unknown fields are an error at every level.

### 3.1 Top level

| Field | Type | Required | Meaning |
|---|---|---|---|
| `ord` | string | yes | Format version. v0.1 accepts `"0.1"` only. |
| `class` | string | yes | A document class id from the register (§4). |
| `title` | string | yes | |
| `subtitle` | string | no | |
| `organisation` | string | no | Shown on the cover and in the footer. |
| `language` | string | no | BCP 47 tag, default `en`. Written into each output. |
| `control` | object | yes | Document control (§3.2). |
| `summary` | Markdown | no | A short summary on the cover page. |
| `notice` | string | no | A notice on the cover and in the footer, such as "Screening tool. Not for construction." |
| `contents` | bool | no | Table of contents. Default from the class. |
| `body` | block[] | yes | The main matter (§3.3). |
| `appendices` | appendix[] | no | Each `{ id?, title, body: block[] }`. Lettered A, B, … (§5). |
| `assets` | map | no | Asset id → `{ media_type, base64 }` or `{ media_type, path }` (§3.4). |
| `template` | object | no | Overrides of the default template (§7). |

### 3.2 Document control

| Field | Type | Required | Meaning |
|---|---|---|---|
| `number` | string | no | The document number. If the class has a `number_prefix`, it must start with `<prefix>-`. |
| `revision` | string | yes | The revision. Its form depends on the status (§4.2). |
| `date` | string | yes | ISO 8601 date (`YYYY-MM-DD`). |
| `status` | string | yes | A status of the class's lifecycle (§4.2). Aliases resolve: case and spaces or underscores do not matter. |
| `sensitivity` | string | no | `public`, `internal`, `confidential` or `restricted`. Default `public`. |
| `prepared`, `checked`, `approved` | string | no | Who prepared, checked and approved it (a name or a role). Required by status (§4.2). |
| `history` | entry[] | no | Earlier and current revisions: `{ revision, date, status, description }`. |

### 3.3 Blocks

Each block is an object with a `type`:

| `type` | Fields | Notes |
|---|---|---|
| `section` | `id?`, `title`, `numbered?` (default true), `body: block[]` | Nesting sets the heading level. At most four levels. |
| `prose` | `markdown` | Markdown, §6. |
| `table` | `id?`, `caption`, `columns: [{ title, align?, unit? }]`, `rows: string[][]`, `notes?` | `align` is `left`, `centre` or `right`. Every row has one cell per column. Numbered "Table n". |
| `figure` | `id?`, `caption`, `asset`, `alt`, `width?` | `width` is a fraction of the text width, 0 < w ≤ 1, default 1. Numbered "Figure n". |
| `map` | `id?`, `caption`, `asset`, `alt`, `width?`, `crs?`, `scale?`, `attribution` | A figure that is a map. Numbered with the figures. Its key line gives the coordinate system, the scale and the source. |
| `data_box` | `id?`, `title?` (default "Data and limits"), `items: [{ label, value }]`, `notes?` (Markdown) | A boxed panel that states the data a result rests on and the limits of the result. |

### 3.4 Assets

An asset is an image: `image/svg+xml` or `image/png`. In the core an asset is bytes. In a JSON file it is
`base64` inline or a `path`; the command-line program and the Python `render_file` read a `path` relative to
the document file. The core refuses a `path` it was not given bytes for.

## 4. Register and lifecycle

### 4.1 Register

The register is a list of document classes. Each class has:

| Field | Meaning |
|---|---|
| `id` | Lower-case id, such as `report`. |
| `name` | Display name, such as "Report". Printed on the cover. |
| `number_prefix` | Optional. A document number must start with `<prefix>-`. |
| `lifecycle` | The id of a lifecycle (§4.2). |
| `contents` | Default for the table of contents. |
| `aliases` | Other names that resolve to this class. |

The built-in register (v0.1): `report` (prefix `RPT`, contents on), `technical-note` (`TN`, contents off),
`schedule` (`SCH`, contents off), `run-report` (`RUN`, contents off). A tool may pass its own register, which
replaces the built-in one. Class lookup trims and lower-cases the name, then tries ids, then aliases. An
unknown class is an error that lists the known classes.

### 4.2 Lifecycle

The built-in lifecycle, `standard`:

| Status | Stage | Revision form | Needs | Furniture |
|---|---|---|---|---|
| `draft` | working | letter | – | the status on every page |
| `for-checking` | working | letter | `prepared` | the status on every page |
| `for-approval` | working | letter | `prepared`, `checked` | the status on every page |
| `approved` | issued | number | `prepared`, `checked`, `approved` | none |
| `issued` | issued | number | `prepared`, `checked`, `approved` | none |
| `superseded` | issued | number | `prepared`, `checked`, `approved` | "Superseded" on every page |

- A **letter** revision is `A` to `Z`, then `AA` to `ZZ`. A **number** revision is a whole number written
  without leading zeros (`0`, `1`, `2`, …).
- Aliases: `for approval`, `for_approval` and `for-sign-off` → `for-approval`; `for review` → `for-checking`;
  `final` → `issued`.
- A status's sign-off needs are all required (every field, not any one).
- If `history` is present: revisions are unique; dates do not go backwards; and the last entry is the current
  revision, date and status.

A register may define other lifecycles in the same shape.

## 5. Resolution

Before any writer runs, the core resolves the document:

- Numbered sections are numbered `1`, `1.1`, `1.1.1`, … An unnumbered section takes no number and its children
  are unnumbered.
- Appendices are lettered `A` to `Z`; a 27th is an error. Sections inside appendix A are `A.1`, `A.1.1`, …
- Tables and figures (maps included) are numbered through the document, `1`, `2`, …; in an appendix `A.1`,
  `A.2`, …
- Ids are unique across the document. A cross-reference `[[#id]]` in prose becomes a link whose text is
  "Section 2.1", "Appendix A", "Table 3", "Figure A.2" or "Data and limits" (a data box's title). A
  reference to an unnumbered section uses its title.
- The table of contents lists the sections to three levels and the appendices.

## 6. Prose

Prose is CommonMark with these parts: paragraphs, emphasis, strong, inline code, links, bullet and numbered
lists (nested), block quotes, code blocks, hard breaks, and `[[#id]]` cross-references.

- **No maths.** A `$` is always a literal dollar sign. "$5 and $10", "US$ 3", `\$` and `$x$` print as written
  in every output. Each writer escapes `$` (Typst would otherwise start maths).
- Headings, tables, images and raw HTML in prose are refused by the checks (§8): structure belongs in typed
  blocks.
- Every other character that is special in a writer's own syntax (Typst markup, HTML, XML) is escaped.

## 7. The default template

The built-in template is the look of Open Waterway Designer.

| Item | Value |
|---|---|
| Page | A4 portrait, margins 25 mm top and bottom, 20 mm left and right |
| Cover | Cream paper (`#f2e8cf`), ink `#2b2925`: class name in spaced capitals, title, subtitle, organisation, summary, the notice, and a document-control block (number, revision, date, status) |
| Body pages | White paper; ink `#2b2925`; secondary ink `#4a463e` |
| Headings | Libre Caslon Text |
| Text, tables, numbers | Public Sans, 10 pt, tabular figures in tables |
| Code | IBM Plex Mono |
| Rules | `#d8caa6`; table header ground `#e8dcbd` |
| Data box | ground `#f8f2e3`, a 1.5 pt ink rule on the left |
| Header | document number and revision (left); status where the status has furniture (right) |
| Footer | organisation and notice (left); "Page n of N" (right) |
| Document control page | after the cover: the control table (number, revision, date, status, sensitivity, prepared, checked, approved) and the revision history |

Template overrides (`template`): `organisation_logo` (an asset id), the colours `paper`, `ink`, `ink_2`,
`rule` and `accent_ground`, and `cover` (bool, default true). The fonts are fixed in v0.1. The fonts are
bundled under `crates/ord/fonts/` with their OFL licences.

## 8. Checks

`check` returns findings: `{ code, severity, path, message }`. `path` is a JSON pointer into the document.
Any `error` refuses the render. Warnings are returned with the output.

| Code | Severity | Finding |
|---|---|---|
| `ORD001` | error | The document does not parse, or does not match the schema (unknown field, wrong type, unsupported `ord` version). |
| `ORD002` | error | Unknown document class. |
| `ORD003` | error | Unknown status for the class's lifecycle. |
| `ORD004` | error | A sign-off the status needs is missing. The message names the status and the fields. |
| `ORD005` | error | The revision's form does not fit the status (a letter where a number is needed, or the reverse). |
| `ORD006` | error | The document number does not start with the class's prefix. |
| `ORD007` | error | The revision history is not consistent (§4.2). |
| `ORD008` | error | A date is not an ISO 8601 date. |
| `ORD009` | error | Two blocks share an id. |
| `ORD010` | error | A cross-reference names an id that does not exist. |
| `ORD011` | error | A figure, map or logo names an asset that does not exist, or whose type is not supported, or whose bytes are not a valid image of that type. |
| `ORD012` | error | Prose holds a heading, a table, an image or raw HTML. |
| `ORD013` | error | A table row does not have one cell per column. |
| `ORD014` | error | A figure or map has empty alt text. |
| `ORD015` | error | Sections nest deeper than four levels, or there are more than 26 appendices. |
| `ORD016` | error | A section or appendix title starts with a typed number ("2.1 Scope", "A Appendix"). |
| `ORD017` | warning | Prose holds a typed section reference ("§3", "Section 3.2"): use `[[#id]]`. |
| `ORD018` | warning | A section is empty. |
| `ORD019` | warning | A table has no rows. |
| `ORD020` | error | A `width` is outside 0 < w ≤ 1. |
| `ORD021` | error | Assembly: a chapter id is missing or listed twice, or a chapter that is not agreed is assembled into a document whose status is `approved` or `issued`. |

## 9. Assembly

`assemble(manifest, chapters)` builds one document from many chapter files.

- The **manifest** is a document without `body` and `appendices`, plus `chapters: [id]` and optionally
  `appendix_chapters: [id]`.
- A **chapter** is `{ id, title, status, body: block[] }`, where `status` is `outline`, `draft`, `review` or
  `agreed`. Each chapter becomes one top-level section (or one appendix) with the chapter's id.
- Chapters are found by their `id`, never by file name. The command-line program reads every `*.json` chapter
  under a folder.
- The order is the manifest's order. A chapter listed twice, or listed and not found, is `ORD021`.
- If any chapter is not `agreed`, the assembled document may not be `approved` or `issued` (`ORD021`).
- The result is checked like any document.

## 10. Writers

- **PDF.** Typst markup from the resolved document, compiled in memory with the bundled fonts only. Headings
  give the PDF outline (bookmarks). The PDF's title, author (organisation), language and creator
  ("Open Report Designer <version>") are set. Hyphenation is off and text is ragged right, in all three writers.
- **HTML.** One file: `<!doctype html>`, `lang`, inline CSS with the template's colours and the fonts as woff2
  data URIs, images as data URIs (SVG inline as an `<img>` data URI), a table of contents of links, and print
  styles (A4, the same margins). Well-formed: it parses as XML.
- **Word.** A .docx with: styles for the title, headings 1 to 4, body text, captions, tables and code; A4 and
  the template's margins; a cover section on a cream page background shape; a document-control table; headers
  and footers with the number, revision, status and "Page n of N" fields; a table of contents field that Word
  fills on open; tables with a repeated header row; figures as PNG (SVG embedded with a PNG fallback); the
  bundled fonts embedded. Section, table and figure numbers are text from the resolved document.

All three writers are deterministic: the same input gives the same bytes.

## 11. Interfaces

### 11.1 Rust (`ord`)

```rust
pub fn parse(json: &str) -> Result<Document, Vec<Finding>>;
pub fn check(doc: &Document, assets: &Assets, register: &Register) -> Vec<Finding>;
pub fn render(doc: &Document, assets: &Assets, register: &Register, format: Format) -> Result<Output, Vec<Finding>>;
pub fn assemble(manifest: &Manifest, chapters: &[Chapter]) -> Result<Document, Vec<Finding>>;
pub fn schema() -> String;
pub fn builtin_register() -> Register;
```

`Output` holds the bytes and the warnings. `Format` is `Pdf`, `Html` or `Docx`.

### 11.2 Command line (`ord`)

```
ord render report.json --format pdf|html|docx|all [--register reg.json] [-o out/]
ord check report.json [--register reg.json] [--json]
ord assemble manifest.json --chapters dir/ -o report.json
ord schema
ord register
```

Exit codes: 0 done (warnings printed), 1 the document has errors, 2 usage or input-output error.

### 11.3 Python (`ord_report`)

`render(document: str | dict, format: str, assets: dict[str, bytes] | None = None, register: str | None = None) -> bytes`,
`render_file(path, format, out=None)`, `check(document, ...) -> list[dict]`, `assemble(manifest, chapters) -> dict`,
`schema() -> str`, `__version__`. A document with errors raises `ord_report.DocumentError`, which carries the
findings.

### 11.4 WebAssembly (`ord-wasm`)

`render(document_json: string, format: string, assets?: Map<string, Uint8Array>, register_json?: string) -> Uint8Array`,
`check(document_json, assets?, register_json?) -> string` (findings as JSON), `schema() -> string`,
`version() -> string`. Errors throw an `Error` whose message holds the findings as JSON.

## 12. Tests

- **Schema.** The committed schema equals the generated one. The example documents validate against it.
- **Register and lifecycle.** Each status's sign-off gate refuses a missing field; aliases resolve; letter and
  number revisions; history rules; the register is data (a tool's register replaces the built-in one).
- **Resolution.** Numbering of sections, appendices, tables and figures; cross-reference text; moving a block
  renumbers it and every reference to it.
- **Prose.** The `$` cases of §6 print as written in all three outputs. Special characters of each writer's
  syntax are escaped (a prose block full of them renders and reads back unchanged).
- **Conformance.** The example documents are rendered to PDF, HTML and Word, and each output is read back by an
  independent reader (a PDF text reader, an XML reader for HTML and for the .docx parts). The headings with
  their numbers, paragraph text, table captions and cells, figure captions, data-box items and the
  document-control values are equal, in order, in all three.
- **Determinism.** Two renders of one document give equal bytes, for each format.
- **Bindings.** Python and WebAssembly render the example in all three formats, and the output equals the
  native output.
- **Checks.** Each finding code has a test that makes it fire, and the clean example raises none.

## 13. Examples

`examples/` holds fictional documents only. `examples/screening-report.json` is a short screening report of
an invented river reach: a summary, three sections, a table, a figure, a map, a data box, two appendices and
full document control. The release attaches it rendered in all three formats.

## 14. Out of scope for v0.1

Custom fonts, landscape pages, footnotes, citations and bibliographies, equations, a template language,
editing a .docx back into a document, tagged-PDF accessibility beyond what Typst gives, and mutation testing
(a later hardening pass).
