//! A font asset answers to the family name in its own name table (spec 0002 §5), even when that name holds a
//! width or weight word, such as "Semi Condensed". The typesetter files such a font under a shorter family
//! ("Fixture Sans"); the template, an SVG figure's text and the HTML must still find it by its full name.
//! The fixture is Public Sans (SIL OFL 1.1, no Reserved Font Name), cut to ASCII and renamed
//! "Fixture Sans Semi Condensed" (`tests/fixtures/FixtureSansSemiCondensed-Regular.ttf`).
//! Label: property. No pinned numbers.

mod common;

use common::*;
use lopdf::Document as Pdf;
use ord::model::MediaType;
use ord::render::Format;
use ord::{Asset, Assets};

const FULL: &str = "Fixture Sans Semi Condensed";
const PS_NAME: &str = "FixtureSansSemiCondensed-Regular";

fn fixture_font() -> Vec<u8> {
    std::fs::read(repo().join("crates/ord/tests/fixtures/FixtureSansSemiCondensed-Regular.ttf")).unwrap()
}

fn svg_with_text(family: &str) -> Vec<u8> {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40" viewBox="0 0 120 40"><text x="4" y="28" font-size="20" font-family="'{family}', sans-serif">Drawn text</text></svg>"#
    )
    .into_bytes()
}

/// A 0.2 document with the fixture as a font asset, the given template and body, and an SVG figure asset.
fn document(template: serde_json::Value, body: serde_json::Value) -> (ord::Document, Assets) {
    let mut v = with_body(body);
    v["ord"] = "0.2".into();
    let mut t = template;
    t["fonts"] = serde_json::json!(["fixture"]);
    v["template"] = t;
    let mut assets = Assets::new();
    assets.insert("fixture".into(), Asset { media_type: MediaType::Font, bytes: fixture_font() });
    assets.insert("drawing".into(), Asset { media_type: MediaType::Svg, bytes: svg_with_text(FULL) });
    (doc(&v), assets)
}

fn render(d: &ord::Document, a: &Assets, f: Format) -> Vec<u8> {
    match ord::render(d, a, &ord::builtin_register(), f) {
        Ok(o) => o.bytes,
        Err(e) => panic!("the render refused: {e:?}"),
    }
}

/// The base names of every font the PDF embeds.
fn pdf_fonts(pdf: &[u8]) -> Vec<String> {
    let d = Pdf::load_mem(pdf).unwrap();
    d.objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"Font".as_slice()))
        .filter_map(|d| d.get(b"BaseFont").and_then(|n| n.as_name()).ok())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .collect()
}

fn embeds_fixture(pdf: &[u8]) -> bool {
    pdf_fonts(pdf).iter().any(|n| n.ends_with(PS_NAME))
}

#[test]
fn the_template_may_name_a_font_asset_by_its_full_family() {
    let (d, a) = document(serde_json::json!({ "body_font": FULL }), serde_json::json!([prose("Body text.")]));
    let pdf = render(&d, &a, Format::Pdf);
    assert!(embeds_fixture(&pdf), "the body is not set in {FULL}: {:?}", pdf_fonts(&pdf));
}

#[test]
fn the_shorter_family_still_works() {
    let (d, a) =
        document(serde_json::json!({ "body_font": "Fixture Sans" }), serde_json::json!([prose("Body.")]));
    assert!(embeds_fixture(&render(&d, &a, Format::Pdf)));
}

#[test]
fn svg_text_that_names_the_full_family_is_set_in_that_font() {
    // The body is in a bundled font, so the fixture can only come from the figure.
    let figure = serde_json::json!({ "type": "figure", "caption": "A drawing", "asset": "drawing", "alt": "A drawing" });
    let (d, a) = document(serde_json::json!({}), serde_json::json!([figure]));
    let pdf = render(&d, &a, Format::Pdf);
    assert!(embeds_fixture(&pdf), "the drawing's text fell back to another font: {:?}", pdf_fonts(&pdf));
}

#[test]
fn the_html_declares_the_font_under_its_full_family() {
    let (d, a) = document(serde_json::json!({ "body_font": FULL }), serde_json::json!([prose("Body text.")]));
    let html = String::from_utf8(render(&d, &a, Format::Html)).unwrap();
    assert!(html.contains(&format!("@font-face {{ font-family: \"{FULL}\";")), "no @font-face for {FULL}");
}
