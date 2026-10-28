# Spec 0002: format 0.2, the photo cover and the template values

- Status: proposed
- Builds on: [spec 0001](0001-report-engine.md)
- Release: not yet released

## 1. Goal

A tool with its own house style can render its reports through ORD: a cover with a photograph and a logo
strip, its own colours, fonts and furniture, and richer tables. The Word file has the same layout as the PDF,
so it can be the editable master of a report. The engine stays domain-free and holds no house style: a tool
passes values, never a template language.

## 2. Format versions

- An engine of this spec reads `"ord": "0.1"` and `"ord": "0.2"`.
- Every field and value below is marked **(0.2)**. A `"0.1"` document that uses one is refused with `ORD001`
  at the field's path, so a `"0.1"` document still renders the same on a 0.1 engine.
- A 0.1 engine refuses a `"0.2"` document at `/ord` (spec 0001 §3.1), so a document never loses fields
  silently on an older engine.
- A `"0.1"` document renders **byte for byte** as on the 0.1 engine, in all three formats, with one fix: a
  Word file with no table of contents no longer sets `updateFields` (§8). (Tested by hand against `main` for
  the example and the stress document, which both have a table of contents.)

## 3. The document

### 3.1 Top level (additions)

| Field | Type | Meaning |
|---|---|---|
| `cover_image` | `{ asset, alt }` | A photograph across the head of a photo cover (§4). `alt` is required and not empty (`ORD014`). |
| `cover_logos` | `[{ asset, name, role? }]` | A logo strip on a photo cover: each logo with a name and a role, such as the parties to a report. |
| `cover_details` | `[{ label, value }]` | The rows of the photo cover's control block, in place of number, revision, date and status. An empty list leaves the block out. |
| `footer` | string | The left of the running foot, in place of the organisation and the notice. |

### 3.2 Tables (additions)

| Field | Meaning |
|---|---|
| `caption` | Optional. A table with no caption has no caption line and no number; the next captioned table is "Table 1". A reference to it prints "the table". |
| `header` | Default `true`. `false` prints no header row: a table of labels and values. |
| `column_groups` | `[{ title, span }]`: a group row above the column titles. Spans are at least 1 and add up to the columns (`ORD013`). A group with an empty title leaves its columns' titles spanning both header rows. |
| `columns[].width` | The column's share of the table, `0 < w ≤ 1`; the given widths add up to at most 1 (`ORD020`). |
| cells | A cell is a string, or `{ "markdown": "..." }`: one paragraph of inline Markdown (emphasis, strong, code, links, hard line breaks, cross-references). Anything more is `ORD012`. |

### 3.3 Assets (additions)

- `image/jpeg`: read by its frame header; refused with `ORD011` when it has none.
- `font/ttf` (also `font/otf`, `font/collection`): a TrueType or OpenType font file, or a collection. A font is
  never a figure, map, cover image or logo (`ORD011`).

### 3.4 The register

A lifecycle status may take the revision form `text`: any revision with no white space, such as `0.2`.

## 4. The photo cover

With `template.cover_layout: "photo"`, the cover is, top to bottom, on the cover paper:

1. the cover image across the full width, `cover_image_height_mm` high, cropped to fill (centred);
2. 12 mm under it, a 14 mm accent rule, then the title in the heading font;
3. the subtitle, the organisation and the notice, each a line in the accent;
4. the summary on a card;
5. the logo strip on a card: one column per logo, a rule between them, each logo 9 mm high, its name and role;
6. the control block: a "Document control" head and one row per `cover_details` row (or number, revision,
   date and status), on lines of the accent;
7. a foot: the number and revision on the left, the sensitivity on the right.

The class name is not printed. Each writer draws the same thing: Typst blocks for the PDF; anchored pictures,
one-cell shaded tables and the cover section's own footer for Word; HTML elements for HTML.

## 5. The template (additions)

| Value | Default | Meaning |
|---|---|---|
| `cover_layout` | `classic` | `classic` (spec 0001) or `photo` (§4). |
| `cover_image_height_mm` | 75 | 10 to 200. |
| `cover_ink`, `cover_accent`, `card_ground` | ink, ink, white | The photo cover's text, accent and card ground. |
| `heading_ink` | ink | Headings. |
| `heading_rule` | none | A rule under each level-1 heading. |
| `table_header_ink` | ink | The text of a table's header rows (the ground is `accent_ground`). |
| `heading_sizes_pt` | none | Levels 1 to 4, points. Unset keeps each writer's 0.1 sizes. |
| `heading_bold` | false | |
| `heading_numbers` | `spaced` | `spaced` ("2.1  Scope") or `dotted` ("2. Scope", "2.1 Scope"). |
| `section_page_break` | true | A new page before each top-level section. |
| `heading_font`, `body_font` | bundled | Family names. A family that is neither bundled nor given by a font in `fonts` refuses the PDF (`ORD022`). Word names the family. |
| `fonts` | none | Asset ids of font files, for the PDF and HTML writers. Word does not embed them. |
| `body_size_pt` | 10 | 6 to 16. |
| `margins_mm` | 25/25/20/20 | `{ top, bottom, left, right }`, 0 to 60. |
| `control_page` | true | The document-control page after the cover. |
| `running_head` | true | The running head. |
| `foot_sensitivity` | false | The sensitivity in capitals in the middle of the running foot. |
| `table_widths` | none | `content`: every writer sizes columns by the rule of §6. Unset keeps each writer's 0.1 widths. |
| `revision_prefix` | `Rev ` | The word before the revision in the running head and on the cover foot. |

When the document has a `footer` or the template sets `foot_sensitivity`, the running foot is three columns of
40, 20 and 40 per cent (text, sensitivity, "Page n of N") in every writer, so a long text wraps in its own
column. Otherwise it is the 0.1 foot.

## 6. Column widths

A column needs its longest word and would like its longest line (the header included), both counted in
characters, plus two for the padding. A column whose line fits in a fair share of the free width gets it; this
repeats with what is left. The others each get their longest word, and the rest goes in proportion to what
each still wants. A width the document gives is kept. Widths are rounded to a thousandth and are the same
numbers in every writer.

## 7. Checks (additions)

| Code | Severity | Finding |
|---|---|---|
| `ORD001` | error | (also) A `"0.1"` document uses a 0.2 field or value. |
| `ORD011` | error | (also) A cover image or logo names a missing, invalid or font asset; a template font is not a valid font. |
| `ORD012` | error | (also) A Markdown cell holds more than one paragraph. |
| `ORD013` | error | (also) Column groups do not span the columns. |
| `ORD014` | error | (also) The cover image has empty alt text. |
| `ORD020` | error | (also) A column width is outside `0 < w ≤ 1`, or the widths add up to more than 1. |
| `ORD022` | error | A font family is neither bundled nor given by a font asset (PDF). |

## 8. Word as a master

The Word file is meant to be edited and saved as the master of a report. So: every paragraph has a named style
(Title, Heading 1 to 4, Table Text, Table Header, Cover Title, Card Text, and so on); the running foot and the
cover foot are text; the only fields are `PAGE` and `NUMPAGES` (no `DOCPROPERTY`, which shows an error when the
property is missing); `updateFields` is set only for a table of contents (Word asks a modal question on every
open when it is set); and every layout value is the template's, so the file looks like the PDF.

## 9. Tests

- **Conformance.** `examples/photo-cover-report.json` (fictional) uses every 0.2 feature and has the same
  structure and text in PDF, HTML and Word. A PDF text reader meets a header cell that spans two rows after
  the rest of its row; the suite expects that order in the PDF only.
- **Determinism.** The example renders the same bytes twice in each format.
- **Checks.** Each new finding has a test that makes it fire (`crates/ord/tests/format_02.rs`).
- **Word furniture.** The example's Word file has no `DOCPROPERTY` field and no field but `PAGE` and
  `NUMPAGES`.
