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
    // A 1600 x 600 photograph in a band of another shape: it is cut to fill, not stretched.
    assert!(body.contains("<a:srcRect "), "no crop");
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

fn photo_with(extra: serde_json::Value) -> (ord::Document, ord::Assets) {
    let path = repo().join("examples/photo-cover-report.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (k, x) in extra.as_object().unwrap() {
        v["template"][k] = x.clone();
    }
    // Asset paths are relative to the example's folder: write the copy there, under a name of its own
    // (tests run at once in one process).
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let copy = path.with_file_name(format!("ord-photo-{}-{n}.json", std::process::id()));
    std::fs::write(&copy, v.to_string()).unwrap();
    let out = load(&copy);
    std::fs::remove_file(&copy).unwrap();
    out
}

#[test]
fn a_filling_photograph_takes_what_the_content_leaves_the_same_in_word() {
    let (d, a) = photo_with(serde_json::json!({
        "cover_image_fill": true, "cover_image_min_mm": 30, "cover_image_height_mm": 200,
        "cover_image_rule_pt": 2.25, "cover_title_pt": 29, "cover_panel_ground": "#1a2833"
    }));
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let band = ord::write::pdf::band_mm(&p);
    assert!(band > 30.0 && band < 200.0, "{band}");
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    let body = docx_part(&docx, "word/document.xml").unwrap();
    // The photograph's anchor in Word is the band the PDF uses (to the rounding of EMUs).
    let photo = body.split("name=\"Photograph\"").next().unwrap();
    let cy: f64 = photo
        .rsplit("<wp:extent cx=\"")
        .next()
        .unwrap()
        .split("cy=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!((cy / 36_000.0 - band).abs() < 0.05, "Word {} mm, PDF {band} mm", cy / 36_000.0);
    // A rule along the photograph's foot replaces the short rule above the title.
    assert!(body.contains("name=\"Rule\""));
    assert!(ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).is_ok());
}

#[test]
fn table_padding_stripes_and_line_height_reach_every_writer() {
    let (d, a) = photo_with(serde_json::json!({
        "table_stripe": "#f7f4ee", "table_padding_mm": [1.3, 2.1], "table_size_pt": 9.1,
        "headless_padding_mm": [1.6, 2.6], "headless_size_pt": 9.4,
        "line_height": 1.5, "paragraph_spacing_pt": 5.25, "list_indent_mm": 5.8
    }));
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    let body = docx_part(&docx, "word/document.xml").unwrap();
    let styles = docx_part(&docx, "word/styles.xml").unwrap();
    assert!(body.contains("w:fill=\"F7F4EE\""), "no stripe");
    assert!(
        body.contains(&format!("<w:top w:w=\"{}\" w:type=\"dxa\"/>", (1.3f32 * 56.692_913).round() as u32))
    );
    assert!(
        styles.contains(&format!(
            "w:line=\"{}\" w:lineRule=\"atLeast\"",
            (1.5f32 * 10.5 * 20.0).round() as u32
        ))
    );
    assert!(styles.contains("TablePlain") && styles.contains("TableLabel"));
    let html = String::from_utf8(ord::render(&d, &a, &ord::builtin_register(), Format::Html).unwrap().bytes)
        .unwrap();
    assert!(html.contains("tbody tr:nth-child(even) { background: #f7f4ee; }"));
    assert!(html.contains("body { line-height: 1.5; }"));
    assert!(ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).is_ok());
}
