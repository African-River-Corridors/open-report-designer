//! The HTML writer and `render` (spec 0001 §10 HTML, §11.1). Read back with an XML reader, not the writer.
//! Label: property. No pinned numbers.

mod common;
use common::*;
use ord::Format;
use serde_json::json;

fn html(d: &ord::Document, a: &ord::Assets) -> String {
    String::from_utf8(ord::render(d, a, &ord::builtin_register(), Format::Html).unwrap().bytes).unwrap()
}

/// The text of every element named `tags`, in document order, whitespace collapsed.
fn texts(xml: &str, tags: &[&str]) -> Vec<String> {
    let body = xml.trim_start_matches("<!doctype html>\n");
    let doc = roxmltree::Document::parse(body).expect("the HTML is well-formed XML");
    doc.descendants()
        .filter(|n| n.is_element() && tags.contains(&n.tag_name().name()))
        .map(|n| n.descendants().filter(|d| d.is_text()).map(|d| d.text().unwrap()).collect::<String>())
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

#[test]
fn the_example_renders_as_one_well_formed_self_contained_file() {
    let (d, a) = example();
    let h = html(&d, &a);
    assert!(h.starts_with("<!doctype html>\n<html lang=\"en\">"));
    roxmltree::Document::parse(h.trim_start_matches("<!doctype html>\n")).unwrap();
    // Self-contained: no reference to anything outside the file.
    for bad in ["src=\"http", "href=\"http://", "@import", "url(http"] {
        assert!(!h.contains(bad), "{bad}");
    }
    assert_eq!(h.matches("@font-face").count(), ord::fonts::bundled().len());
    assert_eq!(h.matches("src=\"data:image/svg+xml;base64,").count(), 2);
    assert_eq!(h.matches("src=\"data:image/png;base64,").count(), 1);
    assert!(h.contains("@page { size: 210mm 297mm; margin: 25mm 20mm 25mm 20mm; }"));
}

#[test]
fn headings_carry_the_engines_numbers_in_order() {
    let (d, a) = example();
    let h = html(&d, &a);
    assert_eq!(
        texts(&h, &["h1", "h2", "h3", "h4", "h5"]),
        [
            "Heron River navigation screening",
            "Document control",
            "Revision history",
            "Contents",
            "1 Introduction",
            "1.1 Scope",
            "2 Method",
            "3 Results",
            "4 Data and limits",
            "Appendix A Rule trace",
            "Appendix B Run record",
        ]
    );
}

#[test]
fn captions_cells_cross_references_and_the_data_box_are_printed() {
    let (d, a) = example();
    let h = html(&d, &a);
    assert_eq!(
        texts(&h, &["caption"]),
        ["Table 1: The three options, screened", "Table A.1: Checks of option 2"]
    );
    assert_eq!(
        texts(&h, &["figcaption"]),
        [
            "Figure 1: The reach and the design line of option 2",
            "Figure 2: Bed level and the design channel bottom along the reach",
            "Figure 3: The colours of the six option stages"
        ]
    );
    assert!(texts(&h, &["th"]).contains(&"Length (km)".to_string()));
    assert!(texts(&h, &["td"]).contains(&"298 000".to_string()));
    let xrefs = texts(&h, &["a"]);
    for x in ["Section 2", "Section 3", "Section 4", "Figure 1", "Figure 2", "Table 1"] {
        assert!(xrefs.contains(&x.to_string()), "{x} in {xrefs:?}");
    }
    assert_eq!(texts(&h, &["dt"])[..4], ["Document", "Revision", "Date", "Status"]);
    assert!(h.contains("<p class=\"data-box-title\">Data and limits</p>"));
    assert!(
        h.contains("Coordinate system: A local grid (invented). Scale: 1:50 000. Source: Invented terrain")
    );
}

#[test]
fn every_special_character_reads_back_unchanged() {
    let nasty = "$5 and $10, US$ 3, <b>&amp; \"q\" 'a' # * _ ` [x] @y ~ = - + / \\";
    // Markdown would read some of these as markup, so they go in a table cell and a caption, which are plain.
    let v = with_body(json!([
        { "type": "table", "caption": nasty, "columns": [{ "title": nasty }], "rows": [[nasty]] },
        prose("Costs: $5 and $10 \\* 3 < 4 & \"so\" `<code>`")
    ]));
    let h = html(&doc(&v), &Default::default());
    assert_eq!(texts(&h, &["th"]).last().unwrap(), &nasty.split_whitespace().collect::<Vec<_>>().join(" "));
    assert_eq!(texts(&h, &["td"]).last().unwrap(), &nasty.split_whitespace().collect::<Vec<_>>().join(" "));
    assert_eq!(texts(&h, &["caption"])[0], format!("Table 1: {nasty}"));
    assert!(texts(&h, &["p"]).contains(&"Costs: $5 and $10 * 3 < 4 & \"so\" <code>".to_string()));
}

#[test]
fn unsafe_links_print_as_text() {
    let v = with_body(json!([prose("[click](javascript:alert(1)) and [ok](https://example.org)")]));
    let h = html(&doc(&v), &Default::default());
    assert!(!h.contains("javascript:"));
    assert!(h.contains("<a href=\"https://example.org\">ok</a>"));
}

#[test]
fn rendering_twice_gives_equal_bytes() {
    let (d, a) = example();
    assert_eq!(html(&d, &a), html(&d, &a));
}

#[test]
fn an_error_refuses_the_render_and_warnings_travel_with_the_output() {
    let bad = with_body(json!([prose("# Heading in prose")]));
    let err =
        ord::render(&doc(&bad), &Default::default(), &ord::builtin_register(), Format::Html).unwrap_err();
    assert_eq!(err[0].code, "ORD012");
    let warn = with_body(json!([section("a", "Empty", json!([]))]));
    let out = ord::render(&doc(&warn), &Default::default(), &ord::builtin_register(), Format::Html).unwrap();
    assert_eq!(out.warnings[0].code, "ORD018");
    let mut colour = with_body(json!([prose("x")]));
    colour["template"] = json!({ "ink": "red" });
    let err =
        ord::render(&doc(&colour), &Default::default(), &ord::builtin_register(), Format::Html).unwrap_err();
    assert_eq!((err[0].code, err[0].path.as_str()), ("ORD001", "/template/ink"));
}

#[test]
fn a_draft_prints_its_furniture_and_no_cover_prints_the_title() {
    let mut v = with_body(json!([prose("x")]));
    v["template"] = json!({ "cover": false, "ink": "#000000" });
    let h = html(&doc(&v), &Default::default());
    assert!(h.contains("<span class=\"furniture\">Draft</span>"));
    assert!(!h.contains("class=\"cover\""));
    assert!(h.contains("<h1 class=\"doc-title\">T</h1>"));
    assert!(h.contains("--ink: #000000"));
}
