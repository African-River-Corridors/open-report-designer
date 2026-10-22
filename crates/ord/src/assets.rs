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

/// Checks that an asset's bytes are an image of its declared type: a PNG signature and header, or an SVG
/// document (UTF-8 text whose root element is `svg`). Returns the image size in pixels when it is known.
pub fn validate(a: &Asset) -> Result<Option<(u32, u32)>, String> {
    match a.media_type {
        MediaType::Png => png_size(&a.bytes).map(Some),
        MediaType::Svg => {
            let text = std::str::from_utf8(&a.bytes).map_err(|_| "not UTF-8 text".to_string())?;
            let mut rest = text.trim_start_matches('\u{feff}').trim_start();
            // Skip the XML declaration, comments and a doctype before the root element.
            loop {
                if let Some(r) = rest.strip_prefix("<?") {
                    rest =
                        r.split_once("?>").map(|(_, r)| r).ok_or("an unclosed XML declaration")?.trim_start();
                } else if let Some(r) = rest.strip_prefix("<!--") {
                    rest = r.split_once("-->").map(|(_, r)| r).ok_or("an unclosed comment")?.trim_start();
                } else if let Some(r) = rest.strip_prefix("<!") {
                    rest = r.split_once('>').map(|(_, r)| r).ok_or("an unclosed doctype")?.trim_start();
                } else {
                    break;
                }
            }
            let root = rest.strip_prefix('<').ok_or("no root element")?;
            let name: String =
                root.chars().take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/').collect();
            if name == "svg" || name.ends_with(":svg") {
                Ok(None)
            } else {
                Err(format!("the root element is <{name}>, not <svg>"))
            }
        }
    }
}

/// The width and height in a PNG's header.
pub fn png_size(b: &[u8]) -> Result<(u32, u32), String> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if b.len() < 24 || &b[..8] != SIG || &b[12..16] != b"IHDR" {
        return Err("no PNG signature and header".into());
    }
    let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    if w == 0 || h == 0 {
        return Err("a PNG of zero size".into());
    }
    Ok((w, h))
}
