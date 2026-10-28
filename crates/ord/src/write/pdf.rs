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
    let fonts: Vec<Vec<u8>> =
        p.template.fonts.iter().filter_map(|id| p.assets.get(id)).map(|a| a.bytes.clone()).collect();
    let world = world::OrdWorld::new(m.source, m.files, fonts, m.date);
    // A family the world does not have would be swapped for another without a word. Refuse instead.
    let missing: Vec<Finding> =
        [("heading_font", &p.template.heading_font), ("body_font", &p.template.body_font)]
            .into_iter()
            .filter(|(_, f)| !world.has_family(f))
            .map(|(field, f)| {
                Finding::error(
                    "ORD022",
                    format!("/template/{field}"),
                    format!(
                        "the font family {f:?} is not bundled and no font asset in /template/fonts gives it"
                    ),
                )
            })
            .collect();
    if !missing.is_empty() {
        return Err(missing);
    }
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

/// The height of the photo cover's photograph, millimetres: `cover_image_height_mm`, or, when the template
/// lets the photograph fill (`cover_image_fill`), the height the cover's content leaves on the page, between
/// `cover_image_min_mm` and `cover_image_height_mm`. The content is measured once, by this typesetter, with
/// the document's own fonts, and the Word writer uses the same number, so both covers have one band.
pub fn band_mm(p: &Prepared) -> f64 {
    let t = &p.template;
    let max = f64::from(t.cover_image_height_mm);
    if !(t.cover && t.cover_image_fill && t.cover_layout == crate::model::CoverLayout::Photo) {
        return max;
    }
    let m = markup::cover_measure(p);
    let fonts: Vec<Vec<u8>> =
        t.fonts.iter().filter_map(|id| p.assets.get(id)).map(|a| a.bytes.clone()).collect();
    let world = world::OrdWorld::new(m.source, m.files, fonts, m.date);
    let Ok(doc) = typst::compile::<PagedDocument>(&world).output else { return max };
    let content: f64 = doc.pages().iter().map(|pg| pg.frame.height().to_mm()).sum();
    let room = f64::from(t.page_height_mm - t.margin_bottom_mm)
        - crate::render::PHOTO.gap_mm
        - content
        - crate::render::PHOTO.slack_mm;
    let band = room.clamp(f64::from(t.cover_image_min_mm), max);
    (band * 10.0).round() / 10.0
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
