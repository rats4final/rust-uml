use std::fmt::Write;

use crate::geom::{Color, Point};
use crate::icons::{IconSource, NoIcons, base64};
use crate::scene::{Prim, Scene};

const MARGIN: f64 = 10.0;

/// Standalone SVG for a scene. Text names Liberation Sans first (the font
/// used for layout) with Arial-compatible fallbacks, so widths match closely.
/// Icons draw as placeholders; see [`to_svg_with`].
pub fn to_svg(scene: &Scene) -> String {
    to_svg_with(scene, &NoIcons)
}

/// [`to_svg`], embedding the icons `icons` has.
pub fn to_svg_with(scene: &Scene, icons: &dyn IconSource) -> String {
    let b = scene.bounds.expand(MARGIN);
    let mut s = String::new();
    // Writing to a String cannot fail, so the results are ignored.
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}" font-family="'Liberation Sans', Arial, Helvetica, sans-serif">"#,
        x = fmt(b.x),
        y = fmt(b.y),
        w = fmt(b.w),
        h = fmt(b.h)
    );
    let _ = writeln!(
        s,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="white"/>"#,
        fmt(b.x),
        fmt(b.y),
        fmt(b.w),
        fmt(b.h)
    );
    for prim in &scene.prims {
        write_prim(&mut s, prim, icons);
    }
    s.push_str("</svg>\n");
    s
}

/// Write one primitive. An icon the source has is embedded as an image;
/// otherwise its placeholder is drawn.
fn write_prim(s: &mut String, prim: &Prim, icons: &dyn IconSource) {
    match prim {
        Prim::Rect {
            rect,
            fill,
            stroke,
            dashed,
        } => {
            let _ = writeln!(
                s,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"{}/>"#,
                fmt(rect.x),
                fmt(rect.y),
                fmt(rect.w),
                fmt(rect.h),
                paint(*fill),
                stroke.to_hex(),
                dash(*dashed)
            );
        }
        Prim::RoundRect {
            rect,
            radius,
            fill,
            stroke,
        } => {
            let _ = writeln!(
                s,
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}" stroke="{}"/>"#,
                fmt(rect.x),
                fmt(rect.y),
                fmt(rect.w),
                fmt(rect.h),
                fmt(*radius),
                paint(*fill),
                stroke.to_hex()
            );
        }
        Prim::Ellipse {
            rect,
            fill,
            stroke,
            dashed,
        } => {
            let _ = writeln!(
                s,
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="{}" stroke="{}"{}/>"#,
                fmt(rect.x + rect.w / 2.0),
                fmt(rect.y + rect.h / 2.0),
                fmt(rect.w / 2.0),
                fmt(rect.h / 2.0),
                paint(*fill),
                stroke.to_hex(),
                dash(*dashed)
            );
        }
        Prim::Line {
            points,
            stroke,
            dashed,
        } => {
            let _ = writeln!(
                s,
                r#"<polyline points="{}" fill="none" stroke="{}"{}/>"#,
                points_attr(points),
                stroke.to_hex(),
                dash(*dashed)
            );
        }
        Prim::Polygon {
            points,
            fill,
            stroke,
        } => {
            let _ = writeln!(
                s,
                r#"<polygon points="{}" fill="{}" stroke="{}"/>"#,
                points_attr(points),
                paint(*fill),
                stroke.to_hex()
            );
        }
        Prim::Icon {
            rect,
            family,
            name,
            fallback,
        } => match icons.svg(family, name) {
            Some(svg) => {
                let _ = writeln!(
                    s,
                    r#"<image x="{}" y="{}" width="{}" height="{}" href="data:image/svg+xml;base64,{}"/>"#,
                    fmt(rect.x),
                    fmt(rect.y),
                    fmt(rect.w),
                    fmt(rect.h),
                    base64(svg.as_bytes())
                );
            }
            None => {
                for p in fallback {
                    write_prim(s, p, icons);
                }
            }
        },
        Prim::Text {
            rect,
            baseline,
            text,
            style,
        } => {
            let mut attrs = format!(r#" font-size="{}""#, fmt(style.size));
            if style.bold {
                attrs += r#" font-weight="bold""#;
            }
            if style.italic {
                attrs += r#" font-style="italic""#;
            }
            if style.underline {
                attrs += r#" text-decoration="underline""#;
            }
            if style.color != Color::BLACK {
                attrs += &format!(r#" fill="{}""#, style.color.to_hex());
            }
            if style.vertical {
                // Turned a quarter left about the start of the baseline.
                let (x, y) = (fmt(rect.x + baseline), fmt(rect.bottom()));
                let _ = writeln!(
                    s,
                    r#"<text x="{x}" y="{y}" transform="rotate(-90 {x} {y})"{attrs}>{}</text>"#,
                    escape(text)
                );
            } else {
                let _ = writeln!(
                    s,
                    r#"<text x="{}" y="{}"{attrs}>{}</text>"#,
                    fmt(rect.x),
                    fmt(rect.y + baseline),
                    escape(text)
                );
            }
        }
    }
}

fn fmt(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

fn dash(dashed: bool) -> &'static str {
    if dashed {
        r#" stroke-dasharray="6 4""#
    } else {
        ""
    }
}

fn paint(c: Option<Color>) -> String {
    c.map_or_else(|| "none".to_owned(), Color::to_hex)
}

fn points_attr(points: &[Point]) -> String {
    points
        .iter()
        .map(|p| format!("{},{}", fmt(p.x), fmt(p.y)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
