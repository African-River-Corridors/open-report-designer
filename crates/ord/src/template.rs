//! The template: page, colours and fonts (spec 0001 §7).
//!
//! [`DEFAULT`] is the one table of the default look. Every writer (PDF, HTML, Word) reads its values from a
//! [`Template`], which starts as [`DEFAULT`] and takes the document's overrides through
//! [`Template::with_overrides`].

use std::borrow::Cow;
use std::fmt;

use crate::assets::Assets;
use crate::finding::Finding;
use crate::model::{CoverLayout, HeadingNumbers, MediaType, TemplateOverrides};

/// A colour, as red, green and blue bytes. Written and read as `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Colour(pub [u8; 3]);

impl Colour {
    /// Reads `#rrggbb` (hex digits in either case). Anything else is `None`.
    pub fn parse(s: &str) -> Option<Colour> {
        let hex = s.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Colour([byte(0)?, byte(2)?, byte(4)?]))
    }

    /// The colour as `#rrggbb`, lower case.
    pub fn hex(&self) -> String {
        self.to_string()
    }

    /// This colour laid over `ground` at `opacity` (0 to 1): the same colour in every writer, where Word has
    /// no transparent rules or text.
    pub fn over(self, ground: Colour, opacity: f32) -> Colour {
        let a = opacity.clamp(0.0, 1.0);
        let mix = |f: u8, g: u8| (f32::from(f) * a + f32::from(g) * (1.0 - a)).round() as u8;
        let ([r, g, b], [gr, gg, gb]) = (self.0, ground.0);
        Colour([mix(r, gr), mix(g, gg), mix(b, gb)])
    }
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b] = self.0;
        write!(f, "#{r:02x}{g:02x}{b:02x}")
    }
}

/// The look of a rendered document.
///
/// Fonts are named by family. A family is bundled (`crate::fonts`) or comes from a font asset in `fonts`.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    /// Page width, millimetres.
    pub page_width_mm: f32,
    /// Page height, millimetres.
    pub page_height_mm: f32,
    pub margin_top_mm: f32,
    pub margin_bottom_mm: f32,
    pub margin_left_mm: f32,
    pub margin_right_mm: f32,
    /// The cover's paper. Override: `paper`.
    pub paper: Colour,
    /// The paper of the body pages.
    pub body_paper: Colour,
    /// Text. Override: `ink`.
    pub ink: Colour,
    /// Secondary text. Override: `ink_2`.
    pub ink_2: Colour,
    /// Rules. Override: `rule`.
    pub rule: Colour,
    /// The table header ground. Override: `accent_ground`.
    pub accent_ground: Colour,
    /// The data box ground.
    pub data_ground: Colour,
    /// The width of the ink rule on the left of a data box, points.
    pub data_rule_pt: f32,
    /// The heading font family. Override (0.2): `heading_font`.
    pub heading_font: Cow<'static, str>,
    /// The font family of text, tables and numbers. Override (0.2): `body_font`.
    pub body_font: Cow<'static, str>,
    /// The body text size, points.
    pub body_size_pt: f32,
    /// Tables use tabular figures.
    pub table_tabular_figures: bool,
    /// The code font family.
    pub mono_font: &'static str,
    /// Whether the document has a cover. Override: `cover`.
    pub cover: bool,
    /// An asset id for a logo on the cover. Override: `organisation_logo`.
    pub organisation_logo: Option<String>,
    // From here on, the values added in 0.2 (spec 0002 §3). Each has an override of the same name.
    pub cover_layout: CoverLayout,
    pub cover_image_height_mm: f32,
    /// The text of the cover. The default is `ink`.
    pub cover_ink: Colour,
    /// Rules, labels and the meta line on a photo cover.
    pub cover_accent: Colour,
    /// The ground of a card on a photo cover.
    pub card_ground: Colour,
    /// Headings. The default is `ink`.
    pub heading_ink: Colour,
    /// A rule under each level-1 heading, when set.
    pub heading_rule: Option<Colour>,
    /// The text of a table's header row. The default is `ink`.
    pub table_header_ink: Colour,
    /// Heading sizes, levels 1 to 4, points. None keeps each writer's 0.1 sizes.
    pub heading_sizes_pt: Option<[f32; 4]>,
    pub heading_bold: bool,
    pub heading_numbers: HeadingNumbers,
    pub section_page_break: bool,
    /// Asset ids of the font files the PDF and HTML writers use.
    pub fonts: Vec<String>,
    pub control_page: bool,
    pub running_head: bool,
    pub foot_sensitivity: bool,
    /// Size table columns from their text, the same in every writer.
    pub content_widths: bool,
    /// The word before the revision in the running head and on the cover foot.
    pub revision_prefix: Cow<'static, str>,
}

/// The default template: the look of Open Waterway Designer (spec 0001 §7). The one table of its values.
pub const DEFAULT: Template = Template {
    page_width_mm: 210.0,
    page_height_mm: 297.0,
    margin_top_mm: 25.0,
    margin_bottom_mm: 25.0,
    margin_left_mm: 20.0,
    margin_right_mm: 20.0,
    paper: Colour([0xf2, 0xe8, 0xcf]),
    body_paper: Colour([0xff, 0xff, 0xff]),
    ink: Colour([0x2b, 0x29, 0x25]),
    ink_2: Colour([0x4a, 0x46, 0x3e]),
    rule: Colour([0xd8, 0xca, 0xa6]),
    accent_ground: Colour([0xe8, 0xdc, 0xbd]),
    data_ground: Colour([0xf8, 0xf2, 0xe3]),
    data_rule_pt: 1.5,
    heading_font: Cow::Borrowed(DEFAULT_HEADING_FONT),
    body_font: Cow::Borrowed(DEFAULT_BODY_FONT),
    body_size_pt: 10.0,
    table_tabular_figures: true,
    mono_font: "IBM Plex Mono",
    cover: true,
    organisation_logo: None,
    cover_layout: CoverLayout::Classic,
    cover_image_height_mm: 75.0,
    cover_ink: Colour([0x2b, 0x29, 0x25]),
    cover_accent: Colour([0x2b, 0x29, 0x25]),
    card_ground: Colour([0xff, 0xff, 0xff]),
    heading_ink: Colour([0x2b, 0x29, 0x25]),
    heading_rule: None,
    table_header_ink: Colour([0x2b, 0x29, 0x25]),
    heading_sizes_pt: None,
    heading_bold: false,
    heading_numbers: HeadingNumbers::Spaced,
    section_page_break: true,
    fonts: Vec::new(),
    control_page: true,
    running_head: true,
    foot_sensitivity: false,
    content_widths: false,
    revision_prefix: Cow::Borrowed("Rev "),
};

/// The default families.
pub const DEFAULT_HEADING_FONT: &str = "Libre Caslon Text";
pub const DEFAULT_BODY_FONT: &str = "Public Sans";

impl Default for Template {
    fn default() -> Self {
        DEFAULT
    }
}

impl Template {
    /// The default template with the document's overrides applied.
    ///
    /// A colour that is not `#rrggbb` gives `ORD001` at `/template/<field>` and keeps the default. A logo
    /// that names no asset in `assets` gives `ORD011` at `/template/organisation_logo` and is left out.
    pub fn with_overrides(overrides: &TemplateOverrides, assets: &Assets) -> (Template, Vec<Finding>) {
        let mut t = Template::default();
        let mut findings = Vec::new();
        let mut colour = |field: &str, value: &Option<String>| -> Option<Colour> {
            let value = value.as_ref()?;
            let c = Colour::parse(value);
            if c.is_none() {
                findings.push(Finding::error(
                    "ORD001",
                    format!("/template/{field}"),
                    format!("the colour {value:?} is not of the form #rrggbb"),
                ));
            }
            c
        };
        let o = overrides;
        // The ink first: the 0.2 colours that default to it follow it.
        if let Some(c) = colour("ink", &o.ink) {
            t.ink = c;
            t.cover_ink = c;
            t.cover_accent = c;
            t.heading_ink = c;
            t.table_header_ink = c;
        }
        for (field, value, slot) in [
            ("paper", &o.paper, &mut t.paper),
            ("ink_2", &o.ink_2, &mut t.ink_2),
            ("rule", &o.rule, &mut t.rule),
            ("accent_ground", &o.accent_ground, &mut t.accent_ground),
            ("cover_ink", &o.cover_ink, &mut t.cover_ink),
            ("cover_accent", &o.cover_accent, &mut t.cover_accent),
            ("card_ground", &o.card_ground, &mut t.card_ground),
            ("heading_ink", &o.heading_ink, &mut t.heading_ink),
            ("table_header_ink", &o.table_header_ink, &mut t.table_header_ink),
        ] {
            if let Some(c) = colour(field, value) {
                *slot = c;
            }
        }
        if let Some(c) = colour("heading_rule", &o.heading_rule) {
            t.heading_rule = Some(c);
        }
        if let Some(v) = o.cover_layout {
            t.cover_layout = v;
        }
        if let Some(v) = o.heading_bold {
            t.heading_bold = v;
        }
        if let Some(v) = o.heading_numbers {
            t.heading_numbers = v;
        }
        for (v, slot) in [
            (o.section_page_break, &mut t.section_page_break),
            (o.control_page, &mut t.control_page),
            (o.running_head, &mut t.running_head),
            (o.foot_sensitivity, &mut t.foot_sensitivity),
        ] {
            if let Some(v) = v {
                *slot = v;
            }
        }
        if o.table_widths.is_some() {
            t.content_widths = true;
        }
        if let Some(r) = &o.revision_prefix {
            t.revision_prefix = Cow::Owned(r.clone());
        }
        if let Some(f) = &o.heading_font {
            t.heading_font = Cow::Owned(f.clone());
        }
        if let Some(f) = &o.body_font {
            t.body_font = Cow::Owned(f.clone());
        }
        let mut length = |field: &str, v: f32, lo: f32, hi: f32| -> bool {
            let ok = v.is_finite() && (lo..=hi).contains(&v);
            if !ok {
                findings.push(Finding::error(
                    "ORD001",
                    format!("/template/{field}"),
                    format!("{v} is outside {lo} to {hi}"),
                ));
            }
            ok
        };
        if let Some(v) = o.cover_image_height_mm
            && length("cover_image_height_mm", v, 10.0, 200.0)
        {
            t.cover_image_height_mm = v;
        }
        if let Some(v) = o.body_size_pt
            && length("body_size_pt", v, 6.0, 16.0)
        {
            t.body_size_pt = v;
        }
        if let Some(sizes) = o.heading_sizes_pt
            && sizes.iter().all(|&v| length("heading_sizes_pt", v, 6.0, 48.0))
        {
            t.heading_sizes_pt = Some(sizes);
        }
        if let Some(m) = o.margins_mm
            && [m.top, m.bottom, m.left, m.right].iter().all(|&v| length("margins_mm", v, 0.0, 60.0))
        {
            (t.margin_top_mm, t.margin_bottom_mm, t.margin_left_mm, t.margin_right_mm) =
                (m.top, m.bottom, m.left, m.right);
        }
        for (i, id) in o.fonts.iter().enumerate() {
            match assets.get(id) {
                Some(a) if a.media_type == MediaType::Font => t.fonts.push(id.clone()),
                Some(_) => findings.push(Finding::error(
                    "ORD011",
                    format!("/template/fonts/{i}"),
                    format!("the asset {id:?} is not a font (font/ttf, font/otf or font/collection)"),
                )),
                None => findings.push(Finding::error(
                    "ORD011",
                    format!("/template/fonts/{i}"),
                    format!("the font names the asset {id:?}, which does not exist"),
                )),
            }
        }
        if let Some(cover) = overrides.cover {
            t.cover = cover;
        }
        if let Some(logo) = &overrides.organisation_logo {
            if assets.contains_key(logo) {
                t.organisation_logo = Some(logo.clone());
            } else {
                findings.push(Finding::error(
                    "ORD011",
                    "/template/organisation_logo",
                    format!("the logo names the asset {logo:?}, which does not exist"),
                ));
            }
        }
        (t, findings)
    }
}
