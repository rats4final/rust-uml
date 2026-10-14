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
    // Where right-angle lines already touch each box side, so a newly routed
    // one can start beside them instead of on top of them.
    let mut taken = Taken::default();
    for id in &views {
        let Some(v) = doc.get(id) else { continue };
        let style = v.num("lineStyle", absent::LINE_STYLE as f64) as i64;
        if !is_orthogonal(style) {
            continue;
        }
        let (Some(tail), Some(head)) = (v.ref_id("tail"), v.ref_id("head")) else {
            continue;
        };
        let stored: Vec<Point> = parse_points(v.str("points").unwrap_or(""))
            .into_iter()
            .map(|(x, y)| Point::new(x, y))
            .collect();
        let rerouted = stored.len() < 2 || touched.contains(tail) || touched.contains(head);
        if rerouted && !keep.contains(id) {
            continue;
        }
        taken.add_ends(tail, head, &stored, &rects);
    }
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
        let mut pts = route(tr, hr, interior, style);
        if is_orthogonal(style) && tail != head {
            taken.spread(tail, head, &mut pts, &rects);
            taken.add_ends(tail, head, &pts, &rects);
        }
        let pts: Vec<(f64, f64)> = pts.into_iter().map(|p| (p.x, p.y)).collect();
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

fn is_orthogonal(style: i64) -> bool {
    style == line_style::RECTILINEAR || style == line_style::ROUND_RECT
}

/// Space kept between lines touching one side of a box, and from its
/// corners: room for end marks such as crow's feet side by side. Short
/// sides (a 48 px icon) keep a quarter of their length instead.
const SPREAD: f64 = 20.0;

/// The spacing used on `side` of `r`.
fn gap(r: &Rect, side: u8) -> f64 {
    let len = if side < 2 { r.h } else { r.w };
    SPREAD.min(len / 4.0)
}

/// Positions ever further from `along`, `toward` (+1/-1) first, in steps
/// small enough to find any gap between taken spots.
fn nearby(along: f64, toward: f64) -> impl Iterator<Item = f64> {
    (1..400).flat_map(move |k| [toward, -toward].map(|d| along + d * 2.0 * f64::from(k)))
}

/// A side of a box: which box, and 0 left, 1 right, 2 top, 3 bottom.
type Side = (Id, u8);

/// Points along box sides where lines already touch.
#[derive(Default)]
struct Taken {
    at: HashMap<Side, Vec<f64>>,
}

/// The side of `r` that `p` lies on, and `p`'s position along it.
fn side_of(r: &Rect, p: Point) -> Option<(u8, f64)> {
    const EPS: f64 = 0.5;
    if (p.x - r.x).abs() < EPS {
        Some((0, p.y))
    } else if (p.x - r.right()).abs() < EPS {
        Some((1, p.y))
    } else if (p.y - r.y).abs() < EPS {
        Some((2, p.x))
    } else if (p.y - r.bottom()).abs() < EPS {
        Some((3, p.x))
    } else {
        None
    }
}

impl Taken {
    fn add_ends(&mut self, tail: &str, head: &str, pts: &[Point], rects: &HashMap<Id, Rect>) {
        for (bx, p) in [(tail, pts.first()), (head, pts.last())] {
            let (Some(r), Some(p)) = (rects.get(bx), p) else {
                continue;
            };
            if let Some((side, along)) = side_of(r, *p) {
                self.at
                    .entry((bx.to_owned(), side))
                    .or_default()
                    .push(along);
            }
        }
    }

    fn is_free(&self, bx: &str, side: u8, along: f64, gap: f64) -> bool {
        self.at
            .get(&(bx.to_owned(), side))
            .is_none_or(|v| v.iter().all(|a| (a - along).abs() >= gap - 0.5))
    }

    /// The free spot on a side nearest `along`, within `lo..=hi`, trying
    /// the `toward` direction (+1/-1: where the line is heading) first.
    fn free_near(
        &self,
        bx: &str,
        side: u8,
        along: f64,
        (lo, hi): (f64, f64),
        toward: f64,
        gap: f64,
    ) -> Option<f64> {
        if self.is_free(bx, side, along, gap) {
            return Some(along);
        }
        nearby(along, toward).find(|&a| a >= lo && a <= hi && self.is_free(bx, side, a, gap))
    }

    /// Move a freshly routed right-angle line's ends off spots already taken
    /// on their box sides, keeping every segment axis-aligned: an end that
    /// turns at once moves with its first bend; a straight line moves whole,
    /// if both boxes still face each other there.
    fn spread(&self, tail: &str, head: &str, pts: &mut [Point], rects: &HashMap<Id, Rect>) {
        let (Some(tr), Some(hr)) = (rects.get(tail), rects.get(head)) else {
            return;
        };
        let n = pts.len();
        if n < 2 {
            return;
        }
        // Range of positions along a side, clear of the corners.
        let range = |r: &Rect, side: u8| {
            let g = gap(r, side);
            match side {
                0 | 1 => (r.y + g, r.bottom() - g),
                _ => (r.x + g, r.right() - g),
            }
        };
        let set = |p: &mut Point, side: u8, along: f64| match side {
            0 | 1 => p.y = along,
            _ => p.x = along,
        };
        if n == 2 {
            let (Some((ts, ta)), Some((hs, _))) = (side_of(tr, pts[0]), side_of(hr, pts[1])) else {
                return;
            };
            let g = gap(tr, ts).min(gap(hr, hs));
            if self.is_free(tail, ts, ta, g) && self.is_free(head, hs, ta, g) {
                return;
            }
            let (t_lo, t_hi) = range(tr, ts);
            let (h_lo, h_hi) = range(hr, hs);
            let (lo, hi) = (t_lo.max(h_lo), t_hi.min(h_hi));
            let moved = nearby(ta, 1.0).find(|&a| {
                a >= lo && a <= hi && self.is_free(tail, ts, a, g) && self.is_free(head, hs, a, g)
            });
            if let Some(a) = moved {
                set(&mut pts[0], ts, a);
                set(&mut pts[1], hs, a);
            }
            return;
        }
        for (bx, r, end, far) in [(tail, tr, 0, n - 1), (head, hr, n - 1, 0)] {
            let next = if end == 0 { 1 } else { n - 2 };
            let Some((side, along)) = side_of(r, pts[end]) else {
                continue;
            };
            let other = match side {
                0 | 1 => pts[far].y,
                _ => pts[far].x,
            };
            let toward = if other < along { -1.0 } else { 1.0 };
            if let Some(a) = self.free_near(bx, side, along, range(r, side), toward, gap(r, side))
                && a != along
            {
                set(&mut pts[end], side, a);
                set(&mut pts[next], side, a);
            }
        }
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
    fn lines_from_one_side_start_apart() {
        let rects: HashMap<Id, Rect> = [
            ("a", Rect::new(0.0, 100.0, 100.0, 120.0)),
            ("up", Rect::new(300.0, 0.0, 100.0, 50.0)),
            ("down", Rect::new(300.0, 300.0, 100.0, 50.0)),
            ("level", Rect::new(300.0, 110.0, 100.0, 120.0)),
        ]
        .into_iter()
        .map(|(k, r)| (k.to_owned(), r))
        .collect();
        let mut taken = Taken::default();
        let mut starts = Vec::new();
        for to in ["up", "down", "level"] {
            let mut pts = route(&rects["a"], &rects[to], &[], line_style::ROUND_RECT);
            taken.spread("a", to, &mut pts, &rects);
            taken.add_ends("a", to, &pts, &rects);
            for w in pts.windows(2) {
                assert!(w[0].x == w[1].x || w[0].y == w[1].y, "{pts:?}");
            }
            assert_eq!(pts[0].x, 100.0, "leaves a's right side");
            starts.push(pts[0].y);
        }
        // The line going down moved down, not up.
        assert!(starts[1] > starts[0], "{starts:?}");
        for (i, a) in starts.iter().enumerate() {
            for b in &starts[i + 1..] {
                assert!((a - b).abs() >= SPREAD - 0.5, "{starts:?}");
            }
            assert!(*a > 100.0 && *a < 220.0, "on the side: {starts:?}");
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
