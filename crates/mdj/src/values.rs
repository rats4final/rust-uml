//! Text encodings StarUML uses for some field values.

/// `EdgeView.lineStyle` codes.
pub mod line_style {
    pub const RECTILINEAR: i64 = 0;
    pub const OBLIQUE: i64 = 1;
    pub const ROUND_RECT: i64 = 2;
    pub const CURVE: i64 = 3;
    pub const DIRECT: i64 = 4;
}

/// `EdgeParasiticView.edgePosition` codes: which end of the line a label follows.
pub mod edge_position {
    pub const HEAD: i64 = 0;
    pub const MIDDLE: i64 = 1;
    pub const TAIL: i64 = 2;
}

pub const DEFAULT_FONT: &str = "Arial;13;0";

/// Parse a `Points` value: `"x1:y1;x2:y2;…"`. Malformed pairs are skipped.
pub fn parse_points(s: &str) -> Vec<(f64, f64)> {
    s.split(';')
        .filter_map(|pair| {
            let (x, y) = pair.split_once(':')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect()
}

pub fn format_points(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{}:{}", x.round() as i64, y.round() as i64))
        .collect::<Vec<_>>()
        .join(";")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub face: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            face: "Arial".into(),
            size: 13.0,
            bold: false,
            italic: false,
        }
    }
}

impl Font {
    /// Parse `"face;size;style"`; `?` in a slot keeps the default.
    pub fn parse(s: &str) -> Self {
        let mut font = Font::default();
        let mut parts = s.split(';');
        if let Some(face) = parts.next().filter(|f| *f != "?" && !f.is_empty()) {
            font.face = face.to_owned();
        }
        if let Some(size) = parts.next().and_then(|p| p.parse().ok()) {
            font.size = size;
        }
        if let Some(style) = parts.next().and_then(|p| p.parse::<u8>().ok()) {
            font.bold = style & 1 != 0;
            font.italic = style & 2 != 0;
        }
        font
    }

    pub fn format(&self) -> String {
        let style = self.bold as u8 | (self.italic as u8) << 1;
        format!("{};{};{}", self.face, self.size, style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_round_trip() {
        let p = parse_points("10:20;30:-4");
        assert_eq!(p, vec![(10.0, 20.0), (30.0, -4.0)]);
        assert_eq!(format_points(&p), "10:20;30:-4");
        assert!(parse_points("").is_empty());
    }

    #[test]
    fn font_parses_style_bits_and_wildcards() {
        let f = Font::parse("Arial;13;3");
        assert!(f.bold && f.italic);
        assert_eq!(f.format(), "Arial;13;3");
        let g = Font::parse("?;?;?");
        assert_eq!(g, Font::default());
    }
}
