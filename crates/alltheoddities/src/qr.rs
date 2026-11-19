//! A QR code drawn with text, for a terminal.

use qrcode::{Color, QrCode};

/// The quiet zone around the code, in modules. A phone needs it to find the
/// code.
const QUIET: usize = 2;

/// Draws the QR code of `text` with half blocks: each line of text holds two
/// rows of modules. A light module is a filled half, so a terminal with a
/// dark background shows dark modules on light, as a phone expects. The text is empty when
/// the data is too long for a QR code.
pub fn render(text: &str) -> String {
    let Ok(code) = QrCode::new(text.as_bytes()) else {
        return String::new();
    };
    let n = code.width();
    let colors = code.to_colors();
    let size = n + 2 * QUIET;
    let dark = |x: usize, y: usize| -> bool {
        if x < QUIET || y < QUIET || x >= n + QUIET || y >= n + QUIET {
            return false;
        }
        colors[(y - QUIET) * n + (x - QUIET)] == Color::Dark
    };
    let mut out = String::new();
    for y in (0..size).step_by(2) {
        for x in 0..size {
            out.push(match (!dark(x, y), !dark(x, y + 1)) {
                (true, true) => '\u{2588}',
                (true, false) => '\u{2580}',
                (false, true) => '\u{2584}',
                (false, false) => ' ',
            });
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_is_square_in_modules() {
        let text = render("WIFI:T:WPA;S:home;P:secret;;");
        let lines: Vec<&str> = text.lines().collect();
        let width = lines[0].chars().count();
        assert!(lines.iter().all(|l| l.chars().count() == width));
        // Two rows of modules in each line of text.
        assert_eq!(lines.len(), width.div_ceil(2));
    }

    #[test]
    fn render_of_the_same_text_is_the_same() {
        assert_eq!(render("abc"), render("abc"));
        assert_ne!(render("abc"), render("abd"));
    }
}
