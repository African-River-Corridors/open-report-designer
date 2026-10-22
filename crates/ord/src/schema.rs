//! The published JSON Schema of the document (spec 0001 §3), generated from the types in `model`.

use crate::model::Document;

/// The schema's `$id`.
pub const SCHEMA_ID: &str = "https://github.com/African-River-Corridors/open-report-designer/blob/main/schema/ord-document.schema.json";

/// The JSON Schema (2020-12) of a document, pretty-printed, ending in a newline. The committed copy is
/// `schema/ord-document.schema.json`; a test fails when it is stale.
pub fn schema() -> String {
    let mut schema = schemars::schema_for!(Document);
    schema.insert("$id".into(), SCHEMA_ID.into());
    schema.insert("title".into(), "Open Report Designer document".into());
    let mut out = serde_json::to_string_pretty(&schema).expect("a schema always serialises");
    out.push('\n');
    out
}
