//! The conformance suite (spec 0001 §12): one document in PDF, HTML and Word has the same structure and text.
//!
//! The expected text comes from the resolved document, in the order the writer contract gives (cover, document
//! control, contents, body, appendices). Each output is read back by an independent reader, never by the
//! writer's code: an XML reader for the HTML, a zip and XML reader for the Word file, a PDF text reader for
//! the PDF. HTML and Word must give exactly the expected list of texts. A PDF has no paragraphs to read back,
//! so its text, with the running heads and feet taken off each page, must hold every expected text in order.
//! Texts are compared with all white space removed: line breaks and spaces differ between formats.
//!
//! Label: property. No pinned numbers.

mod common;

use std::io::Read as _;
use std::path::Path;

use common::*;
use ord::markdown::{MdBlock, plain};
use ord::render::{Format, Prepared, prepare};
use ord::resolve::{ControlView, RBlock, heading_text};

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

// --- What every output must hold ----------------------------------------------------------------------

fn expected(p: &Prepared) -> Vec<String> {
    let d = &p.doc;
    let mut out = Vec::new();
    if p.template.cover {
        out.push(d.control.class_name.clone());
        out.push(d.title.clone());
        out.extend(d.subtitle.clone());
        out.extend(d.organisation.clone());
        if let Some(s) = &d.summary {
            md(s, &mut out);
        }
        out.extend(d.notice.clone());
        for (label, value) in p.cover_control() {
            out.push(label.to_string());
            out.push(value);
        }
    } else {
        out.push(d.title.clone());
    }
    out.push("Document control".into());
    for (label, value) in d.control.rows() {
        out.push(label.to_string());
        out.push(value);
    }
    if !d.control.history.is_empty() {
        out.push("Revision history".into());
        out.extend(ControlView::HISTORY_COLUMNS.iter().map(|s| s.to_string()));
        for e in &d.control.history {
            out.extend([
                e.revision.clone(),
                e.date.clone(),
                p.status_label(&e.status),
                e.description.clone(),
            ]);
        }
    }
    if d.contents && !d.toc.is_empty() {
        out.push("Contents".into());
        out.extend(d.toc.iter().map(|e| heading_text(e.number.as_deref(), &e.title)));
    }
    blocks(&d.body, &mut out);
    for a in &d.appendices {
        out.push(a.heading());
        blocks(&a.body, &mut out);
    }
    out.into_iter().map(|s| squash(&s)).filter(|s| !s.is_empty()).collect()
}

fn blocks(bs: &[RBlock], out: &mut Vec<String>) {
    for b in bs {
        match b {
            RBlock::Section(s) => {
                out.push(s.heading());
                blocks(&s.body, out);
            }
            RBlock::Prose(m) => md(m, out),
            RBlock::Table(t) => {
                out.push(t.caption_text());
                out.extend(t.columns.iter().map(|c| c.header()));
                out.extend(t.rows.iter().flatten().cloned());
                if let Some(n) = &t.notes {
                    md(n, out);
                }
            }
            RBlock::Figure(f) => {
                out.push(f.caption_text());
                out.extend(f.map_key.clone());
            }
            RBlock::DataBox(d) => {
                out.push(d.title.clone());
                for (l, v) in &d.items {
                    out.push(l.clone());
                    out.push(v.clone());
                }
                if let Some(n) = &d.notes {
                    md(n, out);
                }
            }
        }
    }
}

fn md(bs: &[MdBlock], out: &mut Vec<String>) {
    for b in bs {
        match b {
            MdBlock::Paragraph(i) => out.push(plain(i)),
            MdBlock::List { items, .. } => items.iter().for_each(|item| md(item, out)),
            MdBlock::Quote(q) => md(q, out),
            MdBlock::Code(c) => out.push(c.clone()),
        }
    }
}

// --- Readers ------------------------------------------------------------------------------------------

/// The texts of an HTML file: each block element's own text (not that of nested block elements), in
/// document order, leaving out the head and the running head and foot.
fn html_texts(bytes: &[u8]) -> Vec<String> {
    const ATOMS: &[&str] = &[
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "p",
        "li",
        "th",
        "td",
        "caption",
        "figcaption",
        "dt",
        "dd",
        "pre",
    ];
    let text = std::str::from_utf8(bytes).unwrap().trim_start_matches("<!doctype html>\n");
    let doc = roxmltree::Document::parse(text).expect("the HTML is well-formed XML");
    let body = doc.descendants().find(|n| n.has_tag_name("body")).unwrap();
    let mut out = Vec::new();
    for n in body.descendants().filter(|n| n.is_element() && ATOMS.contains(&n.tag_name().name())) {
        if n.ancestors().any(|a| a.attribute("class").is_some_and(|c| c.starts_with("running-"))) {
            continue;
        }
        let mut s = String::new();
        own_text(n, ATOMS, &mut s);
        let s = squash(&s);
        if !s.is_empty() {
            out.push(s);
        }
    }
    out
}

fn own_text(n: roxmltree::Node, atoms: &[&str], s: &mut String) {
    for c in n.children() {
        if c.is_text() {
            s.push_str(c.text().unwrap());
        } else if c.is_element() && !atoms.contains(&c.tag_name().name()) {
            own_text(c, atoms, s);
        }
    }
}

/// The texts of a Word file's body: each paragraph's text, in order, table cells included.
fn docx_texts(bytes: &[u8]) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a zip file");
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let w = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let mut out = Vec::new();
    for p in doc.descendants().filter(|n| n.has_tag_name((w, "p"))) {
        let mut s = String::new();
        for t in p.descendants() {
            if t.has_tag_name((w, "t")) {
                s.push_str(t.text().unwrap_or(""));
            } else if t.has_tag_name((w, "tab")) || t.has_tag_name((w, "br")) {
                s.push(' ');
            }
        }
        // A paragraph inside a text box or a nested paragraph is read once, by its own node.
        let s = squash(&s);
        if !s.is_empty() {
            out.push(s);
        }
    }
    out
}

/// The text of each page of a PDF, as a PDF text reader gives it.
fn pdf_pages(bytes: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(bytes).expect("a PDF");
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    pages.iter().map(|n| doc.extract_text(&[*n]).unwrap_or_default()).collect()
}

/// The PDF's text with each page's running head taken off its start and its running foot off its end. The
/// cover has neither.
fn pdf_text(p: &Prepared, bytes: &[u8]) -> String {
    let pages = pdf_pages(bytes);
    let n = pages.len();
    let mut head: Vec<String> = vec![squash(&p.header_left())];
    head.extend(p.doc.control.furniture.as_deref().map(squash));
    let mut all = String::new();
    for (i, page) in pages.iter().enumerate() {
        let mut s = squash(page);
        if !(i == 0 && p.template.cover) {
            for h in &head {
                if let Some(r) = s.strip_prefix(h.as_str()) {
                    s = r.to_string();
                }
            }
            let foot = [squash(&format!("Page {} of {n}", i + 1)), squash(&p.footer_left())];
            for _ in 0..2 {
                for f in foot.iter().filter(|f| !f.is_empty()) {
                    if let Some(r) = s.strip_suffix(f.as_str()) {
                        s = r.to_string();
                    }
                }
            }
        }
        all.push_str(&s);
    }
    all
}

/// The expected texts that the PDF text does not hold in order, if any. Compared in lower case: the cover
/// sets the class name in capitals, which a PDF holds as capital letters.
fn pdf_missing(p: &Prepared, bytes: &[u8]) -> Vec<String> {
    let text = pdf_text(p, bytes).to_lowercase();
    let mut at = 0;
    let mut missing = Vec::new();
    for e in expected(p).into_iter().map(|e| e.to_lowercase()) {
        match text[at..].find(&e) {
            Some(i) => at += i + e.len(),
            None => missing.push(e),
        }
    }
    missing
}

// --- The suite ----------------------------------------------------------------------------------------

fn outputs(path: &Path) -> (ord::Document, ord::Assets) {
    load(path)
}

fn render(d: &ord::Document, a: &ord::Assets, f: Format) -> Vec<u8> {
    ord::render(d, a, &ord::builtin_register(), f).unwrap().bytes
}

fn conforms(path: &Path) {
    let (d, a) = outputs(path);
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let want = expected(&p);
    assert_eq!(html_texts(&render(&d, &a, Format::Html)), want, "HTML differs from the document");
    assert_eq!(docx_texts(&render(&d, &a, Format::Docx)), want, "Word differs from the document");
    let missing = pdf_missing(&p, &render(&d, &a, Format::Pdf));
    assert!(missing.is_empty(), "the PDF does not hold, in order: {missing:#?}");
}

#[test]
fn the_example_has_the_same_structure_and_text_in_pdf_html_and_word() {
    conforms(&repo().join("examples/screening-report.json"));
}

#[test]
fn the_stress_document_has_the_same_structure_and_text_in_pdf_html_and_word() {
    conforms(&repo().join("crates/ord/tests/fixtures/stress.json"));
}

#[test]
fn a_document_without_a_cover_or_contents_conforms_too() {
    let path = repo().join("examples/screening-report.json");
    let (mut d, a) = load(&path);
    d.template = Some(ord::model::TemplateOverrides { cover: Some(false), ..Default::default() });
    d.contents = Some(false);
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let want = expected(&p);
    assert_eq!(html_texts(&render(&d, &a, Format::Html)), want);
    assert_eq!(docx_texts(&render(&d, &a, Format::Docx)), want);
    assert!(pdf_missing(&p, &render(&d, &a, Format::Pdf)).is_empty());
}

/// The suite can fail: one changed text in any output is caught.
#[test]
fn a_doctored_output_fails_the_suite() {
    let path = repo().join("examples/screening-report.json");
    let (d, a) = load(&path);
    let (p, _) = prepare(&d, &a, &ord::builtin_register()).unwrap();
    let want = expected(&p);

    let html = String::from_utf8(render(&d, &a, Format::Html)).unwrap().replacen("298 000", "298 001", 1);
    assert_ne!(html_texts(html.as_bytes()), want);
    let html = String::from_utf8(render(&d, &a, Format::Html)).unwrap().replacen(
        "<li>the plan of each design line;</li>",
        "",
        1,
    );
    assert_ne!(html_texts(html.as_bytes()), want);

    let docx = render(&d, &a, Format::Docx);
    let doctored = rezip(&docx, "word/document.xml", |x| x.replacen("298 000", "298 001", 1));
    assert_ne!(docx_texts(&doctored), want);

    // A PDF that lacks one expected text: the check of a document with one more paragraph than the PDF holds.
    let mut more = d.clone();
    if let ord::model::Block::Section(s) = &mut more.body[0] {
        s.body.push(ord::model::Block::Prose(ord::model::Prose {
            markdown: "A paragraph the PDF never saw.".into(),
        }));
    }
    let (p2, _) = prepare(&more, &a, &ord::builtin_register()).unwrap();
    assert_eq!(
        pdf_missing(&p2, &render(&d, &a, Format::Pdf)),
        [squash("A paragraph the PDF never saw.").to_lowercase()]
    );
}

/// A copy of a zip file with one entry changed by `f`.
fn rezip(bytes: &[u8], name: &str, f: impl Fn(&str) -> String) -> Vec<u8> {
    let mut src = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut e = src.by_index(i).unwrap();
        let mut data = Vec::new();
        e.read_to_end(&mut data).unwrap();
        if e.name() == name {
            data = f(std::str::from_utf8(&data).unwrap()).into_bytes();
        }
        out.start_file(e.name(), zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut out, &data).unwrap();
    }
    out.finish().unwrap().into_inner()
}

#[test]
fn every_format_renders_the_same_bytes_twice() {
    for path in
        [repo().join("examples/screening-report.json"), repo().join("crates/ord/tests/fixtures/stress.json")]
    {
        let (d, a) = load(&path);
        for f in Format::ALL {
            assert!(
                render(&d, &a, f) == render(&d, &a, f),
                "{f:?} of {} is not deterministic",
                path.display()
            );
        }
    }
}
