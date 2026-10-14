//! `model.validate`: check the model against rules, as StarUML's
//! Model ▸ Validate does. Rules come from StarUML's extensions (evidence:
//! `extensions/essential/erd/rules.js`, behaviour only).

use mdj::{Document, Element};
use serde_json::{Value, json};

use crate::commands::CommandError;
use crate::session::Session;

struct Rule {
    id: &'static str,
    message: &'static str,
    applies_to: &'static str,
    ok: fn(&Document, &Element) -> bool,
}

const RULES: &[Rule] = &[
    Rule {
        id: "ERD001",
        message: "Primary key cannot be nullable.",
        applies_to: "ERDColumn",
        ok: |_, c| !(c.bool("primaryKey", false) && c.bool("nullable", false)),
    },
    Rule {
        id: "ERD002",
        message: "No relationship with the foreign key reference.",
        applies_to: "ERDColumn",
        ok: foreign_key_is_related,
    },
];

/// A foreign key's referenced column belongs to an entity this column's
/// entity has a relationship with.
fn foreign_key_is_related(doc: &Document, column: &Element) -> bool {
    if !column.bool("foreignKey", false) {
        return true;
    }
    let Some(referenced) = column.ref_id("referenceTo").and_then(|r| doc.get(r)) else {
        return true;
    };
    let (Some(entity), Some(other)) = (column.parent.as_deref(), referenced.parent.as_deref())
    else {
        return true;
    };
    let reference = |rel: &Element, key: &str| {
        rel.children(key)
            .first()
            .and_then(|e| doc.get(e))
            .and_then(|e| e.ref_id("reference"))
            .map(str::to_owned)
    };
    doc.iter()
        .filter(|e| e.type_name == "ERDRelationship")
        .any(|rel| {
            let (a, b) = (reference(rel, "end1"), reference(rel, "end2"));
            (a.as_deref() == Some(entity) && b.as_deref() == Some(other))
                || (b.as_deref() == Some(entity) && a.as_deref() == Some(other))
        })
}

pub(crate) fn validate(s: &mut Session, _: &Value) -> Result<Value, CommandError> {
    let doc = &s.doc;
    // Tree order, so the list reads like the model explorer.
    let problems: Vec<Value> = doc
        .subtree_ids(doc.root())
        .iter()
        .filter_map(|id| doc.get(id))
        .flat_map(|el| {
            RULES
                .iter()
                .filter(|r| r.applies_to == el.type_name && !(r.ok)(doc, el))
                .map(|r| {
                    let owner = el.parent.as_deref().and_then(|p| doc.get(p));
                    let name = match owner {
                        Some(o) if !o.name().is_empty() => format!("{}.{}", o.name(), el.name()),
                        _ => el.name().to_owned(),
                    };
                    json!({ "id": el.id, "name": name, "rule": r.id, "message": r.message })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    Ok(json!({ "problems": problems }))
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

    fn rules(s: &mut Session) -> Vec<String> {
        run(s, "model.validate", json!({}))["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                format!(
                    "{} {}",
                    p["rule"].as_str().unwrap(),
                    p["name"].as_str().unwrap()
                )
            })
            .collect()
    }

    #[test]
    fn erd_rules() {
        let mut s = Session::default();
        assert!(rules(&mut s).is_empty());
        run(&mut s, "diagram.add", json!({"type": "ERDDiagram"}));
        let entity = |s: &mut Session, name: &str, x: i64| {
            run(
                s,
                "node.create",
                json!({"kind": "ERDEntity", "x": x, "y": 40, "name": name}),
            )
        };
        let (order, customer) = (entity(&mut s, "Order", 40), entity(&mut s, "Customer", 400));
        let column = |s: &mut Session, owner: &Value, name: &str| {
            run(
                s,
                "feature.add",
                json!({"classifier": owner["model"], "kind": "column", "name": name}),
            )["id"]
                .clone()
        };
        let id = column(&mut s, &order, "id");
        run(
            &mut s,
            "element.set",
            json!({"id": id, "values": {"primaryKey": true, "nullable": true}}),
        );
        let cid = column(&mut s, &customer, "id");
        let fk = column(&mut s, &order, "customer_id");
        run(
            &mut s,
            "element.set",
            json!({"id": fk, "values": {"foreignKey": true, "referenceTo": {"$ref": cid}}}),
        );
        assert_eq!(
            rules(&mut s),
            ["ERD001 Order.id", "ERD002 Order.customer_id"]
        );
        run(
            &mut s,
            "edge.create",
            json!({"kind": "ERDRelationshipOneToMany", "tail": customer["view"], "head": order["view"]}),
        );
        assert_eq!(rules(&mut s), ["ERD001 Order.id"]);
    }
}
