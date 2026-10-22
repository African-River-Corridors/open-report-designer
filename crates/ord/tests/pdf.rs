//! The PDF writer (spec 0001 §10), read back with an independent reader (`lopdf`), not the writer's code.
//! Label: property. No pinned numbers.

mod common;

use base64::Engine as _;
use lopdf::{Document as Pdf, Object};
use ord::resolve::{RBlock, Resolved};

fn render(doc: &ord::Document, assets: &ord::Assets) -> Vec<u8> {
    match ord::render(doc, assets, &ord::builtin_register(), ord::Format::Pdf) {
        Ok(o) => o.bytes,
        Err(f) => panic!("the render refused: {f:?}"),
    }
}

fn resolved(doc: &ord::Document, assets: &ord::Assets) -> Resolved {
    ord::render::prepare(doc, assets, &ord::builtin_register()).unwrap().0.doc
}

/// The text of every page, in order, with runs of white space made one space.
fn text(pdf: &[u8]) -> String {
    let d = Pdf::load_mem(pdf).unwrap();
    let pages: Vec<u32> = d.get_pages().keys().copied().collect();
    let t = d.extract_text(&pages).unwrap();
    squash(&t)
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Asserts that each needle occurs in `hay`, in the given order.
fn in_order(hay: &str, needles: &[String]) {
    let mut at = 0;
    for n in needles {
        let n = squash(n);
        match hay[at..].find(&n) {
            Some(i) => at += i + n.len(),
            None => panic!("{n:?} is missing, or out of order, after byte {at} of:\n{hay}"),
        }
    }
}

/// A PDF text string: UTF-16BE with a byte-order mark, or PDFDocEncoding (ASCII here).
fn pdf_string(o: &Object) -> String {
    let b = o.as_str().unwrap();
    if b.starts_with(&[0xfe, 0xff]) {
        let units: Vec<u16> = b[2..].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        String::from_utf16(&units).unwrap()
    } else {
        b.iter().map(|&c| c as char).collect()
    }
}

/// The titles of the outline (bookmarks), depth first.
fn outline(pdf: &[u8]) -> Vec<String> {
    let d = Pdf::load_mem(pdf).unwrap();
    let cat = d.catalog().unwrap();
    let root = d.dereference(cat.get(b"Outlines").unwrap()).unwrap().1.as_dict().unwrap().clone();
    let mut out = Vec::new();
    fn walk(d: &Pdf, first: Option<&Object>, out: &mut Vec<String>) {
        let mut cur = first.cloned();
        while let Some(o) = cur {
            let item = d.dereference(&o).unwrap().1.as_dict().unwrap();
            out.push(pdf_string(d.dereference(item.get(b"Title").unwrap()).unwrap().1));
            walk(d, item.get(b"First").ok(), out);
            cur = item.get(b"Next").ok().cloned();
        }
    }
    walk(&d, root.get(b"First").ok(), &mut out);
    out
}

/// The document information dictionary entry `key`.
fn info(pdf: &[u8], key: &[u8]) -> Option<String> {
    let d = Pdf::load_mem(pdf).unwrap();
    let info = d.trailer.get(b"Info").ok()?;
    let dict = d.dereference(info).unwrap().1.as_dict().unwrap();
    dict.get(key).ok().map(|o| pdf_string(d.dereference(o).unwrap().1))
}

/// The number of XObject streams of a subtype ("Image" or "Form").
fn xobjects(pdf: &[u8], subtype: &[u8]) -> usize {
    let d = Pdf::load_mem(pdf).unwrap();
    d.objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(subtype)
                && s.dict.get(b"Type").ok().and_then(|o| o.as_name().ok()).is_none_or(|t| t == b"XObject")
        })
        .count()
}

/// Every heading the body and appendices print, in order.
fn headings(r: &Resolved) -> Vec<String> {
    fn walk(bs: &[RBlock], out: &mut Vec<String>) {
        for b in bs {
            if let RBlock::Section(s) = b {
                out.push(s.heading());
                walk(&s.body, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&r.body, &mut out);
    for a in &r.appendices {
        out.push(a.heading());
        walk(&a.body, &mut out);
    }
    out
}

/// Every atom of the tables and figures, in order: captions, header cells, body cells, map keys.
fn table_and_figure_atoms(r: &Resolved) -> Vec<String> {
    fn walk(bs: &[RBlock], out: &mut Vec<String>) {
        for b in bs {
            match b {
                RBlock::Section(s) => walk(&s.body, out),
                RBlock::Table(t) => {
                    out.push(t.caption_text());
                    out.extend(t.columns.iter().map(|c| c.header()));
                    out.extend(t.rows.iter().flatten().cloned());
                }
                RBlock::Figure(f) => {
                    out.push(f.caption_text());
                    out.extend(f.map_key.clone());
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&r.body, &mut out);
    for a in &r.appendices {
        walk(&a.body, &mut out);
    }
    out
}

#[test]
fn the_example_renders_and_reads_back_in_order() {
    let (doc, assets) = common::example();
    let pdf = render(&doc, &assets);
    assert!(pdf.starts_with(b"%PDF-"));
    if let Ok(dir) = std::env::var("ORD_PDF_DUMP") {
        std::fs::write(format!("{dir}/example.pdf"), &pdf).unwrap();
    }
    let r = resolved(&doc, &assets);
    let t = text(&pdf);
    // The cover, then the control page, then the contents, then the body.
    let mut front = vec![r.title.clone()];
    front.extend(r.subtitle.clone());
    front.extend(r.organisation.clone());
    front.extend(r.notice.clone());
    front.extend(["Document control".to_string(), "Revision history".into(), "Contents".into()]);
    in_order(&t, &front);
    for (label, value) in r.control.rows() {
        in_order(&t, &[label.to_string(), value]);
    }
    // Every heading, in order, after the contents page.
    let contents = t.find("Contents").unwrap();
    let body = &t[contents..];
    let hs = headings(&r);
    assert!(hs.len() >= 5);
    in_order(body, &hs);
    in_order(body, &table_and_figure_atoms(&r));
    // The running foot.
    let pages = Pdf::load_mem(&pdf).unwrap().get_pages().len();
    assert!(t.contains(&format!("Page 2 of {pages}")), "{t}");
    assert!(t.contains(&format!("Page {pages} of {pages}")));
}

#[test]
fn the_contents_page_holds_every_entry_with_a_page_number() {
    let (doc, assets) = common::example();
    let pdf = render(&doc, &assets);
    let r = resolved(&doc, &assets);
    let d = Pdf::load_mem(&pdf).unwrap();
    // The contents is page 3: after the cover and the control page.
    let page3 = squash(&d.extract_text(&[3]).unwrap());
    assert!(page3.contains("Contents"), "{page3}");
    for e in &r.toc {
        let h = squash(&ord::resolve::heading_text(e.number.as_deref(), &e.title));
        let at = page3.find(&h).unwrap_or_else(|| panic!("{h:?} is not on the contents page: {page3}"));
        // Leader dots, then a page number.
        let rest = page3[at + h.len()..].trim_start_matches([' ', '.', '\u{2060}']);
        assert!(rest.starts_with(|c: char| c.is_ascii_digit()), "{h:?} has no page number: {rest:?}");
    }
}

#[test]
fn the_outline_holds_the_headings() {
    let (doc, assets) = common::example();
    let pdf = render(&doc, &assets);
    let r = resolved(&doc, &assets);
    let marks: Vec<String> = outline(&pdf).iter().map(|s| squash(s)).collect();
    let want: Vec<String> = headings(&r).iter().map(|s| squash(s)).collect();
    let mut at = 0;
    for h in &want {
        let i = marks[at..].iter().position(|m| m == h).unwrap_or_else(|| panic!("{h:?} not in {marks:?}"));
        at += i + 1;
    }
    assert!(marks.contains(&"Document control".to_string()));
}

#[test]
fn the_metadata_holds_title_author_language_and_creator() {
    let (doc, assets) = common::example();
    let pdf = render(&doc, &assets);
    assert_eq!(info(&pdf, b"Title").as_deref(), Some(doc.title.as_str()));
    assert_eq!(info(&pdf, b"Author"), doc.organisation.clone());
    assert_eq!(info(&pdf, b"Creator"), Some(format!("Open Report Designer {}", ord::VERSION)));
    // The creation date is the document date, not the clock.
    assert!(info(&pdf, b"CreationDate").unwrap().starts_with("D:20261022"));
    let d = Pdf::load_mem(&pdf).unwrap();
    let lang = d.catalog().unwrap().get(b"Lang").map(pdf_string).unwrap();
    assert!(lang.starts_with("en"), "{lang}");
}

#[test]
fn rendering_twice_gives_equal_bytes() {
    let (doc, assets) = common::example();
    assert_eq!(render(&doc, &assets), render(&doc, &assets));
}

#[test]
fn typst_special_characters_read_back_unchanged() {
    // `<a>` is escaped for Markdown (raw HTML is refused); everything else reaches the writer as is.
    let md = "$5 and $10 # * _ [x] \\<a> @b \\ ~ = - + \"q\"\n\n= not a heading #set text(red) $x^2$ \\<lbl> // no comment /* none */ `#raw` \\*";
    let lines = [
        r#"$5 and $10 # * _ [x] <a> @b \ ~ = - + "q""#,
        "= not a heading #set text(red) $x^2$ <lbl> // no comment /* none */ #raw *",
    ];
    // The model the writers get holds exactly these texts.
    let prose = ord::markdown::read(md);
    let plain: Vec<String> = prose
        .blocks
        .iter()
        .map(|b| match b {
            ord::markdown::MdBlock::Paragraph(i) => ord::markdown::plain(i),
            _ => panic!("{b:?}"),
        })
        .collect();
    assert_eq!(plain, lines);
    let v = common::with_body(serde_json::json!([common::section(
        "s",
        "Dollars $1 #x",
        serde_json::json!([common::prose(md)])
    )]));
    let doc = common::doc(&v);
    let pdf = render(&doc, &ord::Assets::new());
    let t = text(&pdf);
    in_order(&t, &["1 Dollars $1 #x".to_string(), lines[0].to_string(), lines[1].to_string()]);
}

fn figure_doc(id: &str, media_type: &str, file: &str) -> (ord::Document, ord::Assets) {
    let bytes = std::fs::read(common::repo().join("examples/assets").join(file)).unwrap();
    let v = serde_json::json!({
        "ord": "0.1", "class": "report", "title": "T",
        "control": { "revision": "A", "date": "2026-01-01", "status": "draft" },
        "assets": { id: { "media_type": media_type, "base64": base64::engine::general_purpose::STANDARD.encode(bytes) } },
        "body": [ { "type": "figure", "id": "f", "caption": "A picture", "asset": id, "alt": "a picture", "width": 0.5 } ]
    });
    let doc = common::doc(&v);
    let (assets, findings) = ord::assets::collect(&doc, &Default::default());
    assert!(findings.is_empty(), "{findings:?}");
    (doc, assets)
}

fn no_figure_doc() -> (ord::Document, ord::Assets) {
    (common::doc(&common::with_body(serde_json::json!([common::prose("No picture.")]))), ord::Assets::new())
}

#[test]
fn a_png_figure_renders_as_an_image_xobject() {
    let (doc, assets) = figure_doc("key", "image/png", "option-key.png");
    let pdf = render(&doc, &assets);
    assert!(xobjects(&pdf, b"Image") >= 1);
    let (doc0, assets0) = no_figure_doc();
    assert_eq!(xobjects(&render(&doc0, &assets0), b"Image"), 0);
    assert!(text(&pdf).contains("Figure 1: A picture"));
}

#[test]
fn an_svg_figure_renders_as_vector_content() {
    // Typst draws an SVG as vector paths and text in the page's content (no raster image).
    let (doc, assets) = figure_doc("plan", "image/svg+xml", "reach-plan.svg");
    let pdf = render(&doc, &assets);
    let (doc0, assets0) = no_figure_doc();
    let pdf0 = render(&doc0, &assets0);
    let (with, without) = (path_ops(&pdf), path_ops(&pdf0));
    assert!(with > without + 20, "the SVG adds too few path operators ({with} vs {without})");
    assert_eq!(xobjects(&pdf, b"Image"), 0, "the SVG was rasterised");
    // The SVG's own text (the scale bar) is drawn as text in the PDF. Its glyphs are placed one by one, so
    // the reader may split the words.
    let t = text(&pdf);
    assert!(t.replace(' ', "").contains("0to2km") && t.contains("Figure 1: A picture"), "{t}");
}

/// The number of path-construction operators (m, l, c, v, y, h, re) in the content of every page, and of the
/// form XObjects they draw.
fn path_ops(pdf: &[u8]) -> usize {
    let d = Pdf::load_mem(pdf).unwrap();
    let mut n = 0;
    let count = |content: &[u8]| {
        lopdf::content::Content::decode(content)
            .unwrap()
            .operations
            .iter()
            .filter(|o| matches!(o.operator.as_str(), "m" | "l" | "c" | "v" | "y" | "h" | "re"))
            .count()
    };
    for id in d.get_pages().values() {
        n += count(&d.get_page_content(*id));
    }
    for o in d.objects.values() {
        if let Ok(s) = o.as_stream()
            && s.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Form")
        {
            n += count(&s.decompressed_content().unwrap_or_else(|_| s.content.clone()));
        }
    }
    n
}
