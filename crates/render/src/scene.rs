use std::collections::HashMap;
use std::f64::consts::PI;

use mdj::values::{DEFAULT_FONT, Font, absent, edge_position, parse_points};
use mdj::{Document, Element, Id};

use crate::geom::{Color, Point, Rect};
use crate::layout::{
    ACTIVE_INSET, Flow, HAlign, ICON_SPACE, Icon, MIND_ROUND, NODE_DEPTH, NOTE_FOLD, NodeLayout,
    ROUND, Shape, Symbol, TEXT_PAD_X, TEXT_PAD_Y, node_layout,
};
use crate::route;
use crate::text::Fonts;
use crate::timing;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Color,
    /// Reads bottom to top (turned a quarter left). The text's `rect` is then
    /// the turned box: one line wide, as tall as the text is long; the
    /// baseline is measured from its left edge.
    pub vertical: bool,
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
        dashed: bool,
    },
    /// A rectangle with rounded corners.
    RoundRect {
        rect: Rect,
        radius: f64,
        fill: Option<Color>,
        stroke: Color,
    },
    /// The ellipse inscribed in `rect`.
    Ellipse {
        rect: Rect,
        fill: Option<Color>,
        stroke: Color,
        dashed: bool,
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
    build_scene_at(doc, diagram, fonts, 0)
}

/// How deep diagrams drawn inside diagrams (inline interactions) may nest.
const MAX_NESTING: usize = 2;

fn build_scene_at(doc: &Document, diagram: &str, fonts: &Fonts, depth: usize) -> Scene {
    let mut scene = Scene::default();
    let Some(diagram) = doc.get(diagram) else {
        return scene;
    };
    let mut views: Vec<&Element> = diagram
        .children("ownedViews")
        .iter()
        .filter_map(|id| doc.get(id))
        .filter(|v| v.bool("visible", true))
        .collect();
    // Containers paint before what sits inside them (stable for the rest).
    views.sort_by_key(|v| container_depth(doc, v));

    let layouts: HashMap<&str, NodeLayout> = views
        .iter()
        .filter_map(|v| Some((v.id.as_str(), node_layout(doc, v, fonts)?)))
        .collect();
    // Timing frames arrange what they hold; those boxes go where they say.
    let arranged: Vec<timing::Item> = views
        .iter()
        .filter(|v| v.type_name == "UMLTimingFrameView")
        .filter_map(|frame| {
            let l = layouts.get(frame.id.as_str())?;
            let Shape::Frame { tag } = l.shape else {
                return None;
            };
            Some(timing::arrange(
                doc,
                frame,
                l.rect,
                tag.bottom(),
                l.font_size,
                fonts,
            ))
        })
        .flatten()
        .collect();
    let arranged_at: HashMap<&str, &timing::Item> =
        arranged.iter().map(|i| (i.view.as_str(), i)).collect();
    let mut rects: HashMap<&str, Rect> = layouts.iter().map(|(k, l)| (*k, l.rect)).collect();
    for item in &arranged {
        rects.insert(item.view.as_str(), item.rect);
    }
    // Sequence messages attach to lifelines' line parts (sub-views): give them rects.
    for v in &views {
        if let (Some(layout), Some(line)) = (layouts.get(v.id.as_str()), v.ref_id("linePart"))
            && let Some(r) = line_part_rect(layout)
        {
            rects.insert(line, r);
        }
    }

    let mut bounds: Option<Rect> = None;
    let mut grow = |r: Rect| bounds = Some(bounds.map_or(r, |b| b.union(&r)));
    // Edge geometry, for views that ride on an edge (drawn after the edges).
    let mut edge_pts: HashMap<&str, Vec<Point>> = HashMap::new();

    for view in views.iter().copied() {
        if timing::is_arranged(&view.type_name) {
            if let Some(item) = arranged_at.get(view.id.as_str()) {
                for text in draw_timing(&mut scene, view, item, fonts) {
                    grow(text);
                }
                scene.hits.push(Hit {
                    view: view.id.clone(),
                    shape: HitShape::Rect(item.rect),
                });
                grow(item.rect);
            }
            continue;
        }
        if let Some(layout) = layouts.get(view.id.as_str()) {
            draw_node(&mut scene, layout, fonts);
            // An inline interaction shows its sequence diagram, shrunk to fit.
            if view.type_name == "UMLInteractionInlineView"
                && depth < MAX_NESTING
                && let Some(target) = view
                    .ref_id("model")
                    .and_then(|m| doc.get(m))
                    .and_then(|action| action.ref_id("target"))
            {
                draw_nested(&mut scene, doc, target, layout.rect, fonts, depth + 1);
            }
            // A frame is picked by its tag, so clicks inside reach what it
            // frames; an interaction-overview box frames nothing clickable.
            let overview_box = matches!(
                view.type_name.as_str(),
                "UMLInteractionUseView" | "UMLInteractionInlineView"
            );
            let hit = match layout.shape {
                Shape::Frame { tag } if !overview_box => tag,
                _ => layout.rect,
            };
            scene.hits.push(Hit {
                view: view.id.clone(),
                shape: HitShape::Rect(hit),
            });
            grow(layout.rect);
        } else if let Some(points) = if view.type_name == "UMLTimingMessageView" {
            timing_message_points(doc, view, &rects)
        } else {
            edge_points(view, &rects).map(|pts| clip_to_outlines(view, pts, &layouts))
        } {
            let label_boxes = draw_edge(&mut scene, doc, view, &points, fonts);
            for r in label_boxes {
                grow(r);
            }
            for p in &points {
                grow(Rect::new(p.x, p.y, 0.0, 0.0));
            }
            edge_pts.insert(view.id.as_str(), points.clone());
            scene.hits.push(Hit {
                view: view.id.clone(),
                shape: HitShape::Polyline(points),
            });
        }
    }
    for view in views {
        if view.type_name != "UMLCommMessageView" {
            continue;
        }
        let Some(host) = view.ref_id("hostEdge").and_then(|h| edge_pts.get(h)) else {
            continue;
        };
        if let Some((arrow, boxes)) = draw_comm_message(&mut scene, doc, view, host, fonts) {
            for r in boxes {
                grow(r);
            }
            grow(arrow);
            scene.hits.push(Hit {
                view: view.id.clone(),
                shape: HitShape::Rect(arrow.expand(6.0)),
            });
        }
    }
    scene.bounds = bounds.unwrap_or_default();
    scene
}

/// Draw `diagram` scaled into `frame` below its tag (as StarUML: 10 px in
/// from the sides, 30 from the top, never enlarged). Not clickable.
fn draw_nested(
    scene: &mut Scene,
    doc: &Document,
    diagram: &str,
    frame: Rect,
    fonts: &Fonts,
    depth: usize,
) {
    let inner = build_scene_at(doc, diagram, fonts, depth);
    let b = inner.bounds;
    let area = Rect::new(
        frame.x + 10.0,
        frame.y + 30.0,
        frame.w - 20.0,
        frame.h - 40.0,
    );
    if inner.prims.is_empty() || b.w <= 0.0 || b.h <= 0.0 || area.w <= 0.0 || area.h <= 0.0 {
        return;
    }
    let k = (area.w / b.w).min(area.h / b.h).min(1.0);
    let (dx, dy) = (area.x - b.x * k, area.y - b.y * k);
    let pt = |p: Point| Point::new(p.x * k + dx, p.y * k + dy);
    let rect = |r: Rect| Rect::new(r.x * k + dx, r.y * k + dy, r.w * k, r.h * k);
    for prim in inner.prims {
        scene.prims.push(match prim {
            Prim::Rect {
                rect: r,
                fill,
                stroke,
                dashed,
            } => Prim::Rect {
                rect: rect(r),
                fill,
                stroke,
                dashed,
            },
            Prim::RoundRect {
                rect: r,
                radius,
                fill,
                stroke,
            } => Prim::RoundRect {
                rect: rect(r),
                radius: radius * k,
                fill,
                stroke,
            },
            Prim::Ellipse {
                rect: r,
                fill,
                stroke,
                dashed,
            } => Prim::Ellipse {
                rect: rect(r),
                fill,
                stroke,
                dashed,
            },
            Prim::Line {
                points,
                stroke,
                dashed,
            } => Prim::Line {
                points: points.into_iter().map(pt).collect(),
                stroke,
                dashed,
            },
            Prim::Polygon {
                points,
                fill,
                stroke,
            } => Prim::Polygon {
                points: points.into_iter().map(pt).collect(),
                fill,
                stroke,
            },
            Prim::Text {
                rect: r,
                baseline,
                text,
                style,
            } => Prim::Text {
                rect: rect(r),
                baseline: baseline * k,
                text,
                style: TextStyle {
                    size: style.size * k,
                    ..style
                },
            },
        });
    }
}

/// Draw a box its timing frame arranged: a lifeline's band and turned name,
/// a state's name, a segment's line (stepping from the previous one), a
/// tick and its text. Returns the boxes of the texts it placed.
fn draw_timing(scene: &mut Scene, view: &Element, item: &timing::Item, fonts: &Fonts) -> Vec<Rect> {
    let stroke = view
        .str("lineColor")
        .and_then(Color::parse)
        .unwrap_or(Color::BLACK);
    let color = view
        .str("fontColor")
        .and_then(Color::parse)
        .unwrap_or(Color::BLACK);
    let font = Font::parse(view.str("font").unwrap_or(DEFAULT_FONT));
    let size = if font.size > 0.0 && font.size.is_finite() {
        font.size
    } else {
        13.0
    };
    let line_h = fonts.line_height(size);
    let ascent = fonts.ascent(size);
    let line = |scene: &mut Scene, a: Point, b: Point| {
        scene.prims.push(Prim::Line {
            points: vec![a, b],
            stroke,
            dashed: false,
        });
    };
    let mut texts = Vec::new();
    let mut text = |scene: &mut Scene, rect: Rect, text: &str, bold: bool, vertical: bool| {
        texts.push(rect);
        scene.prims.push(Prim::Text {
            rect,
            baseline: ascent,
            text: text.to_owned(),
            style: TextStyle {
                size,
                bold,
                italic: false,
                underline: false,
                color,
                vertical,
            },
        });
    };
    let r = item.rect;
    match &item.kind {
        timing::Kind::Lifeline { name, name_rect } => {
            line(
                scene,
                Point::new(r.x, r.bottom()),
                Point::new(r.right(), r.bottom()),
            );
            text(scene, *name_rect, name, true, true);
        }
        timing::Kind::State { name, label } => {
            let w = fonts.width(name, size, false, false);
            let at = Rect::new(
                label.right() - w,
                label.y + (label.h - line_h) / 2.0,
                w,
                line_h,
            );
            text(scene, at, name, false, false);
        }
        timing::Kind::Segment { prev_mid } => {
            let mid = r.y + (r.h / 2.0).floor();
            line(
                scene,
                Point::new(r.x, mid),
                Point::new(r.right() + 1.0, mid),
            );
            if let Some(prev) = prev_mid {
                line(scene, Point::new(r.x, mid), Point::new(r.x, *prev));
            }
        }
        timing::Kind::Tick { text: label } => {
            line(scene, Point::new(r.x, r.y), Point::new(r.x, r.bottom()));
            if !label.is_empty() {
                let w = fonts.width(label, size, false, false);
                text(
                    scene,
                    Rect::new(r.x - w / 2.0, r.bottom() + 2.0, w, line_h),
                    label,
                    false,
                    false,
                );
            }
        }
    }
    texts
}

/// A timing message runs from its tail segment's middle height to its head
/// segment's, at whichever end of each segment its stored points are nearer
/// (judged against the segment's stored box, since the arranged one may be
/// elsewhere); by default from the tail's end to the head's start.
fn timing_message_points(
    doc: &Document,
    view: &Element,
    rects: &HashMap<&str, Rect>,
) -> Option<Vec<Point>> {
    let (tail_id, head_id) = (view.ref_id("tail")?, view.ref_id("head")?);
    let (tail, head) = (rects.get(tail_id)?, rects.get(head_id)?);
    let stored = parse_points(view.str("points").unwrap_or(""));
    let at_end = |id: &str, x: Option<f64>, default_end: bool| {
        let (Some(x), Some(v)) = (x, doc.get(id)) else {
            return default_end;
        };
        let left = v.num("left", 0.0);
        (x - left).abs() > (x - left - v.num("width", 0.0)).abs()
    };
    let x = |r: &Rect, end: bool| if end { r.right() } else { r.x };
    let mid = |r: &Rect| (r.y + r.h / 2.0).round();
    Some(vec![
        Point::new(
            x(tail, at_end(tail_id, stored.first().map(|p| p.0), true)),
            mid(tail),
        ),
        Point::new(
            x(head, at_end(head_id, stored.last().map(|p| p.0), false)),
            mid(head),
        ),
    ])
}

/// How many boxes `view` sits inside (`containerView` chain), capped.
fn container_depth(doc: &Document, view: &Element) -> usize {
    let mut depth = 0;
    let mut cur = view;
    while let Some(next) = cur.ref_id("containerView").and_then(|c| doc.get(c)) {
        depth += 1;
        if depth >= 32 {
            break; // a cycle in a hand-edited file
        }
        cur = next;
    }
    depth
}

fn draw_node(scene: &mut Scene, l: &NodeLayout, fonts: &Fonts) {
    let style = |line: &crate::layout::TextLine| TextStyle {
        size: l.font_size,
        bold: line.bold,
        italic: line.italic,
        underline: line.underline,
        color: l.text,
        vertical: false,
    };
    let (r, fill, stroke, dashed) = (l.rect, Some(l.fill), l.line, l.dashed);
    let rect = |scene: &mut Scene, rect: Rect| {
        scene.prims.push(Prim::Rect {
            rect,
            fill,
            stroke,
            dashed,
        });
    };
    let polygon = |scene: &mut Scene, points: Vec<Point>| {
        scene.prims.push(Prim::Polygon {
            points,
            fill,
            stroke,
        });
    };
    let line = |scene: &mut Scene, points: Vec<Point>| {
        scene.prims.push(Prim::Line {
            points,
            stroke,
            dashed: false,
        });
    };
    match l.shape {
        Shape::Box | Shape::Port => rect(scene, r),
        Shape::Package { tab } => {
            rect(scene, tab);
            rect(scene, Rect::new(r.x, tab.bottom(), r.w, r.h - tab.h));
        }
        Shape::Node3D => {
            let d = NODE_DEPTH;
            // Top and right faces, then the front face over them.
            polygon(
                scene,
                vec![
                    Point::new(r.x, r.y + d),
                    Point::new(r.x + d, r.y),
                    Point::new(r.right(), r.y),
                    Point::new(r.right() - d, r.y + d),
                ],
            );
            polygon(
                scene,
                vec![
                    Point::new(r.right() - d, r.y + d),
                    Point::new(r.right(), r.y),
                    Point::new(r.right(), r.bottom() - d),
                    Point::new(r.right() - d, r.bottom()),
                ],
            );
            rect(scene, Rect::new(r.x, r.y + d, r.w - d, r.h - d));
        }
        Shape::Ellipse => scene.prims.push(Prim::Ellipse {
            rect: r,
            fill,
            stroke,
            dashed,
        }),
        Shape::Note => {
            let f = NOTE_FOLD;
            polygon(
                scene,
                vec![
                    Point::new(r.x, r.y),
                    Point::new(r.right() - f, r.y),
                    Point::new(r.right(), r.y + f),
                    Point::new(r.right(), r.bottom()),
                    Point::new(r.x, r.bottom()),
                ],
            );
            line(
                scene,
                vec![
                    Point::new(r.right() - f, r.y),
                    Point::new(r.right() - f, r.y + f),
                    Point::new(r.right(), r.y + f),
                ],
            );
        }
        Shape::Text => {}
        Shape::RoundRect => scene.prims.push(Prim::RoundRect {
            rect: r,
            radius: ROUND,
            fill,
            stroke,
        }),
        Shape::MindNode => scene.prims.push(Prim::RoundRect {
            rect: r,
            radius: MIND_ROUND,
            fill,
            stroke,
        }),
        Shape::Actor { figure } => draw_actor(scene, figure, stroke),
        Shape::Symbol(symbol) => draw_symbol(scene, symbol, r, stroke, l.fill, fonts),
        Shape::Swimlane { header, .. } => {
            rect(scene, r);
            rect(scene, header);
        }
        Shape::Lifeline { head } => {
            rect(scene, head);
            scene.prims.push(Prim::Line {
                points: vec![
                    Point::new(r.center().x, head.bottom()),
                    Point::new(r.center().x, r.bottom()),
                ],
                stroke,
                dashed: true,
            });
        }
        Shape::Frame { tag } => {
            scene.prims.push(Prim::Rect {
                rect: r,
                fill: None,
                stroke,
                dashed: false,
            });
            // The name tag: a pentagon with its bottom-right corner cut.
            let cut = 6.0;
            scene.prims.push(Prim::Polygon {
                points: vec![
                    Point::new(tag.x, tag.y),
                    Point::new(tag.right(), tag.y),
                    Point::new(tag.right(), tag.bottom() - cut),
                    Point::new(tag.right() - cut, tag.bottom()),
                    Point::new(tag.x, tag.bottom()),
                ],
                fill,
                stroke,
            });
        }
        Shape::Flowchart(flow) => draw_flowchart(scene, flow, r, fill, stroke),
        Shape::Lollipop { circle } => {
            scene.prims.push(Prim::Ellipse {
                rect: circle,
                fill,
                stroke,
                dashed: false,
            });
        }
    }
    if l.active {
        // UML active class: an extra vertical line inside each side.
        for x in [r.x + ACTIVE_INSET, r.right() - ACTIVE_INSET] {
            line(scene, vec![Point::new(x, r.y), Point::new(x, r.bottom())]);
        }
    }
    for (from, to, dashed) in &l.dividers {
        scene.prims.push(Prim::Line {
            points: vec![*from, *to],
            stroke,
            dashed: *dashed,
        });
    }
    if let Some(icon) = l.icon {
        draw_icon(scene, icon, r, stroke, l.fill);
    }

    let line_h = fonts.line_height(l.font_size);
    let ascent = fonts.ascent(l.font_size);
    let pad_x = TEXT_PAD_X + if l.active { ACTIVE_INSET } else { 0.0 };
    for (i, c) in l.compartments.iter().enumerate() {
        if i > 0 && l.separators {
            line(
                scene,
                vec![
                    Point::new(c.rect.x, c.rect.y),
                    Point::new(c.rect.right(), c.rect.y),
                ],
            );
        }
        let mut y = c.rect.y + TEXT_PAD_Y;
        for text_line in &c.lines {
            let s = style(text_line);
            let w = fonts.width(&text_line.text, s.size, s.bold, s.italic);
            let x = match text_line.align {
                HAlign::Left => c.rect.x + pad_x,
                HAlign::Center => c.rect.x + (c.rect.w - w) / 2.0,
                HAlign::Right => c.rect.right() - pad_x - w,
            };
            scene.prims.push(Prim::Text {
                rect: Rect::new(x, y, w, line_h),
                baseline: ascent,
                text: text_line.text.clone(),
                style: s,
            });
            y += line_h;
        }
    }
    for label in &l.outside {
        let s = style(&label.line);
        let w = fonts.width(&label.line.text, s.size, s.bold, s.italic);
        scene.prims.push(Prim::Text {
            rect: Rect::new(
                label.center.x - w / 2.0,
                label.center.y - line_h / 2.0,
                w,
                line_h,
            ),
            baseline: ascent,
            text: label.line.text.clone(),
            style: s,
        });
    }
}

/// Points along a cubic Bézier curve from `a` (excluded) to `d`.
fn cubic(a: Point, b: Point, c: Point, d: Point) -> Vec<Point> {
    (1..=16)
        .map(|i| {
            let t = f64::from(i) / 16.0;
            let u = 1.0 - t;
            let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            Point::new(
                k0 * a.x + k1 * b.x + k2 * c.x + k3 * d.x,
                k0 * a.y + k1 * b.y + k2 * c.y + k3 * d.y,
            )
        })
        .collect()
}

/// Points along a quadratic Bézier curve from `a` (excluded) to `c`.
fn quadratic(a: Point, b: Point, c: Point) -> Vec<Point> {
    // The same curve as a cubic with control points two thirds of the way.
    let third = |p: Point, q: Point| {
        Point::new(p.x + (q.x - p.x) * 2.0 / 3.0, p.y + (q.y - p.y) * 2.0 / 3.0)
    };
    cubic(a, third(a, b), third(c, b), c)
}

/// Draw a flowchart symbol filling `r` (shapes after ISO 5807, sized as
/// StarUML draws them).
fn draw_flowchart(scene: &mut Scene, flow: Flow, r: Rect, fill: Option<Color>, stroke: Color) {
    let (x1, y1, x2, y2) = (r.x, r.y, r.right(), r.bottom());
    let (xm, ym) = (r.center().x, r.center().y);
    let p = Point::new;
    let polygon = |scene: &mut Scene, points: Vec<Point>| {
        scene.prims.push(Prim::Polygon {
            points,
            fill,
            stroke,
        });
    };
    let line = |scene: &mut Scene, points: Vec<Point>| {
        scene.prims.push(Prim::Line {
            points,
            stroke,
            dashed: false,
        });
    };
    let ellipse = |scene: &mut Scene, rect: Rect| {
        scene.prims.push(Prim::Ellipse {
            rect,
            fill,
            stroke,
            dashed: false,
        });
    };
    let boxed = |scene: &mut Scene, radius: f64| {
        scene.prims.push(if radius > 0.0 {
            Prim::RoundRect {
                rect: r,
                radius,
                fill,
                stroke,
            }
        } else {
            Prim::Rect {
                rect: r,
                fill,
                stroke,
                dashed: false,
            }
        });
    };
    // A page with a wavy bottom edge.
    let document = |r: Rect| {
        let h = (r.h / 6.0).floor();
        let g = (r.w / 3.0).floor();
        let mut pts = vec![p(r.x, r.y), p(r.right(), r.y), p(r.right(), r.bottom())];
        pts.extend(cubic(
            p(r.right(), r.bottom()),
            p(r.right() - g, r.bottom() - 2.0 * h),
            p(r.x + g, r.bottom() + 1.5 * h),
            p(r.x, r.bottom() - h),
        ));
        pts
    };
    // Kappa: control-point distance for a quarter ellipse.
    const K: f64 = 0.552_284_8;
    use Flow::*;
    match flow {
        Process => boxed(scene, 0.0),
        Terminator => boxed(scene, r.w.min(r.h) / 2.0),
        AlternateProcess => boxed(scene, 10.0),
        Decision => polygon(scene, vec![p(x1, ym), p(xm, y1), p(x2, ym), p(xm, y2)]),
        Delay => {
            let rad = r.h / 2.0;
            let cx = x2 - rad;
            let mut pts = vec![p(x1, y1), p(cx, y1)];
            pts.extend((1..=16).map(|i| {
                let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * f64::from(i) / 16.0;
                p(cx + rad * a.cos(), ym + rad * a.sin())
            }));
            pts.push(p(x1, y2));
            polygon(scene, pts);
        }
        PredefinedProcess => {
            boxed(scene, 0.0);
            for x in [x1 + 5.0, x2 - 5.0] {
                line(scene, vec![p(x, y1), p(x, y2)]);
            }
        }
        InternalStorage => {
            boxed(scene, 0.0);
            line(scene, vec![p(x1 + 10.0, y1), p(x1 + 10.0, y2)]);
            line(scene, vec![p(x1, y1 + 10.0), p(x2, y1 + 10.0)]);
        }
        Data => {
            let g = (r.w / 6.0).floor();
            polygon(
                scene,
                vec![p(x1 + g, y1), p(x2, y1), p(x2 - g, y2), p(x1, y2)],
            );
        }
        Document => polygon(scene, document(r)),
        MultiDocument => {
            // Three pages, the front one bottom-right.
            for k in 0..3 {
                let o = 5.0 * f64::from(k);
                polygon(
                    scene,
                    document(Rect::new(x1 + o, y1 + o, r.w - 10.0, r.h - 10.0)),
                );
            }
        }
        Preparation => {
            let g = (r.w / 6.0).floor();
            polygon(
                scene,
                vec![
                    p(x1, ym),
                    p(x1 + g, y1),
                    p(x2 - g, y1),
                    p(x2, ym),
                    p(x2 - g, y2),
                    p(x1 + g, y2),
                ],
            );
        }
        Display => {
            let g = (r.w / 6.0).floor();
            let mut pts = vec![p(x1, ym), p(x1 + g, y1), p(x2 - g, y1)];
            pts.extend(quadratic(p(x2 - g, y1), p(x2, y1), p(x2, ym)));
            pts.extend(quadratic(p(x2, ym), p(x2, y2), p(x2 - g, y2)));
            pts.push(p(x1 + g, y2));
            polygon(scene, pts);
        }
        ManualInput => {
            let g = (r.h / 6.0).floor();
            polygon(scene, vec![p(x1, y1 + g), p(x2, y1), p(x2, y2), p(x1, y2)]);
        }
        ManualOperation => {
            let g = (r.w / 6.0).floor();
            polygon(
                scene,
                vec![p(x1, y1), p(x2, y1), p(x2 - g, y2), p(x1 + g, y2)],
            );
        }
        Card => {
            let g = (r.w / 8.0).floor();
            polygon(
                scene,
                vec![
                    p(x1 + g, y1),
                    p(x2, y1),
                    p(x2, y2),
                    p(x1, y2),
                    p(x1, y1 + g),
                ],
            );
        }
        PunchedTape => {
            let h = (r.h / 6.0).floor();
            let g = (r.w / 3.0).floor();
            let mut pts = vec![p(x1, y1)];
            pts.extend(cubic(
                p(x1, y1),
                p(x1 + g, y1 + 2.0 * h),
                p(x2 - g, y1 - 1.5 * h),
                p(x2, y1 + h),
            ));
            pts.push(p(x2, y2));
            pts.extend(cubic(
                p(x2, y2),
                p(x2 - g, y2 - 2.0 * h),
                p(x1 + g, y2 + 1.5 * h),
                p(x1, y2 - h),
            ));
            polygon(scene, pts);
        }
        Connector => ellipse(scene, r),
        OffPageConnector => {
            let g = (r.h / 4.0).floor();
            polygon(
                scene,
                vec![
                    p(x1, y1),
                    p(x2, y1),
                    p(x2, y2 - g),
                    p(xm, y2),
                    p(x1, y2 - g),
                ],
            );
        }
        Or => {
            ellipse(scene, r);
            line(scene, vec![p(xm, y1), p(xm, y2)]);
            line(scene, vec![p(x1, ym), p(x2, ym)]);
        }
        SummingJunction => {
            ellipse(scene, r);
            let d = (std::f64::consts::SQRT_2 * r.w / 4.0).round();
            line(scene, vec![p(xm - d, ym - d), p(xm + d, ym + d)]);
            line(scene, vec![p(xm + d, ym - d), p(xm - d, ym + d)]);
        }
        Collate => polygon(scene, vec![p(x1, y1), p(x2, y1), p(x1, y2), p(x2, y2)]),
        Sort => {
            polygon(scene, vec![p(x1, ym), p(xm, y1), p(x2, ym), p(xm, y2)]);
            line(scene, vec![p(x1, ym), p(x2, ym)]);
        }
        Merge => polygon(scene, vec![p(x1, y1), p(x2, y1), p(xm, y2)]),
        Extract => polygon(scene, vec![p(xm, y1), p(x2, y2), p(x1, y2)]),
        StoredData => {
            // Rounded on the left, hollowed on the right.
            let g = (r.w / 8.0).floor();
            let (ox, oy) = (g * K, r.h / 2.0 * K);
            let mut pts = vec![p(x2, y1)];
            pts.extend(cubic(
                p(x2, y1),
                p(x2 - ox, y1),
                p(x2 - g, ym - oy),
                p(x2 - g, ym),
            ));
            pts.extend(cubic(
                p(x2 - g, ym),
                p(x2 - g, ym + oy),
                p(x2 - ox, y2),
                p(x2, y2),
            ));
            pts.push(p(x1 + g, y2));
            pts.extend(cubic(
                p(x1 + g, y2),
                p(x1 + g - ox, y2),
                p(x1, ym + oy),
                p(x1, ym),
            ));
            pts.extend(cubic(
                p(x1, ym),
                p(x1, ym - oy),
                p(x1 + g - ox, y1),
                p(x1 + g, y1),
            ));
            polygon(scene, pts);
        }
        Database => {
            // A standing cylinder, with the front of its top rim.
            let g = (r.h / 8.0).floor();
            let (ox, oy) = (r.w / 2.0 * K, g * K);
            let mut pts = vec![p(x1, y1 + g)];
            pts.extend(cubic(
                p(x1, y1 + g),
                p(x1, y1 + g - oy),
                p(xm - ox, y1),
                p(xm, y1),
            ));
            pts.extend(cubic(
                p(xm, y1),
                p(xm + ox, y1),
                p(x2, y1 + g - oy),
                p(x2, y1 + g),
            ));
            pts.push(p(x2, y2 - g));
            pts.extend(cubic(
                p(x2, y2 - g),
                p(x2, y2 - g + oy),
                p(xm + ox, y2),
                p(xm, y2),
            ));
            pts.extend(cubic(
                p(xm, y2),
                p(xm - ox, y2),
                p(x1, y2 - g + oy),
                p(x1, y2 - g),
            ));
            polygon(scene, pts);
            let mut rim = vec![p(x1, y1 + g)];
            rim.extend(cubic(
                p(x1, y1 + g),
                p(x1, y1 + g + oy),
                p(xm - ox, y1 + 2.0 * g),
                p(xm, y1 + 2.0 * g),
            ));
            rim.extend(cubic(
                p(xm, y1 + 2.0 * g),
                p(xm + ox, y1 + 2.0 * g),
                p(x2, y1 + g + oy),
                p(x2, y1 + g),
            ));
            line(scene, rim);
        }
        DirectAccessStorage => {
            // A lying cylinder, with the front of its right end.
            let g = (r.w / 8.0).floor();
            let (ox, oy) = (g * K, r.h / 2.0 * K);
            let mut pts = vec![p(x2 - g, y1)];
            pts.extend(cubic(
                p(x2 - g, y1),
                p(x2 - g + ox, y1),
                p(x2, ym - oy),
                p(x2, ym),
            ));
            pts.extend(cubic(
                p(x2, ym),
                p(x2, ym + oy),
                p(x2 - g + ox, y2),
                p(x2 - g, y2),
            ));
            pts.push(p(x1 + g, y2));
            pts.extend(cubic(
                p(x1 + g, y2),
                p(x1 + g - ox, y2),
                p(x1, ym + oy),
                p(x1, ym),
            ));
            pts.extend(cubic(
                p(x1, ym),
                p(x1, ym - oy),
                p(x1 + g - ox, y1),
                p(x1 + g, y1),
            ));
            polygon(scene, pts);
            let mut end = vec![p(x2 - g, y1)];
            end.extend(cubic(
                p(x2 - g, y1),
                p(x2 - g - ox, y1),
                p(x2 - 2.0 * g, ym - oy),
                p(x2 - 2.0 * g, ym),
            ));
            end.extend(cubic(
                p(x2 - 2.0 * g, ym),
                p(x2 - 2.0 * g, ym + oy),
                p(x2 - g - ox, y2),
                p(x2 - g, y2),
            ));
            line(scene, end);
        }
    }
}

/// A stick figure filling `f` (UML actor).
fn draw_actor(scene: &mut Scene, f: Rect, stroke: Color) {
    let cx = f.center().x;
    let unit = (f.h / 5.0).min(f.w / 2.2);
    let head = Rect::new(cx - unit / 2.0, f.y + 2.0, unit, unit);
    let (neck, hip) = (head.bottom(), head.bottom() + unit * 1.6);
    let line = |scene: &mut Scene, a: Point, b: Point| {
        scene.prims.push(Prim::Line {
            points: vec![a, b],
            stroke,
            dashed: false,
        });
    };
    scene.prims.push(Prim::Ellipse {
        rect: head,
        fill: Some(Color::WHITE),
        stroke,
        dashed: false,
    });
    line(scene, Point::new(cx, neck), Point::new(cx, hip));
    let arms = neck + unit * 0.6;
    line(
        scene,
        Point::new(cx - unit, arms),
        Point::new(cx + unit, arms),
    );
    let foot = (hip + unit * 1.4).min(f.bottom() - 2.0);
    line(
        scene,
        Point::new(cx, hip),
        Point::new(cx - unit * 0.9, foot),
    );
    line(
        scene,
        Point::new(cx, hip),
        Point::new(cx + unit * 0.9, foot),
    );
}

/// The small state-machine and activity symbols, fitted to `r`.
fn draw_symbol(
    scene: &mut Scene,
    symbol: Symbol,
    r: Rect,
    stroke: Color,
    fill: Color,
    fonts: &Fonts,
) {
    // Round symbols use the largest circle that fits.
    let d = r.w.min(r.h);
    let circle = Rect::new(r.center().x - d / 2.0, r.center().y - d / 2.0, d, d);
    let ellipse = |scene: &mut Scene, rect: Rect, fill: Option<Color>| {
        scene.prims.push(Prim::Ellipse {
            rect,
            fill,
            stroke,
            dashed: false,
        });
    };
    let line = |scene: &mut Scene, a: Point, b: Point| {
        scene.prims.push(Prim::Line {
            points: vec![a, b],
            stroke,
            dashed: false,
        });
    };
    let cross = |scene: &mut Scene, c: Rect| {
        let k = c.w * 0.35;
        let m = c.center();
        line(
            scene,
            Point::new(m.x - k, m.y - k),
            Point::new(m.x + k, m.y + k),
        );
        line(
            scene,
            Point::new(m.x - k, m.y + k),
            Point::new(m.x + k, m.y - k),
        );
    };
    match symbol {
        Symbol::Dot | Symbol::SmallDot => ellipse(scene, circle, Some(stroke)),
        Symbol::BullsEye => {
            ellipse(scene, circle, Some(fill));
            ellipse(scene, circle.expand(-d * 0.22), Some(stroke));
        }
        Symbol::CircleX => {
            ellipse(scene, circle, Some(fill));
            cross(scene, circle);
        }
        Symbol::Ring => ellipse(scene, circle, Some(fill)),
        Symbol::Cross => cross(scene, Rect::new(r.x, r.y, r.w.max(r.h), r.w.max(r.h))),
        Symbol::Bar => scene.prims.push(Prim::Rect {
            rect: r,
            fill: Some(stroke),
            stroke,
            dashed: false,
        }),
        Symbol::Diamond => {
            let c = r.center();
            scene.prims.push(Prim::Polygon {
                points: vec![
                    Point::new(c.x, r.y),
                    Point::new(r.right(), c.y),
                    Point::new(c.x, r.bottom()),
                    Point::new(r.x, c.y),
                ],
                fill: Some(fill),
                stroke,
            });
        }
        Symbol::History { deep } => {
            ellipse(scene, circle, Some(fill));
            let text = if deep { "H*" } else { "H" };
            let size = (d * 0.55).max(6.0);
            let w = fonts.width(text, size, true, false);
            let h = fonts.line_height(size);
            scene.prims.push(Prim::Text {
                rect: Rect::new(
                    circle.center().x - w / 2.0,
                    circle.center().y - h / 2.0,
                    w,
                    h,
                ),
                baseline: fonts.ascent(size),
                text: text.to_owned(),
                style: TextStyle {
                    size,
                    bold: true,
                    italic: false,
                    underline: false,
                    color: stroke,
                    vertical: false,
                },
            });
        }
    }
}

/// The small component or artifact symbol in a box's top-right corner.
fn draw_icon(scene: &mut Scene, icon: Icon, r: Rect, stroke: Color, fill: Color) {
    let (x, y) = (r.right() - ICON_SPACE + 4.0, r.y + 5.0);
    match icon {
        Icon::Component => {
            // A box with two tabs on its left side.
            scene.prims.push(Prim::Rect {
                rect: Rect::new(x + 3.0, y, 13.0, 15.0),
                fill: Some(fill),
                stroke,
                dashed: false,
            });
            for ty in [y + 3.0, y + 9.0] {
                scene.prims.push(Prim::Rect {
                    rect: Rect::new(x, ty, 7.0, 3.5),
                    fill: Some(fill),
                    stroke,
                    dashed: false,
                });
            }
        }
        Icon::Artifact => {
            // A page with a folded corner.
            let (w, h, f) = (12.0, 15.0, 4.0);
            scene.prims.push(Prim::Polygon {
                points: vec![
                    Point::new(x + 2.0, y),
                    Point::new(x + 2.0 + w - f, y),
                    Point::new(x + 2.0 + w, y + f),
                    Point::new(x + 2.0 + w, y + h),
                    Point::new(x + 2.0, y + h),
                ],
                fill: Some(fill),
                stroke,
            });
            scene.prims.push(Prim::Line {
                points: vec![
                    Point::new(x + 2.0 + w - f, y),
                    Point::new(x + 2.0 + w - f, y + f),
                    Point::new(x + 2.0 + w, y + f),
                ],
                stroke,
                dashed: false,
            });
        }
    }
}

/// The dashed line of a lifeline: from just below the head to the bottom, at the centre.
pub(crate) fn line_part_rect(layout: &NodeLayout) -> Option<Rect> {
    let Shape::Lifeline { head } = layout.shape else {
        return None;
    };
    let r = layout.rect;
    Some(Rect::new(
        r.center().x,
        head.bottom() + 1.0,
        0.0,
        (r.bottom() - head.bottom() - 1.0).max(0.0),
    ))
}

/// The outline a line should end on, when it isn't the box itself.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Outline {
    Diamond,
    Ellipse,
}

fn outline_of(shape: Shape) -> Option<Outline> {
    match shape {
        Shape::Flowchart(Flow::Decision | Flow::Sort) | Shape::Symbol(Symbol::Diamond) => {
            Some(Outline::Diamond)
        }
        Shape::Flowchart(Flow::Connector | Flow::Or | Flow::SummingJunction)
        | Shape::Ellipse
        | Shape::Symbol(
            Symbol::Dot
            | Symbol::BullsEye
            | Symbol::CircleX
            | Symbol::Ring
            | Symbol::SmallDot
            | Symbol::History { .. },
        ) => Some(Outline::Ellipse),
        _ => None,
    }
}

/// Move `p` (on or outside box `r`) in toward `r`'s centre until it lies on
/// the outline; a point already inside the outline stays.
fn clip_point(p: Point, r: Rect, outline: Outline) -> Point {
    let c = r.center();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    let (a, b) = (r.w / 2.0, r.h / 2.0);
    if a <= 0.0 || b <= 0.0 {
        return p;
    }
    let k = match outline {
        Outline::Diamond => dx.abs() / a + dy.abs() / b,
        Outline::Ellipse => ((dx / a).powi(2) + (dy / b).powi(2)).sqrt(),
    };
    if k <= 1.0 {
        return p;
    }
    Point::new(c.x + dx / k, c.y + dy / k)
}

/// End a line on its boxes' outlines where they are diamonds or ovals, not
/// on the boxes around them.
fn clip_to_outlines(
    view: &Element,
    mut pts: Vec<Point>,
    layouts: &HashMap<&str, NodeLayout>,
) -> Vec<Point> {
    let outline = |key: &str| {
        let l = layouts.get(view.ref_id(key)?)?;
        Some((l.rect, outline_of(l.shape)?))
    };
    if let (Some((r, o)), Some(first)) = (outline("tail"), pts.first_mut()) {
        *first = clip_point(*first, r, o);
    }
    if let (Some((r, o)), Some(last)) = (outline("head"), pts.last_mut()) {
        *last = clip_point(*last, r, o);
    }
    pts
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
    let style = view.num("lineStyle", absent::LINE_STYLE as f64) as i64;
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
    /// ⊕ at the container end of a containment line.
    CirclePlus,
    /// Open arrow and an ✕ (a delete message destroys its target).
    Destroy,
    /// Crow's-foot ends of ERD relationships, by cardinality: `1`, `1..*`,
    /// `0..1`, `0..*`.
    CrowOne,
    CrowMany,
    CrowZeroOne,
    CrowZeroMany,
}

/// The crow's-foot end for an ERD relationship end's cardinality (unknown
/// values draw as `1`, as in StarUML).
fn crow_mark(end: Option<&Element>) -> EndMark {
    match end.and_then(|e| e.str("cardinality")).unwrap_or("1") {
        "0..1" => EndMark::CrowZeroOne,
        "0..*" => EndMark::CrowZeroMany,
        "1..*" => EndMark::CrowMany,
        _ => EndMark::CrowOne,
    }
}

/// Line style and end markers for an edge view. Associations look at their
/// ends; other relationships are recognised by view type name so that the
/// many UML edge kinds get sensible notation.
fn edge_style(doc: &Document, view: &Element) -> (bool, EndMark, EndMark) {
    let t = view.type_name.as_str();
    let model = view.ref_id("model").and_then(|m| doc.get(m));
    if t == "UMLSeqMessageView" || t == "UMLTimingMessageView" {
        let (dashed, head) = message_style(model);
        return (dashed, EndMark::None, head);
    }
    if let Some(m) = model.filter(|_| t == "ERDRelationshipView") {
        let end = |k: &str| m.children(k).first().and_then(|id| doc.get(id));
        // Non-identifying relationships are dotted.
        return (
            !m.bool("identifying", true),
            crow_mark(end("end1")),
            crow_mark(end("end2")),
        );
    }
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
    let (dashed, head) = if t == "FCFlowView" {
        (false, EndMark::FilledTriangle)
    } else if t == "UMLInformationFlowView" {
        (true, EndMark::OpenArrow)
    } else if t.contains("Generalization") {
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
    } else if t.contains("Transition") || t.ends_with("FlowView") {
        (false, EndMark::OpenArrow)
    } else if t.contains("Containment") {
        (false, EndMark::CirclePlus)
    } else if t.contains("NoteLink") || t.contains("ConstraintLink") {
        (true, EndMark::None)
    } else {
        (false, EndMark::None)
    };
    (dashed, EndMark::None, head)
}

/// UML message notation by `messageSort`: dashed line, and the arrowhead.
fn message_style(message: Option<&Element>) -> (bool, EndMark) {
    match message
        .and_then(|m| m.str("messageSort"))
        .unwrap_or("synchCall")
    {
        "synchCall" => (false, EndMark::FilledTriangle),
        "reply" | "createMessage" => (true, EndMark::OpenArrow),
        "deleteMessage" => (false, EndMark::Destroy),
        _ => (false, EndMark::OpenArrow),
    }
}

/// Half the length of a communication message's arrow (StarUML: 40 px).
const COMM_ARROW_HALF: f64 = 20.0;

/// A communication message: an arrow beside the middle of its connector,
/// pointing the way the message goes, and its label `n : name` beyond it
/// (`n` its place among the interaction's messages, when the diagram shows
/// sequence numbers). Returns the arrow's box and the labels' boxes.
fn draw_comm_message(
    scene: &mut Scene,
    doc: &Document,
    view: &Element,
    host: &[Point],
    fonts: &Fonts,
) -> Option<(Rect, Vec<Rect>)> {
    if host.len() < 2 {
        return None;
    }
    let model = view.ref_id("model").and_then(|m| doc.get(m));
    let i = (host.len() - 1) / 2;
    let (a, b) = (host[i], host[i + 1]);
    let len = (b.x - a.x).hypot(b.y - a.y);
    let (ux, uy) = if len > 1e-9 {
        ((b.x - a.x) / len, (b.y - a.y) / len)
    } else {
        (1.0, 0.0)
    };
    let centre = label_point(
        host,
        edge_position::MIDDLE,
        view.num("alpha", PI / 2.0),
        view.num("distance", 10.0),
    );
    let back = Point::new(
        centre.x - ux * COMM_ARROW_HALF,
        centre.y - uy * COMM_ARROW_HALF,
    );
    let ahead = Point::new(
        centre.x + ux * COMM_ARROW_HALF,
        centre.y + uy * COMM_ARROW_HALF,
    );
    // The connector runs from its tail lifeline to its head; a message whose
    // source isn't the tail's lifeline points back.
    let host_tail_model = view
        .ref_id("hostEdge")
        .and_then(|h| doc.get(h))
        .and_then(|h| h.ref_id("tail"))
        .and_then(|t| doc.get(t))
        .and_then(|t| t.ref_id("model"));
    let reverse = matches!(
        (model.and_then(|m| m.ref_id("source")), host_tail_model),
        (Some(s), Some(t)) if s != t
    );
    let (from, to) = if reverse {
        (ahead, back)
    } else {
        (back, ahead)
    };
    let stroke = view
        .str("lineColor")
        .and_then(Color::parse)
        .unwrap_or(Color::BLACK);
    let (dashed, head) = message_style(model);
    scene.prims.push(Prim::Line {
        points: vec![from, to],
        stroke,
        dashed,
    });
    draw_mark(scene, to, from, head, stroke);

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
    let name = model
        .map(|m| comm_message_text(doc, view, m))
        .unwrap_or_default();
    let stereotype = model
        .and_then(|m| match m.str("messageSort") {
            Some("createMessage") => Some("«create»".to_owned()),
            Some("deleteMessage") => Some("«destroy»".to_owned()),
            _ => uml::stereotype_text(doc, m),
        })
        .unwrap_or_default();
    let line_h = fonts.line_height(size);
    let ascent = fonts.ascent(size);
    let mut boxes = Vec::new();
    for (key, text, default_distance) in [
        ("nameLabel", name, 10.0),
        ("stereotypeLabel", stereotype, 25.0),
    ] {
        if text.is_empty() {
            continue;
        }
        let label = view.ref_id(key).and_then(|id| doc.get(id));
        let alpha = label.map_or(PI / 2.0, |l| l.num("alpha", PI / 2.0));
        let distance = label.map_or(default_distance, |l| l.num("distance", default_distance));
        // Measured along the connector's direction, so a reverse message's
        // label stays on the same side; then pushed out by the text's half
        // extent in that direction, so it clears the arrow.
        let w = fonts.width(&text, size, false, false);
        let near = label_point(&[back, ahead], edge_position::MIDDLE, alpha, distance);
        let (ox, oy) = (near.x - centre.x, near.y - centre.y);
        let off = ox.hypot(oy);
        let anchor = if off > 1e-9 {
            let (dx, dy) = (ox / off, oy / off);
            let clear = dx.abs() * w / 2.0 + dy.abs() * line_h / 2.0;
            Point::new(near.x + dx * clear, near.y + dy * clear)
        } else {
            near
        };
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
                vertical: false,
            },
        });
        boxes.push(rect);
    }
    let arrow = Rect::new(
        from.x.min(to.x),
        from.y.min(to.y),
        (from.x - to.x).abs(),
        (from.y - to.y).abs(),
    );
    Some((arrow, boxes))
}

/// A communication message's label: `n : name`, `n` its place among the
/// interaction's messages, or its own `sequenceNumber` when the diagram
/// numbers by hand; no number when the diagram hides them.
fn comm_message_text(doc: &Document, view: &Element, message: &Element) -> String {
    let diagram = view.parent.as_deref().and_then(|d| doc.get(d));
    let shown = diagram.is_none_or(|d| d.bool("showSequenceNumber", true));
    let custom = diagram.is_some_and(|d| d.str("sequenceNumbering") == Some("custom"));
    let number = match (shown, custom) {
        (false, _) => None,
        (true, true) => message.str("sequenceNumber").map(str::to_owned),
        (true, false) => message
            .parent
            .as_deref()
            .and_then(|p| doc.get(p))
            .and_then(|p| p.children("messages").iter().position(|m| *m == message.id))
            .map(|i| (i + 1).to_string()),
    };
    uml::numbered_message_text(message, number.as_deref())
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
    // A message's activation bar sits under the arrow, unless the diagram
    // hides activations (`showActivation`, on by default).
    let activations_shown = view
        .parent
        .as_deref()
        .and_then(|d| doc.get(d))
        .is_none_or(|d| d.bool("showActivation", true));
    if let Some(act) = view
        .ref_id("activation")
        .and_then(|a| doc.get(a))
        .filter(|_| activations_shown)
    {
        let fill = view
            .str("fillColor")
            .and_then(Color::parse)
            .unwrap_or(Color::WHITE);
        scene.prims.push(Prim::Rect {
            rect: Rect::new(
                act.num("left", 0.0),
                act.num("top", 0.0),
                act.num("width", 14.0),
                act.num("height", 25.0),
            ),
            fill: Some(fill),
            stroke,
            dashed: false,
        });
    }
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

    // ERD relationships name their ends with `tail/headNameLabel`.
    let (tail_name, head_name) = if view.type_name == "ERDRelationshipView" {
        ("tailNameLabel", "headNameLabel")
    } else {
        ("tailRoleNameLabel", "headRoleNameLabel")
    };
    let texts: Vec<(&str, String)> = vec![
        (
            "nameLabel",
            model
                .map(|m| match m.type_name.as_str() {
                    "UMLTransition" => uml::transition_text(doc, m),
                    "UMLControlFlow" | "UMLObjectFlow" => uml::flow_text(m),
                    // StarUML labels timing messages without parentheses or numbers.
                    "UMLMessage" if view.type_name == "UMLTimingMessageView" => {
                        uml::numbered_message_text(m, None)
                    }
                    "UMLMessage" => uml::message_text(m),
                    // An information flow names what it conveys, if anything.
                    "UMLInformationFlow" => {
                        let conveyed: Vec<&str> = m
                            .get("conveyed")
                            .map(|f| f.refs())
                            .unwrap_or_default()
                            .iter()
                            .filter_map(|id| doc.get(id))
                            .map(Element::name)
                            .collect();
                        if conveyed.is_empty() {
                            m.name().to_owned()
                        } else {
                            conveyed.join(", ")
                        }
                    }
                    _ => m.name().to_owned(),
                })
                .unwrap_or_default(),
        ),
        (
            "stereotypeLabel",
            model
                .and_then(|m| uml::stereotype_text(doc, m))
                .or_else(|| edge_keyword(&view.type_name).map(str::to_owned))
                .unwrap_or_default(),
        ),
        (
            tail_name,
            end("end1").map(|e| e.name().to_owned()).unwrap_or_default(),
        ),
        (
            head_name,
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
                l.num("edgePosition", absent::EDGE_POSITION as f64) as i64,
                l.num("alpha", absent::ALPHA),
                l.num("distance", absent::DISTANCE),
            ),
            None => default_label_placement(key),
        };
        let w = fonts.width(&text, size, false, false);
        let anchor = match (view.type_name.as_str(), key, pts) {
            // A message's name sits above its arrow whichever way it points;
            // a self message's beside its loop.
            ("UMLSeqMessageView", "nameLabel", [a, b, ..]) if pts.len() > 2 => {
                Point::new(b.x + 6.0 + w / 2.0, (a.y + pts[pts.len() - 1].y) / 2.0)
            }
            ("UMLSeqMessageView", "nameLabel", [a, b]) => {
                Point::new((a.x + b.x) / 2.0, a.y.min(b.y) - line_h / 2.0 - 3.0)
            }
            _ => label_point(pts, pos, alpha, distance),
        };
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
                vertical: false,
            },
        });
        boxes.push(rect);
    }
    boxes
}

/// Keyword UML shows on some relationships when they have no stereotype.
fn edge_keyword(view_type: &str) -> Option<&'static str> {
    match view_type {
        "UMLDeploymentView" => Some("«deploy»"),
        "UMLIncludeView" => Some("«include»"),
        "UMLExtendView" => Some("«extend»"),
        "UMLInformationFlowView" => Some("«flow»"),
        _ => None,
    }
}

fn default_label_placement(key: &str) -> (i64, f64, f64) {
    match key {
        "stereotypeLabel" => (edge_position::MIDDLE, PI / 2.0, 30.0),
        "tailRoleNameLabel" | "tailNameLabel" => (edge_position::TAIL, PI / 6.0, 30.0),
        "tailMultiplicityLabel" => (edge_position::TAIL, -PI / 6.0, 25.0),
        "headRoleNameLabel" | "headNameLabel" => (edge_position::HEAD, -PI / 6.0, 30.0),
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
    // StarUML measures `alpha` the other way round from screen angles
    // (checked against StarUML 7.1.1 drawing the same file), so subtract it.
    let theta = (toward.y - anchor.y).atan2(toward.x - anchor.x) - alpha;
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
        EndMark::Destroy => {
            draw_mark(scene, tip, from, EndMark::OpenArrow, stroke);
            let k = 8.0;
            for (dx, dy) in [(k, k), (k, -k)] {
                scene.prims.push(Prim::Line {
                    points: vec![
                        Point::new(tip.x - dx, tip.y - dy),
                        Point::new(tip.x + dx, tip.y + dy),
                    ],
                    stroke,
                    dashed: false,
                });
            }
        }
        EndMark::CrowOne | EndMark::CrowMany | EndMark::CrowZeroOne | EndMark::CrowZeroMany => {
            draw_crow(scene, tip, angle, mark, stroke);
        }
        EndMark::CirclePlus => {
            const R: f64 = 7.0;
            let c = at(R, 0.0);
            scene.prims.push(Prim::Ellipse {
                rect: Rect::new(c.x - R, c.y - R, 2.0 * R, 2.0 * R),
                fill: Some(Color::WHITE),
                stroke,
                dashed: false,
            });
            for (dx, dy) in [(R, 0.0), (0.0, R)] {
                scene.prims.push(Prim::Line {
                    points: vec![
                        Point::new(c.x - dx, c.y - dy),
                        Point::new(c.x + dx, c.y + dy),
                    ],
                    stroke,
                    dashed: false,
                });
            }
        }
    }
}

/// Half the width of a crow's-foot bar or fan.
const CROW_HALF: f64 = 7.0;
/// Where a crow's foot's prongs meet, from the box.
const CROW_LEN: f64 = 16.0;
const CROW_CIRCLE: f64 = 5.0;

/// A crow's-foot end at `tip`, the line leaving at `angle`: bars for "one",
/// three prongs fanning out to the box for "many", a ring for "zero".
fn draw_crow(scene: &mut Scene, tip: Point, angle: f64, mark: EndMark, stroke: Color) {
    let (ux, uy) = (angle.cos(), angle.sin());
    // `along` from the tip toward the line, `side` across it.
    let at = |along: f64, side: f64| {
        Point::new(
            tip.x + along * ux - side * uy,
            tip.y + along * uy + side * ux,
        )
    };
    let mut line = |points: Vec<Point>| {
        scene.prims.push(Prim::Line {
            points,
            stroke,
            dashed: false,
        });
    };
    let bar = |along: f64| vec![at(along, CROW_HALF), at(along, -CROW_HALF)];
    let many = matches!(mark, EndMark::CrowMany | EndMark::CrowZeroMany);
    if many {
        line(vec![
            at(0.0, CROW_HALF),
            at(CROW_LEN, 0.0),
            at(0.0, -CROW_HALF),
        ]);
    }
    match mark {
        EndMark::CrowOne => {
            line(bar(8.0));
            line(bar(13.0));
        }
        EndMark::CrowMany => line(bar(CROW_LEN + 5.0)),
        EndMark::CrowZeroOne => line(bar(8.0)),
        _ => {}
    }
    if matches!(mark, EndMark::CrowZeroOne | EndMark::CrowZeroMany) {
        let along = if many { CROW_LEN } else { 8.0 } + 5.0 + CROW_CIRCLE;
        let c = at(along, 0.0);
        scene.prims.push(Prim::Ellipse {
            rect: Rect::new(
                c.x - CROW_CIRCLE,
                c.y - CROW_CIRCLE,
                2.0 * CROW_CIRCLE,
                2.0 * CROW_CIRCLE,
            ),
            fill: Some(Color::WHITE),
            stroke,
            dashed: false,
        });
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
        assert!((mid.x - 50.0).abs() < 1e-9 && (mid.y + 15.0).abs() < 1e-9);
    }

    /// StarUML 7.1.1 draws an association's head role name below and the
    /// head multiplicity above a left-pointing line (observed on shapes.mdj).
    #[test]
    fn head_labels_match_staruml_sides() {
        // Line from tail (Canvas, right) to head (Shape, left).
        let pts = [Point::new(300.0, 100.0), Point::new(100.0, 100.0)];
        let role = label_point(&pts, edge_position::HEAD, -PI / 6.0, 30.0);
        let mult = label_point(&pts, edge_position::HEAD, PI / 6.0, 25.0);
        assert!(role.y > 100.0, "role name below: {role:?}");
        assert!(mult.y < 100.0, "multiplicity above: {mult:?}");
    }

    /// StarUML leaves `edgePosition` out when it equals the metamodel default
    /// (HEAD). Such a label must sit at the head end, not mid-line.
    #[test]
    fn label_without_edge_position_sits_at_head() {
        let file = r#"{"_type":"Project","_id":"P","ownedElements":[
          {"_type":"UMLModel","_id":"M","ownedElements":[
            {"_type":"UMLClass","_id":"A","name":"A"},
            {"_type":"UMLClass","_id":"B","name":"B"},
            {"_type":"UMLAssociation","_id":"S",
             "end1":{"_type":"UMLAssociationEnd","_id":"E1","reference":{"$ref":"A"}},
             "end2":{"_type":"UMLAssociationEnd","_id":"E2","reference":{"$ref":"B"},"name":"role"}},
            {"_type":"UMLClassDiagram","_id":"D","ownedViews":[
              {"_type":"UMLClassView","_id":"VA","model":{"$ref":"A"},"left":400,"top":0,"width":100,"height":40},
              {"_type":"UMLClassView","_id":"VB","model":{"$ref":"B"},"left":0,"top":0,"width":100,"height":40},
              {"_type":"UMLAssociationView","_id":"VS","model":{"$ref":"S"},"tail":{"$ref":"VA"},"head":{"$ref":"VB"},
               "lineStyle":1,"points":"400:20;100:20","headRoleNameLabel":{"$ref":"L"},
               "subViews":[{"_type":"EdgeLabelView","_id":"L","hostEdge":{"$ref":"VS"},"alpha":-0.5235987755982988,"distance":30}]}
            ]}]}]}"#;
        let doc = Document::from_json_str(file).unwrap();
        let scene = build_scene(&doc, "D", &Fonts::default());
        let role = scene
            .prims
            .iter()
            .find_map(|p| match p {
                Prim::Text { rect, text, .. } if text == "role" => Some(*rect),
                _ => None,
            })
            .expect("role label drawn");
        assert!(
            role.center().x < 160.0,
            "near the head (x=100), not mid-line: {role:?}"
        );
        assert!(
            role.center().y > 20.0,
            "below the line, as StarUML draws it: {role:?}"
        );
    }

    #[test]
    fn communication_messages_ride_beside_their_connector() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let dgm = doc
            .try_edit(|e| uml::create_diagram(e, &model, "UMLCommunicationDiagram"))
            .unwrap();
        let on = "UMLCommunicationDiagram";
        let lifeline = uml::node_spec_on("UMLLifeline", on).unwrap();
        let (_, a) = doc
            .try_edit(|e| uml::create_node(e, &dgm, lifeline, 40.0, 100.0, None))
            .unwrap();
        let (_, b) = doc
            .try_edit(|e| uml::create_node(e, &dgm, lifeline, 400.0, 100.0, None))
            .unwrap();
        let forward = uml::edge_spec_on("UMLForwardMessage", on).unwrap();
        let (_, fwd) = doc
            .try_edit(|e| uml::create_edge(e, &dgm, forward, &a, &b))
            .unwrap();
        let connector = doc
            .get(&fwd)
            .unwrap()
            .ref_id("hostEdge")
            .unwrap()
            .to_owned();
        let reverse = uml::edge_spec_on("UMLReverseMessage", on).unwrap();
        let (_, rev) = doc
            .try_edit(|e| uml::create_edge(e, &dgm, reverse, &b, &connector))
            .unwrap();

        let scene = build_scene(&doc, &dgm, &Fonts::default());
        let texts: Vec<&str> = scene
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"1 : Message1"), "{texts:?}");
        assert!(texts.contains(&"2 : Message2"), "{texts:?}");
        // The two 40 px arrows, above the left-to-right connector, pointing
        // opposite ways (forward first, as drawn).
        let arrows: Vec<(Point, Point)> = scene
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Line { points, .. } if points.len() == 2 => {
                    let (f, t) = (points[0], points[1]);
                    ((f.dist(t) - 40.0).abs() < 1e-6).then_some((f, t))
                }
                _ => None,
            })
            .collect();
        assert_eq!(arrows.len(), 2, "{arrows:?}");
        assert!(arrows[0].1.x > arrows[0].0.x, "forward points to the head");
        assert!(arrows[1].1.x < arrows[1].0.x, "reverse points back");
        assert!(
            arrows[0].0.y < 115.0,
            "beside the line, above it: {arrows:?}"
        );
        for v in [&fwd, &rev] {
            assert!(scene.hits.iter().any(|h| &h.view == v), "{v} is clickable");
        }

        doc.edit(|e| e.set(&dgm, "showSequenceNumber", mdj::Field::bool(false)));
        let scene = build_scene(&doc, &dgm, &Fonts::default());
        assert!(
            scene
                .prims
                .iter()
                .any(|p| matches!(p, Prim::Text { text, .. } if text == "Message1"))
        );
    }

    fn texts(scene: &Scene) -> Vec<(String, f64)> {
        scene
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, style, .. } => Some((text.clone(), style.size)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn interaction_overview_frames() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let on = "UMLInteractionOverviewDiagram";
        let dgm = doc
            .try_edit(|e| uml::create_diagram(e, &model, on))
            .unwrap();
        let place = |doc: &mut Document, tool: &str, x: f64| {
            let spec = uml::node_spec_on(tool, on).unwrap();
            doc.try_edit(|e| uml::create_node(e, &dgm, spec, x, 80.0, None))
                .unwrap()
        };
        let (_, use_view) = place(&mut doc, "UMLInteractionUseInOverview", 40.0);
        let (inline, _) = place(&mut doc, "UMLInteractionInOverview", 300.0);
        // Something to see inside the inline frame.
        let seq = doc
            .get(&inline.unwrap())
            .unwrap()
            .ref_id("target")
            .unwrap()
            .to_owned();
        let lifeline = uml::node_spec_on("UMLLifeline", "UMLSequenceDiagram").unwrap();
        doc.try_edit(|e| uml::create_node(e, &seq, lifeline, 20.0, 20.0, None))
            .unwrap();

        let scene = build_scene(&doc, &dgm, &Fonts::default());
        let t = texts(&scene);
        let has = |s: &str| t.iter().any(|(x, _)| x == s);
        assert!(has("ref") && has("Interaction2"), "{t:?}");
        assert!(has("sd SequenceDiagram1"), "{t:?}");
        let nested = t
            .iter()
            .find(|(x, _)| x == "Lifeline1")
            .expect("nested lifeline");
        assert!(nested.1 < 13.0, "drawn shrunk: {nested:?}");
        // The whole box is clickable, not just its tag.
        let r = scene.node_rect(&use_view).unwrap();
        assert_eq!(
            scene.hit_test(Point::new(r.center().x, r.bottom() - 5.0), 1.0),
            Some(use_view.as_str())
        );
    }

    #[test]
    fn information_flow_notation() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let on = "UMLInformationFlowDiagram";
        let dgm = doc
            .try_edit(|e| uml::create_diagram(e, &model, on))
            .unwrap();
        let place = |doc: &mut Document, tool: &str, x: f64| {
            let spec = uml::node_spec_on(tool, on).unwrap();
            doc.try_edit(|e| uml::create_node(e, &dgm, spec, x, 80.0, None))
                .unwrap()
        };
        let (item, iv) = place(&mut doc, "UMLInformationItem", 40.0);
        let (_, cv) = place(&mut doc, "UMLClass", 400.0);
        let spec = uml::edge_spec_on("UMLInformationFlow", on).unwrap();
        let (flow, _) = doc
            .try_edit(|e| uml::create_edge(e, &dgm, spec, &iv, &cv))
            .unwrap();
        doc.edit(|e| {
            e.set(
                &flow.unwrap(),
                "conveyed",
                mdj::Field::Refs(vec![item.unwrap()]),
            )
        });
        let scene = build_scene(&doc, &dgm, &Fonts::default());
        let t = texts(&scene);
        let has = |s: &str| t.iter().any(|(x, _)| x == s);
        assert!(has("«information»") && has("«flow»"), "{t:?}");
        let named = t.iter().filter(|(x, _)| x == "InformationItem1").count();
        assert_eq!(
            named, 2,
            "the box, and the flow naming what it conveys: {t:?}"
        );
        assert!(
            scene
                .prims
                .iter()
                .any(|p| matches!(p, Prim::Line { dashed: true, .. })),
            "drawn dotted"
        );
    }

    #[test]
    fn timing_frames_arrange_what_they_hold() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let on = "UMLTimingDiagram";
        let dgm = doc
            .try_edit(|e| uml::create_diagram(e, &model, on))
            .unwrap();
        let add = |doc: &mut Document, tool: &str, container: Option<&str>, name: &str| {
            let spec = uml::node_spec_on(tool, on).unwrap();
            let (m, v) = doc
                .try_edit(|e| uml::create_node(e, &dgm, spec, 0.0, 0.0, container))
                .unwrap();
            if let Some(m) = m {
                doc.edit(|e| e.set(&m, "name", mdj::Field::str(name)));
            }
            v
        };
        let door = add(&mut doc, "UMLLifeline", None, "door");
        let open = add(&mut doc, "UMLTimingState", Some(&door), "Open");
        let closed = add(&mut doc, "UMLTimingState", Some(&door), "Closed");
        let s1 = add(&mut doc, "UMLTimeSegment", Some(&closed), "");
        let s2 = add(&mut doc, "UMLTimeSegment", Some(&open), "");
        let alarm = add(&mut doc, "UMLLifeline", None, "alarm");
        let ringing = add(&mut doc, "UMLTimingState", Some(&alarm), "Ringing");
        let a1 = add(&mut doc, "UMLTimeSegment", Some(&ringing), "");
        let tick = add(&mut doc, "UMLTimeTick", None, "");
        let spec = uml::edge_spec_on("UMLMessage", on).unwrap();
        doc.try_edit(|e| uml::create_edge(e, &dgm, spec, &s1, &a1))
            .unwrap();
        // Out-of-date stored positions don't matter: the frame arranges.
        doc.edit(|e| e.set(&s2, "left", mdj::Field::int(900)));

        let scene = build_scene(&doc, &dgm, &Fonts::default());
        let rect = |v: &str| scene.node_rect(v).unwrap();
        let (d, a) = (rect(&door), rect(&alarm));
        assert_eq!(a.y, d.bottom(), "lifelines stack");
        assert_eq!(rect(&closed).y, rect(&open).bottom(), "states stack");
        let (r1, r2) = (rect(&s1), rect(&s2));
        assert_eq!(r2.x, r1.right(), "segments follow one another");
        assert_eq!(rect(&a1).x, r1.x, "every lifeline's axis starts together");
        assert_eq!(rect(&tick).x, r1.x, "ticks too");
        assert!(r1.y > r2.y, "on their state's row (Closed is below Open)");
        // The step from Closed up to Open, and the message from the end of
        // the first segment down to the alarm's segment.
        let has_line = |a: Point, b: Point| {
            scene.prims.iter().any(|p| {
                matches!(p, Prim::Line { points, .. }
                    if points.len() == 2 && points[0] == a && points[1] == b)
            })
        };
        let (m1, m2) = (r1.y + 12.0, r2.y + 12.0);
        assert!(has_line(Point::new(r2.x, m2), Point::new(r2.x, m1)), "step");
        assert!(
            has_line(
                Point::new(r1.right(), (r1.y + r1.h / 2.0).round()),
                Point::new(rect(&a1).x, (rect(&a1).y + 12.5).round())
            ),
            "message"
        );
        // Names: the lifeline's turned, the states' level.
        let turned = scene.prims.iter().any(
            |p| matches!(p, Prim::Text { text, style, .. } if text == "door" && style.vertical),
        );
        assert!(turned);
        let svg = crate::to_svg(&scene);
        assert!(svg.contains("rotate(-90"), "turned in SVG too");
    }

    #[test]
    fn flowchart_symbols() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let on = "FCFlowchartDiagram";
        let dgm = doc
            .try_edit(|e| uml::create_diagram(e, &model, on))
            .unwrap();
        // Every symbol draws something and is clickable, name or not.
        let mut views = Vec::new();
        for (i, spec) in uml::NODES
            .iter()
            .filter(|s| s.id.starts_with("FC"))
            .enumerate()
        {
            let (_, v) = doc
                .try_edit(|e| {
                    uml::create_node(
                        e,
                        &dgm,
                        spec,
                        (i % 6) as f64 * 200.0,
                        (i / 6) as f64 * 150.0,
                        None,
                    )
                })
                .unwrap();
            views.push(v);
        }
        assert_eq!(views.len(), 27);
        let spec = uml::edge_spec_on("FCFlow", on).unwrap();
        doc.try_edit(|e| uml::create_edge(e, &dgm, spec, &views[0], &views[1]))
            .unwrap();
        let scene = build_scene(&doc, &dgm, &Fonts::default());
        for v in &views {
            assert!(scene.node_rect(v).is_some(), "{v}");
        }
        // The decision is a diamond with its name in the middle.
        let decision = scene.node_rect(&views[2]).unwrap();
        let diamond = scene.prims.iter().any(|p| {
            matches!(p, Prim::Polygon { points, .. }
                if points.len() == 4 && points[1] == Point::new(decision.center().x, decision.y))
        });
        assert!(diamond);
        let label = scene
            .prims
            .iter()
            .find_map(|p| match p {
                Prim::Text { text, rect, .. } if text == "Decision1" => Some(*rect),
                _ => None,
            })
            .unwrap();
        assert!((label.center().y - decision.center().y).abs() < 1.0);
        // A database's name sits below its rim.
        let db = scene.node_rect(&views[24]).unwrap();
        let db_label = scene
            .prims
            .iter()
            .find_map(|p| match p {
                Prim::Text { text, rect, .. } if text == "Database1" => Some(*rect),
                _ => None,
            })
            .unwrap();
        assert!(db_label.y >= db.y + 10.0 + 2.0 * (db.h / 8.0).floor());
        // Flows end in a filled arrowhead.
        assert!(scene.prims.iter().any(|p| matches!(
            p,
            Prim::Polygon { points, fill: Some(Color::BLACK), .. } if points.len() == 3
        )));
    }

    #[test]
    fn lines_end_on_diamonds_and_ovals() {
        let r = Rect::new(0.0, 0.0, 100.0, 60.0);
        // From the right-hand corner of the box toward the centre.
        let on_box = Point::new(100.0, 60.0);
        let d = clip_point(on_box, r, Outline::Diamond);
        assert!(((d.x - 50.0).abs() / 50.0 + (d.y - 30.0).abs() / 30.0 - 1.0).abs() < 1e-9);
        let e = clip_point(Point::new(100.0, 30.0), r, Outline::Ellipse);
        assert_eq!(e, Point::new(100.0, 30.0), "already on the oval");
        let inside = Point::new(55.0, 32.0);
        assert_eq!(clip_point(inside, r, Outline::Diamond), inside);
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

    #[test]
    fn entities_draw_a_column_table_and_relationships_crows_feet() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let on = "ERDDiagram";
        let dgm = doc
            .try_edit(|e| uml::create_diagram(e, &model, on))
            .unwrap();
        let spec = uml::node_spec_on("ERDEntity", on).unwrap();
        let mut entity = |x: f64| {
            doc.try_edit(|e| uml::create_node(e, &dgm, spec, x, 0.0, None))
                .unwrap()
        };
        let ((a, av), (_, bv)) = (entity(0.0), entity(400.0));
        let a = a.unwrap();
        for (name, pk, ty) in [("id", true, "INTEGER"), ("email", false, "VARCHAR")] {
            let c = doc.edit(|e| uml::add_feature(e, &a, uml::FeatureKind::Column));
            doc.edit(|e| {
                e.set(&c, "name", mdj::Field::str(name));
                e.set(&c, "type", mdj::Field::str(ty));
                e.set(&c, "primaryKey", mdj::Field::bool(pk));
            });
        }
        let l = crate::node_layout(&doc, doc.get(&av).unwrap(), &Fonts::default()).unwrap();
        // The name, then keys | name | type for each column.
        assert_eq!(l.compartments.len(), 1 + 2 * 3);
        assert!(l.compartments[0].lines[0].bold);
        let cell = |i: usize| {
            (
                l.compartments[i].lines[0].text.clone(),
                l.compartments[i].rect,
            )
        };
        assert_eq!(cell(1).0, "PK");
        assert_eq!(cell(3).0, "INTEGER");
        let (name_x, type_x) = (cell(2).1.x, cell(3).1.x);
        assert_eq!(cell(5).1.x, name_x, "names line up");
        assert!(name_x < type_x);
        // A line under the name and one between each pair of table columns.
        assert_eq!(l.dividers.len(), 3);
        let fits = cell(6).1.right() - TEXT_PAD_X;
        assert!(l.rect.right() >= fits, "the box holds the widest type");

        let rel = uml::edge_spec_on("ERDRelationshipOneToMany", on).unwrap();
        let (m, _) = doc
            .try_edit(|e| uml::create_edge(e, &dgm, rel, &av, &bv))
            .unwrap();
        let ellipses = |scene: &Scene| {
            scene
                .prims
                .iter()
                .filter(|p| matches!(p, Prim::Ellipse { .. }))
                .count()
        };
        let dashed = |scene: &Scene| {
            scene
                .prims
                .iter()
                .any(|p| matches!(p, Prim::Line { dashed: true, .. }))
        };
        let scene = build_scene(&doc, &dgm, &Fonts::default());
        assert_eq!(ellipses(&scene), 1, "only the `0..*` end has a ring");
        assert!(!dashed(&scene), "identifying by default");
        doc.edit(|e| e.set(&m.unwrap(), "identifying", mdj::Field::bool(false)));
        assert!(dashed(&build_scene(&doc, &dgm, &Fonts::default())));
    }
}
