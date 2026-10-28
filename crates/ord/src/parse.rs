//! Reading a document from JSON (spec 0001 §11.1 `parse`).

use crate::finding::Finding;
use crate::model::{Document, FORMAT_VERSIONS};

/// Reads a document. Bad JSON, a wrong type, an unknown field or an unsupported format version gives one
/// `ORD001` finding whose path points at the problem.
pub fn parse(json: &str) -> Result<Document, Vec<Finding>> {
    parse_as::<Document>(json).and_then(|doc| {
        if FORMAT_VERSIONS.contains(&doc.ord.as_str()) {
            Ok(doc)
        } else {
            Err(vec![Finding::error(
                "ORD001",
                "/ord",
                format!(
                    "format version {:?} is not supported; this engine reads {}",
                    doc.ord,
                    FORMAT_VERSIONS.map(|v| format!("{v:?}")).join(" and ")
                ),
            )])
        }
    })
}

/// Reads any engine type from JSON, with `ORD001` findings that carry a JSON pointer path.
pub fn parse_as<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, Vec<Finding>> {
    let de = &mut serde_json::Deserializer::from_str(json);
    serde_path_to_error::deserialize(de).map_err(|e| {
        let (inner, msg) = unmark(&e.inner().to_string());
        vec![Finding::error("ORD001", format!("{}{inner}", pointer(e.path())), msg)]
    })
}

/// A serde path as a JSON pointer: `body[2].rows[0]` becomes `/body/2/rows/0`.
pub(crate) fn pointer(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut out = String::new();
    for seg in path.iter() {
        match seg {
            Segment::Seq { index } => out.push_str(&format!("/{index}")),
            Segment::Map { key } => out.push_str(&format!("/{}", escape(key))),
            Segment::Enum { variant } => out.push_str(&format!("/{}", escape(variant))),
            Segment::Unknown => {}
        }
    }
    out
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Marks an error message with the path inside a value that was read by hand, so the outer reader can join
/// it to its own path. serde errors carry only text, so the path travels in it.
const MARK: char = '\u{1}';

pub(crate) fn marked(path: &str, message: &str) -> String {
    format!("{MARK}{path}{MARK}{message}")
}

/// Splits a marked message into the inner path and the message. serde may append " at line L column C",
/// which is dropped: the path says where.
pub(crate) fn unmark(message: &str) -> (String, String) {
    if let Some(rest) = message.strip_prefix(MARK)
        && let Some((path, msg)) = rest.split_once(MARK)
    {
        let msg = match msg.rfind(" at line ") {
            Some(i) if msg[i..].contains(" column ") => &msg[..i],
            _ => msg,
        };
        return (path.to_string(), msg.to_string());
    }
    (String::new(), message.to_string())
}
