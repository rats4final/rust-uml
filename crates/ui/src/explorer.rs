//! The model explorer: the project tree. Clicking selects an element;
//! double-clicking a diagram opens it.

use egui::collapsing_header::CollapsingState;
use engine::Session;
use mdj::Element;
use serde_json::{Value, json};

/// Child lists shown in the tree. Views are never shown.
const CHILD_FIELDS: &[&str] = &[
    "ownedElements",
    "attributes",
    "operations",
    "literals",
    "slots",
];
const MAX_DEPTH: usize = 64;

pub fn show(ui: &mut egui::Ui, session: &Session) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        node(ui, session, session.doc.root(), 0, &mut out);
    });
    out
}

fn short_type(t: &str) -> &str {
    t.strip_prefix("UML").unwrap_or(t)
}

fn label(el: &Element) -> String {
    let name = el.name();
    let kind = short_type(&el.type_name);
    if name.is_empty() {
        format!("({kind})")
    } else {
        format!("{name}  ·  {kind}")
    }
}

fn node(
    ui: &mut egui::Ui,
    s: &Session,
    id: &str,
    depth: usize,
    out: &mut Vec<(&'static str, Value)>,
) {
    let Some(el) = s.doc.get(id) else { return };
    let children: Vec<&str> = CHILD_FIELDS
        .iter()
        .flat_map(|f| el.children(f))
        .filter(|c| s.doc.get(c).is_some_and(|c| !c.type_name.ends_with("View")))
        .map(String::as_str)
        .collect();
    let selected = s.selection.iter().any(|x| x == id);
    let active = s.active_diagram.as_deref() == Some(id);
    let mut text = egui::RichText::new(label(el));
    if active {
        text = text.strong();
    }

    let mut row = |ui: &mut egui::Ui| {
        let r = ui.selectable_label(selected, text.clone());
        if r.clicked() {
            out.push(("edit.select", json!({ "ids": [id] })));
        }
        if r.double_clicked() && el.type_name.ends_with("Diagram") {
            out.push(("diagram.open", json!({ "id": id })));
        }
    };

    if children.is_empty() || depth >= MAX_DEPTH {
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            row(ui);
        });
        return;
    }
    let state =
        CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(id), depth < 2);
    state.show_header(ui, |ui| row(ui)).body(|ui| {
        for c in children {
            node(ui, s, c, depth + 1, out);
        }
    });
}
