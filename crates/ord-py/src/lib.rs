//! The native core of the `ord_report` Python package (spec 0001 §11.3). The Python layer
//! (`python/ord_report/__init__.py`) turns `FindingsError` into `DocumentError` and reads files.

use std::collections::BTreeMap;

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

create_exception!(
    _ord,
    FindingsError,
    PyException,
    "The document has errors. args[0] is the findings as JSON."
);

fn findings_err(f: &[ord::Finding]) -> PyErr {
    FindingsError::new_err(serde_json::to_string(f).expect("findings serialise"))
}

fn register(json: Option<&str>) -> PyResult<ord::Register> {
    match json {
        None => Ok(ord::builtin_register()),
        Some(j) => ord::Register::from_json(j).map_err(|f| findings_err(&f)),
    }
}

/// Reads a document and its assets. A missing or bad asset is reported with every other finding.
fn load(
    document: &str,
    assets: BTreeMap<String, Vec<u8>>,
    reg: &ord::Register,
) -> PyResult<(ord::Document, ord::Assets)> {
    let doc = ord::parse(document).map_err(|f| findings_err(&f))?;
    let (assets, mut f) = ord::assets::collect(&doc, &assets);
    if f.is_empty() {
        return Ok((doc, assets));
    }
    f.extend(ord::check(&doc, &assets, reg).into_iter().filter(|x| x.is_error()));
    Err(findings_err(&f))
}

/// render(document_json, format, assets, register_json) -> bytes
#[pyfunction]
#[pyo3(signature = (document, format, assets=BTreeMap::new(), register_json=None))]
fn render<'py>(
    py: Python<'py>,
    document: &str,
    format: &str,
    assets: BTreeMap<String, Vec<u8>>,
    register_json: Option<&str>,
) -> PyResult<(Bound<'py, PyBytes>, String)> {
    let f = ord::Format::parse(format)
        .ok_or_else(|| PyValueError::new_err(format!("unknown format {format:?}: pdf, html or docx")))?;
    let reg = register(register_json)?;
    let (doc, assets) = load(document, assets, &reg)?;
    let out = py.detach(|| ord::render(&doc, &assets, &reg, f)).map_err(|f| findings_err(&f))?;
    let warnings = serde_json::to_string(&out.warnings).expect("findings serialise");
    Ok((PyBytes::new(py, &out.bytes), warnings))
}

/// check(document_json, assets, register_json) -> findings JSON
#[pyfunction]
#[pyo3(signature = (document, assets=BTreeMap::new(), register_json=None))]
fn check(document: &str, assets: BTreeMap<String, Vec<u8>>, register_json: Option<&str>) -> PyResult<String> {
    let reg = register(register_json)?;
    let doc = match ord::parse(document) {
        Ok(d) => d,
        Err(f) => return Ok(serde_json::to_string(&f).expect("findings serialise")),
    };
    let (assets, mut f) = ord::assets::collect(&doc, &assets);
    f.extend(ord::check(&doc, &assets, &reg));
    Ok(serde_json::to_string(&f).expect("findings serialise"))
}

/// assemble(manifest_json, [chapter_json], register_json) -> document JSON
#[pyfunction]
#[pyo3(signature = (manifest, chapters, register_json=None))]
fn assemble(manifest: &str, chapters: Vec<String>, register_json: Option<&str>) -> PyResult<String> {
    let reg = register(register_json)?;
    let m: ord::assemble::Manifest = ord::parse::parse_as(manifest).map_err(|f| findings_err(&f))?;
    let mut cs = Vec::new();
    for c in &chapters {
        cs.push(ord::parse::parse_as::<ord::assemble::Chapter>(c).map_err(|f| findings_err(&f))?);
    }
    let d = ord::assemble(&m, &cs, &reg).map_err(|f| findings_err(&f))?;
    Ok(serde_json::to_string(&d).expect("a document serialises"))
}

#[pyfunction]
fn schema() -> String {
    ord::schema()
}

#[pyfunction]
fn builtin_register() -> String {
    serde_json::to_string(&ord::builtin_register()).expect("a register serialises")
}

#[pymodule]
fn _ord(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", ord::VERSION)?;
    m.add("FindingsError", m.py().get_type::<FindingsError>())?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(check, m)?)?;
    m.add_function(wrap_pyfunction!(assemble, m)?)?;
    m.add_function(wrap_pyfunction!(schema, m)?)?;
    m.add_function(wrap_pyfunction!(builtin_register, m)?)?;
    Ok(())
}
