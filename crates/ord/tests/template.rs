//! The default template, its overrides and the bundled fonts (spec 0001 §7).
//! Label: sourced - spec 0001 §7 (page, margins, colours, fonts, sizes) and the font files' own name tables.

use std::path::{Path, PathBuf};

use ord::model::{MediaType, TemplateOverrides};
use ord::template::{Colour, DEFAULT};
use ord::{Asset, Assets, Template};

fn fonts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts")
}

fn hex(c: Colour) -> String {
    c.hex()
}

#[test]
fn the_default_template_is_the_table_of_section_7() {
    let t = Template::default();
    assert_eq!(t, DEFAULT, "Template::default() is the one table");
    assert_eq!((t.page_width_mm, t.page_height_mm), (210.0, 297.0));
    assert_eq!((t.margin_top_mm, t.margin_bottom_mm), (25.0, 25.0));
    assert_eq!((t.margin_left_mm, t.margin_right_mm), (20.0, 20.0));
    assert_eq!(hex(t.paper), "#f2e8cf");
    assert_eq!(hex(t.body_paper), "#ffffff");
    assert_eq!(hex(t.ink), "#2b2925");
    assert_eq!(hex(t.ink_2), "#4a463e");
    assert_eq!(hex(t.rule), "#d8caa6");
    assert_eq!(hex(t.accent_ground), "#e8dcbd");
    assert_eq!(hex(t.data_ground), "#f8f2e3");
    assert_eq!(t.data_rule_pt, 1.5);
    assert_eq!(t.heading_font, "Libre Caslon Text");
    assert_eq!(t.body_font, "Public Sans");
    assert_eq!(t.body_size_pt, 10.0);
    assert!(t.table_tabular_figures);
    assert_eq!(t.mono_font, "IBM Plex Mono");
    assert!(t.cover);
    assert_eq!(t.organisation_logo, None);
}

#[test]
fn every_template_font_is_bundled() {
    let t = Template::default();
    for family in [t.heading_font, t.body_font, t.mono_font] {
        assert!(ord::fonts::family(family).next().is_some(), "{family} is not bundled");
    }
}

#[test]
fn colours_read_and_write_as_rrggbb() {
    assert_eq!(Colour::parse("#A0b1C2"), Some(Colour([0xa0, 0xb1, 0xc2])));
    assert_eq!(Colour([0xa0, 0xb1, 0xc2]).to_string(), "#a0b1c2");
    for bad in ["", "#", "a0b1c2", "#a0b1c", "#a0b1c2d", "#g0b1c2", "#+0b1c2", "#a0b1c²"] {
        assert_eq!(Colour::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn overrides_are_applied() {
    let mut assets = Assets::new();
    assets.insert("logo".into(), Asset { media_type: MediaType::Png, bytes: vec![1, 2, 3] });
    let o = TemplateOverrides {
        organisation_logo: Some("logo".into()),
        paper: Some("#010203".into()),
        ink: Some("#040506".into()),
        ink_2: Some("#070809".into()),
        rule: Some("#0A0B0C".into()),
        accent_ground: Some("#0d0e0f".into()),
        cover: Some(false),
    };
    let (t, findings) = Template::with_overrides(&o, &assets);
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(hex(t.paper), "#010203");
    assert_eq!(hex(t.ink), "#040506");
    assert_eq!(hex(t.ink_2), "#070809");
    assert_eq!(hex(t.rule), "#0a0b0c");
    assert_eq!(hex(t.accent_ground), "#0d0e0f");
    assert!(!t.cover);
    assert_eq!(t.organisation_logo.as_deref(), Some("logo"));
    // What the overrides do not name stays at the default.
    assert_eq!(t.body_paper, DEFAULT.body_paper);
    assert_eq!(t.data_ground, DEFAULT.data_ground);
    assert_eq!(t.body_font, DEFAULT.body_font);
}

#[test]
fn no_overrides_give_the_default() {
    let (t, findings) = Template::with_overrides(&TemplateOverrides::default(), &Assets::new());
    assert!(findings.is_empty());
    assert_eq!(t, DEFAULT);
}

#[test]
fn a_bad_colour_is_ord001_at_its_field_and_keeps_the_default() {
    let o = TemplateOverrides {
        ink: Some("red".into()),
        accent_ground: Some("#12345".into()),
        rule: Some("#123456".into()),
        ..Default::default()
    };
    let (t, findings) = Template::with_overrides(&o, &Assets::new());
    let got: Vec<_> = findings.iter().map(|f| (f.code, f.path.as_str(), f.is_error())).collect();
    assert_eq!(got, [("ORD001", "/template/ink", true), ("ORD001", "/template/accent_ground", true)]);
    assert_eq!(t.ink, DEFAULT.ink);
    assert_eq!(t.accent_ground, DEFAULT.accent_ground);
    assert_eq!(hex(t.rule), "#123456");
}

#[test]
fn an_unknown_logo_is_ord011() {
    let o = TemplateOverrides { organisation_logo: Some("missing".into()), ..Default::default() };
    let (t, findings) = Template::with_overrides(&o, &Assets::new());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "ORD011");
    assert_eq!(findings[0].path, "/template/organisation_logo");
    assert!(findings[0].is_error());
    assert_eq!(t.organisation_logo, None);
}

/// The font's family name: the typographic family (name 16) when present, else the family (name 1).
fn family_name(face: &ttf_parser::Face) -> String {
    let get = |id| face.names().into_iter().filter(|n| n.name_id == id).find_map(|n| n.to_string());
    get(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| get(ttf_parser::name_id::FAMILY))
        .expect("a family name")
}

#[test]
fn every_bundled_font_parses_with_its_family_weight_and_style() {
    let fonts = ord::fonts::bundled();
    assert_eq!(fonts.len(), 9);
    for f in fonts {
        let face = ttf_parser::Face::parse(f.ttf, 0).unwrap_or_else(|e| panic!("{}: {e}", f.file_name));
        assert!(!face.is_variable(), "{} is static", f.file_name);
        assert_eq!(family_name(&face), f.family, "{}", f.file_name);
        assert_eq!(face.weight().to_number(), f.weight, "{}", f.file_name);
        assert_eq!(face.is_italic(), f.italic, "{}", f.file_name);
        assert_eq!(face.is_bold(), f.weight == 700, "{}", f.file_name);
        // A full glyph set, not a subset.
        assert!(face.number_of_glyphs() > 300, "{} has {} glyphs", f.file_name, face.number_of_glyphs());
        // The compiled-in bytes are the files on disk.
        assert_eq!(std::fs::read(fonts_dir().join(f.file_name)).unwrap(), f.ttf, "{}", f.file_name);
    }
    let expected = [
        ("Public Sans", 400, false),
        ("Public Sans", 700, false),
        ("Public Sans", 400, true),
        ("Public Sans", 700, true),
        ("Libre Caslon Text", 400, false),
        ("Libre Caslon Text", 700, false),
        ("Libre Caslon Text", 400, true),
        ("IBM Plex Mono", 400, false),
        ("IBM Plex Mono", 500, false),
    ];
    let got: Vec<_> = fonts.iter().map(|f| (f.family, f.weight, f.italic)).collect();
    assert_eq!(got, expected);
}

#[test]
fn every_woff2_copy_is_woff2() {
    for f in ord::fonts::bundled() {
        assert_eq!(&f.woff2[..4], b"wOF2", "{}", f.file_name);
        let stem = f.file_name.trim_end_matches(".ttf");
        let on_disk = std::fs::read(fonts_dir().join("woff2").join(format!("{stem}.woff2"))).unwrap();
        assert_eq!(on_disk, f.woff2, "{}", f.file_name);
    }
}

#[test]
fn the_licences_and_sources_are_bundled() {
    for (file, family) in [
        ("OFL-PublicSans.txt", "Public Sans"),
        ("OFL-LibreCaslonText.txt", "Libre Caslon Text"),
        ("OFL-IBMPlexMono.txt", "IBM Plex Mono"),
    ] {
        let text = std::fs::read_to_string(fonts_dir().join(file)).unwrap();
        assert!(text.contains("SIL Open Font License, Version 1.1"), "{file}");
        let notice = std::fs::read_to_string(fonts_dir().join("../../../NOTICE")).unwrap();
        assert!(notice.contains(family), "NOTICE names {family}");
    }
    let sources = std::fs::read_to_string(fonts_dir().join("SOURCES.md")).unwrap();
    for f in ord::fonts::bundled() {
        assert!(sources.contains(f.file_name), "SOURCES.md names {}", f.file_name);
    }
    assert!(sources.contains("fontTools.varLib.instancer"), "SOURCES.md gives the instancing command");
}
