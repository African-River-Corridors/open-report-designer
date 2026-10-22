//! Shared by the integration tests: the example documents with their file assets read in.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A document file and its assets, read the way the command-line program reads them.
pub fn load(path: &Path) -> (ord::Document, ord::Assets) {
    let doc = ord::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let dir = path.parent().unwrap();
    let supplied: BTreeMap<String, Vec<u8>> = doc
        .assets
        .iter()
        .filter_map(|(id, a)| a.path.as_ref().map(|p| (id.clone(), std::fs::read(dir.join(p)).unwrap())))
        .collect();
    let (assets, findings) = ord::assets::collect(&doc, &supplied);
    assert!(findings.is_empty(), "{findings:?}");
    (doc, assets)
}

pub fn example() -> (ord::Document, ord::Assets) {
    load(&repo().join("examples/screening-report.json"))
}

/// A minimal valid document with the given body, as JSON.
pub fn with_body(body: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "ord": "0.1", "class": "report", "title": "T",
        "control": { "revision": "A", "date": "2026-01-01", "status": "draft" },
        "body": body
    })
}

pub fn doc(v: &serde_json::Value) -> ord::Document {
    ord::parse(&v.to_string()).unwrap()
}

pub fn prose(md: &str) -> serde_json::Value {
    serde_json::json!({ "type": "prose", "markdown": md })
}

pub fn section(id: &str, title: &str, body: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "type": "section", "id": id, "title": title, "body": body })
}
