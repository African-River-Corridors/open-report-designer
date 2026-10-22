//! Typst markup from the resolved document.
//!
//! Safety rule: the document's text enters the source only through [`lit`] (a Typst string literal) or
//! [`esc`] (the same literal embedded in markup as `#"..."`). A string literal ends only at an unescaped `"`,
//! and the literal escapes `\` and `"`, so no text can close it. A string shown as content is plain text: it
//! never starts maths, a heading, a list, a label or a reference. Every other piece of the source is written
//! here, from numbers and colours the engine made.
//!
//! The numbers are the resolved document's text. Headings have no Typst numbering, and no Typst counter
//! numbers a section, table or figure. Only the page numbers come from Typst.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::markdown::{Inline, MdBlock};
use crate::model::{Align, MediaType};
use crate::render::Prepared;
use crate::resolve::{RBlock, RFigure, RTable};
use crate::template::Colour;

/// The Typst source and the virtual files it uses.
pub struct Markup {
    pub source: String,
    /// Virtual path to bytes: one file per asset.
    pub files: Vec<(String, Vec<u8>)>,
    /// The document date (year, month, day), when it is an ISO date.
    pub date: Option<(i32, u8, u8)>,
}

/// A Typst string literal holding `s`: `"..."` with `\` and `"` escaped. Line breaks and tabs become spaces
/// unless `lines`; other control characters are dropped.
fn lit_with(s: &str, lines: bool) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' if lines => o.push_str("\\n"),
            '\t' if lines => o.push_str("\\t"),
            '\r' if lines => {}
            '\n' | '\t' | '\r' => o.push(' '),
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A string literal of one-line text.
pub fn lit(s: &str) -> String {
    lit_with(s, false)
}

/// Text as markup: the string literal embedded with `#`.
pub fn esc(s: &str) -> String {
    format!("#{}", lit(s))
}

/// The Typst label for an anchor: `<x-...>`, with every byte outside `[A-Za-z0-9-]` written `_hh`. The map is
/// one to one, and a label literal holds only those characters and `_`.
pub fn label(anchor: &str) -> String {
    let mut o = String::from("<x-");
    for b in anchor.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' {
            o.push(b as char);
        } else {
            let _ = write!(o, "_{b:02x}");
        }
    }
    o.push('>');
    o
}

fn rgb(c: Colour) -> String {
    format!("rgb(\"{}\")", c.hex())
}

/// A length in Typst: a finite number with a unit.
fn num(x: f64) -> String {
    let x = if x.is_finite() { (x * 1000.0).round() / 1000.0 } else { 0.0 };
    format!("{x}")
}

/// `lang` and `region` from a language tag such as "en" or "en-GB". Anything Typst would refuse gives "en".
fn language(tag: &str) -> (String, Option<String>) {
    let mut parts = tag.split(['-', '_']);
    let lang = parts
        .next()
        .filter(|l| (2..=3).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(|l| l.to_ascii_lowercase())
        .unwrap_or_else(|| "en".into());
    let region = parts
        .next()
        .filter(|r| r.len() == 2 && r.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(|r| r.to_ascii_uppercase());
    (lang, region)
}

/// An ISO date "YYYY-MM-DD" (a longer date-time keeps its date part).
fn iso_date(s: &str) -> Option<(i32, u8, u8)> {
    let d = s.get(..10)?;
    let b = d.as_bytes();
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i32 = d[..4].parse().ok()?;
    let m: u8 = d[5..7].parse().ok()?;
    let day: u8 = d[8..10].parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&day)).then_some((y, m, day))
}

struct W<'a> {
    p: &'a Prepared<'a>,
    o: String,
    /// Asset id to virtual path.
    paths: Vec<(&'a str, String)>,
    /// Every anchor that carries a label: a link to anything else prints as text.
    anchors: BTreeSet<&'a str>,
    /// Anchors of the headings in the table of contents.
    toc: BTreeSet<&'a str>,
}

pub fn markup(p: &Prepared) -> Markup {
    let d = &p.doc;
    let mut files = Vec::new();
    let mut paths = Vec::new();
    for (i, (id, a)) in p.assets.iter().enumerate() {
        let ext = match a.media_type {
            MediaType::Png => "png",
            MediaType::Svg => "svg",
        };
        let path = format!("/a{i}.{ext}");
        files.push((path.clone(), a.bytes.clone()));
        paths.push((id.as_str(), path));
    }
    let mut anchors = BTreeSet::new();
    collect_anchors(&d.body, &mut anchors);
    for a in &d.appendices {
        anchors.insert(a.anchor.as_str());
        collect_anchors(&a.body, &mut anchors);
    }
    let toc = d.toc.iter().map(|e| e.anchor.as_str()).collect();
    let date = iso_date(&d.control.date);
    let mut w = W { p, o: String::with_capacity(64 * 1024), paths, anchors, toc };
    w.preamble(date);
    if p.template.cover {
        w.cover();
    } else {
        let _ = writeln!(
            w.o,
            "#block(below: 1.5em)[#text(font: {}, size: 2.4em)[{}]]\n",
            lit(p.template.heading_font),
            esc(&d.title)
        );
    }
    w.control();
    if d.contents && !d.toc.is_empty() {
        w.heading(1, false, "Contents", None);
        w.o.push_str("#outline(title: none, depth: 3)\n#pagebreak(weak: true)\n\n");
    }
    w.blocks(&d.body);
    for a in &d.appendices {
        w.o.push_str("#pagebreak(weak: true)\n");
        w.heading(1, w.toc.contains(a.anchor.as_str()), &a.heading(), Some(&a.anchor));
        w.blocks(&a.body);
    }
    Markup { source: w.o, files, date }
}

fn collect_anchors<'a>(bs: &'a [RBlock], out: &mut BTreeSet<&'a str>) {
    for b in bs {
        match b {
            RBlock::Section(s) => {
                out.insert(&s.anchor);
                collect_anchors(&s.body, out);
            }
            RBlock::Table(t) => {
                out.insert(&t.anchor);
            }
            RBlock::Figure(f) => {
                out.insert(&f.anchor);
            }
            RBlock::DataBox(d) => {
                out.insert(&d.anchor);
            }
            RBlock::Prose(_) => {}
        }
    }
}

impl W<'_> {
    fn preamble(&mut self, date: Option<(i32, u8, u8)>) {
        let p = self.p;
        let d = &p.doc;
        let t = &p.template;
        let o = &mut self.o;
        let author = match &d.organisation {
            Some(org) => format!("({},)", lit(org)),
            None => "()".into(),
        };
        let date = match date {
            Some((y, m, day)) => format!("datetime(year: {y}, month: {m}, day: {day})"),
            None => "none".into(),
        };
        let _ = writeln!(o, "#set document(title: {}, author: {author}, date: {date})", lit(&d.title));
        let (lang, region) = language(&d.language);
        let region = region.map(|r| format!(", region: {}", lit(&r))).unwrap_or_default();
        let _ = writeln!(
            o,
            "#set text(font: {}, size: {}pt, fill: {}, lang: {}{region}, hyphenate: false)",
            lit(t.body_font),
            num(t.body_size_pt.into()),
            rgb(t.ink),
            lit(&lang)
        );
        o.push_str("#set par(justify: false)\n");
        o.push_str("#set heading(numbering: none)\n");
        let head = lit(t.heading_font);
        let _ = writeln!(o, "#show heading: set text(font: {head}, weight: \"regular\")");
        o.push_str("#show heading: set block(above: 1.6em, below: 0.8em)\n");
        o.push_str("#show heading.where(level: 1): set text(size: 1.6em)\n");
        o.push_str("#show heading.where(level: 2): set text(size: 1.3em)\n");
        o.push_str("#show heading.where(level: 3): set text(size: 1.1em)\n");
        o.push_str("#show heading.where(level: 4): set text(size: 1em, style: \"italic\")\n");
        let _ = writeln!(o, "#show raw: set text(font: {}, size: 0.9em)", lit(t.mono_font));
        o.push_str("#show outline.entry.where(level: 1): set block(above: 0.9em)\n");
        let rule = rgb(t.rule);
        let ink2 = rgb(t.ink_2);
        // Running head: number and revision left, furniture right. Running foot: organisation and notice
        // left, "Page n of N" right.
        let furniture = d.control.furniture.as_deref().map(esc).unwrap_or_default();
        let foot = p.footer_left();
        let foot = if foot.is_empty() { String::new() } else { esc(&foot) };
        let (pg, of) = crate::render::PAGE_OF;
        let _ = writeln!(
            o,
            "#set page(width: {}mm, height: {}mm, margin: (top: {}mm, bottom: {}mm, left: {}mm, right: {}mm), \
             fill: {}, \
             header: block(width: 100%, stroke: (bottom: 0.5pt + {rule}), inset: (bottom: 3pt))[#set text(size: 8pt, fill: {ink2})\n{}#h(1fr){furniture}], \
             footer: block(width: 100%, stroke: (top: 0.5pt + {rule}), inset: (top: 3pt))[#set text(size: 8pt, fill: {ink2})\n{foot}#h(1fr)#context [{}#str(here().page()){}#str(counter(page).final().first())]])",
            num(t.page_width_mm.into()),
            num(t.page_height_mm.into()),
            num(t.margin_top_mm.into()),
            num(t.margin_bottom_mm.into()),
            num(t.margin_left_mm.into()),
            num(t.margin_right_mm.into()),
            rgb(t.body_paper),
            esc(&p.header_left()),
            esc(pg),
            esc(of),
        );
        o.push('\n');
    }

    fn asset_path(&self, id: &str) -> Option<&str> {
        self.paths.iter().find(|(i, _)| *i == id).map(|(_, p)| p.as_str())
    }

    fn cover(&mut self) {
        let p = self.p;
        let d = &p.doc;
        let t = &p.template;
        let _ = writeln!(self.o, "#page(fill: {}, header: none, footer: none)[", rgb(t.paper));
        if let Some(path) = t.organisation_logo.as_deref().and_then(|id| self.asset_path(id)) {
            let _ = writeln!(self.o, "#block(below: 12mm)[#image({}, height: 14mm)]", lit(path));
        }
        let _ = writeln!(
            self.o,
            "#block(below: 6mm)[#text(size: 0.85em, tracking: 0.2em)[#upper({})]]",
            lit(&d.control.class_name)
        );
        let head = lit(t.heading_font);
        let _ = writeln!(self.o, "#block(below: 4mm)[#text(font: {head}, size: 2.6em)[{}]]", esc(&d.title));
        if let Some(s) = &d.subtitle {
            let _ = writeln!(
                self.o,
                "#block(below: 4mm)[#text(font: {head}, size: 1.3em, style: \"italic\")[{}]]",
                esc(s)
            );
        }
        if let Some(org) = &d.organisation {
            let _ = writeln!(self.o, "#block(above: 10mm)[{}]", esc(org));
        }
        if let Some(s) = &d.summary {
            self.o.push_str("#block(above: 8mm, width: 85%)[\n");
            self.md_blocks(s);
            self.o.push_str("]\n");
        }
        if let Some(n) = &d.notice {
            let _ = writeln!(self.o, "#block(above: 6mm)[#strong[{}]]", esc(n));
        }
        let cc = p.cover_control();
        let _ = writeln!(
            self.o,
            "#v(1fr)\n#block(width: 100%, stroke: (top: 1pt + {}), inset: (top: 8pt))[#grid(columns: ({}), column-gutter: 1em,",
            rgb(t.ink),
            vec!["1fr"; cc.len()].join(", ") + if cc.len() == 1 { "," } else { "" }
        );
        for (l, v) in &cc {
            let _ = writeln!(
                self.o,
                "  [#block(below: 3pt)[#text(size: 0.75em, fill: {})[{}]]#block[{}]],",
                rgb(t.ink_2),
                esc(l),
                esc(v)
            );
        }
        self.o.push_str(")]\n]\n\n");
    }

    fn control(&mut self) {
        let p = self.p;
        let c = &p.doc.control;
        let t = &p.template;
        self.heading(1, false, "Document control", None);
        let _ = writeln!(
            self.o,
            "#table(columns: (30%, 1fr), stroke: (top: none, left: none, right: none, bottom: 0.5pt + {}), inset: (x: 0.5em, y: 0.4em),",
            rgb(t.rule)
        );
        for (l, v) in c.rows() {
            let _ = writeln!(self.o, "  [#strong[{}]], [{}],", esc(l), esc(&v));
        }
        self.o.push_str(")\n\n");
        if !c.history.is_empty() {
            self.heading(2, false, "Revision history", None);
            let cols = crate::resolve::ControlView::HISTORY_COLUMNS;
            self.table_open(&[Align::Left; 4], Some("(auto, auto, auto, 1fr)"));
            self.o.push_str("  table.header(repeat: true, ");
            for h in cols {
                let _ = write!(self.o, "[#strong[{}]], ", esc(h));
            }
            self.o.push_str("),\n");
            for e in &c.history {
                let _ = writeln!(
                    self.o,
                    "  [{}], [{}], [{}], [{}],",
                    esc(&e.revision),
                    esc(&e.date),
                    esc(&p.status_label(&e.status)),
                    esc(&e.description)
                );
            }
            self.o.push_str(")\n\n");
        }
        self.o.push_str("#pagebreak(weak: true)\n\n");
    }

    /// A heading. Its text is printed as given (the number is part of it); Typst numbers nothing.
    fn heading(&mut self, level: u8, outlined: bool, text: &str, anchor: Option<&str>) {
        let _ = write!(
            self.o,
            "#heading(level: {}, outlined: {outlined}, bookmarked: true)[{}]",
            level.clamp(1, 6),
            esc(text)
        );
        if let Some(a) = anchor {
            let _ = write!(self.o, " {}", label(a));
        }
        self.o.push_str("\n\n");
    }

    fn blocks(&mut self, bs: &[RBlock]) {
        for b in bs {
            match b {
                RBlock::Section(s) => {
                    if s.level == 1 {
                        self.o.push_str("#pagebreak(weak: true)\n");
                    }
                    let outlined = self.toc.contains(s.anchor.as_str());
                    self.heading(s.level, outlined, &s.heading(), Some(&s.anchor));
                    self.blocks(&s.body);
                }
                RBlock::Prose(md) => self.md_blocks(md),
                RBlock::Table(t) => self.table(t),
                RBlock::Figure(f) => self.figure(f),
                RBlock::DataBox(db) => {
                    let t = &self.p.template;
                    let _ = writeln!(
                        self.o,
                        "#block(width: 100%, breakable: false, fill: {}, stroke: (left: {}pt + {}), inset: (x: 1em, y: 0.75em), above: 1.5em, below: 1.5em)[",
                        rgb(t.data_ground),
                        num(t.data_rule_pt.into()),
                        rgb(t.ink)
                    );
                    let _ = writeln!(
                        self.o,
                        "#block(below: 0.6em)[#text(font: {}, size: 1.15em)[{}]]",
                        lit(t.heading_font),
                        esc(&db.title)
                    );
                    if !db.items.is_empty() {
                        self.o
                            .push_str("#grid(columns: (30%, 1fr), column-gutter: 1em, row-gutter: 0.5em,\n");
                        for (l, v) in &db.items {
                            let _ = writeln!(self.o, "  [#strong[{}]], [{}],", esc(l), esc(v));
                        }
                        self.o.push_str(")\n");
                    }
                    if let Some(n) = &db.notes {
                        self.notes(n);
                    }
                    let _ = writeln!(self.o, "] {}\n", label(&db.anchor));
                }
            }
        }
    }

    /// Opens a `#table(` call with the template's rules and header ground. The caller closes it.
    fn table_open(&mut self, aligns: &[Align], columns: Option<&str>) {
        let t = &self.p.template;
        let aligns: Vec<&str> = aligns
            .iter()
            .map(|a| match a {
                Align::Left => "left + top",
                Align::Centre => "center + top",
                Align::Right => "right + top",
            })
            .collect();
        let cols = match columns {
            Some(c) => c.to_string(),
            None => aligns.len().max(1).to_string(),
        };
        let al =
            if aligns.is_empty() { "left + top".to_string() } else { format!("({},)", aligns.join(", ")) };
        let _ = writeln!(
            self.o,
            "#table(columns: {cols}, align: {al}, fill: (_, y) => if y == 0 {{ {} }}, stroke: (top: none, left: none, right: none, bottom: 0.5pt + {}), inset: (x: 0.5em, y: 0.35em),",
            rgb(t.accent_ground),
            rgb(t.rule)
        );
    }

    fn table(&mut self, t: &RTable) {
        let aligns: Vec<Align> = t.columns.iter().map(|c| c.align).collect();
        self.o.push_str("#block(width: 100%, above: 1.2em, below: 1.2em)[\n");
        let _ = writeln!(self.o, "#block(below: 0.5em, sticky: true)[#strong[{}]]", esc(&t.caption_text()));
        if self.p.template.table_tabular_figures {
            self.o.push_str("#set text(number-width: \"tabular\")\n");
        }
        self.table_open(&aligns, None);
        self.o.push_str("  table.header(repeat: true, ");
        for c in &t.columns {
            let _ = write!(self.o, "[#strong[{}]], ", esc(&c.header()));
        }
        self.o.push_str("),\n");
        let n = t.columns.len();
        for row in &t.rows {
            self.o.push(' ');
            for i in 0..n {
                let _ = write!(self.o, " [{}],", row.get(i).map(|c| esc(c)).unwrap_or_default());
            }
            self.o.push('\n');
        }
        self.o.push_str(")\n");
        if let Some(notes) = &t.notes {
            self.notes(notes);
        }
        let _ = writeln!(self.o, "] {}\n", label(&t.anchor));
    }

    fn notes(&mut self, n: &[MdBlock]) {
        let _ = writeln!(
            self.o,
            "#block(above: 0.6em)[#set text(size: 0.9em, fill: {})",
            rgb(self.p.template.ink_2)
        );
        self.md_blocks(n);
        self.o.push_str("]\n");
    }

    fn figure(&mut self, f: &RFigure) {
        self.o.push_str("#block(width: 100%, breakable: false, above: 1.5em, below: 1.5em)[\n");
        if let Some(path) = self.asset_path(&f.asset).map(str::to_string) {
            let _ = writeln!(
                self.o,
                "#align(center)[#image({}, width: {}%, alt: {})]",
                lit(&path),
                num((f.width * 100.0).clamp(0.0, 100.0)),
                lit(&f.alt)
            );
        }
        let _ = writeln!(self.o, "#block(above: 0.6em)[#strong[{}]]", esc(&f.caption_text()));
        if let Some(k) = &f.map_key {
            let _ = writeln!(
                self.o,
                "#block(above: 0.3em)[#text(size: 0.85em, fill: {})[{}]]",
                rgb(self.p.template.ink_2),
                esc(k)
            );
        }
        let _ = writeln!(self.o, "] {}\n", label(&f.anchor));
    }

    fn md_blocks(&mut self, bs: &[MdBlock]) {
        for b in bs {
            match b {
                MdBlock::Paragraph(i) => {
                    self.inlines(i);
                    self.o.push_str("\n\n");
                }
                MdBlock::List { ordered, start, items } => {
                    if *ordered {
                        let _ = writeln!(self.o, "#enum(start: {start},");
                    } else {
                        self.o.push_str("#list(\n");
                    }
                    for item in items {
                        self.o.push_str("  [");
                        match item.as_slice() {
                            [MdBlock::Paragraph(i)] => self.inlines(i),
                            _ => {
                                self.o.push('\n');
                                self.md_blocks(item);
                            }
                        }
                        self.o.push_str("],\n");
                    }
                    self.o.push_str(")\n\n");
                }
                MdBlock::Quote(q) => {
                    let t = &self.p.template;
                    let _ = writeln!(
                        self.o,
                        "#block(inset: (left: 1em), stroke: (left: 1pt + {}))[#set text(fill: {})",
                        rgb(t.rule),
                        rgb(t.ink_2)
                    );
                    self.md_blocks(q);
                    self.o.push_str("]\n\n");
                }
                MdBlock::Code(c) => {
                    let _ = writeln!(
                        self.o,
                        "#block(width: 100%, fill: {}, inset: (x: 1em, y: 0.75em))[#raw({}, block: true)]\n",
                        rgb(self.p.template.data_ground),
                        lit_with(c.trim_end_matches('\n'), true)
                    );
                }
            }
        }
    }

    fn inlines(&mut self, is: &[Inline]) {
        for i in is {
            match i {
                Inline::Text(t) => self.o.push_str(&esc(t)),
                Inline::Emph(c) => {
                    self.o.push_str("#emph[");
                    self.inlines(c);
                    self.o.push(']');
                }
                Inline::Strong(c) => {
                    self.o.push_str("#strong[");
                    self.inlines(c);
                    self.o.push(']');
                }
                Inline::Code(c) => {
                    let _ = write!(self.o, "#raw({})", lit(c));
                }
                Inline::Link { url, children } => {
                    let target = if !crate::write::html::safe_url(url) {
                        None
                    } else if let Some(a) = url.trim().strip_prefix('#') {
                        self.anchors.contains(a).then(|| label(a))
                    } else {
                        Some(lit(url.trim()))
                    };
                    match target {
                        Some(t) => {
                            let _ = write!(self.o, "#link({t})[");
                            self.inlines(children);
                            self.o.push(']');
                        }
                        None => self.inlines(children),
                    }
                }
                Inline::XRef { text, anchor, .. } => {
                    if self.anchors.contains(anchor.as_str()) {
                        let _ = write!(self.o, "#link({})[{}]", label(anchor), esc(text));
                    } else {
                        self.o.push_str(&esc(text));
                    }
                }
                Inline::LineBreak => self.o.push_str("#linebreak()"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_and_dates() {
        assert_eq!(language("en-GB"), ("en".into(), Some("GB".into())));
        assert_eq!(language("fr"), ("fr".into(), None));
        assert_eq!(language("not a tag"), ("en".into(), None));
        assert_eq!(iso_date("2026-10-22"), Some((2026, 10, 22)));
        assert_eq!(iso_date("22/10/2026"), None);
        assert_eq!(iso_date("2026-13-01"), None);
    }
}
