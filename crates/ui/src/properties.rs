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
const MESSAGE_SORT: &[&str] = &[
    "synchCall",
    "asynchCall",
    "asynchSignal",
    "createMessage",
    "deleteMessage",
    "reply",
];
const INTERACTION_OPERATOR: &[&str] = &[
    "alt", "opt", "par", "loop", "critical", "neg", "assert", "strict", "seq", "ignore",
    "consider", "break",
];
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
/// An ERD relationship end's cardinality (StarUML's options; `1` is the default).
const CARDINALITY: &[&str] = &["0..1", "1", "0..*", "1..*"];
/// A C4 container's kind (`C4ContainerKind`; new ones are `etc`).
const C4_CONTAINER_KIND: &[&str] = &[
    "server-webapp",
    "client-webapp",
    "desktop-app",
    "mobile-app",
    "console-app",
    "serverless-function",
    "database",
    "blob-store",
    "filesystem",
    "shell-script",
    "etc",
];

#[derive(Default)]
pub struct Properties {
    /// Text being edited, per (element, field). Committed on Enter or when
    /// focus leaves, so one edit is one undo step.
    buffers: HashMap<(String, String), String>,
    /// The element shown last frame; when it changes, other elements'
    /// buffers are dropped (their edits were committed as focus left them).
    shown: Option<String>,
    /// Put the cursor in the name field on the next frame.
    pub focus_name: bool,
    /// What the Azure icon picker is filtered by.
    icon_search: String,
}

/// Icons the picker lists at once.
const ICON_MATCHES: usize = 40;

impl Properties {
    /// The elements with text buffers (for tests).
    #[cfg(test)]
    pub(crate) fn buffered(&self) -> std::collections::HashSet<&str> {
        self.buffers.keys().map(|(id, _)| id.as_str()).collect()
    }

    pub fn show(&mut self, ui: &mut egui::Ui, s: &Session) -> Commands {
        let mut out = Vec::new();
        let Some(el) = s.selected_model() else {
            ui.weak("Nothing selected.");
            self.buffers.clear();
            self.shown = None;
            return out;
        };
        if self.shown.as_deref() != Some(el.id.as_str()) {
            // Keep only the new element's own fields and its parts' (ends,
            // parameters), which this panel edits too.
            let doc = &s.doc;
            self.buffers.retain(|(id, _), _| {
                id == &el.id
                    || doc
                        .get(id)
                        .is_some_and(|e| e.parent.as_deref() == Some(el.id.as_str()))
            });
            self.shown = Some(el.id.clone());
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let t = el.type_name.as_str();
                ui.heading(
                    ["UML", "ERD", "FC", "MM", "DFD", "C4", "Azure"]
                        .iter()
                        .find_map(|p| t.strip_prefix(p))
                        .unwrap_or(t)
                        .trim_end_matches("View"),
                );
                ui.add_space(4.0);
                quick_edits(ui, s, &mut out);
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
                if el.type_name == "AzureService" {
                    self.icon_picker(ui, s, el, &mut out);
                }
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
        if t == "ERDRelationship" {
            check(ui, el, "identifying", "Identifying", true, out);
            ui.end_row();
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
                self.field(ui, end, "Name", "name", end.name(), out);
                ui.label("Cardinality");
                combo(ui, end, "cardinality", CARDINALITY, "1", out);
                ui.end_row();
            }
        } else if el.get("end1").is_some() {
            // Relationships with two ends (associations, connectors, links, paths).
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
            "UMLMessage" => {
                ui.label("Sort");
                combo(ui, el, "messageSort", MESSAGE_SORT, "synchCall", out);
                ui.end_row();
                for (label, key) in [
                    ("Arguments", "arguments"),
                    ("Guard", "guard"),
                    ("Sequence number", "sequenceNumber"),
                    ("Assign to", "assignmentTarget"),
                ] {
                    self.field(ui, el, label, key, el.str(key).unwrap_or(""), out);
                }
            }
            "UMLLifeline" => {
                self.field(
                    ui,
                    el,
                    "Selector",
                    "selector",
                    el.str("selector").unwrap_or(""),
                    out,
                );
                // The lifeline's type is its role's type.
                if let Some(role) = el.ref_id("represent").and_then(|r| doc.get(r)) {
                    self.field(
                        ui,
                        role,
                        "Type",
                        "type",
                        uml::var_text(doc, role, "type"),
                        out,
                    );
                }
            }
            "UMLCombinedFragment" => {
                ui.label("Operator");
                combo(
                    ui,
                    el,
                    "interactionOperator",
                    INTERACTION_OPERATOR,
                    "alt",
                    out,
                );
                ui.end_row();
            }
            "UMLInteractionOperand" => {
                self.field(ui, el, "Guard", "guard", el.str("guard").unwrap_or(""), out);
            }
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
            "AzureService" => {
                self.field(ui, el, "Icon", "icon", el.str("icon").unwrap_or(""), out);
            }
            "AzureGroup" => {
                check(ui, el, "dashed", "Dashed", false, out);
                ui.end_row();
                check(ui, el, "rounded", "Rounded", false, out);
                ui.end_row();
            }
            "AzureConnector" => {
                check(ui, el, "dashed", "Dashed", false, out);
                ui.end_row();
            }
            "C4Person" | "C4SoftwareSystem" | "C4Container" | "C4Component" | "C4Element"
            | "C4Model" | "C4Relationship" => {
                if t == "C4Container" {
                    ui.label("Kind");
                    combo(ui, el, "kind", C4_CONTAINER_KIND, "etc", out);
                    ui.end_row();
                }
                // The description is edited below the grid, full width.
                self.field(
                    ui,
                    el,
                    "Technology",
                    "technology",
                    el.str("technology").unwrap_or(""),
                    out,
                );
            }
            // Shown on the shape, e.g. `1.2` for a process.
            "DFDExternalEntity" | "DFDProcess" | "DFDDataStore" => {
                self.field(ui, el, "ID", "id", el.str("id").unwrap_or(""), out);
            }
            "ERDColumn" => {
                self.field(ui, el, "Type", "type", el.str("type").unwrap_or(""), out);
                // Text in StarUML's files, though its column editor writes numbers.
                let length = match el.str("length") {
                    Some(l) => l.to_owned(),
                    None => match el.num("length", 0.0) {
                        n if n != 0.0 => n.to_string(),
                        _ => String::new(),
                    },
                };
                self.field(ui, el, "Length", "length", &length, out);
                for (k, l) in [
                    ("primaryKey", "Primary key"),
                    ("foreignKey", "Foreign key"),
                    ("nullable", "Nullable"),
                    ("unique", "Unique"),
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
        // C4 elements and relationships: shown on the box or line. (A
        // multi-line box inside the grid above would re-measure it every frame.)
        if t.starts_with("C4") {
            ui.add_space(8.0);
            ui.label("Description");
            self.text(
                ui,
                el,
                "description",
                el.str("description").unwrap_or(""),
                true,
                out,
            );
        }
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
        if t == "UMLCombinedFragment" {
            sections.push(("Operands", "operands", "operand"));
        }
        if t == "ERDEntity" {
            sections.push(("Columns", "columns", "column"));
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
                    "operand" => format!("[{}]", f.str("guard").unwrap_or("")),
                    "column" => {
                        let keys = uml::column_keys(f);
                        let ty = uml::column_type(f);
                        let mut text = format!("{keys} {}", f.name()).trim().to_owned();
                        if !ty.is_empty() {
                            text += &format!(": {ty}");
                        }
                        text
                    }
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

    /// Choose an Azure service's icon from the icon folder, by name. As in
    /// StarUML, the service is renamed after the icon.
    fn icon_picker(&mut self, ui: &mut egui::Ui, s: &Session, el: &Element, out: &mut Commands) {
        let names = s.icons().names("azure");
        ui.add_space(8.0);
        ui.strong("Choose icon");
        if names.is_empty() {
            ui.weak(
                "No icon folder: download Microsoft's Azure icons and choose the folder \
                 with View ▸ Azure Icons Folder… (rust-uml doesn't ship them).",
            );
            return;
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.icon_search)
                .hint_text(format!("Search {} icons", names.len()))
                .desired_width(f32::INFINITY),
        );
        let wanted = self.icon_search.to_lowercase();
        let current = el.str("icon").unwrap_or("");
        let matches: Vec<&String> = names
            .iter()
            .filter(|n| n.to_lowercase().contains(&wanted))
            .take(ICON_MATCHES)
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("icons")
            .max_height(200.0)
            .show(ui, |ui| {
                for name in matches {
                    let title = render::icon_title(name);
                    if ui
                        .selectable_label(name.eq_ignore_ascii_case(current), &title)
                        .clicked()
                    {
                        out.push((
                            "element.set",
                            json!({ "id": el.id, "values": { "icon": name, "name": title } }),
                        ));
                    }
                }
            });
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

/// StarUML's quick-edit buttons for the selected box: add a mind map node
/// beside it, or an entity related to it.
fn quick_edits(ui: &mut egui::Ui, s: &Session, out: &mut Commands) {
    let Some(view) = s.selection.first().and_then(|id| s.doc.get(id)) else {
        return;
    };
    let buttons: &[(&str, &str, Value)] = match view.type_name.as_str() {
        "MMNodeView" => &[
            ("Add node left", "mindmap.add-node", json!({"side": "left"})),
            (
                "Add node right",
                "mindmap.add-node",
                json!({"side": "right"}),
            ),
        ],
        "ERDEntityView" => &[
            (
                "+ One to one",
                "erd.add-related",
                json!({"kind": "one-to-one"}),
            ),
            (
                "+ One to many",
                "erd.add-related",
                json!({"kind": "one-to-many"}),
            ),
            (
                "+ Many to many",
                "erd.add-related",
                json!({"kind": "many-to-many"}),
            ),
        ],
        _ => return,
    };
    ui.horizontal_wrapped(|ui| {
        for (label, command, params) in buttons {
            if ui.button(*label).clicked() {
                let mut params = params.clone();
                params["view"] = json!(view.id);
                out.push((command, params));
            }
        }
    });
    ui.add_space(4.0);
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
