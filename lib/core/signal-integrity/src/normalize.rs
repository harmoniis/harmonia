//! Unicode normalization for injection scanning: zero-width stripping, homoglyph
//! mapping, combining-mark removal, fullwidth-to-ASCII folding.

/// Returns true for zero-width and soft-hyphen characters used to evade scanning.
fn is_zero_width(c: char) -> bool {
    matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{00AD}')
}

/// Map a single character through NFKC-like normalization:
/// fullwidth Latin/digits, Cyrillic homoglyphs, fullwidth space.
fn normalize_char(c: char) -> char {
    match c {
        // Fullwidth Latin uppercase -> ASCII uppercase
        '\u{FF21}'..='\u{FF3A}' => (c as u32 - 0xFF21 + b'A' as u32) as u8 as char,
        // Fullwidth Latin lowercase -> ASCII lowercase
        '\u{FF41}'..='\u{FF5A}' => (c as u32 - 0xFF41 + b'a' as u32) as u8 as char,
        // Fullwidth digits -> ASCII digits
        '\u{FF10}'..='\u{FF19}' => (c as u32 - 0xFF10 + b'0' as u32) as u8 as char,
        // Fullwidth space -> ASCII space
        '\u{3000}' => ' ',
        // Cyrillic homoglyphs -> Latin equivalents
        '\u{0430}' => 'a', '\u{0435}' => 'e', '\u{043E}' => 'o', '\u{0440}' => 'p', '\u{0441}' => 'c',
        '\u{0443}' => 'y', '\u{0445}' => 'x',
        '\u{0410}' => 'A', '\u{0412}' => 'B', '\u{0415}' => 'E', '\u{041A}' => 'K', '\u{041C}' => 'M',
        '\u{041D}' => 'H', '\u{041E}' => 'O', '\u{0420}' => 'P', '\u{0421}' => 'C', '\u{0422}' => 'T',
        '\u{0423}' => 'Y', '\u{0425}' => 'X',
        _ => c,
    }
}

/// Normalize text for injection scanning: strip zero-width chars, map homoglyphs
/// and fullwidth characters to ASCII, strip combining diacritical marks, lowercase.
pub(crate) fn normalize_for_scan(text: &str) -> String {
    text.chars()
        .filter(|c| !is_zero_width(*c))
        .filter(|c| {
            let cp = *c as u32;
            // Strip combining diacritical marks (U+0300..U+036F)
            !(0x0300..=0x036F).contains(&cp)
        })
        .map(normalize_char)
        .collect::<String>()
        .to_lowercase()
}

/// Public alias preserved for backward compatibility.
pub fn normalize_unicode(text: &str) -> String {
    normalize_for_scan(text)
}
