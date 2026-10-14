//! StarUML's quick-edit buttons on a selected box, as commands: add a mind
//! map node beside a node, add an entity related to an entity, and the
//! Format ▸ Suppress Columns toggle.

use mdj::{Field, Id};
use serde_json::{Value, json};

use crate::commands::{CommandError, active, create_edge, create_node, node_view};
use crate::params;
use crate::session::Session;

/// Room between a box and the one added beside it, and between those added
/// on one side.
const BESIDE: f64 = 60.0;
const BELOW: f64 = 20.0;

/// The box the command works on: `view`, else the selected box, which must
/// be a `view_type`.
fn target(s: &Session, p: &Value, view_type: &str) -> Result<Id, CommandError> {
    let id = match params::opt_str(p, "view") {
        Some(v) => v.to_owned(),
        None => s
            .selection
            .first()
            .cloned()
            .ok_or_else(|| CommandError::Disabled("select a box first".into()))?,
    };
    let view = node_view(s, &id)?;
    if view.type_name != view_type {
        return Err(CommandError::Param(format!("`{id}` is not a {view_type}")));
    }
    Ok(view.id.clone())
}

fn rect(s: &Session, view: &str) -> render::Rect {
    s.doc
        .get(view)
        .and_then(|v| render::node_layout(&s.doc, v, s.fonts()))
        .map(|l| l.rect)
        .unwrap_or_default()
}

/// Where a new box goes beside `view` on `side` (+1 right, -1 left): level
/// with it, or below the boxes already joined to it on that side by lines
/// of `edge_type` (`neighbour` picks the far end of such a line).
fn beside(
    s: &Session,
    view: &str,
    side: f64,
    edge_type: &str,
    neighbour: fn(&mdj::Element) -> Option<&str>,
    width: f64,
) -> Result<(f64, f64), CommandError> {
    let diagram = active(s)?;
    let r = rect(s, view);
    let lowest = s
        .doc
        .get(&diagram)
        .map(|d| d.children("ownedViews").to_vec())
        .unwrap_or_default()
        .iter()
        .filter_map(|id| s.doc.get(id))
        .filter(|e| e.type_name == edge_type)
        .filter_map(|e| neighbour(e).filter(|n| *n != view))
        .map(|n| rect(s, n))
        .filter(|n| (n.center().x - r.center().x) * side > 0.0)
        .map(|n| n.bottom())
        .fold(f64::NEG_INFINITY, f64::max);
    let y = if lowest.is_finite() {
        lowest + BELOW
    } else {
        r.y
    };
    let x = if side > 0.0 {
        r.right() + BESIDE
    } else {
        r.x - BESIDE - width
    };
    Ok((x, y))
}

/// Run `steps` as one undo step; on failure the model is left as it was.
fn as_one_step(
    s: &mut Session,
    steps: impl FnOnce(&mut Session) -> Result<Value, CommandError>,
) -> Result<Value, CommandError> {
    let saved = (s.doc.clone(), s.selection.clone());
    let depth = s.doc.undo_depth();
    match steps(s) {
        Ok(v) => {
            s.doc.squash_undo_since(depth);
            Ok(v)
        }
        Err(e) => {
            (s.doc, s.selection) = saved;
            Err(e)
        }
    }
}

fn id_in(v: &Value, key: &str) -> Result<Id, CommandError> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| CommandError::Failed(format!("no `{key}` in {v}")))
}

/// `mindmap.add-node`: a new node left or right of a node, with an edge from
/// the new node to it (StarUML's direction).
pub(crate) fn add_mind_node(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let node = target(s, p, "MMNodeView")?;
    let side = match params::opt_str(p, "side").unwrap_or("right") {
        "right" => 1.0,
        "left" => -1.0,
        other => {
            return Err(CommandError::Param(format!(
                "`side` must be \"left\" or \"right\", not `{other}`"
            )));
        }
    };
    let width = uml::node_spec("MMNode").map_or(80.0, |n| n.size.0);
    let (x, y) = beside(s, &node, side, "MMEdgeView", |e| e.ref_id("tail"), width)?;
    as_one_step(s, |s| {
        let created = create_node(s, &json!({ "kind": "MMNode", "x": x, "y": y }))?;
        let view = id_in(&created, "view")?;
        let edge = create_edge(s, &json!({ "kind": "MMEdge", "tail": view, "head": node }))?;
        s.selection = vec![view.clone()];
        Ok(json!({ "model": created["model"], "view": view, "edge": edge["view"] }))
    })
}

/// `erd.add-related`: a new entity right of an entity, joined by a
/// one-to-one, one-to-many or many-to-many relationship from it.
pub(crate) fn add_related_entity(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let entity = target(s, p, "ERDEntityView")?;
    let kind = match params::opt_str(p, "kind").unwrap_or("one-to-many") {
        "one-to-one" => "ERDRelationship",
        "one-to-many" => "ERDRelationshipOneToMany",
        "many-to-many" => "ERDRelationshipManyToMany",
        other => {
            return Err(CommandError::Param(format!(
                "`kind` must be one-to-one, one-to-many or many-to-many, not `{other}`"
            )));
        }
    };
    let (x, y) = beside(
        s,
        &entity,
        1.0,
        "ERDRelationshipView",
        |e| e.ref_id("head"),
        0.0,
    )?;
    as_one_step(s, |s| {
        let created = create_node(s, &json!({ "kind": "ERDEntity", "x": x, "y": y }))?;
        let view = id_in(&created, "view")?;
        let edge = create_edge(s, &json!({ "kind": kind, "tail": entity, "head": view }))?;
        s.selection = vec![view.clone()];
        Ok(json!({ "model": created["model"], "view": view, "edge": edge["view"] }))
    })
}

/// Whether any selected box is an entity (enables Suppress Columns).
pub(crate) fn has_entity_selected(s: &Session) -> bool {
    s.selection.iter().any(|id| {
        s.doc
            .get(id)
            .is_some_and(|v| v.type_name == "ERDEntityView")
    })
}

/// `view.suppress-columns`: hide the columns of the selected entities, or
/// show them again if all are hidden (StarUML's toggle). A hidden table's
/// box shrinks to its name.
pub(crate) fn suppress_columns(s: &mut Session, _: &Value) -> Result<Value, CommandError> {
    let views: Vec<Id> = s
        .selection
        .iter()
        .filter(|id| {
            s.doc
                .get(id.as_str())
                .is_some_and(|v| v.type_name == "ERDEntityView")
        })
        .cloned()
        .collect();
    if views.is_empty() {
        return Err(CommandError::Disabled("select an entity first".into()));
    }
    let all_hidden = views.iter().all(|v| {
        s.doc
            .get(v)
            .is_some_and(|v| v.bool("suppressColumns", false))
    });
    let hide = !all_hidden;
    as_one_step(s, |s| {
        s.edit(|e, c| {
            for v in &views {
                if hide {
                    e.set(v, "suppressColumns", Field::bool(true));
                } else if e
                    .doc()
                    .get(v)
                    .is_some_and(|el| el.get("suppressColumns").is_some())
                {
                    // `false` is the metamodel default: leave the field out, as StarUML saves it.
                    e.unset(v, "suppressColumns");
                }
                c.touched.insert(v.clone());
            }
            Ok(())
        })?;
        if hide {
            // The table is gone: shrink each box to its name.
            let heights: Vec<(Id, f64)> = views
                .iter()
                .filter_map(|v| {
                    let el = s.doc.get(v)?;
                    let l = render::node_layout(&s.doc, el, s.fonts())?;
                    Some((v.clone(), l.min_h))
                })
                .collect();
            s.edit(|e, c| {
                for (v, h) in &heights {
                    e.set(v, "height", Field::int(h.ceil() as i64));
                    c.touched.insert(v.clone());
                }
                Ok(())
            })?;
        }
        Ok(json!({ "suppressed": hide, "views": views }))
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::{Registry, Session};

    fn run(s: &mut Session, id: &str, p: Value) -> Value {
        Registry::default()
            .run(s, id, &p)
            .unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn left_top(s: &Session, view: &Value) -> (f64, f64) {
        let v = s.doc.get(view.as_str().unwrap()).unwrap();
        (v.num("left", 0.0), v.num("top", 0.0))
    }

    #[test]
    fn mind_map_nodes_added_beside_and_below() {
        let mut s = Session::default();
        run(&mut s, "diagram.add", json!({"type": "MMMindmapDiagram"}));
        let root = run(
            &mut s,
            "node.create",
            json!({"kind": "MMNode", "x": 300, "y": 100}),
        );
        let first = run(&mut s, "mindmap.add-node", json!({}));
        let second = run(&mut s, "mindmap.add-node", json!({"view": root["view"]}));
        let left = run(
            &mut s,
            "mindmap.add-node",
            json!({"view": root["view"], "side": "left"}),
        );
        let (rx, ry) = left_top(&s, &root["view"]);
        let (fx, fy) = left_top(&s, &first["view"]);
        let (sx, sy) = left_top(&s, &second["view"]);
        let (lx, _) = left_top(&s, &left["view"]);
        assert!(fx > rx && fy == ry, "right, level with the root");
        assert!(sx == fx && sy > fy, "the next one below it");
        assert!(lx < rx, "left");
        // The new node is the edge's tail, the node it was added to the head.
        let edge = s.doc.get(first["edge"].as_str().unwrap()).unwrap();
        assert_eq!(edge.ref_id("tail"), first["view"].as_str());
        assert_eq!(edge.ref_id("head"), root["view"].as_str());
        // One undo takes away one node and its edge.
        run(&mut s, "edit.undo", json!({}));
        assert!(s.doc.get(left["view"].as_str().unwrap()).is_none());
        assert!(s.doc.get(second["view"].as_str().unwrap()).is_some());
        assert!(
            Registry::default()
                .run(&mut s, "mindmap.add-node", &json!({"side": "up"}))
                .is_err()
        );
    }

    #[test]
    fn related_entities_and_suppressed_columns() {
        let mut s = Session::default();
        run(&mut s, "diagram.add", json!({"type": "ERDDiagram"}));
        let order = run(
            &mut s,
            "node.create",
            json!({"kind": "ERDEntity", "x": 40, "y": 40}),
        );
        let tag = run(
            &mut s,
            "erd.add-related",
            json!({"view": order["view"], "kind": "many-to-many"}),
        );
        let edge = s.doc.get(tag["edge"].as_str().unwrap()).unwrap();
        let rel = s.doc.get(edge.ref_id("model").unwrap()).unwrap();
        for end in ["end1", "end2"] {
            let e = s.doc.get(&rel.children(end)[0]).unwrap();
            assert_eq!(e.str("cardinality"), Some("0..*"), "{end}");
        }
        assert!(left_top(&s, &tag["view"]).0 > left_top(&s, &order["view"]).0);

        for name in ["id", "total"] {
            run(
                &mut s,
                "feature.add",
                json!({"classifier": order["model"], "kind": "column", "name": name}),
            );
        }
        let view = order["view"].as_str().unwrap().to_owned();
        let height = |s: &Session| s.doc.get(&view).unwrap().num("height", 0.0);
        let tall = height(&s);
        run(&mut s, "edit.select", json!({"ids": [view]}));
        let r = run(&mut s, "view.suppress-columns", json!({}));
        assert_eq!(r["suppressed"], true);
        assert!(height(&s) < tall, "shrinks to its name");
        run(&mut s, "view.suppress-columns", json!({}));
        assert!(s.doc.get(&view).unwrap().get("suppressColumns").is_none());
        assert_eq!(height(&s), tall, "grows back");
    }
}
