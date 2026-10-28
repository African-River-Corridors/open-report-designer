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
use crate::model::{Align, CoverLayout, MediaType};
use crate::render::{CONTROL, PHOTO, Prepared};
use crate::resolve::{RBlock, RFigure, RTable, cell_inlines};
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

/// Text settings for lines `lh` times the size from baseline to baseline, each line box that tall with half
/// the leading above and half below: how Word lays out a line of "exact" or "at least" spacing.
fn line_box(lh: f64) -> String {
    let half = (lh - 1.0) / 2.0;
    format!(
        "#set text(top-edge: {}em, bottom-edge: -{}em)\n#set par(leading: 0pt)",
        num(0.8 + half),
        num(0.2 + half)
    )
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
    /// The photo cover's photograph height, millimetres.
    band_mm: f64,
}

pub fn markup(p: &Prepared) -> Markup {
    markup_with_band(p, crate::write::pdf::band_mm(p), true)
}

/// The source with the photo cover's photograph `band_mm` high. With `full` false, the preamble only.
fn markup_with_band(p: &Prepared, band_mm: f64, full: bool) -> Markup {
    let d = &p.doc;
    let mut files = Vec::new();
    let mut paths = Vec::new();
    for (i, (id, a)) in p.assets.iter().enumerate() {
        let ext = match a.media_type {
            MediaType::Png => "png",
            MediaType::Svg => "svg",
            MediaType::Jpeg => "jpg",
            // Fonts go to the world's font book, not to a virtual file.
            MediaType::Font => continue,
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
    let mut w = W { p, o: String::with_capacity(64 * 1024), paths, anchors, toc, band_mm };
    w.preamble(date);
    if !full {
        return Markup { source: w.o, files, date };
    }
    if p.template.cover {
        match p.template.cover_layout {
            CoverLayout::Classic => w.cover(),
            CoverLayout::Photo => w.photo_cover(),
        }
    } else {
        let _ = writeln!(
            w.o,
            "#block(below: 1.5em)[#text(font: {}, size: 2.4em)[{}]]\n",
            lit(&p.template.heading_font),
            esc(&d.title)
        );
    }
    if p.template.control_page {
        w.control();
    }
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
            lit(&t.body_font),
            num(t.body_size_pt.into()),
            rgb(t.ink),
            lit(&lang)
        );
        o.push_str("#set par(justify: false)\n");
        o.push_str("#set heading(numbering: none)\n");
        let head = lit(&t.heading_font);
        let weight = if t.heading_bold { "bold" } else { "regular" };
        let fill =
            if t.heading_ink == t.ink { String::new() } else { format!(", fill: {}", rgb(t.heading_ink)) };
        let _ = writeln!(o, "#show heading: set text(font: {head}, weight: \"{weight}\"{fill})");
        o.push_str("#show heading: set block(above: 1.6em, below: 0.8em)\n");
        // The 0.1 sizes are relative to Typst's own heading sizes; a template's sizes are points.
        let sizes = match t.heading_sizes_pt {
            Some(s) => s.map(|v| format!("{}pt", num(v.into()))),
            None => ["1.6em", "1.3em", "1.1em", "1em"].map(String::from),
        };
        for (i, size) in sizes.iter().enumerate() {
            let italic = if i == 3 { ", style: \"italic\"" } else { "" };
            let _ = writeln!(o, "#show heading.where(level: {}): set text(size: {size}{italic})", i + 1);
        }
        // With sizes in points (0.2), the space around a heading is in points too: Word's, so the two agree.
        let (h1_above, h1_below) = if t.heading_sizes_pt.is_some() {
            for (lvl, above) in [(1, 18), (2, 15), (3, 12), (4, 10)] {
                let _ =
                    writeln!(o, "#show heading.where(level: {lvl}): set block(above: {above}pt, below: 6pt)");
            }
            ("18pt", "6pt")
        } else {
            ("1.6em", "0.8em")
        };
        if let Some(r) = t.heading_rule {
            let _ = writeln!(
                o,
                "#show heading.where(level: 1): it => block(width: 100%, above: {h1_above}, below: {h1_below}, stroke: (bottom: 1.2pt + {}), inset: (bottom: 3pt))[#it.body]",
                rgb(r)
            );
        }
        // Line height (0.2): every line box is the line height tall, with half the leading above the text
        // and half below, as in CSS and in Word's "at least" line spacing, so a one-line table row is as tall
        // in the PDF as in Word. Headings keep single lines, as Word's do. Paragraph and list spacing follow.
        if let Some(lh) = t.line_height {
            let half = (f64::from(lh) - 1.0) / 2.0;
            let _ = writeln!(
                o,
                "#set text(top-edge: {}em, bottom-edge: -{}em)\n#set par(leading: 0pt)",
                num(0.8 + half),
                num(0.2 + half)
            );
            o.push_str("#show heading: set text(top-edge: 0.8em, bottom-edge: -0.2em)\n#show heading: set par(leading: 0.2em)\n");
            let gap = t.paragraph_spacing_pt.map(f64::from).unwrap_or(6.0);
            let _ = writeln!(o, "#set par(spacing: {}pt)", num(gap));
            o.push_str("#set list(spacing: 3pt)\n#set enum(spacing: 3pt)\n");
        } else if let Some(gap) = t.paragraph_spacing_pt {
            let _ = writeln!(o, "#set par(spacing: {}pt)", num(gap.into()));
        }
        if let Some(i) = t.list_indent_mm {
            // The text of an item starts one indent in per level, as Word's hanging indent puts it.
            let _ = writeln!(
                o,
                "#set list(indent: 0pt, body-indent: {}mm - 0.45em)\n#set enum(indent: 0pt, body-indent: {}mm - 1.1em)",
                num(i.into()),
                num(i.into())
            );
        }
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
        let header = if t.running_head {
            format!(
                "block(width: 100%, stroke: (bottom: 0.5pt + {rule}), inset: (bottom: 3pt))[#set text(size: 8pt, fill: {ink2})\n{}#h(1fr){furniture}]",
                esc(&p.header_left())
            )
        } else {
            "none".into()
        };
        // The foot is a grid, so the left text may wrap without pushing the page number off its line.
        let middle = p.footer_middle().map(|m| esc(&m)).unwrap_or_default();
        let page_of = format!(
            "#context [{}#str(here().page()){}#str(counter(page).final().first())]",
            esc(pg),
            esc(of)
        );
        let foot_line = if p.grid_foot() {
            format!(
                "#grid(columns: (40%, 20%, 40%), align: (left, center, right), [{foot}], [{middle}], [{page_of}])"
            )
        } else {
            format!("{foot}#h(1fr){page_of}")
        };
        let _ = writeln!(
            o,
            "#set page(width: {}mm, height: {}mm, margin: (top: {}mm, bottom: {}mm, left: {}mm, right: {}mm), \
             fill: {}, \
             header: {header}, \
             footer: block(width: 100%, stroke: (top: 0.5pt + {rule}), inset: (top: 3pt))[#set text(size: 8pt, fill: {ink2})\n{foot_line}])",
            num(t.page_width_mm.into()),
            num(t.page_height_mm.into()),
            num(t.margin_top_mm.into()),
            num(t.margin_bottom_mm.into()),
            num(t.margin_left_mm.into()),
            num(t.margin_right_mm.into()),
            rgb(t.body_paper),
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
        let head = lit(&t.heading_font);
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

    /// The photo cover (spec 0002 §4): a photograph across the head of the page, then on the cover paper the
    /// title block (§`photo_body`). Its foot holds the number and revision, and the sensitivity.
    fn photo_cover(&mut self) {
        let p = self.p;
        let d = &p.doc;
        let t = &p.template;
        let foot_ink = rgb(t.cover_ink.over(t.paper, 0.65));
        let _ = writeln!(
            self.o,
            "#page(fill: {}, margin: (top: 0mm, bottom: {}mm, left: {}mm, right: {}mm), header: none, \
             footer: [#set text(size: {}pt, fill: {foot_ink})\n{}#h(1fr){}])[",
            rgb(t.paper),
            num(t.margin_bottom_mm.into()),
            num(t.margin_left_mm.into()),
            num(t.margin_right_mm.into()),
            num(PHOTO.foot_pt),
            esc(&p.header_left()),
            esc(&d.control.sensitivity),
        );
        let band = self.band_mm;
        match d.cover_image.as_ref().and_then(|(id, alt)| self.asset_path(id).map(|p| (p.to_string(), alt))) {
            Some((path, alt)) => {
                let rule = t.cover_image_rule_pt.unwrap_or(0.0);
                let _ = writeln!(
                    self.o,
                    "#place(top + left, dx: -{}mm)[#image({}, width: {}mm, height: {}mm, fit: \"cover\", alt: {})]",
                    num(t.margin_left_mm.into()),
                    lit(&path),
                    num(t.page_width_mm.into()),
                    num(band),
                    lit(alt),
                );
                if rule > 0.0 {
                    let _ = writeln!(
                        self.o,
                        "#place(top + left, dx: -{}mm, dy: {}mm)[#block(width: {}mm, height: {}pt, fill: {})[]]",
                        num(t.margin_left_mm.into()),
                        num(band),
                        num(t.page_width_mm.into()),
                        num(rule.into()),
                        rgb(t.cover_accent)
                    );
                }
                let _ = writeln!(self.o, "#v({}mm)", num(band + PHOTO.gap_mm));
            }
            None => {
                let _ = writeln!(self.o, "#v({}mm)", num(f64::from(t.margin_top_mm)));
            }
        }
        self.photo_body();
        self.o.push_str("]\n\n");
    }

    /// The content of the photo cover under the photograph: the accent rule (only with no rule under a
    /// photograph), the title, the subtitle, organisation and notice in the accent, the summary card, the logo
    /// card and the control block. Every measurement is `PHOTO`'s. Line boxes are one em, so a line height is
    /// the distance from one baseline to the next, as in Word.
    fn photo_body(&mut self) {
        let p = self.p;
        let d = &p.doc;
        let t = &p.template;
        let (ink, accent, card) = (rgb(t.cover_ink), rgb(t.cover_accent), rgb(t.card_ground));
        let _ = writeln!(self.o, "#set text(fill: {ink})");
        let ruled = d.cover_image.is_some() && t.cover_image_rule_pt.is_some_and(|r| r > 0.0);
        if !ruled {
            let _ = writeln!(self.o, "#block(width: 14mm, height: 1.5pt, fill: {accent}, below: 5mm)[]");
        }
        let head = lit(&t.heading_font);
        let weight = if t.heading_bold { "bold" } else { "regular" };
        let _ = writeln!(
            self.o,
            "#block(above: 0mm, below: {}mm)[{}\n#text(font: {head}, weight: \"{weight}\", size: {}pt)[{}]]",
            num(PHOTO.title_after_mm),
            line_box(PHOTO.title_lh),
            num(t.cover_title_pt.into()),
            esc(&d.title)
        );
        let lines: Vec<&String> = [&d.subtitle, &d.organisation, &d.notice].into_iter().flatten().collect();
        for (i, line) in lines.iter().enumerate() {
            let below = if i + 1 == lines.len() { PHOTO.meta_after_mm } else { 1.0 };
            let _ = writeln!(
                self.o,
                "#block(above: 0mm, below: {}mm)[{}\n#text(size: {}pt, fill: {accent})[{}]]",
                num(below),
                line_box(1.35),
                num(PHOTO.meta_pt),
                esc(line)
            );
        }
        if lines.is_empty() {
            let _ = writeln!(self.o, "#v({}mm)", num(PHOTO.meta_after_mm - PHOTO.title_after_mm));
        }
        if let Some(s) = &d.summary {
            let _ = writeln!(
                self.o,
                "#block(width: 100%, fill: {card}, radius: 1mm, inset: (x: {}mm, top: {}mm, bottom: {}mm), above: 0mm, below: {}mm)[#set text(fill: {}, size: {}pt)\n{}\n#set par(spacing: {}mm)",
                num(PHOTO.card_pad_x_mm),
                num(PHOTO.card_pad_top_mm),
                num(PHOTO.card_pad_bottom_mm),
                num(PHOTO.card_gap_mm),
                rgb(t.ink),
                num(PHOTO.card_text_pt),
                line_box(PHOTO.card_lh),
                num(PHOTO.card_par_gap_mm),
            );
            self.md_blocks(s);
            self.o.push_str("]\n");
        }
        if !d.cover_logos.is_empty() {
            let n = d.cover_logos.len();
            let _ = writeln!(
                self.o,
                "#block(width: 100%, fill: {card}, radius: 1mm, inset: (x: 1mm, y: {}mm), above: 0mm, below: 0mm)[#set text(fill: {})\n{}",
                num(PHOTO.logo_pad_y_mm),
                rgb(t.ink),
                line_box(1.25)
            );
            let _ = writeln!(
                self.o,
                "#grid(columns: ({}), align: center + horizon, stroke: (x, _) => if x > 0 {{ (left: 0.5pt + {}) }},",
                vec!["1fr"; n].join(", ") + if n == 1 { "," } else { "" },
                rgb(t.rule)
            );
            for l in &d.cover_logos {
                self.o.push_str("  [");
                if let Some(path) = self.asset_path(&l.asset).map(str::to_string) {
                    let _ = write!(
                        self.o,
                        "#block(height: {}mm, above: 0mm, below: 2mm)[#align(center + horizon)[#image({}, height: {}mm, alt: {})]]",
                        num(PHOTO.logo_box_mm),
                        lit(&path),
                        num(PHOTO.logo_mm),
                        lit(&l.name)
                    );
                }
                let _ = write!(
                    self.o,
                    "#block(above: 0mm, below: 1mm)[#text(size: {}pt)[#strong[{}]]]",
                    num(PHOTO.logo_name_pt),
                    esc(&l.name)
                );
                if let Some(r) = &l.role {
                    let _ = write!(
                        self.o,
                        "#block(above: 0mm, below: 0mm)[#text(size: {}pt, tracking: 0.08em, fill: {})[#upper({})]]",
                        num(PHOTO.logo_role_pt),
                        rgb(t.ink_2),
                        lit(r)
                    );
                }
                self.o.push_str("],\n");
            }
            self.o.push_str(")]\n");
        }
        let rows = p.cover_rows();
        if !rows.is_empty() {
            let line =
                format!("0.5pt + {}", rgb(t.cover_ink.over(t.cover_panel_ground.unwrap_or(t.paper), 0.18)));
            let label_ink = rgb(t.cover_ink.over(t.paper, 0.65));
            let fill = t.cover_panel_ground.map(|c| format!(", fill: {}", rgb(c))).unwrap_or_default();
            let _ = writeln!(
                self.o,
                "#block(width: 100%, above: {}mm, below: 0mm, stroke: {line}, radius: 1mm, clip: true{fill})[{}\n#table(columns: ({}%, 1fr), stroke: (bottom: {line}), inset: (x: {}mm, y: {}mm),",
                num(PHOTO.panel_above_mm),
                line_box(1.3),
                num(PHOTO.panel_label_col * 100.0),
                num(PHOTO.panel_pad_x_mm),
                num(PHOTO.panel_pad_y_mm),
            );
            let _ = writeln!(
                self.o,
                "  table.cell(colspan: 2)[#text(size: {}pt, tracking: 0.14em, fill: {accent})[#strong[#upper({})]]],",
                num(PHOTO.panel_head_pt),
                lit(CONTROL)
            );
            for (l, v) in &rows {
                let _ = writeln!(
                    self.o,
                    "  [#text(size: {}pt, tracking: 0.04em, fill: {label_ink})[#upper({})]], [#text(size: {}pt)[#strong[{}]]],",
                    num(PHOTO.panel_label_pt),
                    lit(l),
                    num(PHOTO.panel_value_pt),
                    esc(v)
                );
            }
            self.o.push_str(")]\n");
        }
    }

    fn control(&mut self) {
        let p = self.p;
        let c = &p.doc.control;
        let t = &p.template;
        self.heading(1, false, CONTROL, None);
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
                    if s.level == 1 && self.p.template.section_page_break {
                        self.o.push_str("#pagebreak(weak: true)\n");
                    }
                    let outlined = self.toc.contains(s.anchor.as_str());
                    self.heading(s.level, outlined, &self.p.heading(s), Some(&s.anchor));
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
                        lit(&t.heading_font),
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
        self.table_open_with(aligns, columns, 1);
    }

    /// `table_open` with `header_rows` rows on the header ground (none for 0).
    fn table_open_with(&mut self, aligns: &[Align], columns: Option<&str>, header_rows: usize) {
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
        // Every second body row takes the stripe (0.2), in a table with a header row.
        let stripe = match t.table_stripe {
            Some(c) if header_rows > 0 => {
                format!(" else if calc.odd(y - {header_rows}) {{ {} }}", rgb(c))
            }
            _ => String::new(),
        };
        let padding =
            if header_rows == 0 { t.headless_padding_mm.or(t.table_padding_mm) } else { t.table_padding_mm };
        let inset = match padding {
            Some([y, x]) => format!("(x: {}mm, y: {}mm)", num(x.into()), num(y.into())),
            None => "(x: 0.5em, y: 0.35em)".into(),
        };
        let _ = writeln!(
            self.o,
            "#table(columns: {cols}, align: {al}, fill: (_, y) => if y < {header_rows} {{ {} }}{stripe}, stroke: (top: none, left: none, right: none, bottom: 0.5pt + {}), inset: {inset},",
            rgb(t.accent_ground),
            rgb(t.rule)
        );
    }

    fn table(&mut self, t: &RTable) {
        let aligns: Vec<Align> = t.columns.iter().map(|c| c.align).collect();
        self.o.push_str("#block(width: 100%, above: 1.2em, below: 1.2em)[\n");
        if let Some(caption) = t.caption_text() {
            let _ = writeln!(self.o, "#block(below: 0.5em, sticky: true)[#strong[{}]]", esc(&caption));
        }
        if self.p.template.table_tabular_figures {
            self.o.push_str("#set text(number-width: \"tabular\")\n");
        }
        let size = if t.header { None } else { self.p.template.headless_size_pt };
        if let Some(size) = size.or(self.p.template.table_size_pt) {
            let _ = writeln!(self.o, "#set text(size: {}pt)", num(size.into()));
        }
        let header_rows = match (t.header, t.groups.is_empty()) {
            (false, _) => 0,
            (true, true) => 1,
            (true, false) => 2,
        };
        let widths = self.p.column_widths(t).map(|w| {
            let parts: Vec<String> = w.iter().map(|f| format!("{}%", num(f * 100.0))).collect();
            format!("({}{})", parts.join(", "), if parts.len() == 1 { "," } else { "" })
        });
        self.table_open_with(&aligns, widths.as_deref(), header_rows);
        let head_ink = rgb(self.p.template.table_header_ink);
        if t.header {
            self.o.push_str("  table.header(repeat: true, ");
            // The group row, in column order: a titled group spans its columns; a column under a group with
            // no title has its own title spanning both rows. Then the other column titles. Typst places each
            // cell in the next free slot, so this is the order the HTML and Word writers use too.
            let lone = |i: usize| t.group_of(i).is_some_and(|(g, _)| t.groups[g].0.is_empty());
            let head = |s: &str| format!("#text(fill: {head_ink})[#strong[{}]]", esc(s));
            if !t.groups.is_empty() {
                for (i, c) in t.columns.iter().enumerate() {
                    match t.group_of(i) {
                        Some((g, first)) if !t.groups[g].0.is_empty() => {
                            if first == i {
                                let (title, span) = &t.groups[g];
                                let _ = write!(
                                    self.o,
                                    "table.cell(colspan: {span}, align: center + top)[{}], ",
                                    head(title)
                                );
                            }
                        }
                        _ => {
                            let _ = write!(self.o, "table.cell(rowspan: 2)[{}], ", head(&c.header()));
                        }
                    }
                }
            }
            for (i, c) in t.columns.iter().enumerate() {
                if t.groups.is_empty() || !lone(i) {
                    let _ = write!(self.o, "[{}], ", head(&c.header()));
                }
            }
            self.o.push_str("),\n");
        }
        let n = t.columns.len();
        // A table with no header row is labels and values: the label column is in the second ink.
        let label_ink = (!t.header).then(|| format!("#set text(fill: {})\n", rgb(self.p.template.ink_2)));
        for row in &t.rows {
            self.o.push(' ');
            for i in 0..n {
                self.o.push_str(" [");
                if i == 0
                    && let Some(l) = &label_ink
                {
                    self.o.push_str(l);
                }
                if let Some(c) = row.get(i) {
                    self.inlines(cell_inlines(c));
                }
                self.o.push_str("],");
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

/// The Typst source of the photo cover's content alone, on a page the width of the text and as tall as the
/// content: its height is what the photograph leaves room for (`crate::write::pdf::band_mm`).
pub fn cover_measure(p: &Prepared) -> Markup {
    let mut m = markup_with_band(p, 0.0, false);
    let t = &p.template;
    let mut w = W {
        p,
        o: std::mem::take(&mut m.source),
        paths: Vec::new(),
        anchors: BTreeSet::new(),
        toc: BTreeSet::new(),
        band_mm: 0.0,
    };
    // `markup_with_band(.., false)` wrote only the preamble; the paths it made are the same.
    for (i, (id, a)) in p.assets.iter().enumerate() {
        let ext = match a.media_type {
            MediaType::Png => "png",
            MediaType::Svg => "svg",
            MediaType::Jpeg => "jpg",
            MediaType::Font => continue,
        };
        w.paths.push((id.as_str(), format!("/a{i}.{ext}")));
    }
    let _ = writeln!(
        w.o,
        "#set page(width: {}mm, height: auto, margin: 0pt, header: none, footer: none, fill: none)",
        num(f64::from(t.page_width_mm - t.margin_left_mm - t.margin_right_mm))
    );
    w.photo_body();
    m.source = w.o;
    m
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
