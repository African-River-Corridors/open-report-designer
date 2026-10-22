//! The WebAssembly package of Open Report Designer (spec 0001 §11.4).
//!
//! ```js
//! import { render, check, schema, version } from "ord-wasm";
//! const pdf = render(documentJson, "pdf", new Map([["plan", pngBytes]]));
//! ```
//! A document with errors throws an `Error` whose message is the findings as JSON.

use std::collections::BTreeMap;

use wasm_bindgen::prelude::*;

fn findings_error(f: &[ord::Finding]) -> JsError {
    JsError::new(&serde_json::to_string(f).expect("findings serialise"))
}

fn register(json: Option<String>) -> Result<ord::Register, JsError> {
    match json {
        None => Ok(ord::builtin_register()),
        Some(j) => ord::Register::from_json(&j).map_err(|f| findings_error(&f)),
    }
}

/// The asset bytes from a `Map<string, Uint8Array>`.
fn assets(map: Option<js_sys::Map>) -> Result<BTreeMap<String, Vec<u8>>, JsError> {
    let mut out = BTreeMap::new();
    if let Some(m) = map {
        for entry in m.entries() {
            let pair: js_sys::Array = entry.map_err(|_| JsError::new("assets: not a Map"))?.into();
            let key = pair.get(0).as_string().ok_or_else(|| JsError::new("assets: a key is not a string"))?;
            let bytes = js_sys::Uint8Array::new(&pair.get(1)).to_vec();
            out.insert(key, bytes);
        }
    }
    Ok(out)
}

fn load(
    document: &str,
    supplied: &BTreeMap<String, Vec<u8>>,
    reg: &ord::Register,
) -> Result<(ord::Document, ord::Assets), JsError> {
    let doc = ord::parse(document).map_err(|f| findings_error(&f))?;
    let (assets, mut f) = ord::assets::collect(&doc, supplied);
    if f.is_empty() {
        return Ok((doc, assets));
    }
    f.extend(ord::check(&doc, &assets, reg).into_iter().filter(|x| x.is_error()));
    Err(findings_error(&f))
}

/// Renders a document (JSON) as "pdf", "html" or "docx". `assets` gives the bytes of `path` assets by id.
#[wasm_bindgen]
pub fn render(
    document_json: &str,
    format: &str,
    assets_map: Option<js_sys::Map>,
    register_json: Option<String>,
) -> Result<Vec<u8>, JsError> {
    let f = ord::Format::parse(format)
        .ok_or_else(|| JsError::new(&format!("unknown format {format:?}: pdf, html or docx")))?;
    let reg = register(register_json)?;
    let (doc, assets) = load(document_json, &assets(assets_map)?, &reg)?;
    ord::render(&doc, &assets, &reg, f).map(|o| o.bytes).map_err(|f| findings_error(&f))
}

/// Every finding about a document, as JSON.
#[wasm_bindgen]
pub fn check(
    document_json: &str,
    assets_map: Option<js_sys::Map>,
    register_json: Option<String>,
) -> Result<String, JsError> {
    let reg = register(register_json)?;
    let f = match ord::parse(document_json) {
        Err(f) => f,
        Ok(doc) => {
            let (assets, mut f) = ord::assets::collect(&doc, &assets(assets_map)?);
            f.extend(ord::check(&doc, &assets, &reg));
            f
        }
    };
    Ok(serde_json::to_string(&f).expect("findings serialise"))
}

/// The JSON Schema of a document.
#[wasm_bindgen]
pub fn schema() -> String {
    ord::schema()
}

/// The engine's version.
#[wasm_bindgen]
pub fn version() -> String {
    ord::VERSION.to_string()
}
