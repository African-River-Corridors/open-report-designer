//! The Word writer (spec 0001 §10), read back by an independent reader: the `zip` crate and `roxmltree`.
//! Label: sourced - spec 0001 §7 (page, margins, fonts, colours), ECMA-376 Part 1 §17.8.1 (font obfuscation)
//! and the page arithmetic of A4 (210 × 297 mm = 11 906 × 16 838 twips).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use ord::{Format, builtin_register, render};
use roxmltree::{Document as Xml, Node};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const ASVG: &str = "http://schemas.microsoft.com/office/drawing/2016/SVG/main";

/// Every file of a package, by name, in the zip's order.
struct Pkg {
    names: Vec<String>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Pkg {
    fn read(bytes: &[u8]) -> Pkg {
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).expect("a zip archive");
        let mut names = Vec::new();
        let mut files = BTreeMap::new();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            names.push(f.name().to_string());
            files.insert(f.name().to_string(), data);
        }
        Pkg { names, files }
    }

    fn text(&self, name: &str) -> String {
        String::from_utf8(self.files.get(name).unwrap_or_else(|| panic!("no part {name}")).clone()).unwrap()
    }

    /// The relationships of a part: id to (type, resolved target, external).
    fn rels(&self, part: &str) -> BTreeMap<String, (String, String, bool)> {
        let (dir, file) = part.rsplit_once('/').map_or(("", part), |(d, f)| (d, f));
        let rels_name =
            if dir.is_empty() { format!("_rels/{file}.rels") } else { format!("{dir}/_rels/{file}.rels") };
        let Some(_) = self.files.get(&rels_name) else { return BTreeMap::new() };
        let text = self.text(&rels_name);
        let x = Xml::parse(&text).unwrap();
        x.descendants()
            .filter(|n| n.has_tag_name("Relationship"))
            .map(|n| {
                let external = n.attribute("TargetMode") == Some("External");
                let target = n.attribute("Target").unwrap().to_string();
                let resolved = if external { target } else { join(dir, &target) };
                (
                    n.attribute("Id").unwrap().to_string(),
                    (n.attribute("Type").unwrap().to_string(), resolved, external),
                )
            })
            .collect()
    }
}

fn join(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> =
        if target.starts_with('/') { Vec::new() } else { dir.split('/').filter(|s| !s.is_empty()).collect() };
    for seg in target.trim_start_matches('/').split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn docx(doc: &ord::Document, assets: &ord::Assets) -> Vec<u8> {
    render(doc, assets, &builtin_register(), Format::Docx).expect("renders").bytes
}

fn example_docx() -> Vec<u8> {
    let (doc, assets) = common::example();
    docx(&doc, &assets)
}

fn wattr<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    n.attribute((W, name))
}

/// The text of a paragraph: its `w:t` (not field instructions), with `w:tab` as a tab and `w:br` as a new line.
fn para_text(p: Node) -> String {
    let mut s = String::new();
    for n in p.descendants() {
        if n.has_tag_name((W, "t")) {
            s.push_str(n.text().unwrap_or(""));
        } else if n.has_tag_name((W, "tab")) && n.parent().is_some_and(|r| r.has_tag_name((W, "r"))) {
            s.push('\t');
        } else if n.has_tag_name((W, "br")) {
            s.push('\n');
        }
    }
    s
}

fn para_style(p: Node) -> String {
    p.children()
        .find(|n| n.has_tag_name((W, "pPr")))
        .and_then(|ppr| ppr.children().find(|n| n.has_tag_name((W, "pStyle"))))
        .and_then(|s| wattr(s, "val"))
        .unwrap_or("Normal")
        .to_string()
}

/// Every non-empty paragraph of the document body, in order, with its style.
fn paragraphs(pkg: &Pkg) -> Vec<(String, String)> {
    let text = pkg.text("word/document.xml");
    let x = Xml::parse(&text).unwrap();
    x.descendants()
        .filter(|n| n.has_tag_name((W, "p")))
        .map(|p| (para_style(p), para_text(p)))
        .filter(|(_, t)| !t.is_empty())
        .collect()
}

#[test]
fn every_part_parses_is_typed_and_every_relationship_resolves() {
    let pkg = Pkg::read(&example_docx());
    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/styles.xml",
        "word/settings.xml",
        "word/numbering.xml",
        "word/fontTable.xml",
        "word/header1.xml",
        "word/footer1.xml",
    ] {
        assert!(pkg.files.contains_key(required), "missing {required}");
    }
    assert_eq!(pkg.names[0], "[Content_Types].xml");
    for (name, data) in &pkg.files {
        if name.ends_with(".xml") || name.ends_with(".rels") || name.ends_with(".svg") {
            let text = std::str::from_utf8(data).unwrap_or_else(|_| panic!("{name} is not UTF-8"));
            Xml::parse(text).unwrap_or_else(|e| panic!("{name} does not parse: {e}"));
        }
    }

    // Content types cover every part.
    let ct = pkg.text("[Content_Types].xml");
    let ct = Xml::parse(&ct).unwrap();
    let defaults: BTreeSet<String> = ct
        .descendants()
        .filter(|n| n.has_tag_name("Default"))
        .map(|n| n.attribute("Extension").unwrap().to_ascii_lowercase())
        .collect();
    let overrides: BTreeSet<String> = ct
        .descendants()
        .filter(|n| n.has_tag_name("Override"))
        .map(|n| n.attribute("PartName").unwrap().trim_start_matches('/').to_string())
        .collect();
    for name in pkg.names.iter().filter(|n| *n != "[Content_Types].xml") {
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        assert!(overrides.contains(name) || defaults.contains(&ext), "{name} has no content type");
    }
    for o in &overrides {
        assert!(pkg.files.contains_key(o), "an override names a missing part {o}");
    }
    assert!(ct.descendants().any(|n| n.attribute("PartName") == Some("/word/document.xml")
        && n.attribute("ContentType")
            == Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml")));

    // Every internal relationship of every part resolves to a part of the package.
    let mut checked = 0;
    for name in pkg.names.iter().filter(|n| n.ends_with(".rels")) {
        let source = name.replace("_rels/", "").trim_end_matches(".rels").to_string();
        for (id, (_, target, external)) in pkg.rels(&source) {
            if !external {
                assert!(pkg.files.contains_key(&target), "{name} {id} -> {target} is missing");
                checked += 1;
            }
        }
    }
    assert!(checked >= 20, "only {checked} relationships");

    // Every r:id and r:embed used in the document, header, footer and font table is declared.
    for part in ["word/document.xml", "word/header1.xml", "word/footer1.xml", "word/fontTable.xml"] {
        let rels = pkg.rels(part);
        let text = pkg.text(part);
        let x = Xml::parse(&text).unwrap();
        for n in x.descendants() {
            for a in n.attributes().filter(|a| a.namespace() == Some(R)) {
                assert!(
                    rels.contains_key(a.value()),
                    "{part}: {} names an undeclared {}",
                    n.tag_name().name(),
                    a.value()
                );
            }
        }
    }
}

#[test]
fn page_styles_fonts_and_properties_follow_the_template() {
    let pkg = Pkg::read(&example_docx());
    let doc = pkg.text("word/document.xml");
    let x = Xml::parse(&doc).unwrap();
    let sects: Vec<Node> = x.descendants().filter(|n| n.has_tag_name((W, "sectPr"))).collect();
    assert_eq!(sects.len(), 2, "a cover section and the body section");
    for s in &sects {
        let sz = s.children().find(|n| n.has_tag_name((W, "pgSz"))).unwrap();
        assert_eq!((wattr(sz, "w"), wattr(sz, "h")), (Some("11906"), Some("16838")), "A4");
        let m = s.children().find(|n| n.has_tag_name((W, "pgMar"))).unwrap();
        // 25 mm = 1417 twips, 20 mm = 1134 twips.
        assert_eq!(
            [wattr(m, "top"), wattr(m, "bottom"), wattr(m, "left"), wattr(m, "right")],
            [Some("1417"), Some("1417"), Some("1134"), Some("1134")]
        );
    }
    let refs = |s: &Node| {
        s.children()
            .filter(|n| n.has_tag_name((W, "headerReference")) || n.has_tag_name((W, "footerReference")))
            .count()
    };
    assert_eq!(refs(&sects[0]), 0, "the cover has no running head or foot");
    assert_eq!(refs(&sects[1]), 2);

    // The cover's paper: a picture behind the text, the size of the page, at the page's corner.
    let anchor = x.descendants().find(|n| n.has_tag_name((WP, "anchor"))).expect("a page-ground picture");
    assert_eq!(anchor.attribute("behindDoc"), Some("1"));
    let ext = anchor.children().find(|n| n.has_tag_name((WP, "extent"))).unwrap();
    assert_eq!((ext.attribute("cx"), ext.attribute("cy")), (Some("7560000"), Some("10692000")));
    let ph = anchor.children().find(|n| n.has_tag_name((WP, "positionH"))).unwrap();
    assert_eq!(ph.attribute("relativeFrom"), Some("page"));
    let blip = anchor.descendants().find(|n| n.has_tag_name((A, "blip"))).unwrap();
    let paper = &pkg.rels("word/document.xml")[blip.attribute((R, "embed")).unwrap()].1;
    let png = &pkg.files[paper];
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!((&png[16..20], &png[20..24]), (&[0, 0, 0, 1][..], &[0, 0, 0, 1][..]), "one pixel");

    // Styles: the fonts, size and colours of the template.
    let styles = pkg.text("word/styles.xml");
    let s = Xml::parse(&styles).unwrap();
    let style = |id: &str| {
        s.descendants()
            .find(|n| n.has_tag_name((W, "style")) && wattr(*n, "styleId") == Some(id))
            .unwrap_or_else(|| panic!("no style {id}"))
    };
    for id in [
        "Normal",
        "Title",
        "Subtitle",
        "Heading1",
        "Heading2",
        "Heading3",
        "Heading4",
        "Heading5",
        "FrontHeading",
        "Caption",
        "MapKey",
        "Code",
        "Quote",
        "TableText",
        "DataBoxTitle",
        "ClassName",
        "Notice",
        "TOC1",
        "TOC2",
        "TOC3",
        "Hyperlink",
        "ReportTable",
    ] {
        style(id);
    }
    let font_of = |n: Node| {
        n.descendants()
            .find(|r| r.has_tag_name((W, "rFonts")))
            .and_then(|r| wattr(r, "ascii"))
            .map(str::to_string)
    };
    let defaults = s.descendants().find(|n| n.has_tag_name((W, "rPrDefault"))).unwrap();
    assert_eq!(font_of(defaults).as_deref(), Some("Public Sans"));
    let sz = defaults.descendants().find(|n| n.has_tag_name((W, "sz"))).unwrap();
    assert_eq!(wattr(sz, "val"), Some("20"), "10 pt");
    let colour = defaults.descendants().find(|n| n.has_tag_name((W, "color"))).unwrap();
    assert_eq!(wattr(colour, "val"), Some("2B2925"));
    for id in ["Title", "Heading1", "Heading4", "FrontHeading", "DataBoxTitle"] {
        assert_eq!(font_of(style(id)).as_deref(), Some("Libre Caslon Text"), "{id}");
    }
    assert_eq!(font_of(style("Code")).as_deref(), Some("IBM Plex Mono"));
    let outline = |id: &str| {
        style(id)
            .descendants()
            .find(|n| n.has_tag_name((W, "outlineLvl")))
            .and_then(|n| wattr(n, "val"))
            .map(str::to_string)
    };
    assert_eq!(outline("Heading1").as_deref(), Some("0"));
    assert_eq!(outline("Heading3").as_deref(), Some("2"));
    assert_eq!(outline("FrontHeading"), None, "front headings stay out of the contents");

    // Settings: fields update on open, fonts are embedded.
    let settings = pkg.text("word/settings.xml");
    assert!(settings.contains("<w:updateFields w:val=\"true\"/>"));
    assert!(settings.contains("<w:embedTrueTypeFonts/>"));

    // Properties.
    let core = pkg.text("docProps/core.xml");
    assert!(core.contains("<dc:title>Heron River navigation screening</dc:title>"));
    assert!(core.contains("<dc:creator>Example Waterways (a fictional consultancy)</dc:creator>"));
    assert!(core.contains("<dc:language>en</dc:language>"));
    assert!(core.contains("2026-10-22T00:00:00Z"));
    let app = pkg.text("docProps/app.xml");
    assert!(app.contains(&format!("<Application>Open Report Designer {}</Application>", ord::VERSION)));

    // Header and footer.
    let header = Xml::parse(&pkg.text("word/header1.xml")).map(|x| para_text(x.root_element())).unwrap();
    assert_eq!(header, "RPT-0042  Rev 1");
    let footer_xml = pkg.text("word/footer1.xml");
    let footer = Xml::parse(&footer_xml).unwrap();
    let instr: Vec<&str> =
        footer.descendants().filter_map(|n| n.attribute((W, "instr"))).map(str::trim).collect();
    assert_eq!(instr, ["PAGE", "NUMPAGES"]);
    assert!(para_text(footer.root_element()).starts_with(
        "Example Waterways (a fictional consultancy) · Screening tool. Not for construction.\tPage "
    ));
}

#[test]
fn a_draft_prints_its_status_in_the_running_head() {
    let mut v = common::with_body(serde_json::json!([common::section(
        "s",
        "Scope",
        serde_json::json!([common::prose("Text.")])
    )]));
    v["control"]["number"] = "RPT-0001".into();
    let pkg = Pkg::read(&docx(&common::doc(&v), &ord::Assets::new()));
    let header = Xml::parse(&pkg.text("word/header1.xml")).map(|x| para_text(x.root_element())).unwrap();
    assert_eq!(header, "RPT-0001  Rev A\tDraft");
}

/// The headings the resolver numbered, in order: what the Word file must print.
fn resolved_headings(doc: &ord::Document) -> Vec<String> {
    fn walk(bs: &[ord::resolve::RBlock], out: &mut Vec<(u8, String)>) {
        for b in bs {
            if let ord::resolve::RBlock::Section(s) = b {
                out.push((s.level, s.heading()));
                walk(&s.body, out);
            }
        }
    }
    let (r, _) = ord::resolve(doc, &builtin_register());
    let mut out = Vec::new();
    walk(&r.body, &mut out);
    for a in &r.appendices {
        out.push((1, a.heading()));
        walk(&a.body, &mut out);
    }
    out.into_iter().map(|(_, h)| h).collect()
}

#[test]
fn the_example_reads_back_in_order_with_its_styles() {
    let (doc, assets) = common::example();
    let pkg = Pkg::read(&docx(&doc, &assets));
    let paras = paragraphs(&pkg);
    let texts: Vec<&str> = paras.iter().map(|(_, t)| t.as_str()).collect();
    let with_style = |style: &str| -> Vec<&str> {
        paras.iter().filter(|(s, _)| s == style).map(|(_, t)| t.as_str()).collect()
    };
    let pos =
        |t: &str| texts.iter().position(|x| *x == t).unwrap_or_else(|| panic!("{t:?} is not a paragraph"));

    // The cover.
    assert_eq!(with_style("ClassName"), ["Report"]);
    assert_eq!(with_style("Title"), ["Heron River navigation screening"]);
    assert_eq!(with_style("Subtitle"), ["Reach from km 12 to km 30"]);
    assert_eq!(with_style("Notice"), ["Screening tool. Not for construction."]);
    assert_eq!(
        with_style("CoverLabel").into_iter().zip(with_style("CoverValue")).collect::<Vec<_>>(),
        [("Document", "RPT-0042"), ("Revision", "1"), ("Date", "2026-10-22"), ("Status", "Issued")]
    );

    // Front matter.
    assert_eq!(with_style("FrontHeading"), ["Document control", "Revision history", "Contents"]);
    let control = pos("Document control");
    let expected_control = [
        "Document number",
        "RPT-0042",
        "Revision",
        "1",
        "Date",
        "2026-10-22",
        "Status",
        "Issued",
        "Sensitivity",
        "Public",
        "Prepared",
        "Screening engineer",
        "Checked",
        "Checking engineer",
        "Approved",
        "Project director",
    ];
    assert_eq!(texts[control + 1..control + 1 + expected_control.len()], expected_control);
    let history = pos("Revision history");
    assert_eq!(texts[history + 1..history + 5], ["Revision", "Date", "Status", "Description"]);
    assert_eq!(texts[history + 5..history + 9], ["A", "2026-10-01", "Draft", "First draft for comment."]);
    assert_eq!(
        texts[history + 9..history + 13],
        ["B", "2026-10-08", "For approval", "Comments on the draft addressed."]
    );

    // Contents: one line per entry, before the body.
    let toc: Vec<&str> =
        paras.iter().filter(|(s, _)| s.starts_with("TOC")).map(|(_, t)| t.as_str()).collect();
    assert_eq!(
        toc,
        [
            "1  Introduction",
            "1.1  Scope",
            "2  Method",
            "3  Results",
            "4  Data and limits",
            "Appendix A  Rule trace",
            "Appendix B  Run record"
        ]
    );

    // Every heading, numbered, in order, with its level's style.
    let headings: Vec<(String, String)> =
        paras.iter().filter(|(s, _)| s.starts_with("Heading")).cloned().collect();
    assert_eq!(headings.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>(), resolved_headings(&doc));
    assert_eq!(
        headings,
        [
            ("Heading1", "1  Introduction"),
            ("Heading2", "1.1  Scope"),
            ("Heading1", "2  Method"),
            ("Heading1", "3  Results"),
            ("Heading1", "4  Data and limits"),
            ("Heading1", "Appendix A  Rule trace"),
            ("Heading1", "Appendix B  Run record"),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
    );

    // Tables: caption above, header cells, body cells, notes after.
    let t1 = pos("Table 1: The three options, screened");
    assert_eq!(paras[t1].0, "Caption");
    assert_eq!(
        texts[t1 + 1..t1 + 6],
        ["Option", "Length (km)", "Smallest radius (m)", "Dredge volume (m³)", "Result"]
    );
    assert_eq!(texts[t1 + 6..t1 + 11], ["Option 1", "18.4", "610", "412 000", "Fails"]);
    assert_eq!(
        texts[t1 + 16..t1 + 22],
        [
            "Option 3",
            "19.6",
            "820",
            "355 000",
            "Passes",
            "Minimum radius 800 m. Volumes are below the design low water."
        ]
    );
    let ta1 = pos("Table A.1: Checks of option 2");
    assert_eq!(texts[ta1 + 6], "Bend radius");
    assert!(ta1 > pos("Appendix A  Rule trace"));

    // Figures and the map: caption after the picture, then the map key.
    let f1 = pos("Figure 1: The reach and the design line of option 2");
    assert_eq!(paras[f1].0, "Caption");
    assert_eq!(paras[f1 + 1].0, "MapKey");
    assert!(texts[f1 + 1].contains("A local grid (invented)") && texts[f1 + 1].contains("1:50 000"));
    assert!(pos("Figure 2: Bed level and the design channel bottom along the reach") > f1);
    assert!(
        pos("Figure 3: The colours of the six option stages") > pos("Table 1: The three options, screened")
    );

    // The data box: title, then each item as label and value, then the notes.
    let dbox = pos("Data and limits");
    assert_eq!(paras[dbox].0, "DataBoxTitle");
    assert_eq!(texts[dbox + 1..dbox + 3], ["Terrain", "Invented, 5 m grid"]);
    assert_eq!(
        texts[dbox + 7..dbox + 10],
        [
            "Coverage",
            "The terrain covers 100 % of every option",
            "Results are screening results. They are not for construction."
        ]
    );

    // Prose: the $ amounts print as written; a quote has its style; the bullets are list paragraphs.
    assert!(texts.iter().any(|t| t.contains("A rate of $5 to $10 per cubic metre, or US$ 7 on average")));
    assert_eq!(with_style("Quote"), ["Screening results rank options. They do not replace a design."]);
    assert_eq!(with_style("ListParagraph").len(), 3);

    // Page breaks before contents, each top-level section and each appendix.
    let xml = pkg.text("word/document.xml");
    let x = Xml::parse(&xml).unwrap();
    let breaks: Vec<String> = x
        .descendants()
        .filter(|n| n.has_tag_name((W, "pageBreakBefore")))
        .map(|n| para_text(n.parent().unwrap().parent().unwrap()))
        .collect();
    assert_eq!(
        breaks,
        [
            "Contents",
            "1  Introduction",
            "2  Method",
            "3  Results",
            "4  Data and limits",
            "Appendix A  Rule trace",
            "Appendix B  Run record"
        ]
    );
}

#[test]
fn contents_is_a_toc_field_with_the_entries_cached() {
    let pkg = Pkg::read(&example_docx());
    let xml = pkg.text("word/document.xml");
    let x = Xml::parse(&xml).unwrap();
    let instr: Vec<&str> =
        x.descendants().filter(|n| n.has_tag_name((W, "instrText"))).filter_map(|n| n.text()).collect();
    assert_eq!(instr, [r#" TOC \o "1-3" \h \z \u "#]);
    let kinds: Vec<&str> = x
        .descendants()
        .filter(|n| n.has_tag_name((W, "fldChar")))
        .filter_map(|n| wattr(n, "fldCharType"))
        .collect();
    assert_eq!(kinds, ["begin", "separate", "end"]);
}

#[test]
fn cross_references_and_contents_link_to_bookmarks() {
    let pkg = Pkg::read(&example_docx());
    let xml = pkg.text("word/document.xml");
    let x = Xml::parse(&xml).unwrap();
    let bookmarks: BTreeMap<&str, String> = x
        .descendants()
        .filter(|n| n.has_tag_name((W, "bookmarkStart")))
        .map(|n| {
            let name = wattr(n, "name").unwrap();
            assert!(name.len() <= 40 && name.starts_with(|c: char| c.is_ascii_alphabetic()), "{name}");
            assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "{name}");
            // The bookmarked text: the runs up to the bookmark's end.
            (name, para_text(n.parent().unwrap()))
        })
        .collect();
    let links: Vec<(String, String)> = x
        .descendants()
        .filter(|n| n.has_tag_name((W, "hyperlink")))
        .filter_map(|n| wattr(n, "anchor").map(|a| (a.to_string(), para_text(n))))
        .collect();
    assert!(!links.is_empty());
    for (anchor, _) in &links {
        assert!(bookmarks.contains_key(anchor.as_str()), "a link to a missing bookmark {anchor}");
    }
    let text_of =
        |link: &str| links.iter().find(|(_, t)| t == link).map(|(a, _)| bookmarks[a.as_str()].clone());
    assert_eq!(text_of("Section 2").as_deref(), Some("2  Method"));
    assert_eq!(text_of("Figure 1").as_deref(), Some("Figure 1: The reach and the design line of option 2"));
    assert_eq!(text_of("Table 1").as_deref(), Some("Table 1: The three options, screened"));
    assert_eq!(text_of("Appendix B  Run record").as_deref(), Some("Appendix B  Run record"));
}

#[test]
fn tables_repeat_their_header_row_on_the_header_ground() {
    let pkg = Pkg::read(&example_docx());
    let xml = pkg.text("word/document.xml");
    let x = Xml::parse(&xml).unwrap();
    let headed: Vec<Node> = x
        .descendants()
        .filter(|n| n.has_tag_name((W, "tr")))
        .filter(|tr| tr.descendants().any(|n| n.has_tag_name((W, "tblHeader"))))
        .collect();
    // The revision history and the two data tables.
    assert_eq!(headed.len(), 3);
    for tr in headed {
        let first_row = tr.parent().unwrap().children().find(|n| n.has_tag_name((W, "tr"))).unwrap();
        assert_eq!(first_row, tr, "the header row is the first row");
        for shd in tr.descendants().filter(|n| n.has_tag_name((W, "shd"))) {
            assert_eq!(wattr(shd, "fill"), Some("E8DCBD"));
        }
    }
    // Column alignment: "Length (km)" is right-aligned, "Result" centred.
    let cell_jc = |text: &str| {
        x.descendants()
            .filter(|n| n.has_tag_name((W, "p")))
            .find(|p| para_text(*p) == text)
            .and_then(|p| p.descendants().find(|n| n.has_tag_name((W, "jc"))))
            .and_then(|j| wattr(j, "val"))
    };
    assert_eq!(cell_jc("Length (km)"), Some("right"));
    assert_eq!(cell_jc("412 000"), Some("right"));
    assert_eq!(cell_jc("Fails"), Some("center"));
    assert_eq!(cell_jc("Option 1"), None);

    // The data box: the data ground and a thick ink rule on the left.
    let title = x
        .descendants()
        .find(|n| n.has_tag_name((W, "pStyle")) && wattr(*n, "val") == Some("DataBoxTitle"))
        .unwrap();
    let tbl = title.ancestors().find(|n| n.has_tag_name((W, "tbl"))).unwrap();
    let left = tbl
        .descendants()
        .find(|n| n.has_tag_name((W, "left")) && n.parent().unwrap().has_tag_name((W, "tblBorders")))
        .unwrap();
    assert_eq!((wattr(left, "sz"), wattr(left, "color")), (Some("12"), Some("2B2925")), "1.5 pt of ink");
    assert!(
        tbl.descendants().filter(|n| n.has_tag_name((W, "shd"))).all(|s| wattr(s, "fill") == Some("F8F2E3"))
    );
}

#[test]
fn svg_figures_carry_a_png_fallback_and_the_svg() {
    let (doc, assets) = common::example();
    let pkg = Pkg::read(&docx(&doc, &assets));
    let rels = pkg.rels("word/document.xml");
    let xml = pkg.text("word/document.xml");
    let x = Xml::parse(&xml).unwrap();
    let mut svgs = 0;
    let mut pngs_as_is = 0;
    for inline in x.descendants().filter(|n| n.has_tag_name((WP, "inline"))) {
        let blip = inline.descendants().find(|n| n.has_tag_name((A, "blip"))).unwrap();
        let png = &pkg.files[&rels[blip.attribute((R, "embed")).unwrap()].1];
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let ext = inline
            .descendants()
            .find(|n| n.has_tag_name((W, "extent")) || n.has_tag_name((WP, "extent")))
            .unwrap();
        let (cx, cy): (f64, f64) =
            (ext.attribute("cx").unwrap().parse().unwrap(), ext.attribute("cy").unwrap().parse().unwrap());
        match inline.descendants().find(|n| n.has_tag_name((ASVG, "svgBlip"))) {
            Some(svg) => {
                let ext = svg.parent().unwrap();
                assert_eq!(ext.attribute("uri"), Some("{96DAC541-7B7A-43D3-8B79-37D633B846F1}"));
                let part = &rels[svg.attribute((R, "embed")).unwrap()].1;
                assert!(part.ends_with(".svg"));
                let bytes = &pkg.files[part];
                assert!(assets.values().any(|a| &a.bytes == bytes), "the SVG is the asset itself");
                // Both example SVGs are 600 px wide at 0.9 of the 170 mm text width.
                assert_eq!(cx, (0.9_f64 * 170.0 * 36_000.0).round());
                let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
                assert!((1000..=1010).contains(&w), "the fallback is drawn at about 150 dpi: {w} px");
                svgs += 1;
                let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
                assert!(((cy / cx) - f64::from(h) / f64::from(w)).abs() < 0.01, "the aspect ratio is kept");
            }
            None => {
                assert!(assets.values().any(|a| &a.bytes == png), "a PNG goes in as it is");
                pngs_as_is += 1;
            }
        }
    }
    assert_eq!((svgs, pngs_as_is), (2, 1));
}

#[test]
fn special_characters_and_dollars_read_back_unchanged() {
    let md = r#"5 < 6 & 7 > 4, "q" 'a' #set {x} @r ~t ^ = / %, $5 and $10, US$ 3, $$ y $$, $x$ and \$ ]]> ]"#;
    let expected =
        r#"5 < 6 & 7 > 4, "q" 'a' #set {x} @r ~t ^ = / %, $5 and $10, US$ 3, $$ y $$, $x$ and $ ]]> ]"#;
    let v = common::with_body(serde_json::json!([common::section(
        "s",
        "Odd <title> & \"quotes\"",
        serde_json::json!([
            common::prose(md),
            common::prose(
                "- one `<code> & $1`\n- two\n  1. nested\n  2. again\n\n```\nfn f() { \"<&>\" }\n\tx\n```\n\n[web](https://example.org/?a=1&b=2) [bad](javascript:alert(1)) [mail](mailto:a@example.org)\\\nnext line"
            ),
        ])
    )]));
    let pkg = Pkg::read(&docx(&common::doc(&v), &ord::Assets::new()));
    let paras = paragraphs(&pkg);
    let texts: Vec<&str> = paras.iter().map(|(_, t)| t.as_str()).collect();
    assert!(texts.contains(&expected), "{texts:#?}");
    assert!(texts.contains(&"1  Odd <title> & \"quotes\""));
    assert!(texts.contains(&"one <code> & $1"));
    assert!(texts.contains(&"fn f() { \"<&>\" }\n\tx"), "{texts:#?}");
    assert!(texts.contains(&"web bad mail\nnext line"));

    // Lists: each item its own paragraph with a numbering; the nested list is a level down and numbered.
    let xml = pkg.text("word/document.xml");
    let x = Xml::parse(&xml).unwrap();
    let num_of = |text: &str| {
        let p = x.descendants().filter(|n| n.has_tag_name((W, "p"))).find(|p| para_text(*p) == text).unwrap();
        let ilvl = p
            .descendants()
            .find(|n| n.has_tag_name((W, "ilvl")))
            .and_then(|n| wattr(n, "val"))
            .unwrap()
            .to_string();
        let id = p
            .descendants()
            .find(|n| n.has_tag_name((W, "numId")))
            .and_then(|n| wattr(n, "val"))
            .unwrap()
            .to_string();
        (ilvl, id)
    };
    assert_eq!(num_of("two").0, "0");
    assert_eq!(num_of("nested").0, "1");
    assert_eq!(num_of("nested").1, num_of("again").1);
    assert_ne!(num_of("nested").1, num_of("two").1);
    let numbering = pkg.text("word/numbering.xml");
    let n = Xml::parse(&numbering).unwrap();
    let fmt_of = |num_id: &str| {
        let abs = n
            .descendants()
            .find(|e| e.has_tag_name((W, "num")) && wattr(*e, "numId") == Some(num_id))
            .and_then(|e| e.children().find(|c| c.has_tag_name((W, "abstractNumId"))))
            .and_then(|e| wattr(e, "val"))
            .unwrap();
        let a = n
            .descendants()
            .find(|e| e.has_tag_name((W, "abstractNum")) && wattr(*e, "abstractNumId") == Some(abs))
            .unwrap();
        wattr(a.descendants().find(|e| e.has_tag_name((W, "numFmt"))).unwrap(), "val").unwrap().to_string()
    };
    assert_eq!(fmt_of(&num_of("two").1), "bullet");
    assert_eq!(fmt_of(&num_of("nested").1), "decimal");

    // Links: web and mail are external hyperlinks; javascript: is text.
    let rels = pkg.rels("word/document.xml");
    let targets: Vec<&str> = x
        .descendants()
        .filter(|n| n.has_tag_name((W, "hyperlink")))
        .filter_map(|h| h.attribute((R, "id")))
        .map(|id| {
            let (kind, target, external) = &rels[id];
            assert!(kind.ends_with("/hyperlink") && *external);
            target.as_str()
        })
        .collect();
    assert_eq!(targets, ["https://example.org/?a=1&b=2", "mailto:a@example.org"]);
}

/// ECMA-376 Part 1 §17.8.1: the key is the GUID's 16 bytes, in reverse order, applied to the first 32 bytes.
fn deobfuscate(data: &[u8], key: &str) -> Vec<u8> {
    let hex: Vec<u8> = key
        .trim_matches(|c| c == '{' || c == '}')
        .split('-')
        .collect::<String>()
        .as_bytes()
        .chunks(2)
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(hex.len(), 16);
    let mut out = data.to_vec();
    for i in 0..32 {
        out[i] ^= hex[15 - (i % 16)];
    }
    out
}

#[test]
fn the_bundled_fonts_are_embedded_and_deobfuscate_to_the_originals() {
    let pkg = Pkg::read(&example_docx());
    let rels = pkg.rels("word/fontTable.xml");
    let xml = pkg.text("word/fontTable.xml");
    let x = Xml::parse(&xml).unwrap();
    let mut found = BTreeSet::new();
    for font in x.descendants().filter(|n| n.has_tag_name((W, "font"))) {
        let family = wattr(font, "name").unwrap();
        for e in font.children().filter(|n| n.tag_name().name().starts_with("embed")) {
            let key = wattr(e, "fontKey").unwrap();
            assert_eq!(key.len(), 38);
            let (kind, part, _) = &rels[e.attribute((R, "id")).unwrap()];
            assert!(kind.ends_with("/font") && part.ends_with(".odttf"));
            let data = &pkg.files[part];
            let plain = deobfuscate(data, key);
            let f = ord::fonts::bundled()
                .iter()
                .find(|f| f.ttf == plain.as_slice())
                .unwrap_or_else(|| panic!("{part} is not a bundled font"));
            assert_ne!(data.as_slice(), f.ttf, "the part is obfuscated");
            assert_eq!(f.family, family);
            let slot = e.tag_name().name();
            assert_eq!(
                slot,
                match (f.weight, f.italic) {
                    (400, false) => "embedRegular",
                    (700, false) => "embedBold",
                    (400, true) => "embedItalic",
                    _ => "embedBoldItalic",
                }
            );
            found.insert(f.file_name);
        }
    }
    // Every regular, bold and italic bundled font; the medium weight has no slot in Word.
    let expected: BTreeSet<&str> =
        ord::fonts::bundled().iter().filter(|f| f.weight != 500).map(|f| f.file_name).collect();
    assert_eq!(found, expected);
    assert_eq!(found.len(), 8);
}

#[test]
fn two_renders_are_byte_equal() {
    let a = example_docx();
    let b = example_docx();
    assert_eq!(a, b);
    eprintln!("example .docx: {} bytes", a.len());
}

#[test]
fn a_document_without_a_cover_or_contents_is_one_section() {
    let mut v = common::with_body(serde_json::json!([common::section(
        "s",
        "Scope",
        serde_json::json!([common::prose("Text.")])
    )]));
    v["template"] = serde_json::json!({ "cover": false });
    v["contents"] = false.into();
    let pkg = Pkg::read(&docx(&common::doc(&v), &ord::Assets::new()));
    let paras = paragraphs(&pkg);
    assert_eq!(paras[0], ("Title".to_string(), "T".to_string()));
    assert_eq!(paras[1].1, "Document control");
    assert!(!paras.iter().any(|(s, _)| s.starts_with("TOC")));
    let xml = pkg.text("word/document.xml");
    assert_eq!(xml.matches("<w:sectPr>").count(), 1);
    assert!(!xml.contains("wp:anchor"));
}

#[test]
fn a_logo_sits_on_the_cover_and_is_embedded_once() {
    let (_, assets) = common::example();
    let mut v = common::with_body(serde_json::json!([common::section(
        "s",
        "Scope",
        serde_json::json!([
            { "type": "figure", "id": "k", "caption": "Key", "asset": "logo", "alt": "A key", "width": 0.5 }
        ])
    )]));
    v["template"] = serde_json::json!({ "organisation_logo": "logo" });
    let mut a = ord::Assets::new();
    a.insert("logo".into(), assets["option-key"].clone());
    let pkg = Pkg::read(&docx(&common::doc(&v), &a));
    let xml = pkg.text("word/document.xml");
    let x = Xml::parse(&xml).unwrap();
    let logo = x
        .descendants()
        .find(|n| n.has_tag_name((WP, "docPr")) && n.attribute("name") == Some("Logo"))
        .unwrap();
    let ext = logo.parent().unwrap().children().find(|n| n.has_tag_name((WP, "extent"))).unwrap();
    let cy: u64 = ext.attribute("cy").unwrap().parse().unwrap();
    assert!(cy <= 12 * 36_000, "at most 12 mm high");
    // The logo and the figure share one image part; the paper is the other.
    assert_eq!(pkg.names.iter().filter(|n| n.starts_with("word/media/")).count(), 2);
}

/// Opens the example in LibreOffice and converts it to PDF, when asked (`ORD_SOFFICE_SMOKE=1`) and when
/// `soffice` is installed. A smoke test only: CI does not depend on it.
#[test]
fn libreoffice_opens_the_example_when_asked() {
    if std::env::var("ORD_SOFFICE_SMOKE").as_deref() != Ok("1") {
        return;
    }
    let Ok(out) = std::process::Command::new("soffice").arg("--version").output() else { return };
    if !out.status.success() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("ord-docx-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("screening-report.docx");
    std::fs::write(&path, example_docx()).unwrap();
    let status = std::process::Command::new("soffice")
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(&dir)
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    let pdf = std::fs::read(dir.join("screening-report.pdf")).expect("LibreOffice wrote a PDF");
    assert!(pdf.starts_with(b"%PDF"));
    eprintln!("LibreOffice PDF: {} bytes at {}", pdf.len(), dir.display());
}
