use std::collections::HashMap;
use std::f64::consts::PI;

use mdj::values::{DEFAULT_FONT, Font, absent, edge_position, parse_points};
use mdj::{Document, Element, Id};

use crate::geom::{Color, Point, Rect};
use crate::layout::{
    ACTIVE_INSET, HAlign, ICON_SPACE, Icon, NODE_DEPTH, NOTE_FOLD, NodeLayout, ROUND, Shape,
    Symbol, TEXT_PAD_X, TEXT_PAD_Y, node_layout,
};
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
    let mut rects: HashMap<&str, Rect> = layouts.iter().map(|(k, l)| (*k, l.rect)).collect();
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

    for view in views {
        if let Some(layout) = layouts.get(view.id.as_str()) {
            draw_node(&mut scene, layout, fonts);
            // A frame is picked by its tag, so clicks inside reach what it frames.
            let hit = match layout.shape {
                Shape::Frame { tag } => tag,
                _ => layout.rect,
            };
            scene.hits.push(Hit {
                view: view.id.clone(),
                shape: HitShape::Rect(hit),
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
}

/// Line style and end markers for an edge view. Associations look at their
/// ends; other relationships are recognised by view type name so that the
/// many UML edge kinds get sensible notation.
fn edge_style(doc: &Document, view: &Element) -> (bool, EndMark, EndMark) {
    let t = view.type_name.as_str();
    let model = view.ref_id("model").and_then(|m| doc.get(m));
    if t == "UMLSeqMessageView" {
        // UML message notation by sort.
        let sort = model
            .and_then(|m| m.str("messageSort"))
            .unwrap_or("synchCall");
        return match sort {
            "synchCall" => (false, EndMark::None, EndMark::FilledTriangle),
            "reply" | "createMessage" => (true, EndMark::None, EndMark::OpenArrow),
            "deleteMessage" => (false, EndMark::None, EndMark::Destroy),
            _ => (false, EndMark::None, EndMark::OpenArrow),
        };
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

    let texts: Vec<(&str, String)> = vec![
        (
            "nameLabel",
            model
                .map(|m| match m.type_name.as_str() {
                    "UMLTransition" => uml::transition_text(doc, m),
                    "UMLControlFlow" | "UMLObjectFlow" => uml::flow_text(m),
                    "UMLMessage" => uml::message_text(m),
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
        _ => None,
    }
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
