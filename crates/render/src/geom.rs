#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn dist(self, o: Point) -> f64 {
        (self.x - o.x).hypot(self.y - o.y)
    }

    /// Distance from `self` to the segment `a`–`b`.
    pub fn dist_to_segment(self, a: Point, b: Point) -> f64 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        if len2 == 0.0 {
            return self.dist(a);
        }
        let t = (((self.x - a.x) * dx + (self.y - a.y) * dy) / len2).clamp(0.0, 1.0);
        self.dist(Point::new(a.x + t * dx, a.y + t * dy))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(
            x,
            y,
            self.right().max(o.right()) - x,
            self.bottom().max(o.bottom()) - y,
        )
    }

    pub fn expand(&self, by: f64) -> Rect {
        Rect::new(
            self.x - by,
            self.y - by,
            self.w + 2.0 * by,
            self.h + 2.0 * by,
        )
    }

    /// Where the ray from the centre towards `toward` leaves the rectangle.
    pub fn border_toward(&self, toward: Point) -> Point {
        let c = self.center();
        let (dx, dy) = (toward.x - c.x, toward.y - c.y);
        if dx == 0.0 && dy == 0.0 {
            return c;
        }
        let sx = if dx != 0.0 {
            (self.w / 2.0) / dx.abs()
        } else {
            f64::INFINITY
        };
        let sy = if dy != 0.0 {
            (self.h / 2.0) / dy.abs()
        } else {
            f64::INFINITY
        };
        let s = sx.min(sy);
        Point::new(c.x + dx * s, c.y + dy * s)
    }
}

/// RGBA colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0, 255);
    pub const WHITE: Color = Color(255, 255, 255, 255);

    /// Parse `#rrggbb` (StarUML's colour fields); `None` if malformed.
    pub fn parse(s: &str) -> Option<Color> {
        let hex = s.strip_prefix('#')?;
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        Some(Color(byte(0)?, byte(2)?, byte(4)?, 255))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_point_lies_on_edge() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(
            r.border_toward(Point::new(500.0, 25.0)),
            Point::new(100.0, 25.0)
        );
        assert_eq!(
            r.border_toward(Point::new(50.0, -100.0)),
            Point::new(50.0, 0.0)
        );
    }

    #[test]
    fn colour_parsing_rejects_garbage() {
        assert_eq!(Color::parse("#ff8000"), Some(Color(255, 128, 0, 255)));
        assert_eq!(Color::parse("ff8000"), None);
        assert_eq!(Color::parse("#ff80"), None);
        assert_eq!(Color::parse("#ééé"), None);
    }
}
