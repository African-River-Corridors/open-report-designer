//! The document (spec 0001 §3): the typed tree a tool writes and the engine reads.
//!
//! Every struct refuses unknown fields, so a typo is an error, not a silent default. The JSON Schema in
//! `schema/ord-document.schema.json` is generated from these types (`crate::schema`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The format version this engine writes in its examples: the newest it reads.
pub const FORMAT_VERSION: &str = "0.2";

/// Every format version this engine reads. A "0.1" document may not use a field added in "0.2"
/// (`crate::check` refuses it), so it still renders on a 0.1 engine.
pub const FORMAT_VERSIONS: [&str; 2] = ["0.1", "0.2"];

/// A report document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Format version: "0.1" or "0.2". Fields marked "(0.2)" need "0.2".
    pub ord: String,
    /// A document class id from the register, such as "report".
    pub class: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Shown on the cover and in the footer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organisation: Option<String>,
    /// BCP 47 language tag. Default "en".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub control: Control,
    /// A short summary on the cover page (Markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// A notice on the cover and in the footer, such as "Screening tool. Not for construction."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    /// Table of contents. Default from the document class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<bool>,
    /// (0.2) A photograph across the head of the cover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<CoverImage>,
    /// (0.2) A strip of logos on the cover, each with a name and a role, such as the parties to a report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cover_logos: Vec<CoverLogo>,
    /// (0.2) The rows of the cover's control block, in place of number, revision, date and status. An empty
    /// list leaves the block out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_details: Option<Vec<DataItem>>,
    /// (0.2) The text on the left of the running foot, in place of the organisation and the notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    /// The main matter.
    pub body: Vec<Block>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub appendices: Vec<Appendix>,
    /// Asset id to image.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, AssetSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateOverrides>,
}

impl Document {
    /// The document's language tag, "en" when it names none.
    pub fn language(&self) -> &str {
        self.language.as_deref().unwrap_or("en")
    }
}

/// A photograph across the head of the cover (0.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoverImage {
    /// An asset id.
    pub asset: String,
    /// Alt text. Required and not empty.
    pub alt: String,
}

/// One logo of the cover's logo strip (0.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoverLogo {
    /// An asset id.
    pub asset: String,
    /// The name printed under the logo. Also its alt text.
    pub name: String,
    /// A role printed under the name, such as "Client".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Document control (spec 0001 §3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Control {
    /// The document number. Starts with "<prefix>-" when the class has a number prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    /// The revision: a letter (A, B, ...) while working, a number (0, 1, ...) once issued.
    pub revision: String,
    /// ISO 8601 date, YYYY-MM-DD.
    pub date: String,
    /// A status of the class's lifecycle.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<Sensitivity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepared: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved: Option<String>,
    /// Earlier and current revisions, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoryEntry>,
}

/// Who may see the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Sensitivity {
    #[default]
    Public,
    Internal,
    Confidential,
    Restricted,
}

impl Sensitivity {
    pub fn label(self) -> &'static str {
        match self {
            Sensitivity::Public => "Public",
            Sensitivity::Internal => "Internal",
            Sensitivity::Confidential => "Confidential",
            Sensitivity::Restricted => "Restricted",
        }
    }
}

/// One row of the revision history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    pub revision: String,
    pub date: String,
    pub status: String,
    pub description: String,
}

/// An appendix: lettered A, B, ... (spec 0001 §5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Appendix {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub title: String,
    pub body: Vec<Block>,
}

/// A block of the document (spec 0001 §3.3).
///
/// Read by hand (`impl Deserialize` below) rather than by serde's internally tagged enum, which buffers the
/// block and so loses the path to an error inside it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Section(Section),
    Prose(Prose),
    Table(Table),
    Figure(Figure),
    Map(Map),
    DataBox(DataBox),
}

impl Block {
    /// The block's id, if it has one.
    pub fn id(&self) -> Option<&str> {
        match self {
            Block::Section(b) => b.id.as_deref(),
            Block::Prose(_) => None,
            Block::Table(b) => b.id.as_deref(),
            Block::Figure(b) => b.id.as_deref(),
            Block::Map(b) => b.id.as_deref(),
            Block::DataBox(b) => b.id.as_deref(),
        }
    }
}

impl<'de> Deserialize<'de> for Block {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut map = serde_json::Map::<String, serde_json::Value>::deserialize(de)?;
        let tag = match map.remove("type") {
            Some(serde_json::Value::String(t)) => t,
            Some(_) => {
                return Err(D::Error::custom(crate::parse::marked("/type", "`type` must be a string")));
            }
            None => return Err(D::Error::missing_field("type")),
        };
        let value = serde_json::Value::Object(map);
        fn read<T: serde::de::DeserializeOwned, E: Error>(v: serde_json::Value) -> Result<T, E> {
            serde_path_to_error::deserialize(v).map_err(|e| {
                let (inner, msg) = crate::parse::unmark(&e.inner().to_string());
                let path = format!("{}{}", crate::parse::pointer(e.path()), inner);
                E::custom(crate::parse::marked(&path, &msg))
            })
        }
        Ok(match tag.as_str() {
            "section" => Block::Section(read(value)?),
            "prose" => Block::Prose(read(value)?),
            "table" => Block::Table(read(value)?),
            "figure" => Block::Figure(read(value)?),
            "map" => Block::Map(read(value)?),
            "data_box" => Block::DataBox(read(value)?),
            other => {
                return Err(D::Error::custom(crate::parse::marked(
                    "/type",
                    &format!(
                        "unknown block type `{other}`, expected one of `section`, `prose`, `table`, `figure`, `map`, `data_box`"
                    ),
                )));
            }
        })
    }
}

/// A section. Nesting sets the heading level (at most four).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Section {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub title: String,
    /// Default true. An unnumbered section takes no number, nor do its children.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub numbered: bool,
    pub body: Vec<Block>,
}

/// Markdown prose (spec 0001 §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prose {
    pub markdown: String,
}

/// A table, numbered "Table n" when it has a caption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Table {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The caption. Required in 0.1. (0.2) Optional: a table with no caption is not numbered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// (0.2) Default true. False prints no header row: a table of labels and values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<bool>,
    /// (0.2) A row of group titles above the column titles. Each group spans one or more columns, in order;
    /// the spans add up to the number of columns. A group with an empty title leaves its columns' titles
    /// spanning both header rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_groups: Vec<ColumnGroup>,
    pub columns: Vec<Column>,
    /// One cell per column in every row. A cell is plain text, or (0.2) `{ "markdown": "..." }`.
    pub rows: Vec<Vec<Cell>>,
    /// Notes under the table (Markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// A group of columns under one title (0.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColumnGroup {
    pub title: String,
    /// The number of columns, at least 1.
    pub span: u32,
}

/// A table cell: plain text, or (0.2) one paragraph of inline Markdown (emphasis, strong, code, links, hard
/// line breaks and cross-references).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Cell {
    Text(String),
    Markdown(MarkdownCell),
}

impl From<&str> for Cell {
    fn from(s: &str) -> Cell {
        Cell::Text(s.to_string())
    }
}

/// A table cell of inline Markdown (0.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkdownCell {
    pub markdown: String,
}

/// A table column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// The unit, shown under the column title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// (0.2) The column's share of the table width, 0 < w <= 1. Columns without one share the rest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
}

/// Horizontal alignment of a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Centre,
    Right,
}

/// A figure, numbered "Figure n".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Figure {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub caption: String,
    /// An asset id.
    pub asset: String,
    /// Alt text. Required and not empty.
    pub alt: String,
    /// A fraction of the text width, 0 < width <= 1. Default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
}

/// A map: a figure with a key line (coordinate system, scale, source). Numbered with the figures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Map {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub caption: String,
    pub asset: String,
    pub alt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// The coordinate reference system, such as "EPSG:32630".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs: Option<String>,
    /// The scale, such as "1:25 000".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<String>,
    /// The source of the map data.
    pub attribution: String,
}

/// A boxed panel that states the data a result rests on and the limits of the result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DataBox {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Default "Data and limits".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub items: Vec<DataItem>,
    /// Notes under the items (Markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl DataBox {
    pub const DEFAULT_TITLE: &'static str = "Data and limits";

    pub fn title(&self) -> &str {
        self.title.as_deref().unwrap_or(Self::DEFAULT_TITLE)
    }
}

/// One line of a data box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DataItem {
    pub label: String,
    pub value: String,
}

/// An image the document uses: inline as base64, or a path that the caller reads (spec 0001 §3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetSource {
    pub media_type: MediaType,
    /// The image bytes, base64 (standard alphabet, padded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    /// A path relative to the document file. The core never reads it; the caller supplies the bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// The asset types the engine reads: images, and (0.2) fonts for the template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MediaType {
    #[serde(rename = "image/svg+xml")]
    Svg,
    #[serde(rename = "image/png")]
    Png,
    /// (0.2)
    #[serde(rename = "image/jpeg")]
    Jpeg,
    /// (0.2) A TrueType or OpenType font file, or a font collection.
    #[serde(rename = "font/ttf", alias = "font/otf", alias = "font/collection")]
    Font,
}

impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaType::Svg => "image/svg+xml",
            MediaType::Png => "image/png",
            MediaType::Jpeg => "image/jpeg",
            MediaType::Font => "font/ttf",
        }
    }

    /// True for the image types.
    pub fn is_image(self) -> bool {
        !matches!(self, MediaType::Font)
    }
}

/// Changes to the default template (spec 0001 §7).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateOverrides {
    /// An asset id for a logo on the cover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organisation_logo: Option<String>,
    /// Colours as "#rrggbb".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink_2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_ground: Option<String>,
    /// Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<bool>,
    // The fields below are added in 0.2 (spec 0002 §3). Each is optional; an unset field keeps the default.
    /// "classic" (the default) or "photo": a photograph band, the summary and the logo strip on cards, and
    /// a control block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_layout: Option<CoverLayout>,
    /// The height of the cover photograph, millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image_height_mm: Option<f32>,
    /// Colours as "#rrggbb": the cover's text, its accent (rules, the line under the title, labels), and the
    /// ground of a cover card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_ink: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_ground: Option<String>,
    /// The colour of headings, and of the rule under a level-1 heading (none when unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_ink: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_rule: Option<String>,
    /// The text of a table's header row; its ground is `accent_ground`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_header_ink: Option<String>,
    /// Heading sizes for levels 1 to 4, points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_sizes_pt: Option<[f32; 4]>,
    /// Bold headings. Default false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_bold: Option<bool>,
    /// "spaced" ("2.1  Scope", the default) or "dotted" ("2. Scope" at level 1, "2.1 Scope" below).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_numbers: Option<HeadingNumbers>,
    /// A new page before each top-level section. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_page_break: Option<bool>,
    /// Font family names. A family that is not bundled must be given by a `font/ttf` asset in `fonts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_font: Option<String>,
    /// Asset ids of font files, used by the PDF and HTML writers. Word uses the family names and does not
    /// embed these files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<String>,
    /// Body text size, points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_size_pt: Option<f32>,
    /// Page margins, millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margins_mm: Option<Margins>,
    /// A document-control page after the cover. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_page: Option<bool>,
    /// The running head (number, revision, furniture). Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_head: Option<bool>,
    /// The sensitivity label in the middle of the running foot. Default false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foot_sensitivity: Option<bool>,
    /// "content": every writer sizes table columns by the same rule, from their text. Unset, each writer
    /// keeps its 0.1 widths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_widths: Option<TableWidths>,
    /// The word before the revision in the running head and on the cover foot. Default "Rev ".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_prefix: Option<String>,
}

/// How table columns are sized (0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TableWidths {
    Content,
}

/// The cover's layout (0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CoverLayout {
    #[default]
    Classic,
    Photo,
}

/// How a heading's number is printed (0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HeadingNumbers {
    #[default]
    Spaced,
    Dotted,
}

/// Page margins, millimetres (0.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Margins {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}
