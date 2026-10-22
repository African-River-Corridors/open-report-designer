//! The HTML writer (spec 0001 §10): one self-contained, well-formed file.
//!
//! Styles, fonts (woff2) and images (data URIs) are inline, so the file opens anywhere with no network. Print
//! styles give an A4 page with the template's margins. The markup is also well-formed XML, so an XML reader
//! can read it back (the conformance suite does).

use std::fmt::Write as _;

use base64::Engine as _;

use crate::markdown::{Inline, MdBlock};
use crate::model::Align;
use crate::render::Prepared;
use crate::resolve::{RBlock, RFigure, RTable, heading_text};

/// Writes the document as one HTML file.
pub fn write(p: &Prepared) -> String {
    let d = &p.doc;
    let mut h = String::with_capacity(512 * 1024);
    h.push_str("<!doctype html>\n");
    let _ = writeln!(h, "<html lang=\"{}\">", esc(&d.language));
    h.push_str("<head>\n<meta charset=\"utf-8\"/>\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>\n");
    let _ = writeln!(h, "<meta name=\"generator\" content=\"Open Report Designer {}\"/>", crate::VERSION);
    if let Some(o) = &d.organisation {
        let _ = writeln!(h, "<meta name=\"author\" content=\"{}\"/>", esc(o));
    }
    let _ = writeln!(h, "<title>{}</title>", esc(&d.title));
    h.push_str("<style>\n");
    style(p, &mut h);
    h.push_str("</style>\n</head>\n<body>\n");

    // Page furniture: repeated on every printed page (position: fixed), shown once on screen.
    h.push_str("<div class=\"running-head\">");
    let _ = write!(h, "<span>{}</span>", esc(&p.header_left()));
    if let Some(f) = &d.control.furniture {
        let _ = write!(h, "<span class=\"furniture\">{}</span>", esc(f));
    }
    h.push_str("</div>\n");

    if p.template.cover {
        cover(p, &mut h);
    } else {
        let _ = writeln!(h, "<h1 class=\"doc-title\">{}</h1>", esc(&d.title));
    }
    control(p, &mut h);
    if d.contents && !d.toc.is_empty() {
        h.push_str(
            "<nav class=\"contents\" aria-label=\"Contents\">\n<h2 class=\"front\">Contents</h2>\n<ul>\n",
        );
        for e in &d.toc {
            let _ = writeln!(
                h,
                "<li class=\"toc-{}\"><a href=\"#{}\">{}</a></li>",
                e.level,
                esc(&e.anchor),
                esc(&heading_text(e.number.as_deref(), &e.title))
            );
        }
        h.push_str("</ul>\n</nav>\n");
    }

    h.push_str("<main>\n");
    blocks(p, &d.body, &mut h);
    for a in &d.appendices {
        let _ = writeln!(h, "<section class=\"appendix\" id=\"{}\">", esc(&a.anchor));
        let _ = writeln!(h, "<h2>{}</h2>", esc(&a.heading()));
        blocks(p, &a.body, &mut h);
        h.push_str("</section>\n");
    }
    h.push_str("</main>\n");

    let foot = p.footer_left();
    if !foot.is_empty() {
        let _ = writeln!(h, "<footer class=\"running-foot\"><span>{}</span></footer>", esc(&foot));
    }
    h.push_str("</body>\n</html>\n");
    h
}

fn cover(p: &Prepared, h: &mut String) {
    let d = &p.doc;
    h.push_str("<header class=\"cover\">\n");
    if let Some(logo) = p.template.organisation_logo.as_ref().and_then(|id| p.assets.get(id)) {
        let _ = writeln!(h, "<img class=\"logo\" src=\"{}\" alt=\"\"/>", data_uri(logo));
    }
    let _ = writeln!(h, "<p class=\"class-name\">{}</p>", esc(&d.control.class_name));
    let _ = writeln!(h, "<h1 class=\"doc-title\">{}</h1>", esc(&d.title));
    if let Some(s) = &d.subtitle {
        let _ = writeln!(h, "<p class=\"subtitle\">{}</p>", esc(s));
    }
    if let Some(o) = &d.organisation {
        let _ = writeln!(h, "<p class=\"organisation\">{}</p>", esc(o));
    }
    if let Some(s) = &d.summary {
        h.push_str("<div class=\"summary\">\n");
        md_blocks(s, h);
        h.push_str("</div>\n");
    }
    if let Some(n) = &d.notice {
        let _ = writeln!(h, "<p class=\"notice\">{}</p>", esc(n));
    }
    h.push_str("<dl class=\"cover-control\">\n");
    for (label, value) in p.cover_control() {
        let _ = writeln!(h, "<div><dt>{}</dt><dd>{}</dd></div>", esc(label), esc(&value));
    }
    h.push_str("</dl>\n</header>\n");
}

fn control(p: &Prepared, h: &mut String) {
    let c = &p.doc.control;
    h.push_str("<section class=\"document-control\">\n<h2 class=\"front\">Document control</h2>\n");
    h.push_str("<table class=\"control\">\n<tbody>\n");
    for (label, value) in c.rows() {
        let _ = writeln!(h, "<tr><th scope=\"row\">{}</th><td>{}</td></tr>", esc(label), esc(&value));
    }
    h.push_str("</tbody>\n</table>\n");
    if !c.history.is_empty() {
        h.push_str("<h3 class=\"front\">Revision history</h3>\n<table class=\"history\">\n<thead><tr>");
        for t in crate::resolve::ControlView::HISTORY_COLUMNS {
            let _ = write!(h, "<th>{}</th>", esc(t));
        }
        h.push_str("</tr></thead>\n<tbody>\n");
        for e in &c.history {
            let _ = writeln!(
                h,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&e.revision),
                esc(&e.date),
                esc(&p.status_label(&e.status)),
                esc(&e.description)
            );
        }
        h.push_str("</tbody>\n</table>\n");
    }
    h.push_str("</section>\n");
}

fn blocks(p: &Prepared, bs: &[RBlock], h: &mut String) {
    for b in bs {
        match b {
            RBlock::Section(s) => {
                let _ = writeln!(h, "<section id=\"{}\">", esc(&s.anchor));
                // The document title is h1; a level-1 section is h2.
                let tag = format!("h{}", s.level + 1);
                let cls = if s.number.is_none() { " class=\"unnumbered\"" } else { "" };
                let _ = writeln!(h, "<{tag}{cls}>{}</{tag}>", esc(&s.heading()));
                blocks(p, &s.body, h);
                h.push_str("</section>\n");
            }
            RBlock::Prose(md) => md_blocks(md, h),
            RBlock::Table(t) => table(t, h),
            RBlock::Figure(f) => figure(p, f, h),
            RBlock::DataBox(d) => {
                let _ = writeln!(h, "<aside class=\"data-box\" id=\"{}\">", esc(&d.anchor));
                let _ = writeln!(h, "<p class=\"data-box-title\">{}</p>", esc(&d.title));
                if !d.items.is_empty() {
                    h.push_str("<dl>\n");
                    for (label, value) in &d.items {
                        let _ = writeln!(h, "<div><dt>{}</dt><dd>{}</dd></div>", esc(label), esc(value));
                    }
                    h.push_str("</dl>\n");
                }
                if let Some(n) = &d.notes {
                    h.push_str("<div class=\"notes\">\n");
                    md_blocks(n, h);
                    h.push_str("</div>\n");
                }
                h.push_str("</aside>\n");
            }
        }
    }
}

fn table(t: &RTable, h: &mut String) {
    let _ = writeln!(h, "<table class=\"data\" id=\"{}\">", esc(&t.anchor));
    let _ = writeln!(h, "<caption>{}</caption>", esc(&t.caption_text()));
    h.push_str("<thead><tr>");
    for c in &t.columns {
        let _ = write!(h, "<th{}>{}</th>", align(c.align), esc(&c.header()));
    }
    h.push_str("</tr></thead>\n<tbody>\n");
    for row in &t.rows {
        h.push_str("<tr>");
        for (i, cell) in row.iter().enumerate() {
            let a = t.columns.get(i).map(|c| c.align).unwrap_or_default();
            let _ = write!(h, "<td{}>{}</td>", align(a), esc(cell));
        }
        h.push_str("</tr>\n");
    }
    h.push_str("</tbody>\n</table>\n");
    if let Some(n) = &t.notes {
        h.push_str("<div class=\"table-notes\">\n");
        md_blocks(n, h);
        h.push_str("</div>\n");
    }
}

fn align(a: Align) -> &'static str {
    match a {
        Align::Left => "",
        Align::Centre => " class=\"centre\"",
        Align::Right => " class=\"right\"",
    }
}

fn figure(p: &Prepared, f: &RFigure, h: &mut String) {
    let cls = if f.map_key.is_some() { "map" } else { "figure" };
    let _ = writeln!(h, "<figure class=\"{cls}\" id=\"{}\">", esc(&f.anchor));
    if let Some(a) = p.assets.get(&f.asset) {
        let _ = writeln!(
            h,
            "<img src=\"{}\" alt=\"{}\" style=\"width: {}%\"/>",
            data_uri(a),
            esc(&f.alt),
            fmt_pct(f.width)
        );
    }
    let _ = writeln!(h, "<figcaption>{}</figcaption>", esc(&f.caption_text()));
    if let Some(k) = &f.map_key {
        let _ = writeln!(h, "<p class=\"map-key\">{}</p>", esc(k));
    }
    h.push_str("</figure>\n");
}

fn fmt_pct(w: f64) -> String {
    let pct = (w * 1000.0).round() / 10.0;
    if pct.fract() == 0.0 { format!("{}", pct as i64) } else { format!("{pct}") }
}

fn data_uri(a: &crate::assets::Asset) -> String {
    format!(
        "data:{};base64,{}",
        a.media_type.as_str(),
        base64::engine::general_purpose::STANDARD.encode(&a.bytes)
    )
}

fn md_blocks(bs: &[MdBlock], h: &mut String) {
    for b in bs {
        match b {
            MdBlock::Paragraph(i) => {
                h.push_str("<p>");
                inlines(i, h);
                h.push_str("</p>\n");
            }
            MdBlock::List { ordered, start, items } => {
                if *ordered {
                    if *start == 1 {
                        h.push_str("<ol>\n");
                    } else {
                        let _ = writeln!(h, "<ol start=\"{start}\">");
                    }
                } else {
                    h.push_str("<ul>\n");
                }
                for item in items {
                    h.push_str("<li>");
                    // A list item of one paragraph prints its text without a <p>.
                    match item.as_slice() {
                        [MdBlock::Paragraph(i)] => inlines(i, h),
                        _ => {
                            h.push('\n');
                            md_blocks(item, h);
                        }
                    }
                    h.push_str("</li>\n");
                }
                h.push_str(if *ordered { "</ol>\n" } else { "</ul>\n" });
            }
            MdBlock::Quote(q) => {
                h.push_str("<blockquote>\n");
                md_blocks(q, h);
                h.push_str("</blockquote>\n");
            }
            MdBlock::Code(c) => {
                let _ = writeln!(h, "<pre><code>{}</code></pre>", esc(c));
            }
        }
    }
}

fn inlines(is: &[Inline], h: &mut String) {
    for i in is {
        match i {
            Inline::Text(t) => h.push_str(&esc(t)),
            Inline::Emph(c) => {
                h.push_str("<em>");
                inlines(c, h);
                h.push_str("</em>");
            }
            Inline::Strong(c) => {
                h.push_str("<strong>");
                inlines(c, h);
                h.push_str("</strong>");
            }
            Inline::Code(c) => {
                let _ = write!(h, "<code>{}</code>", esc(c));
            }
            Inline::Link { url, children } => {
                if safe_url(url) {
                    let _ = write!(h, "<a href=\"{}\">", esc(url));
                    inlines(children, h);
                    h.push_str("</a>");
                } else {
                    inlines(children, h);
                }
            }
            Inline::XRef { text, anchor, .. } => {
                let _ = write!(h, "<a class=\"xref\" href=\"#{}\">{}</a>", esc(anchor), esc(text));
            }
            Inline::LineBreak => h.push_str("<br/>"),
        }
    }
}

/// Only web, mail and in-page links become links; anything else (such as `javascript:`) prints as text.
pub fn safe_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:") || u.starts_with('#')
}

/// Escapes text for HTML (and XML): `&`, `<`, `>`, `"` and `'`. Control characters that XML forbids are dropped.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            '\t' | '\n' | '\r' => o.push(c),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => o.push(c),
        }
    }
    o
}

fn style(p: &Prepared, h: &mut String) {
    let t = &p.template;
    for f in crate::fonts::bundled() {
        let _ = writeln!(
            h,
            "@font-face {{ font-family: \"{}\"; font-weight: {}; font-style: {}; src: url(data:font/woff2;base64,{}) format(\"woff2\"); }}",
            f.family,
            f.weight,
            if f.italic { "italic" } else { "normal" },
            base64::engine::general_purpose::STANDARD.encode(f.woff2)
        );
    }
    let tnum = if t.table_tabular_figures { "font-variant-numeric: tabular-nums;" } else { "" };
    let _ = write!(
        h,
        r#":root {{ --paper: {paper}; --body-paper: {body_paper}; --ink: {ink}; --ink-2: {ink2}; --rule: {rule};
  --accent-ground: {accent}; --data-ground: {data}; }}
html {{ background: var(--body-paper); color: var(--ink); }}
body {{ margin: 0 auto; max-width: 46rem; padding: 2rem 1.25rem 4rem; font-family: "{body}", system-ui, sans-serif;
  font-size: {size}pt; line-height: 1.5; hyphens: manual; text-align: left; }}
h1, h2, h3, h4, h5 {{ font-family: "{head}", Georgia, serif; font-weight: 400; line-height: 1.25; break-after: avoid; }}
h1 {{ font-size: 2em; }} h2 {{ font-size: 1.6em; margin-top: 2em; }} h3 {{ font-size: 1.3em; }}
h4 {{ font-size: 1.1em; }} h5 {{ font-size: 1em; font-style: italic; }}
code, pre {{ font-family: "{mono}", ui-monospace, monospace; font-size: 0.9em; }}
pre {{ background: var(--data-ground); padding: 0.75em 1em; overflow-x: auto; white-space: pre-wrap; }}
a {{ color: inherit; }}
blockquote {{ margin: 1em 0; padding-left: 1em; border-left: 1px solid var(--rule); color: var(--ink-2); }}
.running-head {{ display: flex; justify-content: space-between; color: var(--ink-2); font-size: 0.8em;
  border-bottom: 0.5pt solid var(--rule); padding-bottom: 0.25em; }}
.running-head .furniture {{ text-transform: uppercase; letter-spacing: 0.12em; }}
.running-foot {{ margin-top: 3rem; color: var(--ink-2); font-size: 0.8em; border-top: 0.5pt solid var(--rule); padding-top: 0.25em; }}
.cover {{ background: var(--paper); margin: 1rem -1.25rem 2rem; padding: 3rem 2rem; }}
.cover .class-name {{ text-transform: uppercase; letter-spacing: 0.2em; font-size: 0.85em; }}
.cover .doc-title {{ font-size: 2.4em; margin: 0.25em 0; }}
.cover .subtitle {{ font-family: "{head}", Georgia, serif; font-style: italic; font-size: 1.3em; margin: 0; }}
.cover .organisation {{ margin-top: 1.5em; }}
.cover .logo {{ max-height: 3rem; max-width: 12rem; }}
.cover .notice {{ font-weight: 700; }}
.cover-control {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr)); gap: 0.5rem 1rem;
  border-top: 1pt solid var(--ink); padding-top: 0.75em; margin-top: 2em; }}
.cover-control dt {{ font-size: 0.75em; text-transform: uppercase; letter-spacing: 0.12em; color: var(--ink-2); }}
.cover-control dd {{ margin: 0; }}
table {{ border-collapse: collapse; width: 100%; margin: 1em 0; {tnum} }}
th, td {{ border-bottom: 0.5pt solid var(--rule); padding: 0.3em 0.5em; text-align: left; vertical-align: top; }}
thead th {{ background: var(--accent-ground); font-weight: 700; }}
table.control th {{ width: 30%; font-weight: 700; }}
caption {{ caption-side: top; text-align: left; font-weight: 700; padding-bottom: 0.4em; }}
.right {{ text-align: right; }} .centre {{ text-align: center; }}
.table-notes {{ font-size: 0.9em; color: var(--ink-2); }}
figure {{ margin: 1.5em 0; text-align: center; break-inside: avoid; }}
figure img {{ max-width: 100%; height: auto; }}
figcaption {{ font-weight: 700; text-align: left; margin-top: 0.4em; }}
.map-key {{ text-align: left; font-size: 0.85em; color: var(--ink-2); margin: 0.2em 0 0; }}
.data-box {{ background: var(--data-ground); border-left: {rule_w}pt solid var(--ink); padding: 0.75em 1em; margin: 1.5em 0; break-inside: avoid; }}
.data-box-title {{ font-family: "{head}", Georgia, serif; font-size: 1.15em; margin: 0 0 0.5em; }}
.data-box dl {{ display: grid; grid-template-columns: minmax(8rem, 30%) 1fr; gap: 0.25em 1em; margin: 0; }}
.data-box dl div {{ display: contents; }}
.data-box dt {{ font-weight: 700; }} .data-box dd {{ margin: 0; }}
.contents ul {{ list-style: none; padding: 0; }}
.contents li.toc-2 {{ padding-left: 1.5em; }} .contents li.toc-3 {{ padding-left: 3em; }}
@page {{ size: {pw}mm {ph}mm; margin: {mt}mm {mr}mm {mb}mm {ml}mm; }}
@media print {{
  body {{ max-width: none; padding: 0; }}
  .running-head {{ position: fixed; top: -{head_off}mm; left: 0; right: 0; }}
  .running-foot {{ position: fixed; bottom: -{head_off}mm; left: 0; right: 0; margin: 0; }}
  .cover {{ margin: 0; min-height: {cover_h}mm; break-after: page; print-color-adjust: exact; -webkit-print-color-adjust: exact; }}
  .document-control, .contents {{ break-after: page; }}
  main > section, .appendix {{ break-before: page; }}
  a {{ text-decoration: none; }}
}}
"#,
        paper = t.paper,
        body_paper = t.body_paper,
        ink = t.ink,
        ink2 = t.ink_2,
        rule = t.rule,
        accent = t.accent_ground,
        data = t.data_ground,
        body = t.body_font,
        head = t.heading_font,
        mono = t.mono_font,
        size = t.body_size_pt,
        rule_w = t.data_rule_pt,
        pw = t.page_width_mm,
        ph = t.page_height_mm,
        mt = t.margin_top_mm,
        mr = t.margin_right_mm,
        mb = t.margin_bottom_mm,
        ml = t.margin_left_mm,
        head_off = t.margin_top_mm * 0.6,
        cover_h = t.page_height_mm - t.margin_top_mm - t.margin_bottom_mm,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping() {
        assert_eq!(esc("a<b>&\"c'\u{1}"), "a&lt;b&gt;&amp;&quot;c&#39;");
        assert!(!safe_url("javascript:alert(1)") && safe_url("https://x") && safe_url("#a"));
        assert_eq!([fmt_pct(0.9), fmt_pct(1.0), fmt_pct(0.333)], ["90", "100", "33.3"]);
    }
}
