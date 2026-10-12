use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

pub const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Bold.ttf");
pub const FONT_ITALIC: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Italic.ttf");
pub const FONT_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/LiberationSans-BoldItalic.ttf");

/// Text measurement. If a bundled font fails to parse (it shouldn't), widths
/// fall back to an estimate instead of failing.
pub struct Fonts {
    faces: [Option<FontRef<'static>>; 4],
}

impl Default for Fonts {
    fn default() -> Self {
        let load = |b: &'static [u8]| FontRef::try_from_slice(b).ok();
        Self {
            faces: [
                load(FONT_REGULAR),
                load(FONT_BOLD),
                load(FONT_ITALIC),
                load(FONT_BOLD_ITALIC),
            ],
        }
    }
}

impl Fonts {
    fn face(&self, bold: bool, italic: bool) -> Option<&FontRef<'static>> {
        self.faces
            .get(bold as usize | (italic as usize) << 1)
            .and_then(Option::as_ref)
    }

    /// Advance width of `text` at `size` px.
    pub fn width(&self, text: &str, size: f64, bold: bool, italic: bool) -> f64 {
        let Some(font) = self.face(bold, italic) else {
            return text.chars().count() as f64 * size * 0.55;
        };
        let scaled = font.as_scaled(em_scale(font, size));
        let mut width = 0.0f32;
        let mut prev = None;
        for c in text.chars() {
            let id = scaled.glyph_id(c);
            if let Some(p) = prev {
                width += scaled.kern(p, id);
            }
            width += scaled.h_advance(id);
            prev = Some(id);
        }
        f64::from(width)
    }

    /// Distance from the top of a line box to the baseline.
    pub fn ascent(&self, size: f64) -> f64 {
        match self.face(false, false) {
            Some(font) => f64::from(font.as_scaled(em_scale(font, size)).ascent()),
            None => size * 0.9,
        }
    }

    /// Height of one line of text.
    pub fn line_height(&self, size: f64) -> f64 {
        match self.face(false, false) {
            Some(font) => f64::from(font.as_scaled(em_scale(font, size)).height()).ceil(),
            None => (size * 1.15).ceil(),
        }
    }
}

/// `size` is an em size in px, as in CSS and StarUML fonts. ab_glyph's
/// `PxScale` is the full ascent-to-descent height instead, so convert.
/// (`Font::pt_to_px_scale` would also apply a 96/72 points-to-px factor.)
fn em_scale(font: &FontRef<'static>, size: f64) -> PxScale {
    match font.units_per_em() {
        Some(upem) if upem > 0.0 => PxScale::from(size as f32 * font.height_unscaled() / upem),
        _ => PxScale::from(size as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_load_and_measure() {
        let f = Fonts::default();
        assert!(f.faces.iter().all(Option::is_some));
        // Arial/Liberation Sans advance widths at 1000 units/em: H 722, e 556, l 222, o 556.
        let w = f.width("Hello", 10.0, false, false);
        assert!((w - 22.78).abs() < 0.05, "{w}");
        assert!(f.width("Hello", 13.0, true, false) > w, "bold is wider");
        assert_eq!(f.width("", 13.0, false, false), 0.0);
    }
}
