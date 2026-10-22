# ADR 0001: A pure Rust report engine with one typed document and three writers

- Status: accepted
- Date: 2026-10-22
- Spec: [spec 0001](../specs/0001-report-engine.md)

## Context

Open engineering tools need reports: a screening report with an appendix that traces each rule, a quantity
schedule, a pack of drawings and sections, and a run report for testers and developers. The readers are
decision-makers, engineers, testers and developers. The tools run as a command-line program, as a desktop app,
and as a web app in the browser (WebAssembly). Some are also used from Python.

A report engine for these tools must therefore:

- run in all four places, with no system fonts, no external programs and no network;
- write PDF for print, HTML for the screen, and Word for people who edit and comment, from one source;
- keep document control (revision, date, prepared, checked, approved, status) honest;
- be open source, so it can hold no organisation's private house style, names or examples.

A document tool built on Python, pandoc and an HTML-to-PDF converter cannot meet the first point: it cannot be
shipped inside a desktop app button or a web page. Its *ideas* still hold, and this engine keeps them: a
register of document classes, a header contract, a document-control lifecycle, the assembly of many chapters
into one document, house checks that refuse a bad document, and a guard so that a `$` in prose is never read
as maths.

## Decision

1. **A pure Rust core.** The `ord` crate takes a document and its assets as values and returns bytes and
   findings. It opens no file, reads no clock and uses no network. A command-line program (`ord-cli`), Python
   bindings (`ord-py`) and a WebAssembly package (`ord-wasm`) do the input and output around it.

2. **One typed document, as JSON with a published schema.** The input is not free Markdown. It is a typed tree:
   document control, sections, prose blocks, tables, figures, maps, "Data and limits" boxes and appendices. Only
   prose is Markdown, inside a prose block. The schema (`schema/ord-document.schema.json`) is generated from the
   Rust types, and a test fails if the committed copy is stale. The document names its format version
   (`"ord": "0.1"`).

3. **The engine numbers everything; the writers print what they are given.** Section numbers, appendix letters,
   table and figure numbers, and cross-reference text are worked out once, in the core, into a *resolved
   document*. Each writer prints those numbers as text. No writer counts for itself (not Typst's counters, not
   Word's SEQ fields, not CSS counters), so the three outputs cannot disagree on a number.

4. **Three writers, all in v0.1.**
   - **PDF through Typst**, used as a library (the `typst` and `typst-pdf` crates). The core writes Typst
     markup from the resolved document, with every piece of text escaped, and compiles it in an in-memory
     world that holds only the bundled fonts and the document's assets.
   - **HTML**: one self-contained file, with its styles, fonts and images inline, and print styles.
   - **Word (.docx)**: written directly as Office Open XML by the core, with the same page size, margins,
     fonts, colours, cover, headers and footers as the PDF, and the fonts embedded in the file. A crate that
     writes .docx was considered and not taken: the engine needs fine control of the layout, embedded fonts and
     SVG with a PNG fallback, and the format parts it needs are small.

5. **Rules are data.** The register of document classes and each class's lifecycle (its statuses, and which
   sign-offs each status needs) are data. A built-in register serves most reports; a tool can pass its own.
   The checks run before any writer, and a document with an error is refused, not written.

6. **The built-in default template is the look of Open Waterway Designer**: cream paper on the cover, one dark
   ink for every line and letter, Libre Caslon Text for headings, Public Sans for text and numbers, IBM Plex
   Mono for code. The fonts are SIL OFL 1.1 and are bundled in the crate, so the output is the same on every
   machine. A tool can change the template's colours, organisation name, logo and cover notice. An
   organisation's private house style lives outside this repository.

7. **A conformance suite.** The same documents are rendered to PDF, HTML and Word, and each output is read back
   by an independent reader (not the code that wrote it). The headings, numbers, paragraphs, table cells,
   captions and document-control values must be the same, in the same order, in all three.

8. **Python import name `ord_report`.** The name `ord` would hide Python's built-in `ord()` in any module that
   imports it.

## Consequences

- The same report renders the same way in a terminal, a desktop app, a Python script and a browser.
- The WebAssembly package is large (Typst and the fonts are inside it). That is accepted for v0.1.
- Word's layout cannot match PDF exactly: Word breaks lines and pages itself, and builds the table of contents
  when the file is opened. The engine matches the page, fonts, colours and every piece of text; the
  conformance suite checks structure and text, not line breaks.
- Because only prose is Markdown, a tool cannot slip a heading or a table into a paragraph. The checks refuse a
  Markdown heading inside a prose block.
- Format changes are public changes: they change the schema, need a changelog fragment, and move the format
  version.
