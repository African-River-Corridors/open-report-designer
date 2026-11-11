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
    v["cover_notice"] = "Small print.".into();
    v["template"] = serde_json::json!({ "ink": "#000000", "cover_layout": "photo" });
    let got = codes_of(&v, &Default::default());
    for path in [
        "/footer",
        "/cover_notice",
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

#[test]
fn a_shaded_table_cell_pads_with_paragraph_spacing_not_a_cell_margin() {
    // Word for Mac at 170% drew white ticks at the column edges under a shaded header row, where each
    // cell's bottom margin ended. A shaded cell (header ground or stripe) now has no top or bottom margin
    // of its own; the table's vertical padding is its paragraph's spacing. Unshaded cells keep the margin.
    let (d, a) = photo_with(serde_json::json!({ "table_stripe": "#f7f4ee", "table_padding_mm": [1.3, 2.1] }));
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    let body = docx_part(&docx, "word/document.xml").unwrap();
    let top = (1.3f32 * 56.692_913).round() as u32;
    let spacing = format!("<w:spacing w:before=\"{top}\" w:after=\"{}\"/>", top - 10);
    let zero = "<w:tcMar><w:top w:w=\"0\" w:type=\"dxa\"/><w:bottom w:w=\"0\" w:type=\"dxa\"/></w:tcMar>";
    let (mut shaded, mut plain) = (0, 0);
    for tbl in body
        .split("<w:tbl>")
        .skip(1)
        .filter(|t| t.starts_with("<w:tblPr><w:tblStyle w:val=\"ReportTable\"/>"))
    {
        if !tbl.contains("<w:tblHeader/>") {
            continue; // the document-control table and headless tables are out of scope
        }
        for tc in tbl.split("<w:tc>").skip(1) {
            if tc.contains("<w:shd ") {
                shaded += 1;
                assert!(tc.contains(zero), "a shaded cell keeps a vertical margin: {tc:.200}");
                assert!(tc.contains(&spacing), "a shaded cell lost its padding: {tc:.300}");
            } else {
                plain += 1;
                assert!(!tc.contains("<w:tcMar>"), "an unshaded cell has its own margin");
            }
        }
    }
    assert!(shaded > 0 && plain > 0, "the example has shaded and unshaded cells ({shaded}, {plain})");
}

/// The `<w:tbl>` that holds `marker`, from its opening tag to its close.
fn table_holding<'a>(xml: &'a str, marker: &str) -> &'a str {
    let at = xml.find(marker).expect("marker");
    let start = xml[..at].rfind("<w:tbl>").expect("a table before it");
    let end = at + xml[at..].find("</w:tbl>").expect("its close");
    &xml[start..end]
}

fn twips_of(xml: &str, tag: &str) -> Vec<u32> {
    xml.split(tag).skip(1).map(|s| s.split('"').next().unwrap().parse().unwrap()).collect()
}

#[test]
fn the_logo_strip_columns_fill_the_table_width_exactly_in_word() {
    // Columns of `width / n` lost the remainder: three columns of 3363 in a table of 10091. Word squares
    // the grid with the table on its own, and drew the dividers and the edge unevenly.
    for n in 1..=7 {
        let (mut d, a) = photo();
        let first = d.cover_logos[0].clone();
        d.cover_logos = vec![first; n];
        let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
        let body = docx_part(&docx, "word/document.xml").unwrap();
        let strip = table_holding(&body, "<w:pStyle w:val=\"LogoName\"/>");
        let width = twips_of(strip, "<w:tblW w:w=\"")[0];
        let grid = twips_of(strip, "<w:gridCol w:w=\"");
        let cells = twips_of(strip, "<w:tcW w:w=\"");
        assert_eq!(grid.len(), n);
        assert_eq!(grid.iter().sum::<u32>(), width, "{n} logos: grid {grid:?}, table {width}");
        assert_eq!(cells, grid, "{n} logos: each cell is as wide as its column");
    }
}

#[test]
fn the_word_foot_draws_one_rule_not_one_per_cell() {
    // The Footer style carries a top rule (for the 0.1 foot). The grid foot's table draws the rule itself,
    // so its cells must switch the style's rule off: an empty <w:pBdr/> does not, and Word drew two lines.
    let (d, a) = photo();
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    let foot = docx_part(&docx, "word/footer1.xml").unwrap();
    assert!(foot.contains("<w:tbl>"), "the 0.2 grid foot");
    assert!(!foot.contains("<w:pBdr/>"), "an empty pBdr leaves the style's rule on");
    let cells = foot.matches("<w:pStyle w:val=\"Footer\"/>").count();
    assert_eq!(cells, 3);
    assert_eq!(foot.matches("<w:pBdr><w:top w:val=\"nil\"/></w:pBdr>").count(), cells);
}

#[test]
fn a_cover_logo_stands_on_a_line_that_can_grow_so_word_does_not_cut_its_top() {
    // An inline picture stands on the baseline. A line of an exact height keeps room under the baseline for
    // the font's descent, so a logo nearly as high as the line lost its top in Word and in LibreOffice.
    let (d, a) = photo();
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    let body = docx_part(&docx, "word/document.xml").unwrap();
    let strip = table_holding(&body, "<w:pStyle w:val=\"LogoName\"/>");
    let lines: Vec<&str> = strip.split("<w:p>").filter(|p| p.contains("<wp:inline ")).collect();
    assert_eq!(lines.len(), d.cover_logos.len(), "one picture paragraph for each logo");
    for p in lines {
        assert!(p.contains("w:lineRule=\"atLeast\""), "the logo's line must be able to grow: {p}");
        assert!(!p.contains("w:lineRule=\"exact\""), "an exact line cuts the logo's top: {p}");
    }
}

#[test]
fn the_logo_strip_pads_its_foot_with_paragraph_spacing_not_a_cell_margin() {
    // Word drew a dark line on screen across the strip where the cells' text ended and their bottom margin
    // began (its PDF export did not). The space under the text is now the last paragraph's spacing after.
    for with_role in [true, false] {
        let (mut d, a) = photo();
        if !with_role {
            for l in &mut d.cover_logos {
                l.role = None;
            }
        }
        let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
        let body = docx_part(&docx, "word/document.xml").unwrap();
        let strip = table_holding(&body, "<w:pStyle w:val=\"LogoName\"/>");
        let top = twips_of(strip, "<w:top w:w=\"")[0];
        assert!(top > 0);
        assert_eq!(twips_of(strip, "<w:bottom w:w=\""), vec![0], "no bottom cell margin");
        let n = d.cover_logos.len();
        let style = if with_role { "LogoRole" } else { "LogoName" };
        let tail = format!("<w:pStyle w:val=\"{style}\"/><w:spacing w:after=\"{top}\"/>");
        assert_eq!(strip.matches(&tail).count(), n, "each cell's last paragraph carries the pad");
    }
}

// --- The cover notice ----------------------------------------------------------------------------------

const NOTICE: &str = "Shared as is, for discussion only.\n\nNot an offer of any security. The Example \
Company makes no representation or warranty as to the accuracy of this document, and accepts no liability \
for any loss arising from its use. Each recipient must take its own advice. Forward-looking statements rest \
on assumptions and are not guarantees; actual results may differ materially from them.";

fn pdf_pages(bytes: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(bytes).expect("a PDF");
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    pages
        .iter()
        .map(|n| doc.extract_text(&[*n]).unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

fn with_notice(mut d: ord::Document) -> ord::Document {
    d.cover_notice = Some(NOTICE.into());
    d
}

fn screening() -> (ord::Document, ord::Assets) {
    let (mut d, a) = load(&repo().join("examples/screening-report.json"));
    d.ord = "0.2".into();
    (d, a)
}

#[test]
fn the_cover_notice_prints_on_the_cover_page_only_in_the_pdf() {
    for (name, (d, a)) in [("photo", photo()), ("plain", screening())] {
        let without = ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
        let d = with_notice(d);
        let pdf = ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
        let pages = pdf_pages(&pdf);
        assert!(pages[0].contains("Shared as is, for discussion only."), "{name}: cover: {}", pages[0]);
        assert!(pages[0].contains("actual results may differ materially from them."), "{name}");
        for (i, pg) in pages.iter().enumerate().skip(1) {
            assert!(!pg.contains("Shared as is"), "{name}: page {} holds the notice: {pg}", i + 1);
        }
        // The notice takes no page of its own: the cover is still one page.
        assert_eq!(pages.len(), pdf_pages(&without).len(), "{name}: page count");
    }
}

#[test]
fn the_cover_notice_follows_the_control_block_at_the_foot_of_the_photo_cover() {
    let d = with_notice(photo().0);
    let a = photo().1;
    let pdf = ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let (_, last_row) = p.cover_rows().pop().expect("a control block");
    let cover = &pdf_pages(&pdf)[0];
    let control = cover.find(&last_row).expect("the control block's last row");
    let notice = cover.find("Shared as is").expect("the notice");
    assert!(notice > control, "the notice comes after the control block: {cover}");
}

#[test]
fn a_long_cover_notice_makes_the_photograph_shorter_not_the_cover_longer() {
    let (d, a) = photo_with(serde_json::json!({
        "cover_image_fill": true, "cover_image_min_mm": 30, "cover_image_height_mm": 200
    }));
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let before = ord::write::pdf::band_mm(&p);
    let d2 = with_notice(d.clone());
    let (p2, _) = prepare(&d2, &a, &ord::builtin_register()).unwrap();
    let after = ord::write::pdf::band_mm(&p2);
    assert!(after < before - 10.0, "the band makes room: {before} mm then {after} mm");
    assert!(after > 30.0, "not clamped: {after} mm");
    let pdf = ord::render(&d2, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
    let without = ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
    assert_eq!(pdf_pages(&pdf).len(), pdf_pages(&without).len());
}

#[test]
fn the_cover_notice_is_in_no_running_foot() {
    for (name, (d, a)) in [("photo", photo()), ("plain", screening())] {
        let d = with_notice(d);
        let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
        let body = docx_part(&docx, "word/document.xml").unwrap();
        assert_eq!(body.matches("Shared as is").count(), 1, "{name}: once, on the cover");
        assert!(body.contains("<w:pStyle w:val=\"CoverNotice\"/>"), "{name}");
        for part in ["word/footer1.xml", "word/footer2.xml", "word/header1.xml"] {
            if let Some(x) = docx_part(&docx, part) {
                assert!(!x.contains("Shared as is"), "{name}: {part} holds the notice");
            }
        }
        let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
        assert!(!p.footer_left().contains("Shared as is"), "{name}");
        let html =
            String::from_utf8(ord::render(&d, &a, &ord::builtin_register(), Format::Html).unwrap().bytes)
                .unwrap();
        assert_eq!(html.matches("Shared as is").count(), 1, "{name}: HTML, once");
        let cover_end = html.find("</header>").unwrap();
        assert!(html.find("Shared as is").unwrap() < cover_end, "{name}: HTML, in the cover");
    }
}

#[test]
fn a_document_with_no_cover_notice_has_no_cover_notice_style() {
    let (d, a) = photo();
    let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
    assert!(!docx_part(&docx, "word/styles.xml").unwrap().contains("CoverNotice"));
    let html = String::from_utf8(ord::render(&d, &a, &ord::builtin_register(), Format::Html).unwrap().bytes)
        .unwrap();
    assert!(!html.contains("cover-notice"));
}

fn uncounted(mut d: ord::Document) -> ord::Document {
    d.count_cover = Some(false);
    d
}

/// The "Page n of N" of a page's text, as numbers.
fn page_of(page: &str) -> Option<(usize, usize)> {
    let at = page.rfind("Page ")?;
    let mut w = page[at + 5..].split_whitespace();
    let n = w.next()?.parse().ok()?;
    (w.next()? == "of").then_some(())?;
    Some((n, w.next()?.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok()?))
}

#[test]
fn with_count_cover_false_the_page_after_the_cover_is_page_1_of_the_pages_after_it() {
    for (name, (d, a)) in [("photo", photo()), ("plain", screening())] {
        let pdf = ord::render(&uncounted(d), &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
        let pages = pdf_pages(&pdf);
        let n = pages.len();
        assert!(n >= 2, "{name}: {n} pages");
        assert_eq!(page_of(&pages[0]), None, "{name}: the cover has no page number: {}", pages[0]);
        for (i, pg) in pages.iter().enumerate().skip(1) {
            assert_eq!(page_of(pg), Some((i, n - 1)), "{name}: page {} of the file: {pg}", i + 1);
        }
    }
}

#[test]
fn by_default_the_cover_counts_as_page_1() {
    for (name, (d, a)) in [("photo", photo()), ("plain", screening())] {
        let pdf = ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
        let pages = pdf_pages(&pdf);
        let n = pages.len();
        assert_eq!(page_of(&pages[0]), None, "{name}: the cover has no page number");
        assert_eq!(page_of(&pages[1]), Some((2, n)), "{name}");
        assert_eq!(page_of(&pages[n - 1]), Some((n, n)), "{name}");
        // `count_cover: true` is the default, byte for byte.
        let mut t = d.clone();
        t.count_cover = Some(true);
        let same = ord::render(&t, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
        assert_eq!(pdf_pages(&same), pages, "{name}");
    }
}

#[test]
fn the_contents_list_uses_the_same_page_numbers_as_the_foot() {
    let (d, a) = screening();
    let d = uncounted(ord::Document { contents: Some(true), ..d });
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let pdf = ord::render(&d, &a, &ord::builtin_register(), Format::Pdf).unwrap().bytes;
    let pages = pdf_pages(&pdf);
    let contents = pages.iter().position(|pg| pg.contains("Contents")).expect("a contents page");
    // Every entry's page number in the contents list is the foot number of the page its heading opens.
    let mut checked = 0;
    for e in &p.doc.toc {
        let entry = &pages[contents];
        let Some(at) = entry.find(&e.title) else { continue };
        let listed: usize = entry[at + e.title.len()..]
            .split_whitespace()
            .find_map(|w| w.trim_matches('.').parse().ok())
            .expect("a page number");
        let opens = pages.iter().enumerate().skip(contents + 1).find(|(_, pg)| pg.contains(&e.title));
        let (_, pg) = opens.expect("the heading's page");
        assert_eq!(page_of(pg).map(|(n, _)| n), Some(listed), "{}", e.title);
        checked += 1;
    }
    assert!(checked > 0, "no contents entry checked: {}", pages[contents]);
}

#[test]
fn with_count_cover_false_word_numbers_the_body_section_from_1() {
    for (name, (d, a)) in [("photo", photo()), ("plain", screening())] {
        let docx =
            ord::render(&uncounted(d.clone()), &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
        let body = docx_part(&docx, "word/document.xml").unwrap();
        // Two sections: the cover, then the body, which starts again at 1.
        let sects: Vec<&str> = body.split("<w:sectPr>").skip(1).collect();
        assert_eq!(sects.len(), 2, "{name}");
        assert!(!sects[0].contains("w:pgNumType"), "{name}: the cover section");
        assert!(sects[1].contains("<w:pgNumType w:start=\"1\"/>"), "{name}: the body section");
        let foot = docx_part(&docx, "word/footer1.xml").unwrap();
        let instr: Vec<&str> =
            foot.split("w:instr=\"").skip(1).map(|s| s.split('"').next().unwrap().trim()).collect();
        assert_eq!(instr, ["PAGE", "SECTIONPAGES"], "{name}");
        // The default is unchanged.
        let docx = ord::render(&d, &a, &ord::builtin_register(), Format::Docx).unwrap().bytes;
        assert!(!docx_part(&docx, "word/document.xml").unwrap().contains("w:pgNumType"), "{name}");
        let foot = docx_part(&docx, "word/footer1.xml").unwrap();
        assert!(foot.contains(" NUMPAGES ") && !foot.contains("SECTIONPAGES"), "{name}");
    }
}

#[test]
fn without_a_cover_count_cover_changes_nothing() {
    let (d, a) = screening();
    let mut t = d.template.clone().unwrap_or_default();
    t.cover = Some(false);
    let d = ord::Document { template: Some(t), ..d };
    for f in [Format::Pdf, Format::Docx] {
        let a1 = ord::render(&d, &a, &ord::builtin_register(), f).unwrap().bytes;
        let a2 = ord::render(&uncounted(d.clone()), &a, &ord::builtin_register(), f).unwrap().bytes;
        if f == Format::Pdf {
            assert_eq!(pdf_pages(&a1), pdf_pages(&a2));
        } else {
            assert_eq!(docx_part(&a1, "word/document.xml"), docx_part(&a2, "word/document.xml"));
            assert_eq!(docx_part(&a1, "word/footer1.xml"), docx_part(&a2, "word/footer1.xml"));
        }
    }
}
