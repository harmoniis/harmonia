use qrcode::QrCode;

/// Render `data` as a QR code string suitable for terminal display.
///
/// Uses Unicode half-block characters for 2:1 density so the QR appears
/// roughly square on most monospace fonts.  A 2-module quiet zone is
/// included around the code.
///
/// Dark module = foreground block, light module = background (space).
pub fn render_qr_to_string(data: &str) -> Result<String, String> {
    let code = QrCode::new(data.as_bytes()).map_err(|e| format!("qr encode: {e}"))?;
    let modules = code.to_colors();
    let width = code.width();

    let quiet = 2; // quiet-zone modules on each side
    let total_w = width + 2 * quiet;

    // Helper: returns true when the module at (row, col) is dark,
    // treating the quiet zone and any out-of-bounds as light.
    let is_dark = |row: i32, col: i32| -> bool {
        let r = row - quiet as i32;
        let c = col - quiet as i32;
        if r < 0 || c < 0 || r >= width as i32 || c >= width as i32 {
            return false;
        }
        modules[r as usize * width + c as usize] == qrcode::Color::Dark
    };

    let total_h = width + 2 * quiet;
    let mut out = String::new();

    // Process rows in pairs (top, bottom) to produce half-height output.
    let mut row = 0i32;
    while (row as usize) < total_h {
        for col in 0..total_w as i32 {
            let top = is_dark(row, col);
            let bot = is_dark(row + 1, col);
            let ch = match (top, bot) {
                (true, true) => '\u{2588}',  // █  full block
                (true, false) => '\u{2580}', // ▀  upper half
                (false, true) => '\u{2584}', // ▄  lower half
                (false, false) => ' ',
            };
            out.push(ch);
        }
        out.push('\n');
        row += 2;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_render() {
        let qr = render_qr_to_string("hello").unwrap();
        assert!(!qr.is_empty());
        // Should contain at least one full-block character
        assert!(qr.contains('\u{2588}'));
    }

    #[test]
    fn empty_input_still_works() {
        // QR can encode empty string
        let qr = render_qr_to_string("").unwrap();
        assert!(!qr.is_empty());
    }
}
