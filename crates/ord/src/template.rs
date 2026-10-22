//! The template: page, colours and fonts (spec 0001 §7).
//!
//! [`DEFAULT`] is the one table of the default look. Every writer (PDF, HTML, Word) reads its values from a
//! [`Template`], which starts as [`DEFAULT`] and takes the document's overrides through
//! [`Template::with_overrides`].

use std::fmt;

use crate::assets::Assets;
use crate::finding::Finding;
use crate::model::TemplateOverrides;

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
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b] = self.0;
        write!(f, "#{r:02x}{g:02x}{b:02x}")
    }
}

/// The look of a rendered document.
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
    /// The heading font family.
    pub heading_font: &'static str,
    /// The font family of text, tables and numbers.
    pub body_font: &'static str,
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
    heading_font: "Libre Caslon Text",
    body_font: "Public Sans",
    body_size_pt: 10.0,
    table_tabular_figures: true,
    mono_font: "IBM Plex Mono",
    cover: true,
    organisation_logo: None,
};

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
        let colours: [(&str, &Option<String>, &mut Colour); 5] = [
            ("paper", &overrides.paper, &mut t.paper),
            ("ink", &overrides.ink, &mut t.ink),
            ("ink_2", &overrides.ink_2, &mut t.ink_2),
            ("rule", &overrides.rule, &mut t.rule),
            ("accent_ground", &overrides.accent_ground, &mut t.accent_ground),
        ];
        for (field, value, slot) in colours {
            let Some(value) = value else { continue };
            match Colour::parse(value) {
                Some(c) => *slot = c,
                None => findings.push(Finding::error(
                    "ORD001",
                    format!("/template/{field}"),
                    format!("the colour {value:?} is not of the form #rrggbb"),
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
