//! Open Report Designer (ORD): one typed report document to PDF, HTML and Word.
//!
//! The engine is specified in `docs/specs/0001-report-engine.md` and decided in ADR 0001. This crate is the
//! pure core: it opens no file, reads no clock and uses no network.

pub mod assets;
pub mod check;
pub mod finding;
pub mod model;
pub mod parse;
pub mod register;
pub mod schema;

pub use assets::{Asset, Assets};
pub use check::check;
pub use finding::{Finding, Severity};
pub use model::Document;
pub use parse::parse;
pub use register::{Register, builtin_register};
pub use schema::schema;

/// The version of the `ord` crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
