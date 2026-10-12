use std::collections::HashMap;
use std::f64::consts::PI;

use mdj::values::{DEFAULT_FONT, Font, edge_position, parse_points};
use mdj::{Document, Element, Id};

use crate::geom::{Color, Point, Rect};
use crate::layout::{NodeLayout, Shape, TEXT_PAD_X, TEXT_PAD_Y, node_layout};
use crate::route;
use crate::text::Fonts;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
}

/// One drawing instruction, in diagram coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Rect {
        rect: Rect,
        fill: Option<Color>,
        stroke: Color,
    },
    Line {
        points: Vec<Point>,
        stroke: Color,
        dashed: bool,
    },
    Polygon {
        points: Vec<Point>,
        fill: Option<Color>,
        stroke: Color,
    },
    /// `rect` is the text's line box; `baseline` is measured from its top.
    Text {
        rect: Rect,
        baseline: f64,
        text: String,
        style: TextStyle,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum HitShape {
    Rect(Rect),
    Polyline(Vec<Point>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub view: Id,
    pub shape: HitShape,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub prims: Vec<Prim>,
    /// Clickable areas in drawing order (last is on top).
    pub hits: Vec<Hit>,
    pub bounds: Rect,
}

impl Scene {
    /// The top-most view under `p`, with `tolerance` for lines.
    pub fn hit_test(&self, p: Point, tolerance: f64) -> Option<&str> {
        self.hits.iter().rev().find_map(|h| {
            let hit = match &h.shape {
                HitShape::Rect(r) => r.contains(p),
                HitShape::Polyline(pts) => pts
                    .windows(2)
                    .any(|w| matches!(w, [a, b] if p.dist_to_segment(*a, *b) <= tolerance)),
            };
            hit.then_some(h.view.as_str())
        })
    }

    /// The top-most node (not edge) under `p`.
    pub fn node_at(&self, p: Point) -> Option<(&str, Rect)> {
        self.hits.iter().rev().find_map(|h| match &h.shape {
            HitShape::Rect(r) if r.contains(p) => Some((h.view.as_str(), *r)),
            _ => None,
        })
    }

    pub fn node_rect(&self, view: &str) -> Option<Rect> {
        self.hits.iter().find_map(|h| match &h.shape {
            HitShape::Rect(r) if h.view == view => Some(*r),
            _ => None,
        })
    }
}

/// Build the display list for `diagram`. Unknown view types are drawn as
/// plain labelled boxes or lines rather than skipped, so nothing in a
/// StarUML file silently disappears.
pub fn build_scene(doc: &Document, diagram: &str, fonts: &Fonts) -> Scene {
    let mut scene = Scene::default();
    let Some(diagram) = doc.get(diagram) else {
        return scene;
    };
    let views: Vec<&Element> = diagram
        .children("ownedViews")
        .iter()
        .filter_map(|id| doc.get(id))
        .filter(|v| v.bool("visible", true))
        .collect();

    let layouts: HashMap<&str, NodeLayout> = views
        .iter()
        .filter_map(|v| Some((v.id.as_str(), node_layout(doc, v, fonts)?)))
        .collect();
    let rects: HashMap<&str, Rect> = layouts.iter().map(|(k, l)| (*k, l.rect)).collect();

    let mut bounds: Option<Rect> = None;
    let mut grow = |r: Rect| bounds = Some(bounds.map_or(r, |b| b.union(&r)));

    for view in views {
        if let Some(layout) = layouts.get(view.id.as_str()) {
            draw_node(&mut scene, layout, fonts);
            scene.hits.push(Hit {
                view: view.id.clone(),
                shape: HitShape::Rect(layout.rect),
            });
            grow(layout.rect);
        } else if let Some(points) = edge_points(view, &rects) {
            let label_boxes = draw_edge(&mut scene, doc, view, &points, fonts);
            for r in label_boxes {
                grow(r);
            }
            for p in &points {
                grow(Rect::new(p.x, p.y, 0.0, 0.0));
            }
            scene.hits.push(Hit {
                view: view.id.clone(),
                shape: HitShape::Polyline(points),
            });
        }
    }
    scene.bounds = bounds.unwrap_or_default();
    scene
}

fn draw_node(scene: &mut Scene, l: &NodeLayout, fonts: &Fonts) {
    let style = |line: &crate::layout::TextLine| TextStyle {
        size: l.font_size,
        bold: line.bold,
        italic: line.italic,
        underline: line.underline,
        color: l.text,
    };
    match l.shape {
        Shape::Box => scene.prims.push(Prim::Rect {
            rect: l.rect,
            fill: Some(l.fill),
            stroke: l.line,
        }),
        Shape::Package { tab } => {
            scene.prims.push(Prim::Rect {
                rect: tab,
                fill: Some(l.fill),
                stroke: l.line,
            });
            scene.prims.push(Prim::Rect {
                rect: Rect::new(l.rect.x, tab.bottom(), l.rect.w, l.rect.h - tab.h),
                fill: Some(l.fill),
                stroke: l.line,
            });
        }
    }
    let line_h = fonts.line_height(l.font_size);
    let ascent = fonts.ascent(l.font_size);
    for (i, c) in l.compartments.iter().enumerate() {
        if i > 0 {
            scene.prims.push(Prim::Line {
                points: vec![
                    Point::new(c.rect.x, c.rect.y),
                    Point::new(c.rect.right(), c.rect.y),
                ],
                stroke: l.line,
                dashed: false,
            });
        }
        let mut y = c.rect.y + TEXT_PAD_Y;
        for line in &c.lines {
            let s = style(line);
            let w = fonts.width(&line.text, s.size, s.bold, s.italic);
            let x = if line.centered {
                c.rect.x + (c.rect.w - w) / 2.0
            } else {
                c.rect.x + TEXT_PAD_X
            };
            scene.prims.push(Prim::Text {
                rect: Rect::new(x, y, w, line_h),
                baseline: ascent,
                text: line.text.clone(),
                style: s,
            });
            y += line_h;
        }
    }
}

fn edge_points(view: &Element, rects: &HashMap<&str, Rect>) -> Option<Vec<Point>> {
    let tail = rects.get(view.ref_id("tail")?)?;
    let head = rects.get(view.ref_id("head")?)?;
    let stored: Vec<Point> = parse_points(view.str("points").unwrap_or(""))
        .into_iter()
        .map(|(x, y)| Point::new(x, y))
        .collect();
    if stored.len() >= 2 {
        return Some(stored);
    }
    let style = view.num("lineStyle", 1.0) as i64;
    Some(route::route(tail, head, &[], style))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EndMark {
    None,
    OpenArrow,
    HollowTriangle,
    FilledTriangle,
    HollowDiamond,
    FilledDiamond,
}

/// Line style and end markers for an edge view. Associations look at their
/// ends; other relationships are recognised by view type name so that the
/// many UML edge kinds get sensible notation.
fn edge_style(doc: &Document, view: &Element) -> (bool, EndMark, EndMark) {
    let t = view.type_name.as_str();
    let model = view.ref_id("model").and_then(|m| doc.get(m));
    if let Some(m) = model.filter(|m| m.get("end1").is_some()) {
        let end = |k: &str| m.children(k).first().and_then(|id| doc.get(id));
        let mark = |k: &str| match end(k) {
            Some(e) => match (e.str("aggregation"), e.str("navigable")) {
                (Some("composite"), _) => EndMark::FilledDiamond,
                (Some("shared"), _) => EndMark::HollowDiamond,
                (_, Some("navigable")) => EndMark::OpenArrow,
                _ => EndMark::None,
            },
            None => EndMark::None,
        };
        return (false, mark("end1"), mark("end2"));
    }
    let (dashed, head) = if t.contains("Generalization") {
        (false, EndMark::HollowTriangle)
    } else if t.contains("Realization") {
        (true, EndMark::HollowTriangle)
    } else if t.contains("Extension") && !t.contains("Extend") {
        (false, EndMark::FilledTriangle)
    } else if [
        "Dependency",
        "Abstraction",
        "Include",
        "Extend",
        "Deployment",
        "Usage",
        "Manifestation",
    ]
    .iter()
    .any(|k| t.contains(k))
    {
        (true, EndMark::OpenArrow)
    } else if t.contains("NoteLink") || t.contains("ConstraintLink") {
        (true, EndMark::None)
    } else {
        (false, EndMark::None)
    };
    (dashed, EndMark::None, head)
}

/// Draw an edge; returns the boxes of the labels it placed.
fn draw_edge(
    scene: &mut Scene,
    doc: &Document,
    view: &Element,
    pts: &[Point],
    fonts: &Fonts,
) -> Vec<Rect> {
    let stroke = view
        .str("lineColor")
        .and_then(Color::parse)
        .unwrap_or(Color::BLACK);
    let (dashed, tail_mark, head_mark) = edge_style(doc, view);
    scene.prims.push(Prim::Line {
        points: pts.to_vec(),
        stroke,
        dashed,
    });
    if let ([first, second, ..], [.., before_last, last]) = (pts, pts) {
        draw_mark(scene, *first, *second, tail_mark, stroke);
        draw_mark(scene, *last, *before_last, head_mark, stroke);
    }

    let font = Font::parse(view.str("font").unwrap_or(DEFAULT_FONT));
    let size = if font.size > 0.0 && font.size.is_finite() {
        font.size
    } else {
        13.0
    };
    let color = view
        .str("fontColor")
        .and_then(Color::parse)
        .unwrap_or(Color::BLACK);
    let model = view.ref_id("model").and_then(|m| doc.get(m));
    let end = |k: &str| {
        model
            .and_then(|m| m.children(k).first())
            .and_then(|id| doc.get(id))
    };

    let texts: Vec<(&str, String)> = vec![
        (
            "nameLabel",
            model.map(|m| m.name().to_owned()).unwrap_or_default(),
        ),
        (
            "stereotypeLabel",
            model
                .and_then(|m| uml::stereotype_text(doc, m))
                .unwrap_or_default(),
        ),
        (
            "tailRoleNameLabel",
            end("end1").map(|e| e.name().to_owned()).unwrap_or_default(),
        ),
        (
            "headRoleNameLabel",
            end("end2").map(|e| e.name().to_owned()).unwrap_or_default(),
        ),
        (
            "tailMultiplicityLabel",
            end("end1")
                .and_then(|e| e.str("multiplicity"))
                .unwrap_or("")
                .to_owned(),
        ),
        (
            "headMultiplicityLabel",
            end("end2")
                .and_then(|e| e.str("multiplicity"))
                .unwrap_or("")
                .to_owned(),
        ),
    ];

    let line_h = fonts.line_height(size);
    let ascent = fonts.ascent(size);
    let mut boxes = Vec::new();
    for (key, text) in texts {
        if text.is_empty() {
            continue;
        }
        // A label's own `visible` flag is ignored: StarUML stores `false` for
        // labels that were empty when saved, which would hide text typed since.
        let label = view.ref_id(key).and_then(|id| doc.get(id));
        let (pos, alpha, distance) = match label {
            Some(l) => (
                l.num("edgePosition", edge_position::MIDDLE as f64) as i64,
                l.num("alpha", PI / 2.0),
                l.num("distance", 15.0),
            ),
            None => default_label_placement(key),
        };
        let anchor = label_point(pts, pos, alpha, distance);
        let w = fonts.width(&text, size, false, false);
        let rect = Rect::new(anchor.x - w / 2.0, anchor.y - line_h / 2.0, w, line_h);
        scene.prims.push(Prim::Text {
            rect,
            baseline: ascent,
            text,
            style: TextStyle {
                size,
                bold: false,
                italic: false,
                underline: false,
                color,
            },
        });
        boxes.push(rect);
    }
    boxes
}

fn default_label_placement(key: &str) -> (i64, f64, f64) {
    match key {
        "stereotypeLabel" => (edge_position::MIDDLE, PI / 2.0, 30.0),
        "tailRoleNameLabel" => (edge_position::TAIL, PI / 6.0, 30.0),
        "tailMultiplicityLabel" => (edge_position::TAIL, -PI / 6.0, 25.0),
        "headRoleNameLabel" => (edge_position::HEAD, -PI / 6.0, 30.0),
        "headMultiplicityLabel" => (edge_position::HEAD, PI / 6.0, 25.0),
        _ => (edge_position::MIDDLE, PI / 2.0, 15.0),
    }
}

/// Position of a label that follows one end (or the middle) of the line,
/// rotated `alpha` from the line's direction and `distance` away.
fn label_point(pts: &[Point], position: i64, alpha: f64, distance: f64) -> Point {
    let (anchor, toward) = match (position, pts) {
        (edge_position::HEAD, [.., b, a]) => (*a, *b),
        (edge_position::TAIL, [a, b, ..]) => (*a, *b),
        (_, pts) if pts.len() >= 2 => {
            let i = (pts.len() - 1) / 2;
            match (pts.get(i), pts.get(i + 1)) {
                (Some(a), Some(b)) => (Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0), *b),
                _ => return Point::default(),
            }
        }
        _ => return pts.first().copied().unwrap_or_default(),
    };
    let theta = (toward.y - anchor.y).atan2(toward.x - anchor.x) + alpha;
    Point::new(
        anchor.x + distance * theta.cos(),
        anchor.y + distance * theta.sin(),
    )
}

/// Draw an end marker at `tip`, with the line arriving from `from`.
fn draw_mark(scene: &mut Scene, tip: Point, from: Point, mark: EndMark, stroke: Color) {
    let angle = (from.y - tip.y).atan2(from.x - tip.x);
    let at = |len: f64, rot: f64| {
        Point::new(
            tip.x + len * (angle + rot).cos(),
            tip.y + len * (angle + rot).sin(),
        )
    };
    let wing = PI / 7.0;
    match mark {
        EndMark::None => {}
        EndMark::OpenArrow => scene.prims.push(Prim::Line {
            points: vec![at(12.0, wing), tip, at(12.0, -wing)],
            stroke,
            dashed: false,
        }),
        EndMark::HollowTriangle | EndMark::FilledTriangle => scene.prims.push(Prim::Polygon {
            points: vec![tip, at(14.0, wing), at(14.0, -wing)],
            fill: Some(if mark == EndMark::FilledTriangle {
                stroke
            } else {
                Color::WHITE
            }),
            stroke,
        }),
        EndMark::HollowDiamond | EndMark::FilledDiamond => scene.prims.push(Prim::Polygon {
            points: vec![tip, at(10.0, PI / 6.0), at(18.0, 0.0), at(10.0, -PI / 6.0)],
            fill: Some(if mark == EndMark::FilledDiamond {
                stroke
            } else {
                Color::WHITE
            }),
            stroke,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_follows_requested_end() {
        let pts = [Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
        let head = label_point(&pts, edge_position::HEAD, 0.0, 10.0);
        assert!((head.x - 90.0).abs() < 1e-9 && head.y.abs() < 1e-9);
        let mid = label_point(&pts, edge_position::MIDDLE, PI / 2.0, 15.0);
        assert!((mid.x - 50.0).abs() < 1e-9 && (mid.y - 15.0).abs() < 1e-9);
    }

    #[test]
    fn degenerate_points_do_not_panic() {
        assert_eq!(
            label_point(&[], edge_position::HEAD, 1.0, 5.0),
            Point::default()
        );
        let one = [Point::new(3.0, 4.0)];
        assert_eq!(
            label_point(&one, edge_position::MIDDLE, 1.0, 5.0),
            Point::new(3.0, 4.0)
        );
    }
}
