//! Open Report Designer (ORD): a typed report document to PDF, HTML and Word.
//!
//! The engine is specified in `docs/specs/0001-report-engine.md`. This crate is the core.

/// The version of the `ord` crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn the_version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
