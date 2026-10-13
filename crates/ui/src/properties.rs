//! The property editor for the selected element.

use std::collections::HashMap;

use engine::Session;
use mdj::{Document, Element};
use serde_json::{Value, json};

type Commands = Vec<(&'static str, Value)>;

const CLASSIFIERS: &[&str] = &[
    "UMLClass",
    "UMLInterface",
    "UMLDataType",
    "UMLEnumeration",
    "UMLSignal",
    "UMLPrimitiveType",
    "UMLComponent",
    "UMLArtifact",
    "UMLNode",
    "UMLStereotype",
    "UMLCollaboration",
];
/// Instances: they have a classifier and slots.
const INSTANCES: &[&str] = &[
    "UMLObject",
    "UMLArtifactInstance",
    "UMLComponentInstance",
    "UMLNodeInstance",
];
const CONNECTOR_KIND: &[&str] = &["assembly", "delegation"];
const PSEUDOSTATE_KIND: &[&str] = &[
    "initial",
    "deepHistory",
    "shallowHistory",
    "join",
    "fork",
    "junction",
    "choice",
    "entryPoint",
    "exitPoint",
    "terminate",
];
const ACTION_KIND: &[&str] = &[
    "opaque",
    "create",
    "destroy",
    "read",
    "write",
    "insert",
    "delete",
    "sendSignal",
    "acceptSignal",
    "triggerEvent",
    "acceptEvent",
    "structured",
    "timeEvent",
    "callBehavior",
    "callOperation",
];
const VISIBILITY: &[&str] = &["public", "protected", "private", "package"];
const DIRECTION: &[&str] = &["in", "inout", "out", "return"];
const NAVIGABLE: &[&str] = &["unspecified", "navigable", "notNavigable"];
const AGGREGATION: &[&str] = &["none", "shared", "composite"];

#[derive(Default)]
pub struct Properties {
    /// Text being edited, per (element, field). Committed on Enter or when
    /// focus leaves, so one edit is one undo step.
    buffers: HashMap<(String, String), String>,
    /// Put the cursor in the name field on the next frame.
    pub focus_name: bool,
}

impl Properties {
    pub fn show(&mut self, ui: &mut egui::Ui, s: &Session) -> Commands {
        let mut out = Vec::new();
        let Some(el) = s.selected_model() else {
            ui.weak("Nothing selected.");
            return out;
        };
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.heading(
                    el.type_name
                        .strip_prefix("UML")
                        .unwrap_or(&el.type_name)
                        .trim_end_matches("View"),
                );
                ui.add_space(4.0);
                if el.type_name.ends_with("View") {
                    // A shape without a model element (note, text): its own text.
                    ui.label("Text");
                    self.text(ui, el, "text", el.str("text").unwrap_or(""), true, &mut out);
                    return;
                }
                egui::Grid::new("props")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        self.common(ui, &s.doc, el, &mut out);
                        self.specific(ui, &s.doc, el, &mut out);
                    });
                self.lists(ui, &s.doc, el, &mut out);
                ui.add_space(8.0);
                ui.label("Documentation");
                self.text(
                    ui,
                    el,
                    "documentation",
                    el.str("documentation").unwrap_or(""),
                    true,
                    &mut out,
                );
            });
        out
    }

    fn common(&mut self, ui: &mut egui::Ui, doc: &Document, el: &Element, out: &mut Commands) {
        ui.label("Name");
        let focus = std::mem::take(&mut self.focus_name);
        let r = self.text(ui, el, "name", el.name(), false, out);
        if focus {
            r.request_focus();
        }
        ui.end_row();
        let is_uml = el.type_name.starts_with("UML") && !el.type_name.ends_with("Diagram");
        if is_uml {
            ui.label("Stereotype");
            self.text(
                ui,
                el,
                "stereotype",
                uml::var_text(doc, el, "stereotype"),
                false,
                out,
            );
            ui.end_row();
            ui.label("Visibility");
            combo(ui, el, "visibility", VISIBILITY, "public", out);
            ui.end_row();
        }
    }

    fn specific(&mut self, ui: &mut egui::Ui, doc: &Document, el: &Element, out: &mut Commands) {
        let t = el.type_name.as_str();
        if CLASSIFIERS.contains(&t) {
            check(ui, el, "isAbstract", "Abstract", false, out);
            ui.end_row();
            check(ui, el, "isLeaf", "Leaf", false, out);
            ui.end_row();
        }
        if t == "UMLClass" {
            check(ui, el, "isActive", "Active", false, out);
            ui.end_row();
        }
        if INSTANCES.contains(&t) {
            self.field(
                ui,
                el,
                "Classifier",
                "classifier",
                uml::var_text(doc, el, "classifier"),
                out,
            );
            self.field(ui, el, "Value", "value", el.str("value").unwrap_or(""), out);
        }
        // Relationships with two ends (associations, connectors, links, paths).
        if el.get("end1").is_some() {
            if t == "UMLConnector" {
                ui.label("Kind");
                combo(ui, el, "kind", CONNECTOR_KIND, "assembly", out);
                ui.end_row();
            }
            for (key, title) in [("end1", "End 1 (tail)"), ("end2", "End 2 (head)")] {
                let Some(end) = el.children(key).first().and_then(|id| doc.get(id)) else {
                    continue;
                };
                let target = end
                    .ref_id("reference")
                    .and_then(|r| doc.get(r))
                    .map(Element::name)
                    .unwrap_or("?");
                ui.strong(title);
                ui.weak(target);
                ui.end_row();
                self.field(ui, end, "Role", "name", end.name(), out);
                self.field(
                    ui,
                    end,
                    "Multiplicity",
                    "multiplicity",
                    end.str("multiplicity").unwrap_or(""),
                    out,
                );
                ui.label("Navigable");
                combo(ui, end, "navigable", NAVIGABLE, "unspecified", out);
                ui.end_row();
                ui.label("Aggregation");
                combo(ui, end, "aggregation", AGGREGATION, "none", out);
                ui.end_row();
            }
        }
        match t {
            "UMLTransition" => {
                self.field(ui, el, "Guard", "guard", el.str("guard").unwrap_or(""), out);
            }
            "UMLControlFlow" | "UMLObjectFlow" => {
                self.field(ui, el, "Guard", "guard", el.str("guard").unwrap_or(""), out);
                self.field(
                    ui,
                    el,
                    "Weight",
                    "weight",
                    el.str("weight").unwrap_or(""),
                    out,
                );
            }
            "UMLPseudostate" => {
                ui.label("Kind");
                combo(ui, el, "kind", PSEUDOSTATE_KIND, "initial", out);
                ui.end_row();
            }
            "UMLAction" => {
                ui.label("Kind");
                combo(ui, el, "kind", ACTION_KIND, "opaque", out);
                ui.end_row();
            }
            "UMLObjectNode" => {
                self.field(ui, el, "Type", "type", uml::var_text(doc, el, "type"), out);
            }
            "UMLSlot" => {
                self.field(ui, el, "Value", "value", el.str("value").unwrap_or(""), out);
            }
            "UMLArtifact" => {
                self.field(
                    ui,
                    el,
                    "File name",
                    "fileName",
                    el.str("fileName").unwrap_or(""),
                    out,
                );
            }
            "UMLComponent" => {
                // The metamodel default is true.
                check(
                    ui,
                    el,
                    "isIndirectlyInstantiated",
                    "Indirectly instantiated",
                    true,
                    out,
                );
                ui.end_row();
            }
            "UMLModel" => {
                self.field(
                    ui,
                    el,
                    "Viewpoint",
                    "viewpoint",
                    el.str("viewpoint").unwrap_or(""),
                    out,
                );
            }
            "UMLCollaborationUse" => {
                self.field(ui, el, "Type", "type", uml::var_text(doc, el, "type"), out);
            }
            "UMLPort" => {
                self.field(ui, el, "Type", "type", uml::var_text(doc, el, "type"), out);
                self.field(
                    ui,
                    el,
                    "Multiplicity",
                    "multiplicity",
                    el.str("multiplicity").unwrap_or(""),
                    out,
                );
                for (k, l) in [
                    ("isBehavior", "Behavior"),
                    ("isService", "Service"),
                    ("isConjugated", "Conjugated"),
                ] {
                    check(ui, el, k, l, false, out);
                    ui.end_row();
                }
            }
            "UMLAttribute" => {
                self.field(ui, el, "Type", "type", uml::var_text(doc, el, "type"), out);
                self.field(
                    ui,
                    el,
                    "Multiplicity",
                    "multiplicity",
                    el.str("multiplicity").unwrap_or(""),
                    out,
                );
                self.field(
                    ui,
                    el,
                    "Default",
                    "defaultValue",
                    el.str("defaultValue").unwrap_or(""),
                    out,
                );
                for (k, l) in [
                    ("isStatic", "Static"),
                    ("isReadOnly", "Read only"),
                    ("isDerived", "Derived"),
                ] {
                    check(ui, el, k, l, false, out);
                    ui.end_row();
                }
            }
            "UMLOperation" => {
                ui.label("Returns");
                match uml::return_parameter(doc, el) {
                    Some(ret) => {
                        self.text(ui, ret, "type", uml::var_text(doc, ret, "type"), false, out);
                    }
                    None => {
                        if ui.button("Add return type").clicked() {
                            out.push((
                                "parameter.add",
                                json!({ "operation": el.id, "direction": "return" }),
                            ));
                        }
                    }
                }
                ui.end_row();
                for (k, l) in [
                    ("isStatic", "Static"),
                    ("isAbstract", "Abstract"),
                    ("isQuery", "Query"),
                ] {
                    check(ui, el, k, l, false, out);
                    ui.end_row();
                }
            }
            "UMLParameter" => {
                self.field(ui, el, "Type", "type", uml::var_text(doc, el, "type"), out);
                ui.label("Direction");
                combo(ui, el, "direction", DIRECTION, "in", out);
                ui.end_row();
            }
            _ => {}
        }
    }

    fn lists(&mut self, ui: &mut egui::Ui, doc: &Document, el: &Element, out: &mut Commands) {
        let t = el.type_name.as_str();
        let mut sections: Vec<(&str, &str, &str)> = Vec::new();
        if t == "UMLEnumeration" {
            sections.push(("Literals", "literals", "literal"));
        }
        if CLASSIFIERS.contains(&t) {
            sections.push(("Attributes", "attributes", "attribute"));
            sections.push(("Operations", "operations", "operation"));
        }
        if INSTANCES.contains(&t) {
            sections.push(("Slots", "slots", "slot"));
        }
        for (title, key, kind) in sections {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.strong(title);
                if ui
                    .small_button("+")
                    .on_hover_text(format!("Add {kind}"))
                    .clicked()
                {
                    out.push(("feature.add", json!({ "classifier": el.id, "kind": kind })));
                }
            });
            for id in el.children(key) {
                let Some(f) = doc.get(id) else { continue };
                let text = match kind {
                    "attribute" => uml::attribute_text(doc, f),
                    "operation" => uml::operation_text(doc, f),
                    "slot" => uml::slot_text(doc, f),
                    _ => f.name().to_owned(),
                };
                feature_row(ui, id, &text, out);
            }
        }
        if t == "UMLOperation" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.strong("Parameters");
                if ui
                    .small_button("+")
                    .on_hover_text("Add parameter")
                    .clicked()
                {
                    out.push(("parameter.add", json!({ "operation": el.id })));
                }
            });
            for id in el.children("parameters") {
                let Some(p) = doc.get(id) else { continue };
                if p.str("direction") == Some("return") {
                    continue;
                }
                ui.horizontal(|ui| {
                    self.text(ui, p, "name", p.name(), false, out);
                    ui.label(":");
                    self.text(ui, p, "type", uml::var_text(doc, p, "type"), false, out);
                    if ui.small_button("✕").on_hover_text("Remove").clicked() {
                        out.push(("element.delete", json!({ "ids": [id] })));
                    }
                });
            }
        }
    }

    fn field(
        &mut self,
        ui: &mut egui::Ui,
        el: &Element,
        label: &str,
        key: &str,
        current: &str,
        out: &mut Commands,
    ) {
        ui.label(label);
        self.text(ui, el, key, current, false, out);
        ui.end_row();
    }

    /// A text box bound to `el.key`; emits `element.set` when the edit is committed.
    fn text(
        &mut self,
        ui: &mut egui::Ui,
        el: &Element,
        key: &str,
        current: &str,
        multiline: bool,
        out: &mut Commands,
    ) -> egui::Response {
        let buf = self
            .buffers
            .entry((el.id.clone(), key.to_owned()))
            .or_insert_with(|| current.to_owned());
        let widget = if multiline {
            egui::TextEdit::multiline(buf)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
        } else {
            egui::TextEdit::singleline(buf).desired_width(160.0)
        };
        let r = ui.add(widget);
        if r.lost_focus() {
            if buf.as_str() != current {
                out.push((
                    "element.set",
                    json!({ "id": el.id, "key": key, "value": buf.clone() }),
                ));
            }
        } else if !r.has_focus() && buf.as_str() != current {
            // Not being edited: follow the model (undo, other panels, …).
            *buf = current.to_owned();
        }
        r
    }
}

fn feature_row(ui: &mut egui::Ui, id: &str, text: &str, out: &mut Commands) {
    ui.horizontal(|ui| {
        if ui.selectable_label(false, text).clicked() {
            out.push(("edit.select", json!({ "ids": [id] })));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("✕").on_hover_text("Remove").clicked() {
                out.push(("element.delete", json!({ "ids": [id] })));
            }
            if ui.small_button("↓").on_hover_text("Move down").clicked() {
                out.push(("feature.move", json!({ "id": id, "delta": 1 })));
            }
            if ui.small_button("↑").on_hover_text("Move up").clicked() {
                out.push(("feature.move", json!({ "id": id, "delta": -1 })));
            }
        });
    });
}

/// A checkbox bound to `el.key`; `default` is the metamodel default used when
/// the field is missing (StarUML's rule, see docs/evidence.md §3).
fn check(
    ui: &mut egui::Ui,
    el: &Element,
    key: &str,
    label: &str,
    default: bool,
    out: &mut Commands,
) {
    ui.label("");
    let mut v = el.bool(key, default);
    if ui.checkbox(&mut v, label).changed() {
        out.push((
            "element.set",
            json!({ "id": el.id, "key": key, "value": v }),
        ));
    }
}

fn combo(
    ui: &mut egui::Ui,
    el: &Element,
    key: &str,
    options: &[&str],
    default: &str,
    out: &mut Commands,
) {
    let current = el.str(key).unwrap_or(default);
    egui::ComboBox::from_id_salt((&el.id, key))
        .selected_text(current)
        .show_ui(ui, |ui| {
            for o in options {
                if ui.selectable_label(current == *o, *o).clicked() && current != *o {
                    out.push((
                        "element.set",
                        json!({ "id": el.id, "key": key, "value": o }),
                    ));
                }
            }
        });
}
