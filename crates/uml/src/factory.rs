//! Building elements and views in the shape StarUML stores them.

use std::collections::HashSet;
use std::f64::consts::PI;

use mdj::values::{DEFAULT_FONT, align, edge_position, format_points, line_style};
use mdj::{Document, Editor, Element, Field, Id};

use crate::catalog::{
    DIAGRAMS, EDGES, EdgeLabels, EdgeModel, EdgeOwner, EdgeSpec, Init, NodeSpec, Placement, Slot,
    edge_spec,
};

/// Collects a subtree in insertion order (root first), wiring `_parent` and
/// ownership fields as children are added.
struct Tree {
    elements: Vec<Element>,
}

impl Tree {
    fn new(root: Element) -> Self {
        Self {
            elements: vec![root],
        }
    }

    /// Add `child` to the list field `key` of the element at `parent`.
    fn child(&mut self, parent: usize, key: &str, mut child: Element) -> (usize, Id) {
        let pid = self.elements[parent].id.clone();
        let cid = child.id.clone();
        child.parent = Some(pid);
        match self.elements[parent]
            .fields
            .entry(key.to_owned())
            .or_insert_with(|| Field::Children(Vec::new()))
        {
            Field::Children(list) => list.push(cid.clone()),
            other => *other = Field::Children(vec![cid.clone()]),
        }
        self.elements.push(child);
        (self.elements.len() - 1, cid)
    }

    /// Add `child` as the single-child field `key`.
    fn single(&mut self, parent: usize, key: &str, mut child: Element) -> Id {
        let cid = child.id.clone();
        child.parent = Some(self.elements[parent].id.clone());
        self.elements[parent]
            .fields
            .insert(key.to_owned(), Field::Child(cid.clone()));
        self.elements.push(child);
        cid
    }

    fn set(&mut self, idx: usize, key: &str, value: Field) {
        self.elements[idx].fields.insert(key.to_owned(), value);
    }
}

fn init(v: Init) -> Field {
    match v {
        Init::Str(s) => Field::str(s),
        Init::Bool(b) => Field::bool(b),
        Init::Int(n) => Field::int(n),
    }
}

fn view(e: &mut Editor, type_name: &str) -> Element {
    Element::new(e.new_id(), type_name).with("font", Field::str(DEFAULT_FONT))
}

fn sub_view(e: &mut Editor, type_name: &str) -> Element {
    view(e, type_name).with("parentStyle", Field::bool(true))
}

/// One row of a list compartment (an attribute, operation, literal or slot).
/// StarUML draws rows left-aligned but would load a missing alignment as
/// centred (see `mdj::values::align`), so store it.
fn row_view(e: &mut Editor, type_name: &str, model: &str) -> Element {
    sub_view(e, type_name)
        .with("model", Field::Ref(model.to_owned()))
        .with("horizontalAlignment", Field::int(align::LEFT))
}

/// The model whose `ownedElements` new elements on `diagram` go into.
pub fn owner_for_new_elements(doc: &Document, diagram: &str) -> Id {
    doc.get(diagram)
        .and_then(|d| d.parent.clone())
        .unwrap_or_else(|| doc.root().to_owned())
}

/// Next free "Base1", "Base2", … among the names in `parent`'s children.
fn unique_name(doc: &Document, parent: &str, base: &str) -> String {
    let taken: HashSet<&str> = doc
        .get(parent)
        .into_iter()
        .flat_map(|p| p.all_children())
        .filter_map(|id| doc.get(id))
        .map(Element::name)
        .collect();
    let mut n = 1usize;
    loop {
        let candidate = format!("{base}{n}");
        if !taken.contains(candidate.as_str()) {
            return candidate;
        }
        n += 1;
    }
}

const PACKAGE_LIKE: &[&str] = &["UMLPackage", "UMLModel", "UMLSubsystem", "UMLProfile"];
/// Classifiers that can own parts and ports.
const STRUCTURED: &[&str] = &[
    "UMLClass",
    "UMLComponent",
    "UMLNode",
    "UMLCollaboration",
    "UMLSubsystem",
    "UMLArtifact",
    "UMLSignal",
];
/// Deployment targets that can hold nested deployable things.
const DEPLOYMENT_TARGETS: &[&str] = &["UMLNode", "UMLNodeInstance", "UMLComponent"];
const DEPLOYABLE: &[&str] = &[
    "UMLArtifact",
    "UMLComponent",
    "UMLNode",
    "UMLArtifactInstance",
    "UMLComponentInstance",
    "UMLNodeInstance",
    "UMLClass",
    "UMLInterface",
];

/// Where a new `spec` element dropped on `container_view` belongs: the model
/// that should own it. `None` if that box can't hold it (it then goes on
/// the diagram as usual, unless the spec requires a container).
pub fn container_owner(doc: &Document, container_view: &str, spec: &NodeSpec) -> Option<Id> {
    let cv = doc.get(container_view)?;
    if cv.get("head").is_some() {
        return None; // edges hold nothing
    }
    let cm = doc.get(cv.ref_id("model")?)?;
    match spec.placement {
        Placement::OnClassifier => {
            // A part box stands for its type: parts inside it belong to that type.
            let owner = if cv.type_name == "UMLPartView" {
                doc.get(cm.ref_id("type")?)?
            } else {
                cm
            };
            STRUCTURED
                .contains(&owner.type_name.as_str())
                .then(|| owner.id.clone())
        }
        Placement::Free => {
            let t = cm.type_name.as_str();
            let model_type = spec.model_type?;
            let fits = PACKAGE_LIKE.contains(&t)
                || (DEPLOYMENT_TARGETS.contains(&t) && DEPLOYABLE.contains(&model_type));
            fits.then(|| cm.id.clone())
        }
    }
}

/// Where a new element goes: the model that owns it, the list it goes in,
/// and the box its view sits inside (if any).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub owner: Id,
    pub list: &'static str,
    pub container: Option<Id>,
}

/// Decide where a `spec` element created on `diagram` (dropped on
/// `container`, if given) belongs. Errors explain what's needed instead.
pub fn target_for(
    doc: &Document,
    diagram: &str,
    spec: &NodeSpec,
    container: Option<&str>,
) -> Result<Target, String> {
    let diagram_owner = owner_for_new_elements(doc, diagram);
    let container_view = container
        .and_then(|c| doc.get(c))
        .filter(|v| v.get("head").is_none());
    let container_model = container_view
        .and_then(|v| v.ref_id("model"))
        .and_then(|m| doc.get(m));
    match spec.slot {
        Slot::OwnedElements | Slot::Attributes => {
            let in_container = container.and_then(|c| container_owner(doc, c, spec));
            if spec.placement == Placement::OnClassifier && in_container.is_none() {
                return Err(format!(
                    "Place a {} on a class, component or node.",
                    spec.label
                ));
            }
            Ok(Target {
                container: in_container.as_ref().and(container).map(str::to_owned),
                owner: in_container.unwrap_or(diagram_owner),
                list: if spec.slot == Slot::Attributes {
                    "attributes"
                } else {
                    "ownedElements"
                },
            })
        }
        Slot::Vertices => {
            // Inside a composite state: its first region.
            if let Some(region) = container_model
                .filter(|m| m.type_name == "UMLState")
                .and_then(|m| m.children("regions").first())
            {
                return Ok(Target {
                    owner: region.clone(),
                    list: "vertices",
                    container: container.map(str::to_owned),
                });
            }
            let machine = doc
                .get(&diagram_owner)
                .filter(|o| o.type_name == "UMLStateMachine")
                .ok_or("States belong to a state machine: add a Statechart Diagram (Model ▸ Add Diagram).")?;
            let region = machine
                .children("regions")
                .first()
                .ok_or("This state machine has no region to put states in.")?;
            Ok(Target {
                owner: region.clone(),
                list: "vertices",
                container: None,
            })
        }
        Slot::ActivityNodes | Slot::ActivityGroups => {
            let activity = doc
                .get(&diagram_owner)
                .filter(|o| o.type_name == "UMLActivity")
                .ok_or(
                    "Actions belong to an activity: add an Activity Diagram (Model ▸ Add Diagram).",
                )?;
            // Nodes dropped in a swimlane sit inside it on screen; the model
            // stays in the activity.
            let lane = container_view
                .filter(|v| v.type_name == "UMLSwimlaneView" && spec.slot == Slot::ActivityNodes)
                .map(|v| v.id.clone());
            Ok(Target {
                owner: activity.id.clone(),
                list: if spec.slot == Slot::ActivityNodes {
                    "nodes"
                } else {
                    "groups"
                },
                container: lane,
            })
        }
    }
}

/// Create a model element (if the spec has one) and its view on `diagram`
/// at `(x, y)`, inside `container` if given and allowed. Returns
/// `(model_id, view_id)`.
pub fn create_node(
    e: &mut Editor,
    diagram: &str,
    spec: &NodeSpec,
    x: f64,
    y: f64,
    container: Option<&str>,
) -> Result<(Option<Id>, Id), String> {
    let target = target_for(e.doc(), diagram, spec, container)?;
    let model_id = match spec.model_type {
        Some(model_type) => {
            let mut model = Element::new(e.new_id(), model_type);
            if !spec.base_name.is_empty() {
                let name = unique_name(e.doc(), &target.owner, spec.base_name);
                model = model.with("name", Field::str(name));
            }
            for (k, v) in spec.model_fields {
                model = model.with(k, init(*v));
            }
            let mut tree = Tree::new(model);
            for (list, child_type) in spec.model_children {
                let child = Element::new(e.new_id(), *child_type);
                tree.child(0, list, child);
            }
            Some(e.append(&target.owner, target.list, tree.elements))
        }
        None => None,
    };
    let container = target.container.as_deref();
    let (x, y) = match (spec.id, container.and_then(|c| e.doc().get(c))) {
        ("UMLPort", Some(c)) => snap_to_border(c, x, y, spec.size),
        _ => (x, y),
    };
    let view = add_node_view(e, diagram, spec, model_id.as_deref(), x, y, container);
    Ok((model_id, view))
}

/// Top-left for a box of `size` centred on the nearest point of `container`'s border.
fn snap_to_border(container: &Element, x: f64, y: f64, size: (f64, f64)) -> (f64, f64) {
    let (l, t) = (container.num("left", 0.0), container.num("top", 0.0));
    let (r, b) = (
        l + container.num("width", 0.0),
        t + container.num("height", 0.0),
    );
    let (cx, cy) = (x.clamp(l, r), y.clamp(t, b));
    let candidates = [(l, cy), (r, cy), (cx, t), (cx, b)];
    let (px, py) = candidates
        .into_iter()
        .min_by(|a, b| {
            let da = (a.0 - x).hypot(a.1 - y);
            let db = (b.0 - x).hypot(b.1 - y);
            da.total_cmp(&db)
        })
        .unwrap_or((l, t));
    (px - size.0 / 2.0, py - size.1 / 2.0)
}

/// Add a view of `model` (or a view-only shape) to `diagram`.
pub fn add_node_view(
    e: &mut Editor,
    diagram: &str,
    spec: &NodeSpec,
    model: Option<&str>,
    x: f64,
    y: f64,
    container: Option<&str>,
) -> Id {
    let mut root = view(e, spec.view_type)
        .with("left", Field::int(x.round() as i64))
        .with("top", Field::int(y.round() as i64))
        .with("width", Field::int(spec.size.0 as i64))
        .with("height", Field::int(spec.size.1 as i64));
    if let Some(m) = model {
        root.fields.insert("model".into(), Field::Ref(m.to_owned()));
        // Keep `model` first, as StarUML writes it.
        root.fields.move_index(root.fields.len() - 1, 0);
    }
    for (k, v) in spec.view_fields {
        root = root.with(k, init(*v));
    }
    if let Some(c) = container {
        root = root.with("containerView", Field::Ref(c.to_owned()));
    }
    let root_id = root.id.clone();
    let mut t = Tree::new(root);

    if spec.name_compartment {
        let mut name_comp = sub_view(e, "UMLNameCompartmentView");
        if let Some(m) = model {
            name_comp = name_comp.with("model", Field::Ref(m.to_owned()));
        }
        let (nc, nc_id) = t.child(0, "subViews", name_comp);
        for key in [
            "stereotypeLabel",
            "nameLabel",
            "namespaceLabel",
            "propertyLabel",
        ] {
            let mut label = sub_view(e, "LabelView");
            if key == "propertyLabel" {
                // StarUML right-aligns `{property}` text; centre is the stored default.
                label = label.with("horizontalAlignment", Field::int(align::RIGHT));
            }
            if key == "nameLabel" {
                let name = model
                    .and_then(|m| e.doc().get(m))
                    .map(Element::name)
                    .unwrap_or("");
                label = label.with("text", Field::str(name));
            }
            let (_, lid) = t.child(nc, "subViews", label);
            t.set(nc, key, Field::Ref(lid));
        }
        t.set(0, "nameCompartment", Field::Ref(nc_id));
    }

    for key in spec.labels {
        let name = model
            .and_then(|m| e.doc().get(m))
            .map(Element::name)
            .unwrap_or("")
            .to_owned();
        let label = sub_view(e, "LabelView").with("text", Field::str(name));
        let (_, lid) = t.child(0, "subViews", label);
        t.set(0, key, Field::Ref(lid));
    }

    if spec.floating_labels {
        // Placement as StarUML's floating node labels: distance and angle
        // from the node's centre. Neither has a metamodel default, so write them.
        for (key, distance, alpha) in [
            ("nameLabel", 20.0, 3.0 * PI / 4.0),
            ("stereotypeLabel", 35.0, 3.0 * PI / 4.0),
            ("propertyLabel", 20.0, -3.0 * PI / 4.0),
        ] {
            let mut label = sub_view(e, "NodeLabelView")
                .with("hostNode", Field::Ref(root_id.clone()))
                .with("alpha", Field::Value(alpha.into()))
                .with("distance", Field::Value(distance.into()));
            if let Some(m) = model {
                label = label.with("model", Field::Ref(m.to_owned()));
            }
            let (_, lid) = t.child(0, "subViews", label);
            t.set(0, key, Field::Ref(lid));
        }
    }

    for (key, type_name) in spec.compartments {
        let mut comp = sub_view(e, type_name);
        if let Some(m) = model {
            comp = comp.with("model", Field::Ref(m.to_owned()));
        }
        let (ci, cid) = t.child(0, "subViews", comp);
        if let (Some((list_key, item_view)), Some(m)) = (compartment_items(type_name), model) {
            let items = e
                .doc()
                .get(m)
                .map(|el| el.children(list_key).to_vec())
                .unwrap_or_default();
            for item in items {
                let v = row_view(e, item_view, &item);
                t.child(ci, "subViews", v);
            }
        }
        t.set(0, key, Field::Ref(cid));
    }

    let id = e.append(diagram, "ownedViews", t.elements);
    if let Some(c) = container {
        add_contained(e, c, &id);
    }
    id
}

/// Record `view` in `container`'s `containedViews`.
fn add_contained(e: &mut Editor, container: &str, view: &str) {
    let mut list: Vec<Id> = e
        .doc()
        .get(container)
        .map(|c| {
            c.get("containedViews")
                .map(|f| f.refs().to_vec())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    list.push(view.to_owned());
    e.set(container, "containedViews", Field::Refs(list));
}

/// For a list compartment type: the model field it lists and the view type of each row.
pub fn compartment_items(compartment_type: &str) -> Option<(&'static str, &'static str)> {
    match compartment_type {
        "UMLAttributeCompartmentView" => Some(("attributes", "UMLAttributeView")),
        "UMLOperationCompartmentView" => Some(("operations", "UMLOperationView")),
        "UMLEnumerationLiteralCompartmentView" => Some(("literals", "UMLEnumerationLiteralView")),
        "UMLSlotCompartmentView" => Some(("slots", "UMLSlotView")),
        "UMLDecompositionCompartmentView" => Some(("regions", "UMLRegionView")),
        _ => None,
    }
}

/// Create a relationship (when the spec has a model) between what two views
/// show, plus its edge view. `tail` is where the drag started. Returns
/// `(model_id, view_id)`.
pub fn create_edge(
    e: &mut Editor,
    diagram: &str,
    spec: &EdgeSpec,
    tail_view: &str,
    head_view: &str,
) -> Result<(Option<Id>, Id), String> {
    // A self transition always loops back to the state it starts on.
    let head_view = if spec.id == "UMLSelfTransition" {
        tail_view
    } else {
        head_view
    };
    let model_of = |v: &str| {
        e.doc()
            .get(v)
            .and_then(|el| el.ref_id("model"))
            .map(str::to_owned)
    };
    let (tail_model, head_model) = (model_of(tail_view), model_of(head_view));

    if spec.id == "UMLContainment" {
        let (Some(child), Some(container)) = (tail_model, head_model) else {
            return Err("Containment links two model elements.".into());
        };
        if !e.relocate(&child, &container, "ownedElements") {
            return Err("That element can't be moved inside itself.".into());
        }
        let view = add_edge_view(e, diagram, spec, None, tail_view, head_view);
        return Ok((None, view));
    }

    let model_id = match (spec.model, spec.model_type) {
        (EdgeModel::ViewOnly, _) | (_, None) => None,
        (EdgeModel::Directed, Some(model_type)) => {
            let (Some(source), Some(target)) = (tail_model, head_model) else {
                return Err(format!(
                    "A {} needs a model element at both ends.",
                    spec.label
                ));
            };
            let rel = Element::new(e.new_id(), model_type)
                .with("source", Field::Ref(source.clone()))
                .with("target", Field::Ref(target));
            let (owner, list) = edge_owner(e.doc(), spec, &source)?;
            Some(e.append(&owner, list, vec![rel]))
        }
        (
            EdgeModel::Undirected {
                end_type,
                tail_aggregation,
                head_navigable,
            },
            Some(model_type),
        ) => {
            let (Some(tail), Some(head)) = (tail_model, head_model) else {
                return Err(format!(
                    "A {} needs a model element at both ends.",
                    spec.label
                ));
            };
            let mut rel = Tree::new(Element::new(e.new_id(), model_type));
            let mut end1 =
                Element::new(e.new_id(), end_type).with("reference", Field::Ref(tail.clone()));
            if tail_aggregation != "none" {
                end1 = end1.with("aggregation", Field::str(tail_aggregation));
            }
            let mut end2 = Element::new(e.new_id(), end_type).with("reference", Field::Ref(head));
            if head_navigable != "unspecified" {
                end2 = end2.with("navigable", Field::str(head_navigable));
            }
            rel.single(0, "end1", end1);
            rel.single(0, "end2", end2);
            Some(e.append(&tail, "ownedElements", rel.elements))
        }
    };
    let view = add_edge_view(e, diagram, spec, model_id.as_deref(), tail_view, head_view);
    Ok((model_id, view))
}

/// Where a new relationship from `source` goes, per the spec.
fn edge_owner(doc: &Document, spec: &EdgeSpec, source: &str) -> Result<(Id, &'static str), String> {
    let parent_of_type = |t: &str| {
        doc.get(source)
            .and_then(|s| s.parent.as_deref())
            .and_then(|p| doc.get(p))
            .filter(|p| p.type_name == t)
            .map(|p| p.id.clone())
    };
    match spec.owner {
        // Most relationships live with their source, as in StarUML.
        EdgeOwner::Source => Ok((source.to_owned(), "ownedElements")),
        EdgeOwner::SourceRegion => parent_of_type("UMLRegion")
            .map(|r| (r, "transitions"))
            .ok_or_else(|| "Transitions start from a state or pseudostate.".to_owned()),
        EdgeOwner::SourceActivity => parent_of_type("UMLActivity")
            .map(|a| (a, "edges"))
            .ok_or_else(|| "Flows start from an action or activity node.".to_owned()),
    }
}

pub fn add_edge_view(
    e: &mut Editor,
    diagram: &str,
    spec: &EdgeSpec,
    model: Option<&str>,
    tail_view: &str,
    head_view: &str,
) -> Id {
    let mut root = view(e, spec.view_type);
    if let Some(m) = model {
        root = root.with("model", Field::Ref(m.to_owned()));
    }
    let root = root
        .with("head", Field::Ref(head_view.to_owned()))
        .with("tail", Field::Ref(tail_view.to_owned()))
        .with("lineStyle", Field::int(line_style::OBLIQUE))
        .with("points", Field::str(format_points(&[])));
    let root_id = root.id.clone();
    let mut t = Tree::new(root);

    let mut labels: Vec<(&str, i64, f64, f64)> = Vec::new();
    if spec.labels != EdgeLabels::None {
        labels.extend([
            ("nameLabel", edge_position::MIDDLE, PI / 2.0, 15.0),
            ("stereotypeLabel", edge_position::MIDDLE, PI / 2.0, 30.0),
            ("propertyLabel", edge_position::MIDDLE, -PI / 2.0, 15.0),
        ]);
    }
    if matches!(
        spec.labels,
        EdgeLabels::Ends | EdgeLabels::EndsAndQualifiers
    ) {
        labels.extend([
            ("tailRoleNameLabel", edge_position::TAIL, PI / 6.0, 30.0),
            ("tailPropertyLabel", edge_position::TAIL, PI / 4.0, 40.0),
            (
                "tailMultiplicityLabel",
                edge_position::TAIL,
                -PI / 6.0,
                25.0,
            ),
            ("headRoleNameLabel", edge_position::HEAD, -PI / 6.0, 30.0),
            ("headPropertyLabel", edge_position::HEAD, -PI / 4.0, 40.0),
            ("headMultiplicityLabel", edge_position::HEAD, PI / 6.0, 25.0),
        ]);
    }
    for (key, pos, alpha, distance) in labels {
        let mut label = sub_view(e, "EdgeLabelView");
        if let Some(m) = model {
            label = label.with("model", Field::Ref(m.to_owned()));
        }
        let label = label
            .with("hostEdge", Field::Ref(root_id.clone()))
            .with("edgePosition", Field::int(pos))
            .with("alpha", Field::Value(alpha.into()))
            .with("distance", Field::Value(distance.into()));
        let (_, lid) = t.child(0, "subViews", label);
        t.set(0, key, Field::Ref(lid));
    }
    if spec.labels == EdgeLabels::EndsAndQualifiers {
        for key in ["tailQualifiersCompartment", "headQualifiersCompartment"] {
            let mut comp = sub_view(e, "UMLQualifierCompartmentView");
            if let Some(m) = model {
                comp = comp.with("model", Field::Ref(m.to_owned()));
            }
            let (_, cid) = t.child(0, "subViews", comp);
            t.set(0, key, Field::Ref(cid));
        }
    }
    e.append(diagram, "ownedViews", t.elements)
}

/// Which catalog edge an edge view shows. Associations are told apart by
/// their ends (aggregation, navigability), links by navigability.
pub fn edge_spec_of(doc: &Document, edge_view: &Element) -> Option<&'static EdgeSpec> {
    let model = edge_view.ref_id("model").and_then(|m| doc.get(m));
    let end = |k: &str| {
        model
            .and_then(|m| m.children(k).first())
            .and_then(|id| doc.get(id))
    };
    let agg = |k: &str| end(k).and_then(|e| e.str("aggregation")).unwrap_or("none");
    let nav = |k: &str| {
        end(k)
            .and_then(|e| e.str("navigable"))
            .unwrap_or("unspecified")
    };
    let id = match edge_view.type_name.as_str() {
        "UMLAssociationView" => {
            if agg("end1") == "composite" || agg("end2") == "composite" {
                "UMLComposition"
            } else if agg("end1") == "shared" || agg("end2") == "shared" {
                "UMLAggregation"
            } else if nav("end1") == "navigable" || nav("end2") == "navigable" {
                "UMLDirectedAssociation"
            } else {
                "UMLAssociation"
            }
        }
        "UMLLinkView" if nav("end1") == "navigable" || nav("end2") == "navigable" => {
            "UMLDirectedLink"
        }
        "UMLTransitionView" if edge_view.ref_id("tail") == edge_view.ref_id("head") => {
            "UMLSelfTransition"
        }
        other => return EDGES.iter().find(|s| s.view_type == other),
    };
    edge_spec(id)
}

// ---------------------------------------------------------------------------
// Features: attributes, operations, literals, slots

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureKind {
    Attribute,
    Operation,
    Literal,
    Slot,
}

impl FeatureKind {
    pub fn list_key(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attributes",
            FeatureKind::Operation => "operations",
            FeatureKind::Literal => "literals",
            FeatureKind::Slot => "slots",
        }
    }
    fn model_type(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "UMLAttribute",
            FeatureKind::Operation => "UMLOperation",
            FeatureKind::Literal => "UMLEnumerationLiteral",
            FeatureKind::Slot => "UMLSlot",
        }
    }
    fn compartment_key(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attributeCompartment",
            FeatureKind::Operation => "operationCompartment",
            FeatureKind::Literal => "enumerationLiteralCompartment",
            FeatureKind::Slot => "slotCompartment",
        }
    }
    fn row_view(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "UMLAttributeView",
            FeatureKind::Operation => "UMLOperationView",
            FeatureKind::Literal => "UMLEnumerationLiteralView",
            FeatureKind::Slot => "UMLSlotView",
        }
    }
    fn base_name(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attribute",
            FeatureKind::Operation => "operation",
            FeatureKind::Literal => "literal",
            FeatureKind::Slot => "slot",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "attribute" => FeatureKind::Attribute,
            "operation" => FeatureKind::Operation,
            "literal" => FeatureKind::Literal,
            "slot" => FeatureKind::Slot,
            _ => return None,
        })
    }

    /// Whether elements of `model_type` can have this kind of feature.
    pub fn allowed_on(self, model_type: &str) -> bool {
        match self {
            FeatureKind::Literal => model_type == "UMLEnumeration",
            FeatureKind::Slot => matches!(
                model_type,
                "UMLObject" | "UMLArtifactInstance" | "UMLComponentInstance" | "UMLNodeInstance"
            ),
            _ => matches!(
                model_type,
                "UMLClass"
                    | "UMLInterface"
                    | "UMLEnumeration"
                    | "UMLDataType"
                    | "UMLPrimitiveType"
                    | "UMLSignal"
                    | "UMLComponent"
                    | "UMLArtifact"
                    | "UMLNode"
                    | "UMLStereotype"
                    | "UMLCollaboration"
            ),
        }
    }
}

/// Add a feature to `owner`, and a row for it in every view of `owner` that
/// has the matching compartment. Returns the new feature's id.
pub fn add_feature(e: &mut Editor, owner: &str, kind: FeatureKind) -> Id {
    let existing = e
        .doc()
        .get(owner)
        .map(|c| c.children(kind.list_key()).len())
        .unwrap_or(0);
    let name = format!("{}{}", kind.base_name(), existing + 1);
    let feature = Element::new(e.new_id(), kind.model_type()).with("name", Field::str(name));
    let feature_id = e.append(owner, kind.list_key(), vec![feature]);

    let compartments: Vec<Id> = views_of(e.doc(), owner)
        .into_iter()
        .filter_map(|v| {
            e.doc()
                .get(&v)?
                .ref_id(kind.compartment_key())
                .map(str::to_owned)
        })
        .collect();
    for comp in compartments {
        let row = row_view(e, kind.row_view(), &feature_id);
        e.append(&comp, "subViews", vec![row]);
    }
    feature_id
}

/// Add a parameter to an operation. Callers adding a `return` parameter
/// should check [`return_parameter`] first.
pub fn add_parameter(e: &mut Editor, operation: &str, direction: &str) -> Id {
    let count = e
        .doc()
        .get(operation)
        .map(|o| o.children("parameters").len())
        .unwrap_or(0);
    let mut p = Element::new(e.new_id(), "UMLParameter");
    if direction == "return" {
        p = p
            .with("type", Field::str("void"))
            .with("direction", Field::str("return"));
    } else {
        p = p.with("name", Field::str(format!("p{}", count + 1)));
        if direction != "in" {
            p = p.with("direction", Field::str(direction));
        }
    }
    e.append(operation, "parameters", vec![p])
}

/// The operation's `return` parameter, if any.
pub fn return_parameter<'a>(doc: &'a Document, operation: &Element) -> Option<&'a Element> {
    operation
        .children("parameters")
        .iter()
        .filter_map(|id| doc.get(id))
        .find(|p| p.str("direction") == Some("return"))
}

/// Views (anywhere) whose `model` is `model`.
pub fn views_of(doc: &Document, model: &str) -> Vec<Id> {
    doc.referrers(model)
        .into_iter()
        .filter(|(_, key)| key == "model")
        .map(|(id, _)| id)
        .collect()
}

/// `view` and every view inside it, transitively (`containedViews`).
pub fn with_contained(doc: &Document, view: &str) -> Vec<Id> {
    let mut out = Vec::new();
    let mut stack = vec![view.to_owned()];
    let mut seen = HashSet::new();
    while let Some(v) = stack.pop() {
        if !seen.insert(v.clone()) {
            continue; // guard against cycles in hand-edited files
        }
        if let Some(f) = doc.get(&v).and_then(|el| el.get("containedViews")) {
            stack.extend(f.refs().iter().cloned());
        }
        out.push(v);
    }
    out
}

/// Create a diagram of `diagram_type` under `parent`, the way StarUML does:
/// a statechart diagram lives in a state machine (created with one region),
/// an activity diagram in an activity, a profile diagram in a profile —
/// unless `parent` already is one. Returns the diagram's id.
pub fn create_diagram(e: &mut Editor, parent: &str, diagram_type: &str) -> Result<Id, String> {
    let &(_, _, base) = DIAGRAMS
        .iter()
        .find(|(t, _, _)| *t == diagram_type)
        .ok_or_else(|| format!("unknown diagram type `{diagram_type}`"))?;
    let parent_type = e
        .doc()
        .get(parent)
        .map(|p| p.type_name.clone())
        .ok_or("no such parent")?;
    let wrapper = match diagram_type {
        "UMLStatechartDiagram" if parent_type != "UMLStateMachine" => {
            Some(("UMLStateMachine", "StateMachine"))
        }
        "UMLActivityDiagram" if parent_type != "UMLActivity" => Some(("UMLActivity", "Activity")),
        "UMLProfileDiagram" if parent_type != "UMLProfile" => Some(("UMLProfile", "Profile")),
        _ => None,
    };
    let owner = match wrapper {
        Some((wrapper_type, wrapper_base)) => {
            let name = unique_name(e.doc(), parent, wrapper_base);
            let mut tree =
                Tree::new(Element::new(e.new_id(), wrapper_type).with("name", Field::str(name)));
            if wrapper_type == "UMLStateMachine" {
                let region = Element::new(e.new_id(), "UMLRegion");
                tree.child(0, "regions", region);
            }
            e.append(parent, "ownedElements", tree.elements)
        }
        None => parent.to_owned(),
    };
    let name = unique_name(e.doc(), &owner, base);
    let diagram = Element::new(e.new_id(), diagram_type).with("name", Field::str(name));
    Ok(e.append(&owner, "ownedElements", vec![diagram]))
}
