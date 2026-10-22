//! The Word writer (spec 0001 §10): Office Open XML written directly by the core.
//!
//! The package holds the document, styles, settings, numbering, a font table with the bundled fonts embedded
//! (obfuscated, ECMA-376 Part 1 §17.8.1), one header and one footer, the core and application properties,
//! and the images. SVG images go in as SVG with a PNG fallback that the engine draws itself.
//!
//! Layout follows the PDF as far as Word allows: an A4 page with the template's margins, a cover section on
//! the cream paper (a full-page picture behind the text, since Word has no page colour per section), then a
//! section with the running head and foot. Numbers are text from the resolved document: no heading uses Word
//! numbering and no caption uses a SEQ field. The table of contents is a TOC field whose cached result holds
//! the entries, and `updateFields` asks Word to fill in the page numbers when the file opens.
//!
//! The zip has a fixed file order and a fixed time, so the same input gives the same bytes.

mod font;
mod image;
mod parts;
mod zip;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::finding::Finding;
use crate::markdown::{Inline, MdBlock};
use crate::model::{Align, MediaType};
use crate::render::{PAGE_OF, Prepared};
use crate::resolve::{ControlView, RBlock, RDataBox, RFigure, RTable, heading_text};

use parts::{DECL, REL};

/// The namespaces of the document, header and footer parts.
const NS_DOC: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
    xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
    xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
    xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
    xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"";

/// The extension that carries an SVG next to its PNG fallback (Office 2016 and later).
const SVG_EXT: &str = "{96DAC541-7B7A-43D3-8B79-37D633B846F1}";
const NS_ASVG: &str = "http://schemas.microsoft.com/office/drawing/2016/SVG/main";

/// The relationship ids of the fixed parts of `word/document.xml`; images and links follow.
const RID_HEADER: &str = "rId5";
const RID_FOOTER: &str = "rId6";
const FIXED_RELS: usize = 6;

/// Writes the prepared document as a .docx package.
pub fn write(p: &Prepared) -> Result<Vec<u8>, Vec<Finding>> {
    let mut w = Writer::new(p);
    let document = w.document();
    if !w.errors.is_empty() {
        return Err(w.errors);
    }

    // The relationships of the document part: the fixed parts, then images and links in order of use.
    let mut doc_rels: Vec<(String, String, String, bool)> = [
        ("styles", "styles.xml"),
        ("settings", "settings.xml"),
        ("numbering", "numbering.xml"),
        ("fontTable", "fontTable.xml"),
        ("header", "header1.xml"),
        ("footer", "footer1.xml"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (kind, target))| (format!("rId{}", i + 1), format!("{REL}/{kind}"), target.to_string(), false))
    .collect();
    doc_rels.extend(w.rels.iter().cloned());

    // The embedded fonts.
    let mut embedded = Vec::new();
    let mut font_parts = Vec::new();
    for f in crate::fonts::bundled() {
        let Some(slot) = font::slot(f) else { continue };
        let n = embedded.len() + 1;
        let key = font::key(f.file_name);
        let part = format!("fonts/font{n}.odttf");
        font_parts.push((format!("word/{part}"), font::obfuscate(f.ttf, &key)));
        embedded.push(parts::Embedded { family: f.family, slot, rid: format!("rId{n}"), key, part });
    }
    let font_rels: Vec<_> =
        embedded.iter().map(|e| (e.rid.clone(), format!("{REL}/font"), e.part.clone(), false)).collect();

    let xml = |name: &str, s: String| zip::Entry { name: name.into(), data: s.into_bytes(), deflate: true };
    let mut entries = vec![
        xml("[Content_Types].xml", parts::content_types()),
        xml("_rels/.rels", parts::package_rels()),
        xml("docProps/core.xml", parts::core(p)),
        xml("docProps/app.xml", parts::app(p)),
        xml("word/document.xml", document),
        xml("word/_rels/document.xml.rels", parts::rels(&doc_rels)),
        xml("word/styles.xml", parts::styles(p)),
        xml("word/settings.xml", parts::settings(p)),
        xml("word/numbering.xml", parts::numbering(p, &w.ordered)),
        xml("word/fontTable.xml", parts::font_table(&p.template, &embedded)),
        xml("word/_rels/fontTable.xml.rels", parts::rels(&font_rels)),
        xml("word/header1.xml", w.header()),
        xml("word/footer1.xml", w.footer()),
    ];
    for (name, data) in std::mem::take(&mut w.media) {
        let deflate = name.ends_with(".svg");
        entries.push(zip::Entry { name, data, deflate });
    }
    for (name, data) in font_parts {
        entries.push(zip::Entry { name, data, deflate: true });
    }
    Ok(zip::write(&entries))
}

/// Millimetres to twentieths of a point.
fn twips(mm: f32) -> u32 {
    (f64::from(mm) * 1440.0 / 25.4).round() as u32
}

/// Millimetres to English Metric Units (36 000 per millimetre).
fn emu(mm: f64) -> u64 {
    (mm * 36_000.0).round() as u64
}

/// Escapes text for XML: `&`, `<`, `>`, `"` and `'`. Control characters that XML forbids are dropped.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            '\t' | '\n' | '\r' => o.push(c),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => o.push(c),
        }
    }
    o
}

/// Only web, mail and in-document links become links (as in the HTML writer); anything else is text.
fn safe_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:") || u.starts_with('#')
}

/// A run's character formatting.
#[derive(Debug, Clone, Copy, Default)]
struct Fmt {
    bold: bool,
    italic: bool,
    link: bool,
}

impl Fmt {
    fn rpr(self, style: Option<&str>) -> String {
        let style = style.or(if self.link { Some("Hyperlink") } else { None });
        let mut r = String::new();
        if let Some(s) = style {
            let _ = write!(r, "<w:rStyle w:val=\"{s}\"/>");
        }
        if self.bold {
            r.push_str("<w:b/><w:bCs/>");
        }
        if self.italic {
            r.push_str("<w:i/><w:iCs/>");
        }
        r
    }
}

/// One run of text: tabs become `w:tab`, new lines `w:br`.
fn run(text: &str, rpr: &str) -> String {
    let mut x = String::from("<w:r>");
    if !rpr.is_empty() {
        let _ = write!(x, "<w:rPr>{rpr}</w:rPr>");
    }
    let mut buf = String::new();
    let flush = |buf: &mut String, x: &mut String| {
        if !buf.is_empty() {
            let _ = write!(x, "<w:t xml:space=\"preserve\">{}</w:t>", esc(buf));
            buf.clear();
        }
    };
    for c in text.chars() {
        match c {
            '\t' => {
                flush(&mut buf, &mut x);
                x.push_str("<w:tab/>");
            }
            '\n' => {
                flush(&mut buf, &mut x);
                x.push_str("<w:br/>");
            }
            '\r' => {}
            c => buf.push(c),
        }
    }
    flush(&mut buf, &mut x);
    x.push_str("</w:r>");
    x
}

/// Paragraph properties, written in the schema's order.
#[derive(Default)]
struct PPr<'a> {
    style: Option<&'a str>,
    page_break: bool,
    num: Option<(usize, u8)>,
    ind_left: Option<u32>,
    jc: Option<&'a str>,
    sect: Option<String>,
}

impl PPr<'_> {
    fn style(s: &str) -> PPr<'_> {
        PPr { style: Some(s), ..PPr::default() }
    }

    fn xml(&self) -> String {
        let mut x = String::new();
        if let Some(s) = self.style.filter(|s| *s != "Normal") {
            let _ = write!(x, "<w:pStyle w:val=\"{s}\"/>");
        }
        if self.page_break {
            x.push_str("<w:pageBreakBefore/>");
        }
        if let Some((id, lvl)) = self.num {
            let _ = write!(x, "<w:numPr><w:ilvl w:val=\"{lvl}\"/><w:numId w:val=\"{id}\"/></w:numPr>");
        }
        if let Some(l) = self.ind_left {
            let _ = write!(x, "<w:ind w:left=\"{l}\"/>");
        }
        if let Some(j) = self.jc {
            let _ = write!(x, "<w:jc w:val=\"{j}\"/>");
        }
        if let Some(s) = &self.sect {
            x.push_str(s);
        }
        if x.is_empty() { x } else { format!("<w:pPr>{x}</w:pPr>") }
    }
}

/// An embedded image: its relationship ids and its height over its width.
#[derive(Debug, Clone)]
struct Pic {
    png: String,
    svg: Option<String>,
    aspect: f64,
}

/// A prose context: the paragraph style of its paragraphs and the list depth.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    style: &'a str,
    depth: u8,
}

struct Writer<'a, 'p> {
    p: &'a Prepared<'p>,
    x: String,
    /// Relationships of the document part after the fixed ones: (id, type, target, external).
    rels: Vec<(String, String, String, bool)>,
    /// Package parts of the images: (zip name, bytes).
    media: Vec<(String, Vec<u8>)>,
    pics: BTreeMap<String, Pic>,
    /// Anchor to bookmark name.
    bookmarks: BTreeMap<String, String>,
    next_bookmark: u32,
    next_shape: u32,
    /// The ordered lists: (level, start). List `i` is numbering instance `i + 2`.
    ordered: Vec<(u8, u64)>,
    /// The last thing written was a table (two tables in a row would merge).
    after_table: bool,
    errors: Vec<Finding>,
}

impl<'a, 'p> Writer<'a, 'p> {
    fn new(p: &'a Prepared<'p>) -> Self {
        let mut w = Writer {
            p,
            x: String::with_capacity(256 * 1024),
            rels: Vec::new(),
            media: Vec::new(),
            pics: BTreeMap::new(),
            bookmarks: BTreeMap::new(),
            next_bookmark: 0,
            next_shape: 1,
            ordered: Vec::new(),
            after_table: false,
            errors: Vec::new(),
        };
        w.name_bookmarks();
        w
    }

    /// Gives every anchor a bookmark name Word accepts: at most 40 letters, digits and underscores,
    /// starting with a letter, and unique.
    fn name_bookmarks(&mut self) {
        let mut anchors = Vec::new();
        fn walk(bs: &[RBlock], out: &mut Vec<String>) {
            for b in bs {
                match b {
                    RBlock::Section(s) => {
                        out.push(s.anchor.clone());
                        walk(&s.body, out);
                    }
                    RBlock::Table(t) => out.push(t.anchor.clone()),
                    RBlock::Figure(f) => out.push(f.anchor.clone()),
                    RBlock::DataBox(d) => out.push(d.anchor.clone()),
                    RBlock::Prose(_) => {}
                }
            }
        }
        walk(&self.p.doc.body, &mut anchors);
        for a in &self.p.doc.appendices {
            anchors.push(a.anchor.clone());
            walk(&a.body, &mut anchors);
        }
        let mut used = BTreeSet::new();
        for a in anchors {
            if self.bookmarks.contains_key(&a) {
                continue;
            }
            let mut base: String =
                a.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).take(34).collect();
            if !base.starts_with(|c: char| c.is_ascii_alphabetic()) {
                base.insert(0, 'b');
            }
            let mut name = base.clone();
            let mut n = 1;
            while used.contains(&name) {
                n += 1;
                name = format!("{base}_{n}");
            }
            used.insert(name.clone());
            self.bookmarks.insert(a, name);
        }
    }

    fn rel(&mut self, kind: &str, target: &str, external: bool) -> String {
        let id = format!("rId{}", FIXED_RELS + self.rels.len() + 1);
        self.rels.push((id.clone(), format!("{REL}/{kind}"), target.to_string(), external));
        id
    }

    fn text_width_mm(&self) -> f64 {
        let t = &self.p.template;
        f64::from(t.page_width_mm - t.margin_left_mm - t.margin_right_mm)
    }

    fn para(&mut self, ppr: &PPr, content: &str) {
        let _ = write!(self.x, "<w:p>{}{content}</w:p>", ppr.xml());
        self.after_table = false;
    }

    /// Text wrapped in the bookmark of an anchor, so cross-references can link to it.
    fn bookmarked(&mut self, anchor: &str, content: String) -> String {
        match self.bookmarks.get(anchor) {
            Some(name) => {
                let id = self.next_bookmark;
                self.next_bookmark += 1;
                format!(
                    "<w:bookmarkStart w:id=\"{id}\" w:name=\"{name}\"/>{content}<w:bookmarkEnd w:id=\"{id}\"/>"
                )
            }
            None => content,
        }
    }

    /// A table may not follow a table directly: an empty paragraph keeps them apart.
    fn before_table(&mut self) {
        if self.after_table {
            self.para(&PPr { style: Some("TableNote"), ..PPr::default() }, "");
        }
    }

    fn sect_pr(&self, running: bool) -> String {
        let t = &self.p.template;
        let mut x = String::from("<w:sectPr>");
        if running {
            let _ = write!(
                x,
                "<w:headerReference w:type=\"default\" r:id=\"{RID_HEADER}\"/><w:footerReference w:type=\"default\" r:id=\"{RID_FOOTER}\"/>"
            );
        }
        let _ = write!(
            x,
            "<w:pgSz w:w=\"{}\" w:h=\"{}\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"0\"/>\
             <w:cols w:space=\"709\"/><w:docGrid w:linePitch=\"360\"/></w:sectPr>",
            twips(t.page_width_mm),
            twips(t.page_height_mm),
            twips(t.margin_top_mm),
            twips(t.margin_right_mm),
            twips(t.margin_bottom_mm),
            twips(t.margin_left_mm),
            twips(t.margin_top_mm * 0.5),
            twips(t.margin_bottom_mm * 0.5),
        );
        x
    }

    /// `word/document.xml`.
    fn document(&mut self) -> String {
        let p = self.p;
        let d = &p.doc;
        if p.template.cover {
            self.cover();
        } else {
            let t = run(&d.title, "");
            self.para(&PPr::style("Title"), &t);
        }
        self.control();
        if d.contents && !d.toc.is_empty() {
            self.contents();
        }
        self.blocks(&d.body, true);
        for a in &d.appendices {
            let content = self.bookmarked(&a.anchor, run(&a.heading(), ""));
            self.para(&PPr { style: Some("Heading1"), page_break: true, ..PPr::default() }, &content);
            self.blocks(&a.body, false);
        }
        if self.after_table {
            self.para(&PPr::default(), "");
        }
        let body = std::mem::take(&mut self.x);
        let mut x = String::from(DECL);
        let _ = write!(x, "<w:document {NS_DOC}><w:body>{body}{}</w:body></w:document>", self.sect_pr(true));
        x
    }

    fn cover(&mut self) {
        let p = self.p;
        let d = &p.doc;
        let t = &p.template;
        // The cream paper: a one-pixel picture stretched over the whole page, behind the text.
        let paper = {
            let rid = self.media_part("png", image::pixel_png(t.paper));
            let pic = Pic { png: rid, svg: None, aspect: 1.0 };
            let (w, h) = (emu(f64::from(t.page_width_mm)), emu(f64::from(t.page_height_mm)));
            self.anchor_behind(&pic, w, h)
        };
        let mut first = Some(paper);
        if let Some(logo) = t.organisation_logo.clone()
            && let Some(pic) = self.pic(&logo)
        {
            // About 12 mm high and at most 45 mm wide.
            let mut h = 12.0;
            let mut w = h / pic.aspect;
            if w > 45.0 {
                w = 45.0;
                h = w * pic.aspect;
            }
            let drawing = self.inline(&pic, emu(w), emu(h), "Logo", "");
            let content = format!("{}{drawing}", first.take().unwrap_or_default());
            self.para(&PPr::default(), &content);
        }
        let content = format!("{}{}", first.take().unwrap_or_default(), run(&d.control.class_name, ""));
        self.para(&PPr::style("ClassName"), &content);
        self.para(&PPr::style("Title"), &run(&d.title, ""));
        if let Some(s) = &d.subtitle {
            self.para(&PPr::style("Subtitle"), &run(s, ""));
        }
        if let Some(o) = &d.organisation {
            self.para(&PPr::style("CoverText"), &run(o, ""));
        }
        if let Some(s) = &d.summary {
            self.md_blocks(s, Ctx { style: "Summary", depth: 0 });
        }
        if let Some(n) = &d.notice {
            self.para(&PPr::style("Notice"), &run(n, ""));
        }
        // The cover control block: one row, a label over a value in each cell, an ink rule above.
        let pairs = p.cover_control();
        if !pairs.is_empty() {
            self.before_table();
            let col = twips(self.text_width_mm() as f32) / pairs.len() as u32;
            let _ = write!(
                self.x,
                "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders><w:top w:val=\"single\" w:sz=\"8\" w:space=\"0\" w:color=\"{}\"/></w:tblBorders>\
                 <w:tblLayout w:type=\"fixed\"/><w:tblCellMar><w:left w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"113\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>",
                parts::hex(t.ink)
            );
            for _ in &pairs {
                let _ = write!(self.x, "<w:gridCol w:w=\"{col}\"/>");
            }
            self.x.push_str("</w:tblGrid><w:tr>");
            for (label, value) in &pairs {
                let _ = write!(
                    self.x,
                    "<w:tc><w:tcPr><w:tcW w:w=\"{col}\" w:type=\"dxa\"/></w:tcPr><w:p><w:pPr><w:pStyle w:val=\"CoverLabel\"/></w:pPr>{}</w:p>\
                     <w:p><w:pPr><w:pStyle w:val=\"CoverValue\"/></w:pPr>{}</w:p></w:tc>",
                    run(label, ""),
                    run(value, "")
                );
            }
            self.x.push_str("</w:tr></w:tbl>");
            self.after_table = true;
        }
        // The cover section ends here: it has no running head or foot.
        let sect = self.sect_pr(false);
        self.para(&PPr { sect: Some(sect), ..PPr::default() }, "");
    }

    fn control(&mut self) {
        let p = self.p;
        let c = &p.doc.control;
        self.para(&PPr::style("FrontHeading"), &run("Document control", ""));
        let rows: Vec<Vec<String>> = c.rows().into_iter().map(|(l, v)| vec![l.to_string(), v]).collect();
        self.grid_table(&[0.3, 0.7], None, &rows, &[Align::Left, Align::Left], true);
        if !c.history.is_empty() {
            self.para(&PPr::style("FrontHeading"), &run("Revision history", ""));
            let head: Vec<String> = ControlView::HISTORY_COLUMNS.iter().map(|s| s.to_string()).collect();
            let rows: Vec<Vec<String>> = c
                .history
                .iter()
                .map(|e| {
                    vec![e.revision.clone(), e.date.clone(), p.status_label(&e.status), e.description.clone()]
                })
                .collect();
            self.grid_table(&[0.14, 0.18, 0.2, 0.48], Some(&head), &rows, &[Align::Left; 4], false);
        }
    }

    /// The table of contents: a TOC field whose cached result is the entries, each a link to its heading.
    fn contents(&mut self) {
        let d = &self.p.doc;
        self.para(
            &PPr { style: Some("FrontHeading"), page_break: true, ..PPr::default() },
            &run("Contents", ""),
        );
        let n = d.toc.len();
        for (i, e) in d.toc.iter().enumerate() {
            let mut content = String::new();
            if i == 0 {
                content.push_str(
                    "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"1-3\" \\h \\z \\u </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>",
                );
            }
            let text = run(&heading_text(e.number.as_deref(), &e.title), "");
            match self.bookmarks.get(&e.anchor) {
                Some(name) => {
                    let _ = write!(
                        content,
                        "<w:hyperlink w:anchor=\"{name}\" w:history=\"1\">{text}</w:hyperlink>"
                    );
                }
                None => content.push_str(&text),
            }
            if i + 1 == n {
                content.push_str("<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>");
            }
            let style = format!("TOC{}", e.level.clamp(1, 3));
            self.para(&PPr::style(&style), &content);
        }
    }

    fn blocks(&mut self, bs: &[RBlock], top: bool) {
        for b in bs {
            match b {
                RBlock::Section(s) => {
                    let style = format!("Heading{}", s.level.clamp(1, 5));
                    let content = self.bookmarked(&s.anchor, run(&s.heading(), ""));
                    let page_break = top && s.level == 1;
                    self.para(&PPr { style: Some(&style), page_break, ..PPr::default() }, &content);
                    self.blocks(&s.body, false);
                }
                RBlock::Prose(md) => self.md_blocks(md, Ctx { style: "Normal", depth: 0 }),
                RBlock::Table(t) => self.table(t),
                RBlock::Figure(f) => self.figure(f),
                RBlock::DataBox(d) => self.data_box(d),
            }
        }
    }

    /// A table with the report table's rules: an optional header row (repeated on each page, on the header
    /// ground), then the rows. `widths` are fractions of the text width. With `label_column`, the first
    /// column is bold (the document-control table).
    fn grid_table(
        &mut self,
        widths: &[f64],
        head: Option<&[String]>,
        rows: &[Vec<String>],
        aligns: &[Align],
        label_column: bool,
    ) {
        let tw = twips(self.text_width_mm() as f32);
        let cols: Vec<u32> = widths.iter().map(|f| (f64::from(tw) * f).round() as u32).collect();
        self.before_table();
        self.x.push_str(
            "<w:tbl><w:tblPr><w:tblStyle w:val=\"ReportTable\"/><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblLook w:val=\"0000\" w:firstRow=\"0\" w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"1\" w:noVBand=\"1\"/></w:tblPr><w:tblGrid>",
        );
        for c in &cols {
            let _ = write!(self.x, "<w:gridCol w:w=\"{c}\"/>");
        }
        self.x.push_str("</w:tblGrid>");
        let jc = |i: usize| match aligns.get(i).copied().unwrap_or_default() {
            Align::Left => "",
            Align::Centre => "<w:jc w:val=\"center\"/>",
            Align::Right => "<w:jc w:val=\"right\"/>",
        };
        let ground = parts::hex(self.p.template.accent_ground);
        if let Some(head) = head {
            self.x.push_str("<w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr>");
            for (i, h) in head.iter().enumerate() {
                let _ = write!(
                    self.x,
                    "<w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{ground}\"/></w:tcPr>\
                     <w:p><w:pPr><w:pStyle w:val=\"TableHeader\"/>{}</w:pPr>{}</w:p></w:tc>",
                    cols.get(i).copied().unwrap_or(0),
                    jc(i),
                    run(h, "")
                );
            }
            self.x.push_str("</w:tr>");
        }
        for row in rows {
            self.x.push_str("<w:tr><w:trPr><w:cantSplit/></w:trPr>");
            for (i, cell) in row.iter().enumerate() {
                let style = if label_column && i == 0 { "TableHeader" } else { "TableText" };
                let _ = write!(
                    self.x,
                    "<w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/></w:tcPr><w:p><w:pPr><w:pStyle w:val=\"{style}\"/>{}</w:pPr>{}</w:p></w:tc>",
                    cols.get(i).copied().unwrap_or(0),
                    jc(i),
                    run(cell, "")
                );
            }
            self.x.push_str("</w:tr>");
        }
        self.x.push_str("</w:tbl>");
        self.after_table = true;
    }

    fn table(&mut self, t: &RTable) {
        let caption = self.bookmarked(&t.anchor, run(&t.caption_text(), ""));
        self.para(&PPr::style("Caption"), &caption);
        let n = t.columns.len().max(1);
        let widths = vec![1.0 / n as f64; n];
        let head: Vec<String> = t.columns.iter().map(|c| c.header()).collect();
        let aligns: Vec<Align> = t.columns.iter().map(|c| c.align).collect();
        self.grid_table(&widths, Some(&head), &t.rows, &aligns, false);
        if let Some(notes) = &t.notes {
            self.md_blocks(notes, Ctx { style: "TableNote", depth: 0 });
        }
    }

    /// A data box: one table on the data ground with a thick ink rule on the left. The title spans both
    /// columns, then one row per item (label in bold, value), then the notes across both columns.
    fn data_box(&mut self, d: &RDataBox) {
        let t = &self.p.template;
        let tw = twips(self.text_width_mm() as f32);
        let (c1, c2) = ((f64::from(tw) * 0.3).round() as u32, (f64::from(tw) * 0.7).round() as u32);
        let ground = parts::hex(t.data_ground);
        let rule = (t.data_rule_pt * 8.0).round() as u32;
        let shd = format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{ground}\"/>");
        self.before_table();
        let _ = write!(
            self.x,
            "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders><w:top w:val=\"nil\"/><w:left w:val=\"single\" w:sz=\"{rule}\" w:space=\"0\" w:color=\"{}\"/>\
             <w:bottom w:val=\"nil\"/><w:right w:val=\"nil\"/><w:insideH w:val=\"nil\"/><w:insideV w:val=\"nil\"/></w:tblBorders>\
             <w:tblCellMar><w:top w:w=\"57\" w:type=\"dxa\"/><w:left w:w=\"170\" w:type=\"dxa\"/><w:bottom w:w=\"57\" w:type=\"dxa\"/><w:right w:w=\"170\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr>\
             <w:tblGrid><w:gridCol w:w=\"{c1}\"/><w:gridCol w:w=\"{c2}\"/></w:tblGrid>",
            parts::hex(t.ink)
        );
        let title = self.bookmarked(&d.anchor, run(&d.title, ""));
        let _ = write!(
            self.x,
            "<w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr><w:tcW w:w=\"{tw}\" w:type=\"dxa\"/><w:gridSpan w:val=\"2\"/>{shd}</w:tcPr>\
             <w:p><w:pPr><w:pStyle w:val=\"DataBoxTitle\"/></w:pPr>{title}</w:p></w:tc></w:tr>"
        );
        for (label, value) in &d.items {
            let _ = write!(
                self.x,
                "<w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr><w:tcW w:w=\"{c1}\" w:type=\"dxa\"/>{shd}</w:tcPr>\
                 <w:p><w:pPr><w:pStyle w:val=\"TableHeader\"/></w:pPr>{}</w:p></w:tc>\
                 <w:tc><w:tcPr><w:tcW w:w=\"{c2}\" w:type=\"dxa\"/>{shd}</w:tcPr><w:p><w:pPr><w:pStyle w:val=\"TableText\"/></w:pPr>{}</w:p></w:tc></w:tr>",
                run(label, ""),
                run(value, "")
            );
        }
        if let Some(notes) = &d.notes {
            let _ = write!(
                self.x,
                "<w:tr><w:tc><w:tcPr><w:tcW w:w=\"{tw}\" w:type=\"dxa\"/><w:gridSpan w:val=\"2\"/>{shd}</w:tcPr>"
            );
            let before = self.x.len();
            self.md_blocks(notes, Ctx { style: "TableText", depth: 0 });
            if self.x.len() == before {
                self.x.push_str("<w:p/>");
            }
            self.x.push_str("</w:tc></w:tr>");
        }
        self.x.push_str("</w:tbl>");
        self.after_table = true;
    }

    fn figure(&mut self, f: &RFigure) {
        if let Some(pic) = self.pic(&f.asset) {
            let w = self.text_width_mm() * f.width.clamp(0.01, 1.0);
            let drawing = self.inline(&pic, emu(w), emu(w * pic.aspect), &f.label, &f.alt);
            self.para(&PPr::style("Figure"), &drawing);
        }
        let caption = self.bookmarked(&f.anchor, run(&f.caption_text(), ""));
        self.para(&PPr::style("Caption"), &caption);
        if let Some(k) = &f.map_key {
            self.para(&PPr::style("MapKey"), &run(k, ""));
        }
    }

    /// Adds a media part and its relationship; returns the relationship id.
    fn media_part(&mut self, ext: &str, bytes: Vec<u8>) -> String {
        let name = format!("media/image{}.{ext}", self.media.len() + 1);
        self.media.push((format!("word/{name}"), bytes));
        self.rel("image", &name, false)
    }

    /// The embedded image of an asset, added once. A PNG goes in as it is; an SVG goes in with a PNG
    /// fallback drawn at the text width.
    fn pic(&mut self, id: &str) -> Option<Pic> {
        if let Some(p) = self.pics.get(id) {
            return Some(p.clone());
        }
        let a = self.p.assets.get(id)?;
        let pic = match a.media_type {
            MediaType::Png => match crate::assets::png_size(&a.bytes) {
                Ok((w, h)) => {
                    let png = self.media_part("png", a.bytes.clone());
                    Pic { png, svg: None, aspect: f64::from(h) / f64::from(w) }
                }
                Err(e) => {
                    self.errors.push(asset_error(id, &e));
                    return None;
                }
            },
            MediaType::Svg => {
                let drawn = image::parse_svg(&a.bytes).and_then(|tree| {
                    let size = tree.size();
                    let png = image::svg_fallback(&tree, self.text_width_mm() as f32)?;
                    Ok((png, f64::from(size.height()) / f64::from(size.width())))
                });
                match drawn {
                    Ok((png, aspect)) => {
                        let png = self.media_part("png", png);
                        let svg = self.media_part("svg", a.bytes.clone());
                        Pic { png, svg: Some(svg), aspect }
                    }
                    Err(e) => {
                        self.errors.push(asset_error(id, &e));
                        return None;
                    }
                }
            }
        };
        self.pics.insert(id.to_string(), pic.clone());
        Some(pic)
    }

    fn graphic(&mut self, pic: &Pic, cx: u64, cy: u64, name: &str, descr: &str) -> (u32, String) {
        let id = self.next_shape;
        self.next_shape += 1;
        let ext = match &pic.svg {
            Some(svg) => format!(
                "<a:extLst><a:ext uri=\"{SVG_EXT}\"><asvg:svgBlip xmlns:asvg=\"{NS_ASVG}\" r:embed=\"{svg}\"/></a:ext></a:extLst>"
            ),
            None => String::new(),
        };
        let g = format!(
            "<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic>\
             <pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"{n}\" descr=\"{d}\"/><pic:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></pic:cNvPicPr></pic:nvPicPr>\
             <pic:blipFill><a:blip r:embed=\"{png}\">{ext}</a:blip><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
             <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
             </pic:pic></a:graphicData></a:graphic>",
            n = esc(name),
            d = esc(descr),
            png = pic.png,
        );
        (id, g)
    }

    /// A picture in the line of text, as a run.
    fn inline(&mut self, pic: &Pic, cx: u64, cy: u64, name: &str, descr: &str) -> String {
        let (id, g) = self.graphic(pic, cx, cy, name, descr);
        format!(
            "<w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
             <wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:docPr id=\"{id}\" name=\"{n}\" descr=\"{d}\"/>\
             <wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>{g}</wp:inline></w:drawing></w:r>",
            n = esc(name),
            d = esc(descr),
        )
    }

    /// A picture at the top left of the page, the given size, behind the text, as a run.
    fn anchor_behind(&mut self, pic: &Pic, cx: u64, cy: u64) -> String {
        let (id, g) = self.graphic(pic, cx, cy, "Paper", "");
        format!(
            "<w:r><w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"0\" behindDoc=\"1\" locked=\"1\" layoutInCell=\"1\" allowOverlap=\"1\">\
             <wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"page\"><wp:posOffset>0</wp:posOffset></wp:positionH>\
             <wp:positionV relativeFrom=\"page\"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
             <wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:wrapNone/><wp:docPr id=\"{id}\" name=\"Paper\" descr=\"\"/>\
             <wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>{g}</wp:anchor></w:drawing></w:r>"
        )
    }

    fn md_blocks(&mut self, bs: &[MdBlock], ctx: Ctx) {
        for b in bs {
            match b {
                MdBlock::Paragraph(i) => {
                    let content = self.inlines(i, Fmt::default());
                    let ind_left = (ctx.depth > 0).then(|| 397 * u32::from(ctx.depth));
                    self.para(&PPr { style: Some(ctx.style), ind_left, ..PPr::default() }, &content);
                }
                MdBlock::List { ordered, start, items } => {
                    let level = ctx.depth.min(8);
                    let num = if *ordered {
                        self.ordered.push((level, *start));
                        self.ordered.len() + 1
                    } else {
                        1
                    };
                    let style = if ctx.style == "Normal" { "ListParagraph" } else { ctx.style };
                    for item in items {
                        let (first, rest) = match item.split_first() {
                            Some((MdBlock::Paragraph(i), rest)) => (self.inlines(i, Fmt::default()), rest),
                            _ => (String::new(), item.as_slice()),
                        };
                        self.para(
                            &PPr { style: Some(style), num: Some((num, level)), ..PPr::default() },
                            &first,
                        );
                        self.md_blocks(rest, Ctx { style, depth: level + 1 });
                    }
                }
                MdBlock::Quote(q) => self.md_blocks(q, Ctx { style: "Quote", ..ctx }),
                MdBlock::Code(c) => {
                    let ind_left = (ctx.depth > 0).then(|| 397 * u32::from(ctx.depth));
                    let content = run(c.trim_end_matches('\n'), "");
                    self.para(&PPr { style: Some("Code"), ind_left, ..PPr::default() }, &content);
                }
            }
        }
    }

    fn inlines(&mut self, is: &[Inline], f: Fmt) -> String {
        let mut x = String::new();
        for i in is {
            match i {
                Inline::Text(t) => x.push_str(&run(t, &f.rpr(None))),
                Inline::Emph(c) => x.push_str(&self.inlines(c, Fmt { italic: true, ..f })),
                Inline::Strong(c) => x.push_str(&self.inlines(c, Fmt { bold: true, ..f })),
                Inline::Code(c) => x.push_str(&run(c, &f.rpr(Some("InlineCode")))),
                Inline::Link { url, children } => {
                    if !safe_url(url) || f.link {
                        x.push_str(&self.inlines(children, f));
                        continue;
                    }
                    let inner = self.inlines(children, Fmt { link: true, ..f });
                    let url = url.trim();
                    if let Some(anchor) = url.strip_prefix('#') {
                        match self.bookmarks.get(anchor) {
                            Some(name) => {
                                let _ = write!(
                                    x,
                                    "<w:hyperlink w:anchor=\"{name}\" w:history=\"1\">{inner}</w:hyperlink>"
                                );
                            }
                            None => x.push_str(&inner),
                        }
                    } else {
                        let rid = self.rel("hyperlink", url, true);
                        let _ =
                            write!(x, "<w:hyperlink r:id=\"{rid}\" w:history=\"1\">{inner}</w:hyperlink>");
                    }
                }
                Inline::XRef { text, anchor, .. } => {
                    let r = run(text, &Fmt { link: true, ..f }.rpr(None));
                    match self.bookmarks.get(anchor) {
                        Some(name) => {
                            let _ = write!(
                                x,
                                "<w:hyperlink w:anchor=\"{name}\" w:history=\"1\">{r}</w:hyperlink>"
                            );
                        }
                        None => x.push_str(&run(text, &f.rpr(None))),
                    }
                }
                Inline::LineBreak => x.push_str("<w:r><w:br/></w:r>"),
            }
        }
        x
    }

    /// `word/header1.xml`: the number and revision on the left, the status furniture on the right.
    fn header(&self) -> String {
        let p = self.p;
        let mut content = run(&p.header_left(), "");
        if let Some(f) = &p.doc.control.furniture {
            content.push_str("<w:r><w:tab/></w:r>");
            content.push_str(&run(f, "<w:caps/><w:spacing w:val=\"20\"/>"));
        }
        let mut x = String::from(DECL);
        let _ = write!(
            x,
            "<w:hdr {NS_DOC}><w:p><w:pPr><w:pStyle w:val=\"Header\"/></w:pPr>{content}</w:p></w:hdr>"
        );
        x
    }

    /// `word/footer1.xml`: the organisation and notice on the left, "Page n of N" on the right.
    fn footer(&self) -> String {
        let p = self.p;
        let mut content = run(&p.footer_left(), "");
        content.push_str("<w:r><w:tab/></w:r>");
        content.push_str(&run(PAGE_OF.0, ""));
        content.push_str("<w:fldSimple w:instr=\" PAGE \"><w:r><w:t>1</w:t></w:r></w:fldSimple>");
        content.push_str(&run(PAGE_OF.1, ""));
        content.push_str("<w:fldSimple w:instr=\" NUMPAGES \"><w:r><w:t>1</w:t></w:r></w:fldSimple>");
        let mut x = String::from(DECL);
        let _ = write!(
            x,
            "<w:ftr {NS_DOC}><w:p><w:pPr><w:pStyle w:val=\"Footer\"/></w:pPr>{content}</w:p></w:ftr>"
        );
        x
    }
}

fn asset_error(id: &str, e: &str) -> Finding {
    let path = format!("/assets/{}", id.replace('~', "~0").replace('/', "~1"));
    Finding::error("ORD011", path, format!("asset {id:?} cannot be drawn for Word: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_and_runs() {
        assert_eq!(esc("a<b>&\"c'\u{1}"), "a&lt;b&gt;&amp;&quot;c&apos;");
        assert_eq!(
            run("a\tb\nc", ""),
            "<w:r><w:t xml:space=\"preserve\">a</w:t><w:tab/><w:t xml:space=\"preserve\">b</w:t><w:br/><w:t xml:space=\"preserve\">c</w:t></w:r>"
        );
        assert_eq!(twips(25.4), 1440);
        assert_eq!(emu(1.0), 36_000);
    }
}
