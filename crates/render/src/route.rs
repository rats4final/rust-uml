//! Edge routing, and keeping stored geometry in step with what is drawn.

use std::collections::{HashMap, HashSet};

use mdj::values::{absent, format_points, line_style, parse_points};
use mdj::{Editor, Field, Id};

use crate::geom::{Point, Rect};
use crate::layout::node_layout;
use crate::text::Fonts;

const LOOP: f64 = 30.0;

/// Points for an edge from `tail` to `head`. `interior` bend points are kept
/// for oblique-style lines; right-angle lines are re-routed from scratch.
pub(crate) fn route(tail: &Rect, head: &Rect, interior: &[Point], style: i64) -> Vec<Point> {
    if tail == head {
        let (r, x) = (tail, tail.right() - tail.w * 0.25);
        let y = tail.y + tail.h * 0.25;
        return vec![
            Point::new(x, r.y),
            Point::new(x, r.y - LOOP),
            Point::new(r.right() + LOOP, r.y - LOOP),
            Point::new(r.right() + LOOP, y),
            Point::new(r.right(), y),
        ];
    }
    if style == line_style::RECTILINEAR || style == line_style::ROUND_RECT {
        return orthogonal(tail, head);
    }
    let first = interior.first().copied().unwrap_or(head.center());
    let last = interior.last().copied().unwrap_or(tail.center());
    let mut pts = Vec::with_capacity(interior.len() + 2);
    pts.push(tail.border_toward(first));
    pts.extend_from_slice(interior);
    pts.push(head.border_toward(last));
    pts
}

fn orthogonal(t: &Rect, h: &Rect) -> Vec<Point> {
    let (tc, hc) = (t.center(), h.center());
    let (ox1, ox2) = (t.x.max(h.x), t.right().min(h.right()));
    if ox1 < ox2 {
        let x = (ox1 + ox2) / 2.0;
        return if hc.y > tc.y {
            vec![Point::new(x, t.bottom()), Point::new(x, h.y)]
        } else {
            vec![Point::new(x, t.y), Point::new(x, h.bottom())]
        };
    }
    let (oy1, oy2) = (t.y.max(h.y), t.bottom().min(h.bottom()));
    if oy1 < oy2 {
        let y = (oy1 + oy2) / 2.0;
        return if hc.x > tc.x {
            vec![Point::new(t.right(), y), Point::new(h.x, y)]
        } else {
            vec![Point::new(t.x, y), Point::new(h.right(), y)]
        };
    }
    if (hc.y - tc.y).abs() >= (hc.x - tc.x).abs() {
        let mid = (tc.y + hc.y) / 2.0;
        let (ty, hy) = if hc.y > tc.y {
            (t.bottom(), h.y)
        } else {
            (t.y, h.bottom())
        };
        vec![
            Point::new(tc.x, ty),
            Point::new(tc.x, mid),
            Point::new(hc.x, mid),
            Point::new(hc.x, hy),
        ]
    } else {
        let mid = (tc.x + hc.x) / 2.0;
        let (tx, hx) = if hc.x > tc.x {
            (t.right(), h.x)
        } else {
            (t.x, h.right())
        };
        vec![
            Point::new(tx, tc.y),
            Point::new(mid, tc.y),
            Point::new(mid, hc.y),
            Point::new(hx, hc.y),
        ]
    }
}

/// Bring stored geometry in `diagram` up to date, inside the current edit:
/// grow boxes whose content no longer fits, then re-route edges attached to
/// any box in `touched` or grown here, and edges that have no points yet.
/// Edges listed in `keep` (already translated by the caller) are left alone.
pub fn reflow(
    e: &mut Editor,
    diagram: &str,
    touched: &HashSet<Id>,
    keep: &HashSet<Id>,
    fonts: &Fonts,
) {
    let doc = e.doc();
    let Some(dgm) = doc.get(diagram) else {
        return;
    };
    let views: Vec<Id> = dgm.children("ownedViews").to_vec();

    let mut touched = touched.clone();
    let mut rects: HashMap<Id, Rect> = HashMap::new();
    // Line part → its lifeline view, so moving a lifeline re-routes its messages.
    let mut owner_of_line: HashMap<Id, Id> = HashMap::new();
    let mut grow: Vec<(Id, f64, f64)> = Vec::new();
    for id in &views {
        let Some(layout) = doc.get(id).and_then(|v| node_layout(doc, v, fonts)) else {
            continue;
        };
        let Some(v) = doc.get(id) else { continue };
        if v.num("width", 0.0) < layout.min_w || v.num("height", 0.0) < layout.min_h {
            grow.push((id.clone(), layout.rect.w, layout.rect.h));
            touched.insert(id.clone());
        }
        if let (Some(line), Some(r)) = (v.ref_id("linePart"), crate::scene::line_part_rect(&layout))
        {
            rects.insert(line.to_owned(), r);
            owner_of_line.insert(line.to_owned(), id.clone());
        }
        rects.insert(id.clone(), layout.rect);
    }

    let mut reroute: Vec<(Id, String)> = Vec::new();
    let mut shift_activation: Vec<(Id, f64)> = Vec::new();
    for id in &views {
        let Some(v) = doc.get(id) else { continue };
        let (Some(tail), Some(head)) = (v.ref_id("tail"), v.ref_id("head")) else {
            continue;
        };
        let (Some(tr), Some(hr)) = (rects.get(tail), rects.get(head)) else {
            continue;
        };
        if v.type_name == "UMLSeqMessageView" {
            // Messages keep their height and follow their lifelines sideways.
            let moved = |line: &str| owner_of_line.get(line).is_some_and(|l| touched.contains(l));
            let stored = parse_points(v.str("points").unwrap_or(""));
            if keep.contains(id) || (stored.len() >= 2 && !moved(tail) && !moved(head)) {
                continue;
            }
            let y = stored.first().map_or(tr.y + 30.0, |p| p.1);
            let (x1, x2) = (tr.x, hr.x);
            let pts = if tail == head {
                vec![
                    (x1, y),
                    (x1 + 40.0, y),
                    (x1 + 40.0, y + 20.0),
                    (x1, y + 20.0),
                ]
            } else {
                vec![(x1, y), (x2, y)]
            };
            reroute.push((id.clone(), format_points(&pts)));
            if let Some(act) = v.ref_id("activation").and_then(|a| doc.get(a)) {
                let w = act.num("width", 14.0);
                shift_activation.push((act.id.clone(), x2 - w / 2.0));
            }
            continue;
        }
        let stored: Vec<Point> = parse_points(v.str("points").unwrap_or(""))
            .into_iter()
            .map(|(x, y)| Point::new(x, y))
            .collect();
        let needs = stored.len() < 2 || touched.contains(tail) || touched.contains(head);
        if !needs || keep.contains(id) {
            continue;
        }
        let interior = stored.get(1..stored.len().saturating_sub(1)).unwrap_or(&[]);
        let style = v.num("lineStyle", absent::LINE_STYLE as f64) as i64;
        let pts: Vec<(f64, f64)> = route(tr, hr, interior, style)
            .into_iter()
            .map(|p| (p.x, p.y))
            .collect();
        reroute.push((id.clone(), format_points(&pts)));
    }

    for (id, w, h) in grow {
        e.set(&id, "width", Field::int(w.ceil() as i64));
        e.set(&id, "height", Field::int(h.ceil() as i64));
    }
    for (id, pts) in reroute {
        e.set(&id, "points", Field::str(pts));
    }
    for (id, left) in shift_activation {
        e.set(&id, "left", Field::int(left.round() as i64));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orthogonal_routes_are_axis_aligned() {
        let a = Rect::new(0.0, 0.0, 100.0, 50.0);
        for b in [
            Rect::new(20.0, 200.0, 100.0, 50.0),
            Rect::new(300.0, 10.0, 100.0, 50.0),
            Rect::new(300.0, 300.0, 100.0, 50.0),
        ] {
            let pts = route(&a, &b, &[], line_style::RECTILINEAR);
            for w in pts.windows(2) {
                assert!(w[0].x == w[1].x || w[0].y == w[1].y, "{pts:?}");
            }
        }
    }

    #[test]
    fn oblique_keeps_bends_and_touches_borders() {
        let a = Rect::new(0.0, 0.0, 100.0, 50.0);
        let b = Rect::new(300.0, 0.0, 100.0, 50.0);
        let bend = Point::new(200.0, 200.0);
        let pts = route(&a, &b, &[bend], line_style::OBLIQUE);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[1], bend);
        assert!(pts[0].y == 50.0 || pts[0].x == 100.0);
    }
}
