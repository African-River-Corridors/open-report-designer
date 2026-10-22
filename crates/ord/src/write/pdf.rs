//! The PDF writer (spec 0001 §10). Built in its own ticket.

use crate::finding::Finding;
use crate::render::Prepared;

/// Writes the prepared document. Not built yet: refuses with `ORD000`.
pub fn write(_p: &Prepared) -> Result<Vec<u8>, Vec<Finding>> {
    Err(vec![Finding::error("ORD000", "", "this writer is not built yet")])
}
