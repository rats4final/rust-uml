//! Timing diagrams: the frame arranges what it holds, as StarUML does when
//! drawing (so stored positions only give the order).
//!
//! - Lifelines are bands across the frame, stacked below its tag in the order
//!   of their stored tops, each tall enough for its states.
//! - A lifeline lists its states (sorted by stored top) one 25 px row each,
//!   names right-aligned up to the time axis's start.
//! - Its time segments (from all its states, sorted by stored left) follow
//!   one another from that start; each sits on its state's row, and a step
//!   joins it to the one before.
//! - Time ticks run along the frame's bottom edge from the same start.

use std::collections::HashMap;

use mdj::{Document, Element, Id};

use crate::geom::Rect;
use crate::layout::heading;
use crate::text::Fonts;

/// StarUML's timing constants: lifeline top/bottom padding, state row and
/// segment height, compartment padding.
const PAD: f64 = 25.0;
const ROW: f64 = 25.0;
const GAP: f64 = 5.0;
const SEGMENT_MIN_W: f64 = 8.0;
const TICK_MIN_W: f64 = 9.0;
const TICK_H: f64 = 11.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Kind {
    /// The name is drawn turned, in `name` (one line wide).
    Lifeline {
        name: String,
        name_rect: Rect,
    },
    /// The name, right-aligned in `label`.
    State {
        name: String,
        label: Rect,
    },
    /// The middle height of the previous segment on the lifeline, if any.
    Segment {
        prev_mid: Option<f64>,
    },
    Tick {
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    pub view: Id,
    pub rect: Rect,
    pub kind: Kind,
}

fn stored(v: &Element) -> Rect {
    Rect::new(
        v.num("left", 0.0),
        v.num("top", 0.0),
        v.num("width", 0.0),
        v.num("height", 0.0),
    )
}

/// The views of `ids` with view type `t`, sorted by `key` of their stored box.
fn sorted<'a>(
    doc: &'a Document,
    ids: &[Id],
    t: &str,
    key: impl Fn(&Rect) -> f64,
) -> Vec<&'a Element> {
    let mut out: Vec<&Element> = ids
        .iter()
        .filter_map(|id| doc.get(id))
        .filter(|v| v.type_name == t)
        .collect();
    out.sort_by(|a, b| key(&stored(a)).total_cmp(&key(&stored(b))));
    out
}

fn contained(v: &Element) -> Vec<Id> {
    v.get("containedViews")
        .map(|f| f.refs().to_vec())
        .unwrap_or_default()
}

/// Arrange what the timing frame `frame` holds, inside `rect` (its drawn box)
/// below `tag_bottom`. Items come lifeline by lifeline (lifeline, states,
/// segments), then ticks, so later items sit on top for clicks.
pub(crate) fn arrange(
    doc: &Document,
    frame: &Element,
    rect: Rect,
    tag_bottom: f64,
    size: f64,
    fonts: &Fonts,
) -> Vec<Item> {
    let line_h = fonts.line_height(size);
    let held = contained(frame);
    let lifelines = sorted(doc, &held, "UMLTimingLifelineView", |r| r.y);
    // Each lifeline's states, and where its state column starts: right of
    // the turned name, as StarUML (name 2 paddings in, states 2 more after).
    let name_right = rect.x + 2.0 * GAP + line_h;
    let state_left = name_right + 2.0 * GAP;
    let states: Vec<Vec<&Element>> = lifelines
        .iter()
        .map(|l| sorted(doc, &contained(l), "UMLTimingStateView", |r| r.y))
        .collect();
    let state_name = |s: &Element| {
        s.ref_id("model")
            .and_then(|m| doc.get(m))
            .map_or(String::new(), |m| m.name().to_owned())
    };
    // The time axis starts right of the widest state name.
    let start_x = states
        .iter()
        .flatten()
        .map(|s| state_left + fonts.width(&state_name(s), size, false, false) + GAP)
        .fold(rect.x, f64::max);

    let mut items = Vec::new();
    let mut y = tag_bottom + GAP;
    for (lifeline, its_states) in lifelines.iter().zip(&states) {
        let name = lifeline
            .ref_id("model")
            .and_then(|m| doc.get(m))
            .map_or(String::new(), |m| heading(doc, lifeline, m).0);
        let name_len = fonts.width(&name, size, true, false);
        let min_h = (2.0 * GAP + name_len).max(2.0 * PAD + ROW * its_states.len() as f64);
        let band = Rect::new(rect.x, y, rect.w, stored(lifeline).h.max(min_h));
        items.push(Item {
            view: lifeline.id.clone(),
            rect: band,
            kind: Kind::Lifeline {
                name_rect: Rect::new(
                    name_right - line_h,
                    band.y + (band.h - name_len) / 2.0,
                    line_h,
                    name_len,
                ),
                name,
            },
        });
        let mut rows: HashMap<&str, Rect> = HashMap::new();
        let mut sy = band.y + PAD;
        for s in its_states {
            let row = Rect::new(state_left, sy, (band.right() - state_left).max(0.0), ROW);
            rows.insert(s.id.as_str(), row);
            items.push(Item {
                view: s.id.clone(),
                rect: row,
                kind: Kind::State {
                    name: state_name(s),
                    label: Rect::new(state_left, sy, (start_x - GAP - state_left).max(0.0), ROW),
                },
            });
            sy += ROW;
        }
        // Segments of all its states, one after another along the axis.
        let mut segments: Vec<(&Element, Rect)> = its_states
            .iter()
            .flat_map(|s| {
                let row = rows.get(s.id.as_str()).copied().unwrap_or_default();
                sorted(doc, &contained(s), "UMLTimeSegmentView", |r| r.x)
                    .into_iter()
                    .map(move |sg| (sg, row))
            })
            .collect();
        segments.sort_by(|a, b| stored(a.0).x.total_cmp(&stored(b.0).x));
        let mut x = start_x;
        let mut prev_mid = None;
        for (sg, row) in segments {
            let w = stored(sg).w.max(SEGMENT_MIN_W);
            let top = (row.y + row.h / 2.0).round() - (ROW / 2.0).floor();
            let r = Rect::new(x, top, w, ROW);
            items.push(Item {
                view: sg.id.clone(),
                rect: r,
                kind: Kind::Segment { prev_mid },
            });
            prev_mid = Some(r.y + (ROW / 2.0).floor());
            x += w;
        }
        y += band.h;
    }
    let mut x = start_x;
    for tick in sorted(doc, &held, "UMLTimeTickView", |r| r.x) {
        let w = stored(tick).w.max(TICK_MIN_W);
        items.push(Item {
            view: tick.id.clone(),
            rect: Rect::new(x, rect.bottom() - GAP, w, TICK_H),
            kind: Kind::Tick {
                text: tick.str("text").unwrap_or("").to_owned(),
            },
        });
        x += w;
    }
    items
}

/// Whether a view is laid out by its timing frame.
pub(crate) fn is_arranged(view_type: &str) -> bool {
    matches!(
        view_type,
        "UMLTimingLifelineView" | "UMLTimingStateView" | "UMLTimeSegmentView" | "UMLTimeTickView"
    )
}
