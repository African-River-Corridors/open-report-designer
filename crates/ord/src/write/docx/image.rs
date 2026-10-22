//! Images for the Word writer: the size of a PNG or SVG, an SVG drawn to a PNG fallback, and a one-pixel PNG
//! of the cover's paper colour.

use std::sync::Arc;

use crate::template::Colour;

/// The pixel width of an SVG's fallback: the text width (170 mm in the default template) at about 150 dpi.
pub const FALLBACK_DPI: f32 = 150.0;
/// The tallest fallback, in pixels: an extreme aspect ratio cannot ask for an enormous image.
const MAX_FALLBACK_SIDE: f32 = 6000.0;

/// A font database holding only the bundled fonts: never the system's.
fn fontdb() -> resvg::usvg::fontdb::Database {
    let mut db = resvg::usvg::fontdb::Database::new();
    for f in crate::fonts::bundled() {
        db.load_font_data(f.ttf.to_vec());
    }
    db.set_sans_serif_family(crate::template::DEFAULT.body_font);
    db.set_serif_family(crate::template::DEFAULT.heading_font);
    db.set_monospace_family(crate::template::DEFAULT.mono_font);
    db
}

/// An SVG read with the bundled fonts only and no access to files: an `<image>` that names a file is left out.
pub fn parse_svg(bytes: &[u8]) -> Result<resvg::usvg::Tree, String> {
    let opt = resvg::usvg::Options {
        font_family: crate::template::DEFAULT.body_font.to_string(),
        fontdb: Arc::new(fontdb()),
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_data: resvg::usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..resvg::usvg::Options::default()
    };
    resvg::usvg::Tree::from_data(bytes, &opt).map_err(|e| e.to_string())
}

/// The SVG drawn as a PNG `width_mm` wide at [`FALLBACK_DPI`].
pub fn svg_fallback(tree: &resvg::usvg::Tree, width_mm: f32) -> Result<Vec<u8>, String> {
    let size = tree.size();
    let px_w = (width_mm / 25.4 * FALLBACK_DPI).round().max(1.0);
    let scale = px_w / size.width();
    let px_h = (size.height() * scale).round().clamp(1.0, MAX_FALLBACK_SIDE);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px_w as u32, px_h as u32)
        .ok_or_else(|| "the SVG has no drawable size".to_string())?;
    resvg::render(tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| e.to_string())
}

/// A PNG of one pixel of the colour.
pub fn pixel_png(c: Colour) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    // Width 1, height 1, bit depth 8, colour type 2 (RGB), compression, filter, interlace 0.
    chunk(&mut png, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0]);
    let raw = [0, c.0[0], c.0[1], c.0[2]]; // filter byte 0, then the pixel
    chunk(&mut png, b"IDAT", &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9));
    chunk(&mut png, b"IEND", &[]);
    png
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32fast::hash(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}
