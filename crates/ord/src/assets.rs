//! Assets: the images a document uses, as bytes (spec 0001 §3.4).
//!
//! The core never reads a file. Inline assets are decoded from base64; a caller that reads files (the
//! command-line program, the Python `render_file`) supplies the bytes of each `path` asset.

use std::collections::BTreeMap;

use base64::Engine as _;

use crate::finding::Finding;
use crate::model::{Document, MediaType};

/// One image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub media_type: MediaType,
    pub bytes: Vec<u8>,
}

/// Asset id to image.
pub type Assets = BTreeMap<String, Asset>;

/// The document's inline assets, decoded, merged with the bytes the caller supplies for the rest.
///
/// `supplied` maps an asset id to its bytes; it wins over an inline copy. An inline asset that is not
/// valid base64, or a `path` asset with no supplied bytes, gives `ORD011`.
pub fn collect(doc: &Document, supplied: &BTreeMap<String, Vec<u8>>) -> (Assets, Vec<Finding>) {
    let mut out = Assets::new();
    let mut findings = Vec::new();
    for (id, src) in &doc.assets {
        let path = format!("/assets/{}", id.replace('~', "~0").replace('/', "~1"));
        let bytes = if let Some(b) = supplied.get(id) {
            Some(b.clone())
        } else if let Some(b64) = &src.base64 {
            match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
                Ok(b) => Some(b),
                Err(e) => {
                    findings.push(Finding::error(
                        "ORD011",
                        &path,
                        format!("asset {id:?} is not valid base64: {e}"),
                    ));
                    None
                }
            }
        } else if let Some(p) = &src.path {
            findings.push(Finding::error(
                "ORD011",
                &path,
                format!("asset {id:?} names the file {p:?}, and its bytes were not supplied"),
            ));
            None
        } else {
            findings.push(Finding::error(
                "ORD011",
                &path,
                format!("asset {id:?} has neither base64 nor a path"),
            ));
            None
        };
        if let Some(bytes) = bytes {
            out.insert(id.clone(), Asset { media_type: src.media_type, bytes });
        }
    }
    (out, findings)
}
