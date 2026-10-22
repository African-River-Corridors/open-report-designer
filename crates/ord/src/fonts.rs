//! The bundled fonts (spec 0001 §7): static TrueType files for PDF and Word, and woff2 copies for HTML.
//!
//! All are under the SIL Open Font License 1.1. `crates/ord/fonts/SOURCES.md` says where each came from.
//! The bytes are compiled in, so the core reads no file.

/// One bundled font file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFile {
    /// The family name, as the template names it (for example "Public Sans").
    pub family: &'static str,
    /// The style name (for example "Bold Italic").
    pub style: &'static str,
    /// The OpenType weight class (400 regular, 500 medium, 700 bold).
    pub weight: u16,
    pub italic: bool,
    /// The file name under `crates/ord/fonts/`, with `.ttf`. The woff2 copy has the same stem.
    pub file_name: &'static str,
    /// The TrueType file.
    pub ttf: &'static [u8],
    /// The woff2 copy, for HTML.
    pub woff2: &'static [u8],
}

macro_rules! font {
    ($family:literal, $style:literal, $weight:literal, $italic:literal, $stem:literal) => {
        FontFile {
            family: $family,
            style: $style,
            weight: $weight,
            italic: $italic,
            file_name: concat!($stem, ".ttf"),
            ttf: include_bytes!(concat!("../fonts/", $stem, ".ttf")),
            woff2: include_bytes!(concat!("../fonts/woff2/", $stem, ".woff2")),
        }
    };
}

static BUNDLED: [FontFile; 9] = [
    font!("Public Sans", "Regular", 400, false, "PublicSans-Regular"),
    font!("Public Sans", "Bold", 700, false, "PublicSans-Bold"),
    font!("Public Sans", "Italic", 400, true, "PublicSans-Italic"),
    font!("Public Sans", "Bold Italic", 700, true, "PublicSans-BoldItalic"),
    font!("Libre Caslon Text", "Regular", 400, false, "LibreCaslonText-Regular"),
    font!("Libre Caslon Text", "Bold", 700, false, "LibreCaslonText-Bold"),
    font!("Libre Caslon Text", "Italic", 400, true, "LibreCaslonText-Italic"),
    font!("IBM Plex Mono", "Regular", 400, false, "IBMPlexMono-Regular"),
    font!("IBM Plex Mono", "Medium", 500, false, "IBMPlexMono-Medium"),
];

/// Every bundled font file.
pub fn bundled() -> &'static [FontFile] {
    &BUNDLED
}

/// The bundled files of one family.
pub fn family(name: &str) -> impl Iterator<Item = &'static FontFile> {
    BUNDLED.iter().filter(move |f| f.family == name)
}
