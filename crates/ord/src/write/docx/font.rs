//! Font embedding (ECMA-376 Part 1, §17.8.1 and §17.8.3.3): each bundled TrueType file is stored as an
//! obfuscated `.odttf` part, with a font key GUID in the font table.
//!
//! Obfuscation: write the key as 32 hex digits (no braces or dashes), read it as 16 bytes, reverse their
//! order, and XOR the first 32 bytes of the font with that key (bytes 0 to 15 and again 16 to 31). The key is
//! derived from the font's file name, so the output is the same on every render.

use crate::fonts::FontFile;

/// Which embedding slot a bundled font fills, or None when Word has no slot for it (a medium weight).
pub fn slot(f: &FontFile) -> Option<&'static str> {
    match (f.weight, f.italic) {
        (400, false) => Some("embedRegular"),
        (700, false) => Some("embedBold"),
        (400, true) => Some("embedItalic"),
        (700, true) => Some("embedBoldItalic"),
        _ => None,
    }
}

/// The font key: a GUID in braces, upper case, derived from the file name.
pub fn key(file_name: &str) -> String {
    let a = fnv1a64(file_name.as_bytes(), 0xcbf2_9ce4_8422_2325);
    let b = fnv1a64(file_name.as_bytes(), 0x6c62_272e_07bb_0142);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    // Mark it as a version 4 (random-form), variant 1 GUID, so readers accept its shape.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    format!("{{{}-{}-{}-{}-{}}}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// The font obfuscated with the key (the same operation undoes it).
pub fn obfuscate(ttf: &[u8], key: &str) -> Vec<u8> {
    let hex: String = key.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let mut k = [0u8; 16];
    for (i, slot) in k.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    k.reverse();
    let mut out = ttf.to_vec();
    for (i, b) in out.iter_mut().take(32).enumerate() {
        *b ^= k[i % 16];
    }
    out
}

fn fnv1a64(data: &[u8], seed: u64) -> u64 {
    let mut h = seed;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_shape_and_round_trip() {
        let k = key("PublicSans-Regular.ttf");
        assert_eq!(k.len(), 38);
        assert_eq!(k, key("PublicSans-Regular.ttf"));
        assert_ne!(k, key("PublicSans-Bold.ttf"));
        let data: Vec<u8> = (0..40).collect();
        let o = obfuscate(&data, &k);
        assert_ne!(o[..32], data[..32]);
        assert_eq!(o[32..], data[32..]);
        assert_eq!(obfuscate(&o, &k), data);
    }
}
