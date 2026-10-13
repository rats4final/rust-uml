use std::collections::HashSet;

use base64::Engine as _;
use mdj::{Document, Element, Field, Id};
use serde_json::{Value, json};
use uml::FeatureKind;

use crate::params;
use crate::session::{Session, diagram_of, is_diagram};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CommandError {
    #[error("unknown command `{0}`")]
    Unknown(String),
    #[error("`{0}` is not available right now")]
    Disabled(String),
    #[error("invalid parameter: {0}")]
    Param(String),
    #[error("{0}")]
    Failed(String),
}

type Run = fn(&mut Session, &Value) -> Result<Value, CommandError>;

/// One command: what menus, shortcuts, the CLI and automation all call.
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Top-level menu it appears in, if any.
    pub menu: Option<&'static str>,
    /// Display form, e.g. `Ctrl+Z`. The UI maps these to keys.
    pub shortcut: Option<&'static str>,
    /// Human-readable parameter description.
    pub params: &'static str,
    pub enabled: fn(&Session) -> bool,
    run: Run,
}

pub struct Registry {
    specs: Vec<CommandSpec>,
}

impl Default for Registry {
    fn default() -> Self {
        Self { specs: specs() }
    }
}

impl Registry {
    pub fn specs(&self) -> &[CommandSpec] {
        &self.specs
    }

    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.specs.iter().find(|s| s.id == id)
    }

    /// Run command `id` with JSON `params`. Never panics on bad input.
    pub fn run(
        &self,
        session: &mut Session,
        id: &str,
        params: &Value,
    ) -> Result<Value, CommandError> {
        let spec = self
            .get(id)
            .ok_or_else(|| CommandError::Unknown(id.to_owned()))?;
        if !(spec.enabled)(session) {
            return Err(CommandError::Disabled(id.to_owned()));
        }
        let result = (spec.run)(session, params);
        session.prune();
        result
    }
}

fn always(_: &Session) -> bool {
    true
}
fn has_diagram(s: &Session) -> bool {
    s.active_diagram.is_some()
}
fn has_selection(s: &Session) -> bool {
    !s.selection.is_empty()
}

fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "file.new",
            label: "New",
            menu: Some("File"),
            shortcut: Some("Ctrl+N"),
            params: "{}",
            enabled: always,
            run: |s, _| {
                *s = Session::default();
                Ok(Value::Null)
            },
        },
        CommandSpec {
            id: "edit.undo",
            label: "Undo",
            menu: Some("Edit"),
            shortcut: Some("Ctrl+Z"),
            params: "{}",
            enabled: |s| s.doc.can_undo(),
            run: |s, _| Ok(json!(s.doc.undo())),
        },
        CommandSpec {
            id: "edit.redo",
            label: "Redo",
            menu: Some("Edit"),
            shortcut: Some("Ctrl+Y"),
            params: "{}",
            enabled: |s| s.doc.can_redo(),
            run: |s, _| Ok(json!(s.doc.redo())),
        },
        CommandSpec {
            id: "edit.select",
            label: "Select",
            menu: None,
            shortcut: None,
            params: r#"{"ids": [id, …]} — views or model elements; [] clears"#,
            enabled: always,
            run: select,
        },
        CommandSpec {
            id: "edit.delete",
            label: "Delete",
            menu: Some("Edit"),
            shortcut: Some("Delete"),
            params: "{} — selected views leave the diagram; selected model elements are deleted",
            enabled: has_selection,
            run: delete,
        },
        CommandSpec {
            id: "edit.delete-from-model",
            label: "Delete from Model",
            menu: Some("Edit"),
            shortcut: Some("Ctrl+Delete"),
            params: "{} — deletes the selected elements (or the models of selected views) everywhere",
            enabled: has_selection,
            run: delete_from_model,
        },
        CommandSpec {
            id: "element.delete",
            label: "Delete Elements",
            menu: None,
            shortcut: None,
            params: r#"{"ids": [model element id, …]} — like Delete from Model, without changing the selection"#,
            enabled: always,
            run: delete_elements,
        },
        CommandSpec {
            id: "diagram.open",
            label: "Open Diagram",
            menu: None,
            shortcut: None,
            params: r#"{"id": diagram id}"#,
            enabled: always,
            run: open_diagram,
        },
        CommandSpec {
            id: "diagram.add",
            label: "Add Diagram",
            menu: Some("Model"),
            shortcut: None,
            params: r#"{"type": "UMLClassDiagram"|"UMLPackageDiagram"|"UMLObjectDiagram"|"UMLComponentDiagram"|"UMLDeploymentDiagram"|"UMLCompositeStructureDiagram"|"UMLProfileDiagram", "parent"?: model or package id}"#,
            enabled: always,
            run: add_diagram,
        },
        CommandSpec {
            id: "diagram.add-class-diagram",
            label: "Add Class Diagram",
            menu: None,
            shortcut: None,
            params: r#"{"parent"?: model or package id} — same as diagram.add with type UMLClassDiagram"#,
            enabled: always,
            run: |s, p| {
                let mut p = p.as_object().cloned().unwrap_or_default();
                p.insert("type".into(), json!("UMLClassDiagram"));
                add_diagram(s, &Value::Object(p))
            },
        },
        CommandSpec {
            id: "node.create",
            label: "Create Element",
            menu: None,
            shortcut: None,
            params: r#"{"kind": toolbox id or label (UMLClass, "Port", "Note", … see `rust-uml-cli tools`), "x": n, "y": n, "container"?: box view id to create it inside}"#,
            enabled: has_diagram,
            run: create_node,
        },
        CommandSpec {
            id: "edge.create",
            label: "Create Relationship",
            menu: None,
            shortcut: None,
            params: r#"{"kind": toolbox id or label (UMLAssociation, "Composition", "Containment", "Note Link", …), "tail": view id, "head": view id}"#,
            enabled: has_diagram,
            run: create_edge,
        },
        CommandSpec {
            id: "view.move",
            label: "Move",
            menu: None,
            shortcut: None,
            params: r#"{"views": [node view id, …], "dx": n, "dy": n}"#,
            enabled: has_diagram,
            run: move_views,
        },
        CommandSpec {
            id: "view.resize",
            label: "Resize",
            menu: None,
            shortcut: None,
            params: r#"{"view": node view id, "width": n, "height": n}"#,
            enabled: has_diagram,
            run: resize_view,
        },
        CommandSpec {
            id: "element.set",
            label: "Set Property",
            menu: None,
            shortcut: None,
            params: r#"{"id": element id, "key": field name, "value": string|bool|number|null}"#,
            enabled: always,
            run: set_property,
        },
        CommandSpec {
            id: "feature.add",
            label: "Add Feature",
            menu: None,
            shortcut: None,
            params: r#"{"classifier": id, "kind": "attribute"|"operation"|"literal"|"slot"}"#,
            enabled: always,
            run: add_feature,
        },
        CommandSpec {
            id: "parameter.add",
            label: "Add Parameter",
            menu: None,
            shortcut: None,
            params: r#"{"operation": id, "direction"?: "in"|"inout"|"out"|"return"}"#,
            enabled: always,
            run: add_parameter,
        },
        CommandSpec {
            id: "feature.move",
            label: "Move Feature",
            menu: None,
            shortcut: None,
            params: r#"{"id": attribute/operation/literal id, "delta": -1|1}"#,
            enabled: always,
            run: move_feature,
        },
        CommandSpec {
            id: "model.diagrams",
            label: "List Diagrams",
            menu: None,
            shortcut: None,
            params: "{}",
            enabled: always,
            run: list_diagrams,
        },
        CommandSpec {
            id: "model.get",
            label: "Get Element",
            menu: None,
            shortcut: None,
            params: r#"{"id": element id}"#,
            enabled: always,
            run: get_element,
        },
        CommandSpec {
            id: "export.svg",
            label: "Export Diagram as SVG",
            menu: Some("File"),
            shortcut: None,
            params: r#"{"diagram"?: id} → {"svg": text}"#,
            enabled: has_diagram,
            run: |s, p| {
                let scene = scene_for(s, p)?;
                Ok(json!({ "svg": render::to_svg(&scene) }))
            },
        },
        CommandSpec {
            id: "export.png",
            label: "Export Diagram as PNG",
            menu: Some("File"),
            shortcut: None,
            params: r#"{"diagram"?: id, "scale"?: 0.1–8} → {"png_base64": text}"#,
            enabled: has_diagram,
            run: |s, p| {
                let scale = match p.get("scale") {
                    None | Some(Value::Null) => 2.0,
                    Some(v) => v
                        .as_f64()
                        .filter(|v| (0.1..=8.0).contains(v))
                        .ok_or_else(|| CommandError::Param("`scale` must be 0.1–8".into()))?,
                };
                let scene = scene_for(s, p)?;
                let png = render::to_png(&scene, scale as f32).map_err(CommandError::Failed)?;
                Ok(json!({ "png_base64": base64::engine::general_purpose::STANDARD.encode(png) }))
            },
        },
    ]
}

// ---------------------------------------------------------------------------

fn get<'a>(doc: &'a Document, id: &str) -> Result<&'a Element, CommandError> {
    doc.get(id)
        .ok_or_else(|| CommandError::Param(format!("no element `{id}`")))
}

fn active(s: &Session) -> Result<Id, CommandError> {
    s.active_diagram
        .clone()
        .ok_or_else(|| CommandError::Disabled("no open diagram".into()))
}

/// A top-level node view of the active diagram.
fn node_view<'a>(s: &'a Session, id: &str) -> Result<&'a Element, CommandError> {
    let el = get(&s.doc, id)?;
    let in_diagram = s.active_diagram.as_deref() == el.parent.as_deref();
    if in_diagram && render::node_layout(&s.doc, el, s.fonts()).is_some() {
        Ok(el)
    } else {
        Err(CommandError::Param(format!(
            "`{id}` is not a box on the open diagram"
        )))
    }
}

fn is_view(el: &Element) -> bool {
    el.get("model").is_some() && (el.get("left").is_some() || el.get("head").is_some())
        || el.type_name.ends_with("View")
}

fn scene_for(s: &Session, p: &Value) -> Result<render::Scene, CommandError> {
    let diagram = match params::opt_str(p, "diagram") {
        Some(d) => d.to_owned(),
        None => active(s)?,
    };
    if !is_diagram(get(&s.doc, &diagram)?) {
        return Err(CommandError::Param(format!("`{diagram}` is not a diagram")));
    }
    Ok(render::build_scene(&s.doc, &diagram, s.fonts()))
}

fn select(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let ids = params::ids(p, "ids")?;
    for id in &ids {
        get(&s.doc, id)?;
    }
    s.selection = ids;
    Ok(Value::Null)
}

fn delete(s: &mut Session, _: &Value) -> Result<Value, CommandError> {
    let root = s.doc.root().to_owned();
    let (views, models): (Vec<Id>, Vec<Id>) = s
        .selection
        .iter()
        .filter(|id| **id != root)
        .cloned()
        .partition(|id| s.doc.get(id).is_some_and(is_view));
    s.edit(|e, _| {
        uml::delete_views(e, &views);
        uml::delete_elements(e, &models);
        Ok(())
    })?;
    s.selection.clear();
    Ok(Value::Null)
}

fn delete_from_model(s: &mut Session, _: &Value) -> Result<Value, CommandError> {
    let root = s.doc.root().to_owned();
    let models: Vec<Id> = s
        .selection
        .iter()
        .filter_map(|id| {
            let el = s.doc.get(id)?;
            if is_view(el) {
                el.ref_id("model").map(str::to_owned)
            } else {
                Some(id.clone())
            }
        })
        .filter(|id| *id != root)
        .collect();
    s.edit(|e, _| {
        uml::delete_elements(e, &models);
        Ok(())
    })?;
    s.selection.clear();
    Ok(Value::Null)
}

fn delete_elements(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let ids = params::ids(p, "ids")?;
    let root = s.doc.root().to_owned();
    for id in &ids {
        if get(&s.doc, id).map(is_view)? || *id == root {
            return Err(CommandError::Param(format!(
                "`{id}` is not a deletable model element"
            )));
        }
    }
    s.edit(|e, c| {
        for id in &ids {
            if let Some(parent) = e.doc().get(id).and_then(|el| el.parent.clone()) {
                c.model_changed(e.doc(), &parent);
                if let Some(grand) = e.doc().get(&parent).and_then(|el| el.parent.clone()) {
                    c.model_changed(e.doc(), &grand);
                }
            }
        }
        uml::delete_elements(e, &ids);
        Ok(Value::Null)
    })
}

fn add_parameter(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let op = params::str(p, "operation")?.to_owned();
    let direction = params::opt_str(p, "direction").unwrap_or("in").to_owned();
    if !["in", "inout", "out", "return"].contains(&direction.as_str()) {
        return Err(CommandError::Param(format!(
            "unknown direction `{direction}`"
        )));
    }
    let el = get(&s.doc, &op)?;
    if el.type_name != "UMLOperation" {
        return Err(CommandError::Param(format!("`{op}` is not an operation")));
    }
    if direction == "return" && uml::return_parameter(&s.doc, el).is_some() {
        return Err(CommandError::Failed(
            "the operation already has a return type".into(),
        ));
    }
    s.edit(|e, c| {
        let id = uml::add_parameter(e, &op, &direction);
        if let Some(owner) = e.doc().get(&op).and_then(|o| o.parent.clone()) {
            c.model_changed(e.doc(), &owner);
        }
        Ok(json!({ "id": id }))
    })
}

fn open_diagram(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let id = params::str(p, "id")?;
    if !is_diagram(get(&s.doc, id)?) {
        return Err(CommandError::Param(format!("`{id}` is not a diagram")));
    }
    s.active_diagram = Some(id.to_owned());
    s.selection.clear();
    Ok(Value::Null)
}

fn add_diagram(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let ty = params::str(p, "type")?;
    let Some(&(diagram_type, _, _)) = uml::DIAGRAMS
        .iter()
        .find(|(t, label, _)| *t == ty || label.eq_ignore_ascii_case(ty))
    else {
        return Err(CommandError::Param(format!("unknown diagram type `{ty}`")));
    };
    let parent = match params::opt_str(p, "parent") {
        Some(id) => {
            let el = get(&s.doc, id)?;
            if is_view(el) || is_diagram(el) {
                return Err(CommandError::Param(format!("`{id}` cannot own a diagram")));
            }
            id.to_owned()
        }
        None => match &s.active_diagram {
            Some(d) => uml::owner_for_new_elements(&s.doc, d),
            None => {
                let root = s.doc.root();
                s.doc
                    .get(root)
                    .and_then(|r| r.children("ownedElements").first().cloned())
                    .unwrap_or_else(|| root.to_owned())
            }
        },
    };
    // Statechart/activity/profile diagrams get their state machine, activity
    // or profile, as in StarUML.
    let id =
        s.edit(|e, _| uml::create_diagram(e, &parent, diagram_type).map_err(CommandError::Failed))?;
    s.active_diagram = Some(id.clone());
    s.selection = vec![id.clone()];
    Ok(json!({ "id": id }))
}

fn create_node(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let kind = params::str(p, "kind")?;
    let spec = uml::node_spec(kind)
        .ok_or_else(|| CommandError::Param(format!("unknown kind `{kind}`")))?;
    let (x, y) = (params::coord(p, "x")?, params::coord(p, "y")?);
    let container = match params::opt_str(p, "container") {
        Some(c) => Some(node_view(s, c)?.id.clone()),
        None => None,
    };
    let diagram = active(s)?;
    let (model, view) = s.edit(|e, c| {
        let (m, v) = uml::create_node(e, &diagram, spec, x, y, container.as_deref())
            .map_err(CommandError::Failed)?;
        c.touched.insert(v.clone());
        Ok((m, v))
    })?;
    s.selection = vec![view.clone()];
    Ok(json!({ "model": model, "view": view }))
}

fn create_edge(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let kind = params::str(p, "kind")?;
    let spec = uml::edge_spec(kind)
        .ok_or_else(|| CommandError::Param(format!("unknown kind `{kind}`")))?;
    let tail = node_view(s, params::str(p, "tail")?)?.id.clone();
    let head = node_view(s, params::str(p, "head")?)?.id.clone();
    let diagram = active(s)?;
    let (model, view) = s.edit(|e, _| {
        uml::create_edge(e, &diagram, spec, &tail, &head).map_err(CommandError::Failed)
    })?;
    s.selection = vec![view.clone()];
    Ok(json!({ "model": model, "view": view }))
}

fn move_views(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let ids = params::ids(p, "views")?;
    let (dx, dy) = (params::coord(p, "dx")?, params::coord(p, "dy")?);
    for id in &ids {
        node_view(s, id)?;
    }
    // Boxes carry what sits inside them (ports, parts, nested classes).
    let moved: HashSet<Id> = ids
        .iter()
        .flat_map(|v| uml::with_contained(&s.doc, v))
        .collect();
    let diagram = active(s)?;
    s.edit(|e, c| {
        let doc = e.doc();
        // Everything whose stored position must shift: the boxes and all
        // their sub-views, plus edges (and their labels) with both ends moving.
        let mut shift: Vec<Id> = moved.iter().flat_map(|v| doc.subtree_ids(v)).collect();
        let edges: Vec<Id> = doc
            .get(&diagram)
            .map(|d| d.children("ownedViews").to_vec())
            .unwrap_or_default()
            .into_iter()
            .filter(|id| {
                doc.get(id).is_some_and(|v| {
                    v.ref_id("tail").is_some_and(|t| moved.contains(t))
                        && v.ref_id("head").is_some_and(|h| moved.contains(h))
                })
            })
            .collect();
        for edge in &edges {
            shift.extend(doc.subtree_ids(edge));
        }
        let mut sets: Vec<(Id, &str, Field)> = Vec::new();
        for id in &shift {
            let Some(v) = doc.get(id) else { continue };
            for (key, d) in [("left", dx), ("top", dy)] {
                if v.get(key).and_then(Field::as_f64).is_some() {
                    sets.push((
                        id.clone(),
                        key,
                        Field::int((v.num(key, 0.0) + d).round() as i64),
                    ));
                }
            }
            if let Some(points) = v.str("points") {
                let pts: Vec<(f64, f64)> = mdj::values::parse_points(points)
                    .into_iter()
                    .map(|(x, y)| (x + dx, y + dy))
                    .collect();
                sets.push((
                    id.clone(),
                    "points",
                    Field::str(mdj::values::format_points(&pts)),
                ));
            }
        }
        for (id, key, value) in sets {
            e.set(&id, key, value);
        }
        c.touched.extend(moved.iter().cloned());
        c.keep.extend(edges);
        Ok(Value::Null)
    })
}

fn resize_view(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let id = node_view(s, params::str(p, "view")?)?.id.clone();
    let (w, h) = (params::size(p, "width")?, params::size(p, "height")?);
    s.edit(|e, c| {
        e.set(&id, "width", Field::int(w.round() as i64));
        e.set(&id, "height", Field::int(h.round() as i64));
        c.touched.insert(id.clone());
        Ok(Value::Null)
    })
}

/// Fields that hold structure (children or references) can't be set here.
const PROTECTED: &[&str] = &[
    "model",
    "head",
    "tail",
    "hostEdge",
    "source",
    "target",
    "reference",
];

/// Fields that may be either a reference or text; typing text replaces a reference.
const TEXT_OR_REF: &[&str] = &["type", "stereotype"];

/// Allowed words for enumeration fields.
/// Allowed words for enumeration fields: `(field, element type or "" for any, words)`.
/// The same field name can mean different enums on different types (`kind`).
const ENUMS: &[(&str, &str, &[&str])] = &[
    (
        "visibility",
        "",
        &["public", "protected", "private", "package"],
    ),
    ("aggregation", "", &["none", "shared", "composite"]),
    (
        "navigable",
        "",
        &["unspecified", "navigable", "notNavigable"],
    ),
    ("direction", "", &["in", "inout", "out", "return"]),
    ("kind", "UMLConnector", &["assembly", "delegation"]),
    (
        "kind",
        "UMLPseudostate",
        &[
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
        ],
    ),
    (
        "kind",
        "UMLAction",
        &[
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
        ],
    ),
    ("kind", "UMLTransition", &["external", "internal", "local"]),
    (
        "kind",
        "Tag",
        &["string", "reference", "boolean", "number", "enum"],
    ),
];

fn set_property(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let id = params::str(p, "id")?.to_owned();
    let key = params::str(p, "key")?.to_owned();
    let value = p.get("value").cloned().unwrap_or(Value::Null);
    let el = get(&s.doc, &id)?;
    if key.is_empty() || key.starts_with('_') || PROTECTED.contains(&key.as_str()) {
        return Err(CommandError::Param(format!(
            "`{key}` cannot be set directly"
        )));
    }
    let structural = match el.get(&key) {
        None | Some(Field::Value(_)) => false,
        Some(Field::Ref(_)) => !TEXT_OR_REF.contains(&key.as_str()),
        Some(_) => true,
    };
    if structural {
        return Err(CommandError::Param(format!(
            "`{key}` holds structure and cannot be set directly"
        )));
    }
    if matches!(value, Value::Array(_) | Value::Object(_)) {
        return Err(CommandError::Param(
            "`value` must be a string, bool, number or null".into(),
        ));
    }
    let el_type = el.type_name.clone();
    if let Some((_, _, allowed)) = ENUMS
        .iter()
        .find(|(k, t, _)| *k == key && (t.is_empty() || *t == el_type))
        && !value.is_null()
        && !value.as_str().is_some_and(|v| allowed.contains(&v))
    {
        return Err(CommandError::Param(format!(
            "`{key}` must be one of {allowed:?}"
        )));
    }
    s.edit(|e, c| {
        if value.is_null() {
            e.unset(&id, &key);
        } else {
            e.set(&id, &key, Field::Value(value));
        }
        // The element and its owners may be drawn (a parameter shows inside
        // its operation's text, which shows inside a class box).
        let mut cur = Some(id.clone());
        for _ in 0..4 {
            let Some(c_id) = cur else { break };
            c.model_changed(e.doc(), &c_id);
            cur = e.doc().get(&c_id).and_then(|el| el.parent.clone());
        }
        Ok(Value::Null)
    })
}

fn add_feature(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let classifier = params::str(p, "classifier")?.to_owned();
    let given = params::str(p, "kind")?;
    let kind = FeatureKind::parse(given)
        .ok_or_else(|| CommandError::Param(format!("unknown feature kind `{given}`")))?;
    let t = get(&s.doc, &classifier)?.type_name.as_str();
    if !kind.allowed_on(t) {
        return Err(CommandError::Param(format!(
            "a {t} cannot have that feature"
        )));
    }
    let id = s.edit(|e, c| {
        let id = uml::add_feature(e, &classifier, kind);
        c.model_changed(e.doc(), &classifier);
        Ok(id)
    })?;
    s.selection = vec![id.clone()];
    Ok(json!({ "id": id }))
}

fn move_feature(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let id = params::str(p, "id")?.to_owned();
    let delta = p
        .get("delta")
        .and_then(Value::as_i64)
        .filter(|d| *d == -1 || *d == 1)
        .ok_or_else(|| CommandError::Param("`delta` must be -1 or 1".into()))?;
    let (parent, _, index) = s
        .doc
        .position_of(&id)
        .ok_or_else(|| CommandError::Param(format!("`{id}` is not in a list")))?;
    let index = index.ok_or_else(|| CommandError::Param(format!("`{id}` is not in a list")))?;
    let Some(target) = index.checked_add_signed(delta as isize) else {
        return Ok(json!(false));
    };
    s.edit(|e, c| {
        let moved = e.reorder(&id, target);
        c.model_changed(e.doc(), &parent);
        Ok(json!(moved))
    })
}

fn list_diagrams(s: &mut Session, _: &Value) -> Result<Value, CommandError> {
    let doc = &s.doc;
    let list: Vec<Value> = doc
        .subtree_ids(doc.root())
        .iter()
        .filter_map(|id| doc.get(id))
        .filter(|e| is_diagram(e))
        .map(|d| {
            json!({
                "id": d.id,
                "name": d.name(),
                "type": d.type_name,
                "views": d.children("ownedViews").len(),
                "active": s.active_diagram.as_deref() == Some(d.id.as_str()),
            })
        })
        .collect();
    Ok(Value::Array(list))
}

fn get_element(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let el = get(&s.doc, params::str(p, "id")?)?;
    let fields: serde_json::Map<String, Value> = el
        .fields
        .iter()
        .map(|(k, f)| {
            let v = match f {
                Field::Value(v) => v.clone(),
                Field::Ref(id) => json!({ "$ref": id }),
                Field::Refs(ids) => {
                    json!(ids.iter().map(|i| json!({ "$ref": i })).collect::<Vec<_>>())
                }
                Field::Child(id) => json!({ "child": id }),
                Field::Children(ids) => json!({ "children": ids }),
            };
            (k.clone(), v)
        })
        .collect();
    Ok(json!({
        "_type": el.type_name,
        "_id": el.id,
        "_parent": el.parent,
        "diagram": diagram_of(&s.doc, &el.id),
        "fields": fields,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(reg: &Registry, s: &mut Session, id: &str, p: Value) -> Value {
        reg.run(s, id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    #[test]
    fn build_a_small_diagram_through_commands() {
        let reg = Registry::default();
        let mut s = Session::default();
        let initial = s.doc.to_value();
        let a = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Class", "x": 20, "y": 20}),
        );
        let b = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "interface", "x": 300, "y": 20}),
        );
        let (av, bv) = (a["view"].as_str().unwrap(), b["view"].as_str().unwrap());
        run(
            &reg,
            &mut s,
            "edge.create",
            json!({"kind": "interface-realization", "tail": av, "head": bv}),
        );
        let attr = run(
            &reg,
            &mut s,
            "feature.add",
            json!({"classifier": a["model"], "kind": "attribute"}),
        );
        run(
            &reg,
            &mut s,
            "element.set",
            json!({"id": attr["id"], "key": "type", "value": "int"}),
        );
        run(
            &reg,
            &mut s,
            "view.move",
            json!({"views": [av], "dx": 10, "dy": 200}),
        );

        let svg = run(&reg, &mut s, "export.svg", json!({}));
        let svg = svg["svg"].as_str().unwrap();
        assert!(
            svg.contains("Class1")
                && svg.contains("«interface»")
                && svg.contains("+attribute1: int")
        );
        assert!(svg.contains("stroke-dasharray"), "realization is dashed");

        // Stored geometry is kept current: the edge was re-routed after the move.
        let edge = s
            .doc
            .iter()
            .find(|e| e.type_name == "UMLInterfaceRealizationView")
            .unwrap();
        let pts = mdj::values::parse_points(edge.str("points").unwrap());
        assert!(pts.len() >= 2 && pts[0].1 >= 220.0, "{pts:?}");

        // The whole sequence undoes cleanly.
        while s.doc.can_undo() {
            run(&reg, &mut s, "edit.undo", json!({}));
        }
        assert_eq!(s.doc.to_value(), initial);
    }

    #[test]
    fn delete_view_keeps_model_delete_from_model_removes_it() {
        let reg = Registry::default();
        let mut s = Session::default();
        let a = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Class", "x": 0, "y": 0}),
        );
        let (m, v) = (
            a["model"].as_str().unwrap().to_owned(),
            a["view"].as_str().unwrap().to_owned(),
        );
        run(&reg, &mut s, "edit.select", json!({"ids": [v]}));
        run(&reg, &mut s, "edit.delete", json!({}));
        assert!(s.doc.get(&v).is_none() && s.doc.get(&m).is_some());
        run(&reg, &mut s, "edit.undo", json!({}));
        run(&reg, &mut s, "edit.select", json!({"ids": [v]}));
        run(&reg, &mut s, "edit.delete-from-model", json!({}));
        assert!(s.doc.get(&v).is_none() && s.doc.get(&m).is_none());
    }

    #[test]
    fn rename_grows_box_to_fit() {
        let reg = Registry::default();
        let mut s = Session::default();
        let a = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Class", "x": 0, "y": 0}),
        );
        let v = a["view"].as_str().unwrap();
        let before = s.doc.get(v).unwrap().num("width", 0.0);
        run(
            &reg,
            &mut s,
            "element.set",
            json!({"id": a["model"], "key": "name", "value": "AVeryLongClassNameThatCannotFit"}),
        );
        assert!(s.doc.get(v).unwrap().num("width", 0.0) > before);
    }

    #[test]
    fn enum_fields_reject_non_words() {
        let reg = Registry::default();
        let mut s = Session::default();
        let a = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Class", "x": 0, "y": 0}),
        );
        for bad in [json!(5), json!(true), json!("secret")] {
            let p = json!({"id": a["model"], "key": "visibility", "value": bad});
            assert!(matches!(
                reg.run(&mut s, "element.set", &p),
                Err(CommandError::Param(_))
            ));
        }
        let ok = json!({"id": a["model"], "key": "visibility", "value": "private"});
        assert!(reg.run(&mut s, "element.set", &ok).is_ok());
    }

    #[test]
    fn every_m2_diagram_type_can_be_added_and_offers_tools() {
        let reg = Registry::default();
        let mut s = Session::default();
        for (ty, _, _) in uml::DIAGRAMS {
            let d = run(&reg, &mut s, "diagram.add", json!({ "type": ty }));
            assert_eq!(s.active_diagram.as_deref(), d["id"].as_str());
            assert_eq!(s.doc.get(d["id"].as_str().unwrap()).unwrap().type_name, *ty);
            assert!(uml::tool_groups(ty).count() >= 2, "{ty} has tools");
        }
        // The old command still works.
        run(&reg, &mut s, "diagram.add-class-diagram", json!({}));
    }

    #[test]
    fn ports_need_a_box_and_move_with_it() {
        let reg = Registry::default();
        let mut s = Session::default();
        run(
            &reg,
            &mut s,
            "diagram.add",
            json!({"type": "UMLComponentDiagram"}),
        );
        let err = reg.run(
            &mut s,
            "node.create",
            &json!({"kind": "UMLPort", "x": 0, "y": 0}),
        );
        assert!(matches!(err, Err(CommandError::Failed(_))), "{err:?}");

        let comp = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Component", "x": 100, "y": 100}),
        );
        let cv = comp["view"].as_str().unwrap().to_owned();
        let port = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Port", "x": 240, "y": 120, "container": cv}),
        );
        let pv = port["view"].as_str().unwrap().to_owned();
        let other = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Component", "x": 400, "y": 100}),
        );
        let ov = other["view"].as_str().unwrap();
        run(
            &reg,
            &mut s,
            "edge.create",
            json!({"kind": "Connector", "tail": pv, "head": ov}),
        );

        let left = |s: &Session, v: &str| s.doc.get(v).unwrap().num("left", 0.0);
        let before = (left(&s, &cv), left(&s, &pv));
        run(
            &reg,
            &mut s,
            "view.move",
            json!({"views": [cv], "dx": 30, "dy": 0}),
        );
        assert_eq!(
            (left(&s, &cv), left(&s, &pv)),
            (before.0 + 30.0, before.1 + 30.0)
        );
        let svg = run(&reg, &mut s, "export.svg", json!({}));
        assert!(svg["svg"].as_str().unwrap().contains("Port1"));
    }

    #[test]
    fn containment_line_via_commands() {
        let reg = Registry::default();
        let mut s = Session::default();
        run(
            &reg,
            &mut s,
            "diagram.add",
            json!({"type": "UMLPackageDiagram"}),
        );
        let pkg = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Package", "x": 0, "y": 0}),
        );
        let class = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Class", "x": 300, "y": 0}),
        );
        run(
            &reg,
            &mut s,
            "edge.create",
            json!({"kind": "Containment", "tail": class["view"], "head": pkg["view"]}),
        );
        let owner = s
            .doc
            .get(class["model"].as_str().unwrap())
            .unwrap()
            .parent
            .clone();
        assert_eq!(owner.as_deref(), pkg["model"].as_str());
        let svg = run(&reg, &mut s, "export.svg", json!({}));
        assert!(
            svg["svg"].as_str().unwrap().contains("<ellipse"),
            "⊕ end drawn"
        );
    }

    /// Rule: no command panics, whatever the parameters or state.
    #[test]
    fn no_command_panics_on_hostile_params() {
        let reg = Registry::default();
        let mut s = Session::default();
        let a = run(
            &reg,
            &mut s,
            "node.create",
            json!({"kind": "Class", "x": 0, "y": 0}),
        );
        let real = [a["view"].clone(), a["model"].clone(), json!(s.doc.root())];
        let hostile = [
            json!(null),
            json!({}),
            json!([]),
            json!("x"),
            json!({"id": 5, "ids": "nope", "views": [1, 2], "kind": {}, "x": "1"}),
            json!({"x": f64::MAX, "y": -1e300, "dx": 1e9, "dy": 0, "width": 0, "height": -5}),
            json!({"id": "missing", "key": "_id", "value": [1]}),
            json!({"id": real[1], "key": "ownedElements", "value": "boom"}),
            json!({"id": real[1], "key": "visibility", "value": "secret"}),
            json!({"id": real[1], "key": "visibility", "value": 5}),
            json!({"id": real[0], "key": "model", "value": "x"}),
            json!({"views": [real[1]], "dx": 1, "dy": 1}),
            json!({"view": real[2], "width": 10, "height": 10}),
            json!({"classifier": real[0], "kind": "attribute"}),
            json!({"id": real[1], "delta": 5}),
            json!({"tail": real[0], "head": real[2], "kind": "Association"}),
            json!({"diagram": real[1], "scale": 1e9}),
            json!({"ids": [real[2]]}),
            json!({"operation": real[1], "direction": "sideways"}),
            json!({"ids": [real[0], "missing"]}),
            json!({"kind": "UMLPort", "x": 0, "y": 0}),
            json!({"kind": "UMLPort", "x": 0, "y": 0, "container": real[1]}),
            json!({"kind": "Note", "x": 0, "y": 0, "container": "missing"}),
            json!({"kind": "UMLContainment", "tail": real[0], "head": real[0]}),
            json!({"type": "UMLSequenceDiagram"}),
            json!({"type": "UMLClassDiagram", "parent": real[0]}),
            json!({"id": real[1], "key": "kind", "value": "sideways"}),
        ];
        for spec in reg.specs() {
            if spec.id == "file.new" {
                continue;
            }
            for p in &hostile {
                let mut fresh = Session::default();
                let _ = reg.run(&mut fresh, spec.id, p);
                let _ = reg.run(&mut s, spec.id, p);
            }
        }
    }
}
