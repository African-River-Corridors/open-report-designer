//! The document (spec 0001 §3): the typed tree a tool writes and the engine reads.
//!
//! Every struct refuses unknown fields, so a typo is an error, not a silent default. The JSON Schema in
//! `schema/ord-document.schema.json` is generated from these types (`crate::schema`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The format version this engine reads.
pub const FORMAT_VERSION: &str = "0.1";

/// A report document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Format version. v0.1 accepts "0.1" only.
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

/// A table, numbered "Table n".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Table {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub caption: String,
    pub columns: Vec<Column>,
    /// One cell per column in every row. Cells are plain text.
    pub rows: Vec<Vec<String>>,
    /// Notes under the table (Markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
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

/// The image types the engine reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MediaType {
    #[serde(rename = "image/svg+xml")]
    Svg,
    #[serde(rename = "image/png")]
    Png,
}

impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaType::Svg => "image/svg+xml",
            MediaType::Png => "image/png",
        }
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
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}
