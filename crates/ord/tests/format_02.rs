//! Format 0.2 (spec 0002): the cover photograph and logos, the cover rows, the running foot, tables without a
//! caption or a header, grouped headers, Markdown cells, column widths, JPEG images, font assets, the text
//! revision form, and the template values. Each check has a test that makes it fire.
//! Label: property. No pinned numbers.

mod common;

use std::io::Read as _;

use common::*;
use ord::render::{Format, prepare};

fn v02(body: serde_json::Value) -> serde_json::Value {
    let mut v = with_body(body);
    v["ord"] = "0.2".into();
    v
}

fn codes_of(v: &serde_json::Value, assets: &ord::Assets) -> Vec<(&'static str, String)> {
    let d = doc(v);
    ord::check(&d, assets, &ord::builtin_register()).into_iter().map(|f| (f.code, f.path)).collect()
}

fn table(extra: serde_json::Value) -> serde_json::Value {
    let mut t = serde_json::json!({
        "type": "table",
        "columns": [{ "title": "A" }, { "title": "B" }],
        "rows": [["1", "2"]]
    });
    for (k, v) in extra.as_object().unwrap() {
        t[k] = v.clone();
    }
    t
}

fn png() -> Vec<u8> {
    std::fs::read(repo().join("examples/assets/option-key.png")).unwrap()
}

fn jpeg() -> Vec<u8> {
    std::fs::read(repo().join("examples/assets/reach-photo.jpg")).unwrap()
}

fn photo() -> (ord::Document, ord::Assets) {
    load(&repo().join("examples/photo-cover-report.json"))
}

#[test]
fn the_photo_cover_example_raises_no_finding() {
    let (d, a) = photo();
    assert_eq!(ord::check(&d, &a, &ord::builtin_register()), Vec::<ord::Finding>::new());
}

#[test]
fn a_01_document_may_not_use_a_02_field() {
    let mut v = with_body(serde_json::json!([
        table(serde_json::json!({ "header": false })),
        { "type": "table", "columns": [{ "title": "A", "width": 0.5 }], "rows": [[{ "markdown": "*x*" }]] }
    ]));
    v["footer"] = "Foot".into();
    v["template"] = serde_json::json!({ "ink": "#000000", "cover_layout": "photo" });
    let got = codes_of(&v, &Default::default());
    for path in [
        "/footer",
        "/template/cover_layout",
        "/body/0/header",
        "/body/1/caption",
        "/body/1/columns/0/width",
        "/body/1/rows/0/0",
    ] {
        assert!(got.contains(&("ORD001", path.to_string())), "{path} not refused: {got:?}");
    }
    // A 0.1 field in the same template is not refused.
    assert!(!got.iter().any(|(_, p)| p == "/template/ink"), "{got:?}");
    // The same document as 0.2 has none of those findings.
    v["ord"] = "0.2".into();
    assert!(codes_of(&v, &Default::default()).iter().all(|(c, _)| *c != "ORD001"));
}

#[test]
fn a_table_with_no_caption_is_not_numbered_and_the_next_one_is_table_1() {
    let v = v02(serde_json::json!([
        table(serde_json::json!({})),
        table(serde_json::json!({ "caption": "Counted", "id": "t" })),
        prose("See [[#t]].")
    ]));
    let (r, f) = ord::resolve::resolve(&doc(&v), &ord::builtin_register());
    assert!(f.is_empty(), "{f:?}");
    let labels: Vec<_> = r
        .body
        .iter()
        .filter_map(|b| match b {
            ord::resolve::RBlock::Table(t) => Some(t.label.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, [None, Some("Table 1".to_string())]);
}

#[test]
fn column_groups_must_span_the_columns() {
    for groups in [
        serde_json::json!([{ "title": "G", "span": 1 }]),
        serde_json::json!([{ "title": "G", "span": 3 }]),
        serde_json::json!([{ "title": "G", "span": 0 }, { "title": "H", "span": 2 }]),
    ] {
        let v = v02(serde_json::json!([table(serde_json::json!({ "column_groups": groups }))]));
        assert!(codes_of(&v, &Default::default()).contains(&("ORD013", "/body/0/column_groups".into())));
    }
    let ok = v02(serde_json::json!([table(
        serde_json::json!({ "column_groups": [{ "title": "", "span": 1 }, { "title": "G", "span": 1 }] })
    )]));
    assert!(codes_of(&ok, &Default::default()).is_empty());
}

#[test]
fn a_markdown_cell_holds_one_paragraph() {
    let v = v02(serde_json::json!([{
        "type": "table", "columns": [{ "title": "A" }],
        "rows": [[{ "markdown": "- a list" }], [{ "markdown": "one\n\ntwo" }], [{ "markdown": "**bold** and a\\\nbreak" }]]
    }]));
    let got = codes_of(&v, &Default::default());
    assert!(got.contains(&("ORD012", "/body/0/rows/0/0/markdown".into())), "{got:?}");
    assert!(got.contains(&("ORD012", "/body/0/rows/1/0/markdown".into())), "{got:?}");
    assert!(!got.iter().any(|(_, p)| p.starts_with("/body/0/rows/2")), "{got:?}");
}

#[test]
fn column_widths_are_checked_and_kept() {
    let bad = v02(
        serde_json::json!([{ "type": "table", "columns": [{ "title": "A", "width": 1.5 }], "rows": [["x"]] }]),
    );
    assert!(codes_of(&bad, &Default::default()).contains(&("ORD020", "/body/0/columns/0/width".into())));
    let over = v02(serde_json::json!([{
        "type": "table", "columns": [{ "title": "A", "width": 0.7 }, { "title": "B", "width": 0.6 }], "rows": [["x", "y"]]
    }]));
    assert!(codes_of(&over, &Default::default()).contains(&("ORD020", "/body/0/columns".into())));

    let (d, a) = photo();
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let mut seen = 0;
    for b in &p.doc.body {
        if let ord::resolve::RBlock::Section(s) = b {
            for c in &s.body {
                if let ord::resolve::RBlock::Table(t) = c {
                    let w = p.column_widths(t).unwrap();
                    let sum: f64 = w.iter().sum();
                    assert!((sum - 1.0).abs() < 0.01, "{w:?}");
                    for (col, share) in t.columns.iter().zip(&w) {
                        if let Some(given) = col.width {
                            assert!((given - share).abs() < 1e-9);
                        }
                    }
                    seen += 1;
                }
            }
        }
    }
    assert!(seen >= 2);
}

#[test]
fn the_cover_photograph_and_logos_must_be_images_with_alt_text() {
    let mut v = v02(serde_json::json!([prose("Hello.")]));
    v["cover_image"] = serde_json::json!({ "asset": "missing", "alt": " " });
    v["cover_logos"] = serde_json::json!([{ "asset": "font", "name": "N" }]);
    let mut assets = ord::Assets::new();
    assets.insert(
        "font".into(),
        ord::assets::Asset { media_type: ord::model::MediaType::Font, bytes: b"OTTO".to_vec() },
    );
    let got = codes_of(&v, &assets);
    assert!(got.contains(&("ORD011", "/cover_image/asset".into())), "{got:?}");
    assert!(got.contains(&("ORD014", "/cover_image/alt".into())), "{got:?}");
    assert!(got.contains(&("ORD011", "/cover_logos/0/asset".into())), "{got:?}");
}

#[test]
fn jpeg_and_font_assets_are_read_by_their_bytes() {
    use ord::assets::{Asset, jpeg_size, validate};
    use ord::model::MediaType;
    assert_eq!(jpeg_size(&jpeg()), Ok((1600, 600)));
    assert!(jpeg_size(&png()).is_err());
    assert!(validate(&Asset { media_type: MediaType::Jpeg, bytes: png() }).is_err());
    let font = std::fs::read(repo().join("crates/ord/fonts/PublicSans-Regular.ttf")).unwrap();
    assert!(validate(&Asset { media_type: MediaType::Font, bytes: font }).is_ok());
    assert!(validate(&Asset { media_type: MediaType::Font, bytes: png() }).is_err());
}

#[test]
fn a_font_family_that_is_neither_bundled_nor_given_refuses_the_pdf() {
    let mut v = v02(serde_json::json!([prose("Hello.")]));
    v["template"] = serde_json::json!({ "body_font": "No Such Sans" });
    let err = ord::render(&doc(&v), &Default::default(), &ord::builtin_register(), Format::Pdf).unwrap_err();
    assert_eq!((err[0].code, err[0].path.as_str()), ("ORD022", "/template/body_font"));
    // Word names the family and does not need its file.
    assert!(ord::render(&doc(&v), &Default::default(), &ord::builtin_register(), Format::Docx).is_ok());
}

#[test]
fn a_text_revision_fits_a_lifecycle_that_asks_for_it() {
    use ord::register::RevisionForm;
    assert!(RevisionForm::Text.fits("0.2") && RevisionForm::Text.fits("P01"));
    assert!(
        !RevisionForm::Text.fits("") && !RevisionForm::Text.fits("0 2") && !RevisionForm::Text.fits(" 1")
    );
}

#[test]
fn dotted_numbers_print_a_point_after_a_top_level_number_only() {
    use ord::model::HeadingNumbers::{Dotted, Spaced};
    use ord::resolve::heading_text_as;
    assert_eq!(heading_text_as(Dotted, Some("3"), "Close"), "3. Close");
    assert_eq!(heading_text_as(Dotted, Some("2.1"), "Bank works"), "2.1 Bank works");
    assert_eq!(heading_text_as(Spaced, Some("2.1"), "Bank works"), "2.1  Bank works");
    assert_eq!(heading_text_as(Dotted, None, "Notes"), "Notes");
}

fn docx_part(bytes: &[u8], name: &str) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    zip.by_name(name).ok()?.read_to_string(&mut s).unwrap();
    Some(s)
}

#[test]
fn the_word_furniture_is_plain_text_and_page_fields_only() {
    // A Word master must open with no field error: the running foot and the cover foot are text, and
    // the only fields are PAGE and NUMPAGES. No DOCPROPERTY field can be left unresolved.
    let (d, a) = photo();
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    let foot = docx_part(&docx, "word/footer1.xml").unwrap();
    let cover = docx_part(&docx, "word/footer2.xml").expect("the photo cover has its own foot");
    let body = docx_part(&docx, "word/document.xml").unwrap();
    for x in [&foot, &cover, &body] {
        assert!(!x.contains("DOCPROPERTY"), "a DOCPROPERTY field");
        for instr in x.split("w:instr=\"").skip(1) {
            let field = instr.split('"').next().unwrap().trim();
            assert!(["PAGE", "NUMPAGES"].contains(&field), "field {field:?}");
        }
    }
    assert!(foot.contains("Northbank Reach Steering Group · Meeting 4"));
    assert!(foot.contains("INTERNAL"));
    assert!(cover.contains("RPT-0107  Rev B"));
    assert!(docx_part(&docx, "word/header1.xml").is_some());
    // No running head: the body section names no header.
    assert!(!body.contains("w:headerReference"));
}

#[test]
fn the_photo_is_cropped_to_fill_its_band_in_word() {
    let (d, a) = photo();
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    let body = docx_part(&docx, "word/document.xml").unwrap();
    // A 1600 x 600 photograph in a 210 x 75 mm band: a little is cut from the top and bottom.
    assert!(body.contains("<a:srcRect l=\"0\" t=\""), "no crop");
    assert!(docx_part(&docx, "[Content_Types].xml").unwrap().contains("Extension=\"jpeg\""));
}

#[test]
fn word_asks_to_update_fields_only_for_a_table_of_contents() {
    // Word answers `updateFields` with a modal question on every open. A document with no contents list
    // has nothing to update (PAGE and NUMPAGES update anyway), so it must open without asking.
    let (d, a) = photo();
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    assert!(!docx_part(&docx, "word/settings.xml").unwrap().contains("updateFields"));
    let (d, a) = example();
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    assert!(docx_part(&docx, "word/settings.xml").unwrap().contains("<w:updateFields w:val=\"true\"/>"));
}
