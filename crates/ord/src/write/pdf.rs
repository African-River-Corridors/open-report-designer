//! The PDF writer (spec 0001 §10): Typst markup from the resolved document, compiled in memory.
//!
//! `markup` writes the Typst source. Every piece of the document's text goes into the source as a Typst string
//! literal (`#"..."`), never as markup, so no character of it can start maths, a heading, a label or a call.
//! `world` is the compiler's whole environment: the source, the bundled fonts and the document's assets as
//! virtual files. It reads no file, no clock and no network, and it loads no package.

mod markup;
mod world;

use typst::diag::SourceDiagnostic;
use typst::foundations::Smart;
use typst_layout::PagedDocument;

use crate::finding::Finding;
use crate::render::Prepared;

/// Writes the prepared document as a PDF.
///
/// A failure of the typesetter (which the checks should make impossible) refuses with `ORD000` and the
/// typesetter's message.
pub fn write(p: &Prepared) -> Result<Vec<u8>, Vec<Finding>> {
    let m = markup::markup(p);
    let world = world::OrdWorld::new(m.source, m.files, m.date);
    let doc = typst::compile::<PagedDocument>(&world).output.map_err(|e| diagnostics(e.as_slice()))?;
    let options = typst_pdf::PdfOptions {
        ident: Smart::Auto,
        creator: Smart::Custom(Some(format!("Open Report Designer {}", crate::VERSION))),
        // The creation date is the document date, set in the markup (`set document(date: ..)`).
        timestamp: None,
        ..Default::default()
    };
    typst_pdf::pdf(&doc, &options).map_err(|e| diagnostics(e.as_slice()))
}

fn diagnostics(e: &[SourceDiagnostic]) -> Vec<Finding> {
    e.iter()
        .map(|d| Finding::error("ORD000", "", format!("the PDF typesetter failed: {}", d.message)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::markup::{esc, label};

    #[test]
    fn strings_are_literals() {
        assert_eq!(esc("a\"b\\c"), r#"#"a\"b\\c""#);
        assert_eq!(esc("$x$ #f() [y] <z> @w * _ ~"), r##"#"$x$ #f() [y] <z> @w * _ ~""##);
        assert_eq!(esc("a\nb\u{1}"), r#"#"a b""#);
    }

    #[test]
    fn labels_are_injective_and_safe() {
        assert_eq!(label("ord-section-1"), "<x-ord-section-1>");
        assert_eq!(label("a_b"), "<x-a_5fb>");
        assert_eq!(label("a b"), "<x-a_20b>");
        assert_ne!(label("a_20b"), label("a b"));
        assert_eq!(label("é"), "<x-_c3_a9>");
    }
}
