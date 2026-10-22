//! Open Report Designer (ORD): one typed report document to PDF, HTML and Word.
//!
//! The engine is specified in `docs/specs/0001-report-engine.md` and decided in ADR 0001. This crate is the
//! pure core: it opens no file, reads no clock and uses no network.

pub mod assets;
pub mod check;
pub mod finding;
pub mod fonts;
pub mod markdown;
pub mod model;
pub mod parse;
pub mod register;
pub mod resolve;
pub mod schema;
pub mod template;

pub use assets::{Asset, Assets};
pub use check::check;
pub use finding::{Finding, Severity};
pub use model::Document;
pub use parse::parse;
pub use register::{Register, builtin_register};
pub use resolve::{Resolved, resolve};
pub use schema::schema;
pub use template::Template;

/// The version of the `ord` crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
