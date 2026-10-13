use std::collections::HashSet;

use mdj::{Editor, Field, Id};

/// References that tie a view to another view it can't live without:
/// edge ends, label hosts, and the box a view sits inside.
const VIEW_DEPENDENCIES: &[&str] = &["head", "tail", "hostEdge", "hostNode", "containerView"];

/// Delete views only (StarUML's "Delete"), plus edges left without an end and
/// boxes that sat inside a deleted box.
pub fn delete_views(e: &mut Editor, views: &[Id]) {
    let mut gone: HashSet<Id> = views.iter().cloned().collect();
    loop {
        let before = gone.len();
        for el in e.doc().iter() {
            let dangling = VIEW_DEPENDENCIES
                .iter()
                .any(|k| el.ref_id(k).is_some_and(|r| gone.contains(r)));
            if dangling {
                gone.insert(el.id.clone());
            }
        }
        if gone.len() == before {
            break;
        }
    }
    remove_set(e, gone);
}

/// Delete model elements (StarUML's "Delete from Model"): their subtrees,
/// every view showing them, relationships that would lose an end, and views
/// that would lose an end, host or container. Remaining references to deleted
/// elements are cleared; a deleted type reference becomes its plain name.
pub fn delete_elements(e: &mut Editor, models: &[Id]) {
    let doc = e.doc();
    let mut gone: HashSet<Id> = models.iter().flat_map(|m| doc.subtree_ids(m)).collect();
    loop {
        let before = gone.len();
        let mut add = Vec::new();
        for el in doc.iter() {
            if gone.contains(&el.id) {
                continue;
            }
            let hit = |k: &str| el.ref_id(k).is_some_and(|r| gone.contains(r));
            let loses_end = ["end1", "end2"].iter().any(|k| {
                el.children(k)
                    .first()
                    .and_then(|end| doc.get(end))
                    .and_then(|end| end.ref_id("reference"))
                    .is_some_and(|r| gone.contains(r))
            });
            if hit("model")
                || VIEW_DEPENDENCIES.iter().any(|k| hit(k))
                || hit("source")
                || hit("target")
                || loses_end
            {
                add.extend(doc.subtree_ids(&el.id));
            }
        }
        gone.extend(add);
        if gone.len() == before {
            break;
        }
    }
    remove_set(e, gone);
}

/// Remove every element in `gone` (top-most ones only; descendants go with
/// them) and clean references to them from the elements that remain.
fn remove_set(e: &mut Editor, gone: HashSet<Id>) {
    let doc = e.doc();
    let mut fixes: Vec<(Id, String, Option<Field>)> = Vec::new();
    for el in doc.iter().filter(|el| !gone.contains(&el.id)) {
        for (key, field) in &el.fields {
            match field {
                Field::Ref(r) if gone.contains(r) => {
                    // A typed element keeps the deleted type's name as text.
                    let replacement = (key == "type" || key == "classifier")
                        .then(|| doc.get(r).map(|t| Field::str(t.name())))
                        .flatten();
                    fixes.push((el.id.clone(), key.clone(), replacement));
                }
                Field::Refs(rs) if rs.iter().any(|r| gone.contains(r)) => {
                    let kept: Vec<Id> = rs.iter().filter(|r| !gone.contains(*r)).cloned().collect();
                    fixes.push((el.id.clone(), key.clone(), Some(Field::Refs(kept))));
                }
                _ => {}
            }
        }
    }
    let mut roots: Vec<Id> = gone
        .iter()
        .filter(|id| {
            doc.get(id)
                .and_then(|el| el.parent.as_ref())
                .is_none_or(|p| !gone.contains(p))
        })
        .cloned()
        .collect();
    roots.sort(); // deterministic order for undo
    for (id, key, value) in fixes {
        match value {
            Some(v) => e.set(&id, &key, v),
            None => e.unset(&id, &key),
        }
    }
    for id in roots {
        e.remove(&id);
    }
}
