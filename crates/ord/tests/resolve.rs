//! Resolution, prose and the structural checks (spec 0001 §5, §6, §8).
//! Label: property. The expected numbers follow from the numbering rules of §5, worked by hand per case.

mod common;
use common::*;
use ord::markdown::{Inline, MdBlock, read};
use ord::resolve::{RBlock, resolve};
use serde_json::json;

fn findings(v: &serde_json::Value) -> Vec<ord::Finding> {
    ord::check(&doc(v), &Default::default(), &ord::builtin_register())
}

fn codes(v: &serde_json::Value) -> Vec<&'static str> {
    findings(v).into_iter().map(|f| f.code).collect()
}

fn xref_texts(blocks: &[MdBlock]) -> Vec<String> {
    let mut out = Vec::new();
    let mut b = blocks.to_vec();
    ord::markdown::for_each_xref(&mut b, &mut |_, t, _| out.push(t.clone()));
    out
}

fn sections(blocks: &[RBlock]) -> Vec<(u8, Option<String>, String)> {
    let mut out = Vec::new();
    for b in blocks {
        if let RBlock::Section(s) = b {
            out.push((s.level, s.number.clone(), s.title.clone()));
            out.extend(sections(&s.body));
        }
    }
    out
}

#[test]
fn the_example_resolves_with_no_finding() {
    let (d, assets) = example();
    let f = ord::check(&d, &assets, &ord::builtin_register());
    assert!(f.is_empty(), "{f:#?}");
}

#[test]
fn sections_are_numbered_and_unnumbered_ones_take_no_number_nor_do_their_children() {
    let v = with_body(json!([
        section("a", "One", json!([section("a1", "One one", json!([prose("x")])), section("a2", "One two", json!([prose("x")]))])),
        { "type": "section", "title": "Unnumbered", "numbered": false, "body": [ section("u1", "Child", json!([prose("x")])) ] },
        section("b", "Two", json!([section("b1", "Two one", json!([section("b11", "Deep", json!([prose("x")]))]))]))
    ]));
    let (r, f) = resolve(&doc(&v), &ord::builtin_register());
    assert!(f.is_empty(), "{f:?}");
    let s = |l, n: Option<&str>, t: &str| (l, n.map(String::from), t.to_string());
    assert_eq!(
        sections(&r.body),
        [
            s(1, Some("1"), "One"),
            s(2, Some("1.1"), "One one"),
            s(2, Some("1.2"), "One two"),
            s(1, None, "Unnumbered"),
            s(2, None, "Child"),
            s(1, Some("2"), "Two"),
            s(2, Some("2.1"), "Two one"),
            s(3, Some("2.1.1"), "Deep"),
        ]
    );
}

#[test]
fn appendices_tables_and_figures_are_numbered_with_the_letter_in_appendices() {
    let (d, assets) = example();
    let (r, _) = resolve(&d, &ord::builtin_register());
    let _ = assets;
    let mut labels = Vec::new();
    fn walk(b: &[RBlock], out: &mut Vec<String>) {
        for x in b {
            match x {
                RBlock::Section(s) => walk(&s.body, out),
                RBlock::Table(t) => out.extend(t.label.clone()),
                RBlock::Figure(f) => out.push(f.label.clone()),
                _ => {}
            }
        }
    }
    walk(&r.body, &mut labels);
    for a in &r.appendices {
        walk(&a.body, &mut labels);
    }
    assert_eq!(labels, ["Figure 1", "Figure 2", "Table 1", "Figure 3", "Table A.1"]);
    let headings: Vec<_> = r.appendices.iter().map(|a| a.heading()).collect();
    assert_eq!(headings, ["Appendix A  Rule trace", "Appendix B  Run record"]);
}

#[test]
fn sections_inside_an_appendix_are_lettered() {
    let mut v = with_body(json!([prose("x")]));
    v["appendices"] = json!([
        { "title": "First", "body": [prose("x")] },
        { "title": "Second", "body": [section("s", "Part", json!([section("t", "Sub", json!([prose("x")]))]))] }
    ]);
    let (r, f) = resolve(&doc(&v), &ord::builtin_register());
    assert!(f.is_empty(), "{f:?}");
    let s = sections(&r.appendices[1].body);
    assert_eq!(s[0], (2, Some("B.1".to_string()), "Part".to_string()));
    assert_eq!(s[1], (3, Some("B.1.1".to_string()), "Sub".to_string()));
}

#[test]
fn cross_references_print_the_number_or_the_title() {
    let mut v = with_body(json!([
        section("a", "Alpha", json!([prose("See [[#b1]], [[#u]], [[#t]], [[#f]], [[#m]], [[#box]], [[#app]] and [[#apptable]].")])),
        section("b", "Beta", json!([section("b1", "Beta one", json!([prose("x")]))])),
        { "type": "section", "id": "u", "title": "Notes", "numbered": false, "body": [prose("x")] },
        { "type": "table", "id": "t", "caption": "C", "columns": [{ "title": "A" }], "rows": [["1"]] },
        { "type": "figure", "id": "f", "caption": "C", "asset": "x", "alt": "a" },
        { "type": "map", "id": "m", "caption": "C", "asset": "x", "alt": "a", "attribution": "s" },
        { "type": "data_box", "id": "box", "items": [] }
    ]));
    v["appendices"] = json!([{ "id": "app", "title": "Extra", "body": [
        { "type": "table", "id": "apptable", "caption": "C", "columns": [{ "title": "A" }], "rows": [["1"]] }
    ]}]);
    let (r, f) = resolve(&doc(&v), &ord::builtin_register());
    assert!(f.is_empty(), "{f:?}");
    let RBlock::Section(a) = &r.body[0] else { panic!() };
    let RBlock::Prose(p) = &a.body[0] else { panic!() };
    assert_eq!(
        xref_texts(p),
        [
            "Section 2.1",
            "Notes",
            "Table 1",
            "Figure 1",
            "Figure 2",
            "Data and limits",
            "Appendix A",
            "Table A.1"
        ]
    );
    let mut anchors = Vec::new();
    ord::markdown::for_each_xref(&mut p.clone(), &mut |_, _, a| anchors.push(a.clone()));
    assert_eq!(anchors[0], "b1");
}

#[test]
fn moving_a_block_renumbers_it_and_every_reference_to_it() {
    let table =
        json!({ "type": "table", "id": "t", "caption": "C", "columns": [{ "title": "A" }], "rows": [["1"]] });
    let other = json!({ "type": "table", "caption": "D", "columns": [{ "title": "A" }], "rows": [["1"]] });
    let before =
        with_body(json!([section("s", "S", json!([table.clone(), other.clone(), prose("See [[#t]].")]))]));
    let after = with_body(json!([section("s", "S", json!([other, table, prose("See [[#t]].")]))]));
    let text = |v: &serde_json::Value| {
        let (r, _) = resolve(&doc(v), &ord::builtin_register());
        let RBlock::Section(s) = &r.body[0] else { panic!() };
        let RBlock::Prose(p) = &s.body[2] else { panic!() };
        xref_texts(p)[0].clone()
    };
    assert_eq!(text(&before), "Table 1");
    assert_eq!(text(&after), "Table 2");
}

#[test]
fn the_contents_lists_three_levels_and_the_appendices() {
    let (d, _) = example();
    let (r, _) = resolve(&d, &ord::builtin_register());
    assert!(r.contents);
    let toc: Vec<_> =
        r.toc.iter().map(|e| (e.level, e.number.clone().unwrap_or_default(), e.title.clone())).collect();
    let e = |l, n: &str, t: &str| (l, n.to_string(), t.to_string());
    assert_eq!(
        toc,
        [
            e(1, "1", "Introduction"),
            e(2, "1.1", "Scope"),
            e(1, "2", "Method"),
            e(1, "3", "Results"),
            e(1, "4", "Data and limits"),
            e(1, "Appendix A", "Rule trace"),
            e(1, "Appendix B", "Run record"),
        ]
    );
    let deep = with_body(json!([section(
        "a",
        "A",
        json!([section(
            "b",
            "B",
            json!([section("c", "C", json!([section("d", "D", json!([prose("x")]))]))])
        )])
    )]));
    let (r, _) = resolve(&doc(&deep), &ord::builtin_register());
    assert_eq!(r.toc.iter().map(|e| e.level).max(), Some(3));
}

#[test]
fn a_dollar_is_always_literal() {
    for (md, text) in [
        ("$5 and $10", "$5 and $10"),
        ("US$ 3", "US$ 3"),
        ("$x$ is not maths", "$x$ is not maths"),
        ("$$ y $$", "$$ y $$"),
        ("costs \\$5", "costs $5"),
    ] {
        let p = read(md);
        assert_eq!(p.blocks, [MdBlock::Paragraph(vec![Inline::Text(text.into())])], "{md}");
    }
}

#[test]
fn prose_reads_into_the_shared_model() {
    let p = read(
        "A *b* **c** `d` [e](https://example.org)\\\nf\n\n- one\n- two\n  1. x\n\n> q\n\n```\ncode $\n```",
    );
    assert!(p.refused.is_empty());
    assert_eq!(p.blocks.len(), 4);
    let MdBlock::Paragraph(i) = &p.blocks[0] else { panic!() };
    assert_eq!(
        i,
        &[
            Inline::Text("A ".into()),
            Inline::Emph(vec![Inline::Text("b".into())]),
            Inline::Text(" ".into()),
            Inline::Strong(vec![Inline::Text("c".into())]),
            Inline::Text(" ".into()),
            Inline::Code("d".into()),
            Inline::Text(" ".into()),
            Inline::Link { url: "https://example.org".into(), children: vec![Inline::Text("e".into())] },
            Inline::LineBreak,
            Inline::Text("f".into()),
        ]
    );
    let MdBlock::List { ordered: false, items, .. } = &p.blocks[1] else { panic!() };
    assert!(matches!(items[1][1], MdBlock::List { ordered: true, start: 1, .. }));
    assert!(matches!(&p.blocks[2], MdBlock::Quote(_)));
    assert_eq!(p.blocks[3], MdBlock::Code("code $".into()));
}

#[test]
fn a_cross_reference_is_found_inside_emphasis_and_next_to_punctuation() {
    let p = read("(*see [[#a]]*), [[#b]].");
    let mut ids = Vec::new();
    ord::markdown::for_each_xref(&mut p.blocks.clone(), &mut |id, _, _| ids.push(id.to_string()));
    assert_eq!(ids, ["a", "b"]);
    let p = read("[[# a]] and [[#]] stay text");
    assert_eq!(p.blocks, [MdBlock::Paragraph(vec![Inline::Text("[[# a]] and [[#]] stay text".into())])]);
}

#[test]
fn ord009_duplicate_ids_and_ord010_unknown_references() {
    let v = with_body(json!([
        section("a", "A", json!([prose("x")])),
        section("a", "B", json!([prose("See [[#zz]].")]))
    ]));
    let f = findings(&v);
    let got: Vec<_> = f.iter().map(|f| (f.code, f.path.as_str())).collect();
    assert_eq!(got, [("ORD009", "/body/1"), ("ORD010", "/body/1/body/0/markdown")]);
}

#[test]
fn ord012_prose_may_not_hold_headings_tables_images_or_html() {
    for md in [
        "# Head",
        "Text\n\n## Head",
        "| a | b |\n|---|---|\n| 1 | 2 |",
        "![i](x.png)",
        "<div>x</div>",
        "a <b>bold</b>",
    ] {
        assert_eq!(codes(&with_body(json!([prose(md)]))), ["ORD012"], "{md}");
    }
}

#[test]
fn ord013_ord019_table_rows() {
    let t = |rows| {
        with_body(
            json!([{ "type": "table", "caption": "C", "columns": [{ "title": "A" }, { "title": "B" }], "rows": rows }]),
        )
    };
    let f = findings(&t(json!([["1", "2"], ["3"]])));
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD013", "/body/0/rows/1"));
    let f = findings(&t(json!([])));
    assert_eq!((f[0].code, f[0].severity), ("ORD019", ord::Severity::Warning));
}

#[test]
fn ord014_ord020_and_ord011_figures() {
    let fig = |alt: &str, width: f64| {
        with_body(json!([{ "type": "figure", "caption": "C", "asset": "x", "alt": alt, "width": width }]))
    };
    let svg: ord::Assets = [(
        "x".to_string(),
        ord::Asset {
            media_type: ord::model::MediaType::Svg,
            bytes: b"<?xml version=\"1.0\"?>\n<!-- c --><svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        },
    )]
    .into();
    let reg = ord::builtin_register();
    let c = |v: &serde_json::Value, a: &ord::Assets| {
        ord::check(&doc(v), a, &reg).into_iter().map(|f| f.code).collect::<Vec<_>>()
    };
    assert!(c(&fig("a", 0.5), &svg).is_empty());
    assert_eq!(c(&fig("  ", 0.5), &svg), ["ORD014"]);
    assert_eq!(c(&fig("a", 0.0), &svg), ["ORD020"]);
    assert_eq!(c(&fig("a", 1.5), &svg), ["ORD020"]);
    assert_eq!(c(&fig("a", 1.0), &Default::default()), ["ORD011"]);
    let html: ord::Assets = [(
        "x".to_string(),
        ord::Asset { media_type: ord::model::MediaType::Svg, bytes: b"<html/>".to_vec() },
    )]
    .into();
    assert_eq!(c(&fig("a", 1.0), &html), ["ORD011"]);
    let png: ord::Assets =
        [("x".to_string(), ord::Asset { media_type: ord::model::MediaType::Png, bytes: b"<svg/>".to_vec() })]
            .into();
    assert_eq!(c(&fig("a", 1.0), &png), ["ORD011"]);
}

#[test]
fn ord015_too_deep_and_too_many_appendices() {
    let deep = with_body(json!([section(
        "a",
        "A",
        json!([section(
            "b",
            "B",
            json!([section(
                "c",
                "C",
                json!([section("d", "D", json!([section("e", "E", json!([prose("x")]))]))])
            )])
        )])
    )]));
    let f = findings(&deep);
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD015", "/body/0/body/0/body/0/body/0/body/0"));
    let mut many = with_body(json!([prose("x")]));
    many["appendices"] = (0..27).map(|_| json!({ "title": "X", "body": [prose("x")] })).collect();
    assert_eq!(codes(&many), ["ORD015"]);
}

#[test]
fn ord016_a_typed_number_in_a_title() {
    let f = findings(&with_body(json!([section("a", "2.1 Scope", json!([prose("x")]))])));
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD016", "/body/0/title"));
    let mut v = with_body(json!([prose("x")]));
    v["appendices"] = json!([{ "title": "A. Trace", "body": [prose("x")] }]);
    assert_eq!(codes(&v), ["ORD016"]);
}

#[test]
fn ord017_a_typed_reference_is_a_warning() {
    let f = findings(&with_body(json!([prose("As section 3.2 shows, and §4.")])));
    assert_eq!((f[0].code, f[0].severity), ("ORD017", ord::Severity::Warning));
}

#[test]
fn ord018_an_empty_section_is_a_warning() {
    let f = findings(&with_body(json!([section("a", "A", json!([]))])));
    assert_eq!(
        (f[0].code, f[0].severity, f[0].path.as_str()),
        ("ORD018", ord::Severity::Warning, "/body/0/body")
    );
}

#[test]
fn the_summary_and_notes_are_prose_too() {
    let mut v = with_body(json!([
        { "type": "table", "id": "t", "caption": "C", "columns": [{ "title": "A" }], "rows": [["1"]], "notes": "From [[#t]]." }
    ]));
    v["summary"] = "See [[#nope]].".into();
    let f = findings(&v);
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD010", "/summary"));
    let (r, _) = resolve(&doc(&v), &ord::builtin_register());
    let RBlock::Table(t) = &r.body[0] else { panic!() };
    assert_eq!(xref_texts(t.notes.as_ref().unwrap()), ["Table 1"]);
}

#[test]
fn the_control_view_holds_what_the_writers_print() {
    let (d, _) = example();
    let (r, _) = resolve(&d, &ord::builtin_register());
    assert_eq!(r.control.class_name, "Report");
    assert_eq!(r.control.status, "Issued");
    assert!(r.control.issued && r.control.furniture.is_none());
    let labels: Vec<_> = r.control.rows().into_iter().map(|(l, _)| l).collect();
    assert_eq!(
        labels,
        ["Document number", "Revision", "Date", "Status", "Sensitivity", "Prepared", "Checked", "Approved"]
    );
}
