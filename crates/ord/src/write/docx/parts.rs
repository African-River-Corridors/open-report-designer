//! The package parts that do not depend on the body: styles, settings, numbering, font table, properties,
//! content types and the package relationships.

use std::fmt::Write as _;

use super::{esc, twips};
use crate::render::Prepared;
use crate::template::Template;

pub const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
pub const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
pub const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const CT_W: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";

/// `[Content_Types].xml`: a default for each extension and an override for each XML part. `cover_foot` adds
/// the photo cover's own footer part; `jpeg` the JPEG extension.
pub fn content_types(cover_foot: bool, jpeg: bool) -> String {
    let mut x = String::from(DECL);
    x.push_str("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    for (ext, ct) in [
        ("rels", "application/vnd.openxmlformats-package.relationships+xml"),
        ("xml", "application/xml"),
        ("png", "image/png"),
        ("jpeg", "image/jpeg"),
        ("svg", "image/svg+xml"),
        ("odttf", "application/vnd.openxmlformats-officedocument.obfuscatedFont"),
    ] {
        if ext == "jpeg" && !jpeg {
            continue;
        }
        let _ = write!(x, "<Default Extension=\"{ext}\" ContentType=\"{ct}\"/>");
    }
    for (part, ct) in [
        ("/word/document.xml", format!("{CT_W}.document.main+xml")),
        ("/word/styles.xml", format!("{CT_W}.styles+xml")),
        ("/word/settings.xml", format!("{CT_W}.settings+xml")),
        ("/word/numbering.xml", format!("{CT_W}.numbering+xml")),
        ("/word/fontTable.xml", format!("{CT_W}.fontTable+xml")),
        ("/word/header1.xml", format!("{CT_W}.header+xml")),
        ("/word/footer1.xml", format!("{CT_W}.footer+xml")),
        ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml".into()),
        ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml".into()),
    ] {
        let _ = write!(x, "<Override PartName=\"{part}\" ContentType=\"{ct}\"/>");
    }
    if cover_foot {
        let _ = write!(x, "<Override PartName=\"/word/footer2.xml\" ContentType=\"{CT_W}.footer+xml\"/>");
    }
    x.push_str("</Types>");
    x
}

/// A relationships part from (id, type, target, external).
pub fn rels(items: &[(String, String, String, bool)]) -> String {
    let mut x = String::from(DECL);
    let _ = write!(x, "<Relationships xmlns=\"{NS_PKG_REL}\">");
    for (id, kind, target, external) in items {
        let mode = if *external { " TargetMode=\"External\"" } else { "" };
        let _ = write!(x, "<Relationship Id=\"{id}\" Type=\"{kind}\" Target=\"{}\"{mode}/>", esc(target));
    }
    x.push_str("</Relationships>");
    x
}

/// `_rels/.rels`.
pub fn package_rels() -> String {
    rels(&[
        ("rId1".into(), format!("{REL}/officeDocument"), "word/document.xml".into(), false),
        (
            "rId2".into(),
            "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties".into(),
            "docProps/core.xml".into(),
            false,
        ),
        ("rId3".into(), format!("{REL}/extended-properties"), "docProps/app.xml".into(), false),
    ])
}

/// `docProps/core.xml`: title, subject, creator (the organisation), language, and the document date as the
/// created and modified time (the engine reads no clock).
pub fn core(p: &Prepared) -> String {
    let d = &p.doc;
    let mut x = String::from(DECL);
    x.push_str(
        "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
         xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
    );
    let _ = write!(x, "<dc:title>{}</dc:title>", esc(&d.title));
    if let Some(s) = &d.subtitle {
        let _ = write!(x, "<dc:subject>{}</dc:subject>", esc(s));
    }
    if let Some(o) = &d.organisation {
        let _ = write!(x, "<dc:creator>{}</dc:creator>", esc(o));
    }
    if let Some(n) = &d.control.number {
        let _ = write!(x, "<dc:identifier>{}</dc:identifier>", esc(n));
    }
    let _ = write!(x, "<dc:language>{}</dc:language>", esc(&d.language));
    let _ = write!(x, "<cp:version>{}</cp:version>", esc(&d.control.revision));
    let when = format!("{}T00:00:00Z", d.control.date);
    let _ = write!(x, "<dcterms:created xsi:type=\"dcterms:W3CDTF\">{}</dcterms:created>", esc(&when));
    let _ = write!(x, "<dcterms:modified xsi:type=\"dcterms:W3CDTF\">{}</dcterms:modified>", esc(&when));
    x.push_str("</cp:coreProperties>");
    x
}

/// `docProps/app.xml`.
pub fn app(p: &Prepared) -> String {
    let mut x = String::from(DECL);
    x.push_str(
        "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\">",
    );
    let _ = write!(x, "<Application>Open Report Designer {}</Application>", esc(crate::VERSION));
    if let Some(o) = &p.doc.organisation {
        let _ = write!(x, "<Company>{}</Company>", esc(o));
    }
    x.push_str("</Properties>");
    x
}

/// `word/settings.xml`: fonts embedded, hyphenation off, and, when the document has a table of contents,
/// fields update on open (so Word fills in its page numbers). The children are in the schema's order.
///
/// `updateFields` is written only for a table of contents: Word answers it with a modal "update the
/// fields?" question on every open, which a master that people edit should not ask, and which stops a
/// scripted export until someone clicks. PAGE and NUMPAGES update without it.
pub fn settings(p: &Prepared) -> String {
    let mut x = String::from(DECL);
    let _ = write!(x, "<w:settings xmlns:w=\"{NS_W}\">");
    x.push_str("<w:zoom w:percent=\"100\"/><w:embedTrueTypeFonts/>");
    x.push_str("<w:defaultTabStop w:val=\"709\"/><w:autoHyphenation w:val=\"false\"/>");
    x.push_str("<w:characterSpacingControl w:val=\"doNotCompress\"/>");
    if p.doc.contents && !p.doc.toc.is_empty() {
        x.push_str("<w:updateFields w:val=\"true\"/>");
    }
    x.push_str(
        "<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat>",
    );
    let _ = write!(x, "<w:themeFontLang w:val=\"{}\"/>", esc(&p.doc.language));
    x.push_str("</w:settings>");
    x
}

/// `word/numbering.xml`: abstract list 0 is bullets, 1 is numbers. Instance 1 is the bullets; instance
/// `2 + i` is ordered list `i`, which restarts at its own start number on its level.
pub fn numbering(p: &Prepared, ordered: &[(u8, u64)]) -> String {
    let font = esc(&p.template.body_font);
    let mut x = String::from(DECL);
    let _ = write!(x, "<w:numbering xmlns:w=\"{NS_W}\">");
    for (id, bullets) in [(0, true), (1, false)] {
        let _ = write!(
            x,
            "<w:abstractNum w:abstractNumId=\"{id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>"
        );
        for lvl in 0..9u32 {
            let (fmt, text) = if bullets {
                ("bullet", if lvl % 2 == 0 { "\u{2022}".to_string() } else { "\u{2013}".to_string() })
            } else {
                ("decimal", format!("%{}.", lvl + 1))
            };
            let step = p.template.list_indent_mm.map(|i| (i * 56.693).round() as u32).unwrap_or(397);
            let left = step * (lvl + 1);
            let _ = write!(
                x,
                "<w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"{text}\"/>\
                 <w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{left}\" w:hanging=\"{step}\"/></w:pPr>\
                 <w:rPr><w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\" w:cs=\"{font}\"/></w:rPr></w:lvl>"
            );
        }
        x.push_str("</w:abstractNum>");
    }
    x.push_str("<w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num>");
    for (i, (lvl, start)) in ordered.iter().enumerate() {
        let _ = write!(
            x,
            "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"1\"/><w:lvlOverride w:ilvl=\"{lvl}\"><w:startOverride w:val=\"{start}\"/></w:lvlOverride></w:num>",
            i + 2
        );
    }
    x.push_str("</w:numbering>");
    x
}

/// One embedded font: the family, its slot, the relationship id, the font key and the part name.
pub struct Embedded {
    pub family: &'static str,
    pub slot: &'static str,
    pub rid: String,
    pub key: String,
    pub part: String,
}

/// `word/fontTable.xml`, with the embedded fonts of each family.
pub fn font_table(t: &Template, embedded: &[Embedded]) -> String {
    let mut x = String::from(DECL);
    let _ = write!(x, "<w:fonts xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\">");
    let mut families = vec![(&*t.body_font, "swiss", "variable")];
    if t.heading_font != t.body_font {
        families.push((&*t.heading_font, "roman", "variable"));
    }
    families.push((t.mono_font, "modern", "fixed"));
    for (family, kind, pitch) in families {
        let _ = write!(
            x,
            "<w:font w:name=\"{}\"><w:charset w:val=\"00\"/><w:family w:val=\"{kind}\"/><w:pitch w:val=\"{pitch}\"/>",
            esc(family)
        );
        // The schema's order: regular, bold, italic, bold italic.
        for slot in ["embedRegular", "embedBold", "embedItalic", "embedBoldItalic"] {
            for e in embedded.iter().filter(|e| e.family == family && e.slot == slot) {
                let _ = write!(x, "<w:{slot} r:id=\"{}\" w:fontKey=\"{}\"/>", e.rid, e.key);
            }
        }
        x.push_str("</w:font>");
    }
    x.push_str("</w:fonts>");
    x
}

fn rfonts(f: &str) -> String {
    let f = esc(f);
    format!("<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:cs=\"{f}\" w:eastAsia=\"{f}\"/>")
}

/// `word/styles.xml`: the document defaults and every style the writer uses, in the template's fonts and
/// colours. Elements are in the schema's order (Word refuses a file otherwise).
pub fn styles(p: &Prepared) -> String {
    let t = &p.template;
    let ink = hex(t.ink);
    let ink2 = hex(t.ink_2);
    let rule = hex(t.rule);
    let data = hex(t.data_ground);
    let body_sz = (t.body_size_pt * 2.0).round() as u32;
    let text_w = twips(t.page_width_mm - t.margin_left_mm - t.margin_right_mm);
    let head = rfonts(&t.heading_font);
    // A 0.2 colour that equals its 0.1 default is not written, so a 0.1 document's styles stay as they were.
    let hink = if t.heading_ink == t.ink {
        String::new()
    } else {
        format!("<w:color w:val=\"{}\"/>", hex(t.heading_ink))
    };
    let hbold = if t.heading_bold { "<w:b/><w:bCs/>" } else { "" };
    let h_sz = t.heading_sizes_pt.unwrap_or([16.0, 13.0, 11.0, 10.0]).map(|v| (v * 2.0).round() as u32);
    let think = hex(t.table_header_ink);
    let (cink, caccent, card_ink) = (hex(t.cover_ink), hex(t.cover_accent), hex(t.ink));
    let clabel = hex(t.cover_ink.over(t.paper, 0.65));
    let mono = rfonts(t.mono_font);
    let lang = esc(&p.doc.language);

    // Line height and paragraph spacing (0.2): "at least" the line height, the same baseline-to-baseline
    // distance as the PDF's line boxes and leading.
    let default_spacing = match (t.line_height, t.paragraph_spacing_pt) {
        (None, None) => "<w:spacing w:after=\"120\" w:line=\"288\" w:lineRule=\"auto\"/>".to_string(),
        (lh, gap) => {
            let after = (gap.unwrap_or(6.0) * 20.0).round() as u32;
            match lh {
                Some(lh) => format!(
                    "<w:spacing w:after=\"{after}\" w:line=\"{}\" w:lineRule=\"atLeast\"/>",
                    (lh * t.body_size_pt * 20.0).round() as u32
                ),
                None => format!("<w:spacing w:after=\"{after}\" w:line=\"288\" w:lineRule=\"auto\"/>"),
            }
        }
    };
    let table_pt = t.table_size_pt.unwrap_or(t.body_size_pt);
    let table_spacing = match (t.line_height, t.table_padding_mm) {
        (None, None) => {
            "<w:spacing w:before=\"40\" w:after=\"40\" w:line=\"240\" w:lineRule=\"auto\"/>".to_string()
        }
        (lh, pad) => {
            let edge = if pad.is_some() { 0 } else { 40 };
            match lh {
                Some(lh) => format!(
                    "<w:spacing w:before=\"{edge}\" w:after=\"{edge}\" w:line=\"{}\" w:lineRule=\"atLeast\"/>",
                    (lh * table_pt * 20.0).round() as u32
                ),
                None => format!(
                    "<w:spacing w:before=\"{edge}\" w:after=\"{edge}\" w:line=\"240\" w:lineRule=\"auto\"/>"
                ),
            }
        }
    };
    // A table with no header row: its own size, and the line height at that size.
    let plain_pt = t.headless_size_pt.unwrap_or(table_pt);
    let plain_size = match t.headless_size_pt {
        Some(pt) => format!("<w:sz w:val=\"{0}\"/><w:szCs w:val=\"{0}\"/>", (pt * 2.0).round() as u32),
        None => String::new(),
    };
    let plain_spacing = match t.line_height {
        Some(lh) if t.headless_size_pt.is_some() => format!(
            "<w:spacing w:line=\"{}\" w:lineRule=\"atLeast\"/>",
            (lh * plain_pt * 20.0).round() as u32
        ),
        _ => String::new(),
    };
    let table_size = match t.table_size_pt {
        Some(pt) => format!("<w:sz w:val=\"{0}\"/><w:szCs w:val=\"{0}\"/>", (pt * 2.0).round() as u32),
        None => String::new(),
    };
    let mut x = String::from(DECL);
    let _ = write!(
        x,
        "<w:styles xmlns:w=\"{NS_W}\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \
         xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\" mc:Ignorable=\"w14\">"
    );
    let _ = write!(
        x,
        "<w:docDefaults><w:rPrDefault><w:rPr>{}<w:color w:val=\"{ink}\"/><w:sz w:val=\"{body_sz}\"/>\
         <w:szCs w:val=\"{body_sz}\"/><w:lang w:val=\"{lang}\"/></w:rPr></w:rPrDefault>\
         <w:pPrDefault><w:pPr>{default_spacing}</w:pPr></w:pPrDefault></w:docDefaults>",
        rfonts(&t.body_font)
    );

    // (id, name, based on, next, pPr, rPr)
    let tnum = if t.table_tabular_figures { "<w14:numSpacing w14:val=\"tabular\"/>" } else { "" };
    let heading = |lvl: u32, sz: u32, before: u32, italic: bool| {
        (
            format!("Heading{lvl}"),
            format!("heading {lvl}"),
            "Normal",
            "Normal",
            format!(
                "<w:keepNext/><w:keepLines/><w:spacing w:before=\"{before}\" w:after=\"120\" w:line=\"240\" w:lineRule=\"auto\"/><w:outlineLvl w:val=\"{}\"/>",
                lvl - 1
            ),
            format!(
                "{head}{hbold}{}{hink}<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>",
                if italic { "<w:i/>" } else { "" }
            ),
        )
    };
    // The rule under a level-1 heading, when the template has one.
    let h1_rule = |mut h: (String, String, &'static str, &'static str, String, String)| {
        if let Some(r) = t.heading_rule {
            h.4 = h.4.replacen(
                "<w:spacing",
                &format!("<w:pBdr><w:bottom w:val=\"single\" w:sz=\"10\" w:space=\"3\" w:color=\"{}\"/></w:pBdr><w:spacing", hex(r)),
                1,
            );
        }
        h
    };
    let mut paragraph: Vec<(String, String, &str, &str, String, String)> = vec![
        ("Normal".into(), "Normal".into(), "", "", String::new(), String::new()),
        (
            "Title".into(),
            "Title".into(),
            "Normal",
            "Normal",
            "<w:spacing w:before=\"120\" w:after=\"120\" w:line=\"240\" w:lineRule=\"auto\"/>".into(),
            format!("{head}<w:sz w:val=\"48\"/><w:szCs w:val=\"48\"/>"),
        ),
        (
            "Subtitle".into(),
            "Subtitle".into(),
            "Normal",
            "Normal",
            "<w:spacing w:after=\"240\" w:line=\"240\" w:lineRule=\"auto\"/>".into(),
            format!("{head}<w:i/><w:color w:val=\"{ink2}\"/><w:sz w:val=\"26\"/><w:szCs w:val=\"26\"/>"),
        ),
        (
            "ClassName".into(),
            "Class Name".into(),
            "Normal",
            "Normal",
            "<w:spacing w:before=\"2835\" w:after=\"120\"/>".into(),
            format!("<w:caps/><w:color w:val=\"{ink2}\"/><w:spacing w:val=\"40\"/><w:sz w:val=\"17\"/><w:szCs w:val=\"17\"/>"),
        ),
        (
            "CoverText".into(),
            "Cover Text".into(),
            "Normal",
            "Normal",
            "<w:spacing w:before=\"360\" w:after=\"240\"/>".into(),
            "<w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/>".into(),
        ),
        ("Summary".into(), "Summary".into(), "Normal", "Summary", String::new(), String::new()),
        (
            "Notice".into(),
            "Notice".into(),
            "Normal",
            "Normal",
            "<w:spacing w:before=\"240\" w:after=\"240\"/>".into(),
            "<w:b/><w:bCs/>".into(),
        ),
        (
            "CoverLabel".into(),
            "Cover Label".into(),
            "Normal",
            "CoverValue",
            "<w:spacing w:before=\"80\" w:after=\"0\"/>".into(),
            format!("<w:caps/><w:color w:val=\"{ink2}\"/><w:spacing w:val=\"24\"/><w:sz w:val=\"15\"/><w:szCs w:val=\"15\"/>"),
        ),
        (
            "CoverValue".into(),
            "Cover Value".into(),
            "Normal",
            "Normal",
            "<w:spacing w:after=\"0\"/>".into(),
            String::new(),
        ),
        h1_rule(heading(1, h_sz[0], 360, false)),
        heading(2, h_sz[1], 300, false),
        heading(3, h_sz[2], 240, false),
        heading(4, h_sz[3], 200, false),
        heading(5, h_sz[3], 200, true),
        (
            "FrontHeading".into(),
            "Front Heading".into(),
            "Normal",
            "Normal",
            "<w:keepNext/><w:keepLines/><w:spacing w:before=\"360\" w:after=\"160\" w:line=\"240\" w:lineRule=\"auto\"/>".into(),
            format!("{head}<w:sz w:val=\"32\"/><w:szCs w:val=\"32\"/>"),
        ),
        (
            "Caption".into(),
            "caption".into(),
            "Normal",
            "Normal",
            "<w:keepNext/><w:spacing w:before=\"160\" w:after=\"80\"/>".into(),
            "<w:b/><w:bCs/>".into(),
        ),
        (
            "Figure".into(),
            "Figure".into(),
            "Normal",
            "Caption",
            "<w:keepNext/><w:spacing w:before=\"240\" w:after=\"80\" w:line=\"240\" w:lineRule=\"auto\"/><w:jc w:val=\"center\"/>".into(),
            String::new(),
        ),
        (
            "MapKey".into(),
            "Map Key".into(),
            "Normal",
            "Normal",
            "<w:spacing w:after=\"240\"/>".into(),
            format!("<w:color w:val=\"{ink2}\"/><w:sz w:val=\"17\"/><w:szCs w:val=\"17\"/>"),
        ),
        (
            "Code".into(),
            "Code".into(),
            "Normal",
            "Normal",
            format!(
                "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{data}\"/><w:spacing w:before=\"60\" w:after=\"180\" w:line=\"240\" w:lineRule=\"auto\"/><w:ind w:left=\"113\" w:right=\"113\"/>"
            ),
            format!("{mono}<w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/>"),
        ),
        (
            "Quote".into(),
            "Quote".into(),
            "Normal",
            "Normal",
            format!(
                "<w:pBdr><w:left w:val=\"single\" w:sz=\"4\" w:space=\"8\" w:color=\"{rule}\"/></w:pBdr><w:ind w:left=\"340\"/>"
            ),
            format!("<w:color w:val=\"{ink2}\"/>"),
        ),
        (
            "ListParagraph".into(),
            "List Paragraph".into(),
            "Normal",
            "ListParagraph",
            "<w:spacing w:after=\"60\"/>".into(),
            String::new(),
        ),
        (
            "TableText".into(),
            "Table Text".into(),
            "Normal",
            "TableText",
            table_spacing.clone(),
            format!("{table_size}{tnum}"),
        ),
        (
            "TableHeader".into(),
            "Table Header".into(),
            "TableText",
            "TableText",
            "<w:keepNext/>".into(),
            format!("<w:b/><w:bCs/>{tnum}"),
        ),
        (
            "TableNote".into(),
            "Table Note".into(),
            "Normal",
            "TableNote",
            "<w:spacing w:before=\"60\" w:after=\"80\"/>".into(),
            format!("<w:color w:val=\"{ink2}\"/><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/>"),
        ),
        (
            "DataBoxTitle".into(),
            "Data Box Title".into(),
            "Normal",
            "TableText",
            "<w:keepNext/><w:spacing w:before=\"60\" w:after=\"80\"/>".into(),
            format!("{head}<w:sz w:val=\"23\"/><w:szCs w:val=\"23\"/>"),
        ),
        (
            "Header".into(),
            "header".into(),
            "Normal",
            "Header",
            format!(
                "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"2\" w:color=\"{rule}\"/></w:pBdr><w:tabs><w:tab w:val=\"right\" w:pos=\"{text_w}\"/></w:tabs><w:spacing w:after=\"0\"/>"
            ),
            format!("<w:color w:val=\"{ink2}\"/><w:sz w:val=\"16\"/><w:szCs w:val=\"16\"/>"),
        ),
        (
            "Footer".into(),
            "footer".into(),
            "Normal",
            "Footer",
            format!(
                "<w:pBdr><w:top w:val=\"single\" w:sz=\"4\" w:space=\"2\" w:color=\"{rule}\"/></w:pBdr><w:tabs><w:tab w:val=\"right\" w:pos=\"{text_w}\"/></w:tabs><w:spacing w:after=\"0\"/>"
            ),
            format!("<w:color w:val=\"{ink2}\"/><w:sz w:val=\"16\"/><w:szCs w:val=\"16\"/>"),
        ),
    ];
    // The 0.2 styles: the table head in its own ink, and the photo cover. Written only when used.
    // The photo cover's styles: every size and line from `render::PHOTO`, as the PDF writer reads them.
    let ph = crate::render::PHOTO;
    let hp = |pt: f64| (pt * 2.0).round() as u32;
    let exact = |pt: f64, lh: f64| (pt * lh * 20.0).round() as u32;
    let mm = |v: f64| (v * 56.693).round() as u32;
    let title_pt = f64::from(t.cover_title_pt);
    let panel_line_ink = hex(t.cover_ink.over(t.cover_panel_ground.unwrap_or(t.paper), 0.18));
    let _ = &panel_line_ink;
    let extra: Vec<(String, String, &str, &str, String, String)> = vec![
        (
            "TableHead".into(),
            "Table Head".into(),
            "TableHeader",
            "TableText",
            String::new(),
            format!("<w:color w:val=\"{think}\"/>"),
        ),
        (
            "CoverTitle".into(),
            "Cover Title".into(),
            "Normal",
            "Normal",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"{}\" w:line=\"{}\" w:lineRule=\"exact\"/>",
                mm(ph.title_after_mm),
                exact(title_pt, ph.title_lh)
            ),
            format!(
                "{head}{hbold}<w:color w:val=\"{cink}\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(title_pt),
                hp(title_pt)
            ),
        ),
        (
            "CoverMeta".into(),
            "Cover Meta".into(),
            "Normal",
            "Normal",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"{}\" w:line=\"{}\" w:lineRule=\"exact\"/>",
                mm(1.0),
                exact(ph.meta_pt, 1.35)
            ),
            format!(
                "<w:color w:val=\"{caccent}\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.meta_pt),
                hp(ph.meta_pt)
            ),
        ),
        (
            "CardText".into(),
            "Card Text".into(),
            "Normal",
            "CardText",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"{}\" w:line=\"{}\" w:lineRule=\"exact\"/>",
                mm(ph.card_par_gap_mm),
                exact(ph.card_text_pt, ph.card_lh)
            ),
            format!(
                "<w:color w:val=\"{card_ink}\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.card_text_pt),
                hp(ph.card_text_pt)
            ),
        ),
        (
            "LogoName".into(),
            "Logo Name".into(),
            "Normal",
            "LogoRole",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"{}\" w:line=\"{}\" w:lineRule=\"exact\"/><w:jc w:val=\"center\"/>",
                mm(1.0),
                exact(ph.logo_name_pt, 1.25)
            ),
            format!(
                "<w:b/><w:bCs/><w:color w:val=\"{card_ink}\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.logo_name_pt),
                hp(ph.logo_name_pt)
            ),
        ),
        (
            "LogoRole".into(),
            "Logo Role".into(),
            "Normal",
            "Normal",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"0\" w:line=\"{}\" w:lineRule=\"exact\"/><w:jc w:val=\"center\"/>",
                exact(ph.logo_role_pt, 1.25)
            ),
            format!(
                "<w:caps/><w:color w:val=\"{ink2}\"/><w:spacing w:val=\"10\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.logo_role_pt),
                hp(ph.logo_role_pt)
            ),
        ),
        (
            "CoverRowsHead".into(),
            "Cover Rows Head".into(),
            "Normal",
            "Normal",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"0\" w:line=\"{}\" w:lineRule=\"exact\"/>",
                exact(ph.panel_head_pt, 1.3)
            ),
            format!(
                "<w:b/><w:bCs/><w:caps/><w:color w:val=\"{caccent}\"/><w:spacing w:val=\"22\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.panel_head_pt),
                hp(ph.panel_head_pt)
            ),
        ),
        (
            "CoverRowLabel".into(),
            "Cover Row Label".into(),
            "Normal",
            "Normal",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"0\" w:line=\"{}\" w:lineRule=\"exact\"/>",
                exact(ph.panel_label_pt, 1.3)
            ),
            format!(
                "<w:caps/><w:color w:val=\"{clabel}\"/><w:spacing w:val=\"6\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.panel_label_pt),
                hp(ph.panel_label_pt)
            ),
        ),
        (
            "CoverRowValue".into(),
            "Cover Row Value".into(),
            "Normal",
            "Normal",
            format!(
                "<w:spacing w:before=\"0\" w:after=\"0\" w:line=\"{}\" w:lineRule=\"exact\"/>",
                exact(ph.panel_value_pt, 1.3)
            ),
            format!(
                "<w:b/><w:bCs/><w:color w:val=\"{cink}\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.panel_value_pt),
                hp(ph.panel_value_pt)
            ),
        ),
        (
            "CoverFoot".into(),
            "Cover Foot".into(),
            "Normal",
            "CoverFoot",
            format!("<w:tabs><w:tab w:val=\"right\" w:pos=\"{text_w}\"/></w:tabs><w:spacing w:after=\"0\"/>"),
            format!(
                "<w:color w:val=\"{clabel}\"/><w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>",
                hp(ph.foot_pt),
                hp(ph.foot_pt)
            ),
        ),
        (
            "TablePlain".into(),
            "Table Plain".into(),
            "TableText",
            "TablePlain",
            plain_spacing.clone(),
            plain_size.clone(),
        ),
        (
            "TableLabel".into(),
            "Table Label".into(),
            "TablePlain",
            "TablePlain",
            String::new(),
            format!("<w:color w:val=\"{ink2}\"/>"),
        ),
    ];
    for style in extra {
        let used = match style.0.as_str() {
            "TableHead" => t.table_header_ink != t.ink,
            "TableLabel" | "TablePlain" => crate::resolve::has_headless(&p.doc),
            _ => t.cover && t.cover_layout == crate::model::CoverLayout::Photo,
        };
        if used {
            paragraph.push(style);
        }
    }
    for lvl in 1..=3u32 {
        paragraph.push((
            format!("TOC{lvl}"),
            format!("toc {lvl}"),
            "Normal",
            "Normal",
            format!(
                "<w:tabs><w:tab w:val=\"right\" w:leader=\"dot\" w:pos=\"{text_w}\"/></w:tabs><w:spacing w:before=\"{}\" w:after=\"40\"/><w:ind w:left=\"{}\"/>",
                if lvl == 1 { 120 } else { 0 },
                (lvl - 1) * 340
            ),
            String::new(),
        ));
    }
    for (id, name, based, next, ppr, rpr) in &paragraph {
        let default = if id == "Normal" { " w:default=\"1\"" } else { "" };
        let _ =
            write!(x, "<w:style w:type=\"paragraph\"{default} w:styleId=\"{id}\"><w:name w:val=\"{name}\"/>");
        if !based.is_empty() {
            let _ = write!(x, "<w:basedOn w:val=\"{based}\"/>");
        }
        if !next.is_empty() {
            let _ = write!(x, "<w:next w:val=\"{next}\"/>");
        }
        x.push_str("<w:qFormat/>");
        if !ppr.is_empty() {
            let _ = write!(x, "<w:pPr>{ppr}</w:pPr>");
        }
        if !rpr.is_empty() {
            let _ = write!(x, "<w:rPr>{rpr}</w:rPr>");
        }
        x.push_str("</w:style>");
    }

    // Character styles.
    let _ = write!(
        x,
        "<w:style w:type=\"character\" w:default=\"1\" w:styleId=\"DefaultParagraphFont\"><w:name w:val=\"Default Paragraph Font\"/><w:uiPriority w:val=\"1\"/><w:semiHidden/><w:unhideWhenUsed/></w:style>\
         <w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:basedOn w:val=\"DefaultParagraphFont\"/><w:rPr><w:color w:val=\"{ink}\"/><w:u w:val=\"single\"/></w:rPr></w:style>\
         <w:style w:type=\"character\" w:styleId=\"InlineCode\"><w:name w:val=\"Inline Code\"/><w:basedOn w:val=\"DefaultParagraphFont\"/><w:rPr>{mono}<w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>"
    );

    // Table styles.
    let _ = write!(
        x,
        "<w:style w:type=\"table\" w:default=\"1\" w:styleId=\"TableNormal\"><w:name w:val=\"Normal Table\"/><w:uiPriority w:val=\"99\"/><w:semiHidden/><w:unhideWhenUsed/>\
         <w:tblPr><w:tblInd w:w=\"0\" w:type=\"dxa\"/><w:tblCellMar><w:top w:w=\"0\" w:type=\"dxa\"/><w:left w:w=\"108\" w:type=\"dxa\"/><w:bottom w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"108\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr></w:style>\
         <w:style w:type=\"table\" w:styleId=\"ReportTable\"><w:name w:val=\"Report Table\"/><w:basedOn w:val=\"TableNormal\"/><w:pPr><w:spacing w:after=\"0\"/></w:pPr>\
         <w:tblPr><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"{rule}\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"{rule}\"/>\
         <w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"{rule}\"/></w:tblBorders>\
         <w:tblCellMar><w:top w:w=\"0\" w:type=\"dxa\"/><w:left w:w=\"85\" w:type=\"dxa\"/><w:bottom w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"85\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr></w:style>"
    );
    x.push_str("</w:styles>");
    x
}

/// A colour as Word writes it: six hex digits, no `#`.
pub fn hex(c: crate::template::Colour) -> String {
    c.hex().trim_start_matches('#').to_ascii_uppercase()
}
