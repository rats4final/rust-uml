//! Building elements and views in the shape StarUML stores them.

use std::collections::HashSet;
use std::f64::consts::PI;

use mdj::values::{DEFAULT_FONT, align, edge_position, format_points};
use mdj::{Document, Editor, Element, Field, Id};

use crate::catalog::{
    DIAGRAMS, EDGES, EdgeLabels, EdgeModel, EdgeOwner, EdgeSpec, Init, NodeSpec, Placement, Slot,
    edge_spec, edge_spec_on,
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
        Slot::Participants | Slot::Fragments => {
            let interaction = doc
                .get(&diagram_owner)
                .filter(|o| o.type_name == "UMLInteraction")
                .ok_or("Lifelines belong to an interaction: add a Sequence or Communication Diagram (Model ▸ Add Diagram).")?;
            Ok(Target {
                owner: interaction.id.clone(),
                list: if spec.slot == Slot::Participants {
                    "participants"
                } else {
                    "fragments"
                },
                container: None,
            })
        }
        Slot::TimingLifeline | Slot::TimingState | Slot::TimingSegment | Slot::TimingTick => {
            timing_target(doc, diagram, &diagram_owner, spec, container_view)
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

/// Where a timing-diagram element goes: lifelines and ticks inside the
/// timing frame, states inside a lifeline, segments inside a state.
fn timing_target(
    doc: &Document,
    diagram: &str,
    diagram_owner: &str,
    spec: &NodeSpec,
    container_view: Option<&Element>,
) -> Result<Target, String> {
    let interaction = doc
        .get(diagram_owner)
        .filter(|o| o.type_name == "UMLInteraction")
        .ok_or(
            "Timing elements belong to an interaction: add a Timing Diagram (Model ▸ Add Diagram).",
        )?;
    let of_type = |t: &str| container_view.filter(|v| v.type_name == t);
    match spec.slot {
        Slot::TimingState => {
            // On a lifeline, or on one of its states.
            let lifeline = of_type("UMLTimingLifelineView")
                .or_else(|| {
                    of_type("UMLTimingStateView")
                        .and_then(|s| s.ref_id("containerView"))
                        .and_then(|c| doc.get(c))
                })
                .ok_or("Place a State/Condition on a lifeline.")?;
            let model = lifeline
                .ref_id("model")
                .ok_or("That lifeline has no model element.")?;
            Ok(Target {
                owner: model.to_owned(),
                list: "ownedElements",
                container: Some(lifeline.id.clone()),
            })
        }
        Slot::TimingSegment => {
            let state = of_type("UMLTimingStateView")
                .ok_or("Place a Time Segment on a State/Condition.")?;
            Ok(Target {
                owner: interaction.id.clone(),
                list: "fragments",
                container: Some(state.id.clone()),
            })
        }
        // Lifelines and ticks: on the timing frame (the diagram's own, unless
        // dropped on one).
        _ => {
            let frame = of_type("UMLTimingFrameView")
                .map(|f| f.id.clone())
                .or_else(|| timing_frame(doc, diagram))
                .ok_or("This timing diagram has no frame to put it in.")?;
            Ok(Target {
                owner: interaction.id.clone(),
                list: "participants",
                container: Some(frame),
            })
        }
    }
}

/// The timing diagram's frame view.
fn timing_frame(doc: &Document, diagram: &str) -> Option<Id> {
    doc.get(diagram)?
        .children("ownedViews")
        .iter()
        .find(|v| {
            doc.get(v)
                .is_some_and(|v| v.type_name == "UMLTimingFrameView")
        })
        .cloned()
}

/// Starting geometry for a timing view, as the frame would arrange it
/// (StarUML and our renderer re-arrange on drawing): lifelines stacked
/// across the frame, states stacked in their lifeline (which grows), segments
/// and ticks one after another along the time axis.
fn place_timing_view(e: &mut Editor, view: &str, slot: Slot, container: &str) {
    let doc = e.doc();
    let rect = |id: &str| {
        doc.get(id).map_or((0.0, 0.0, 0.0, 0.0), |v| {
            (
                v.num("left", 0.0),
                v.num("top", 0.0),
                v.num("width", 0.0),
                v.num("height", 0.0),
            )
        })
    };
    let contained = |id: &str| -> Vec<Id> {
        doc.get(id)
            .and_then(|c| c.get("containedViews"))
            .map(|f| f.refs().to_vec())
            .unwrap_or_default()
            .into_iter()
            .filter(|v| v != view)
            .collect()
    };
    let of_type = |ids: &[Id], t: &str| -> Vec<Id> {
        ids.iter()
            .filter(|v| doc.get(v).is_some_and(|v| v.type_name == t))
            .cloned()
            .collect()
    };
    let (cl, ct, cw, ch) = rect(container);
    let mut sets: Vec<(Id, &str, f64)> = Vec::new();
    match slot {
        Slot::TimingLifeline => {
            let below = of_type(&contained(container), "UMLTimingLifelineView")
                .iter()
                .map(|l| {
                    let (_, t, _, h) = rect(l);
                    t + h
                })
                .fold(ct + TIMING_FRAME_HEAD, f64::max);
            sets.extend([
                (view.to_owned(), "left", cl),
                (view.to_owned(), "top", below),
                (view.to_owned(), "width", cw),
                (view.to_owned(), "height", 2.0 * TIMING_PAD),
            ]);
        }
        Slot::TimingState => {
            let states = of_type(&contained(container), "UMLTimingStateView").len() as f64;
            sets.extend([
                (view.to_owned(), "left", cl + TIMING_NAME_BAND),
                (
                    view.to_owned(),
                    "top",
                    ct + TIMING_PAD + TIMING_ROW * states,
                ),
                (view.to_owned(), "width", (cw - TIMING_NAME_BAND).max(0.0)),
                (view.to_owned(), "height", TIMING_ROW),
                // The lifeline grows to hold its states.
                (
                    container.to_owned(),
                    "height",
                    ch.max(2.0 * TIMING_PAD + TIMING_ROW * (states + 1.0)),
                ),
            ]);
        }
        Slot::TimingSegment => {
            // After the last segment on any state of the same lifeline.
            let lifeline = doc
                .get(container)
                .and_then(|s| s.ref_id("containerView"))
                .unwrap_or(container)
                .to_owned();
            let after = of_type(&contained(&lifeline), "UMLTimingStateView")
                .iter()
                .flat_map(|st| of_type(&contained(st), "UMLTimeSegmentView"))
                .map(|sg| {
                    let (l, _, w, _) = rect(&sg);
                    l + w
                })
                .fold(cl + TIMING_STATE_LABEL, f64::max);
            sets.extend([
                (view.to_owned(), "left", after),
                (view.to_owned(), "top", ct),
                (view.to_owned(), "height", TIMING_ROW),
            ]);
        }
        Slot::TimingTick => {
            let after = of_type(&contained(container), "UMLTimeTickView")
                .iter()
                .map(|t| {
                    let (l, _, w, _) = rect(t);
                    l + w
                })
                .fold(cl + TIMING_NAME_BAND + TIMING_STATE_LABEL, f64::max);
            sets.extend([
                (view.to_owned(), "left", after),
                (view.to_owned(), "top", ct + ch - 5.0),
            ]);
        }
        _ => {}
    }
    for (id, key, v) in sets {
        e.set(&id, key, Field::int(v.round() as i64));
    }
}

/// Timing layout sizes (StarUML: frame tag ≈ 25, lifeline padding 25, state
/// row 25); the name band and state label width are starting guesses.
const TIMING_FRAME_HEAD: f64 = 30.0;
const TIMING_PAD: f64 = 25.0;
const TIMING_ROW: f64 = 25.0;
const TIMING_NAME_BAND: f64 = 40.0;
const TIMING_STATE_LABEL: f64 = 120.0;

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
            let id = e.append(&target.owner, target.list, tree.elements);
            if spec.id == "UMLLifeline" {
                add_lifeline_role(e, &target.owner, &id);
            }
            if matches!(
                spec.id,
                "UMLInteractionUseInOverview" | "UMLInteractionInOverview"
            ) {
                add_overview_target(e, &target.owner, &id, spec.id);
            }
            if spec.slot == Slot::TimingSegment {
                // The segment covers the state's lifeline, in that state.
                let state = target
                    .container
                    .as_deref()
                    .and_then(|c| e.doc().get(c))
                    .and_then(|c| c.ref_id("model"))
                    .map(str::to_owned);
                if let Some(state) = state {
                    let lifeline = e.doc().get(&state).and_then(|s| s.parent.clone());
                    if let Some(lifeline) = lifeline {
                        e.set(&id, "covered", Field::Ref(lifeline));
                    }
                    e.set(&id, "invariant", Field::Ref(state));
                }
            }
            Some(id)
        }
        None => None,
    };
    let container = target.container.as_deref();
    let (x, y) = match (spec.id, container.and_then(|c| e.doc().get(c))) {
        ("UMLPort", Some(c)) => snap_to_border(c, x, y, spec.size),
        _ => (x, y),
    };
    let view = add_node_view(e, diagram, spec, model_id.as_deref(), x, y, container);
    if let (
        Slot::TimingLifeline | Slot::TimingState | Slot::TimingSegment | Slot::TimingTick,
        Some(c),
    ) = (spec.slot, container)
    {
        place_timing_view(e, &view, spec.slot, c);
    }
    Ok((model_id, view))
}

/// As StarUML: a new lifeline represents a new role attribute `RoleN` of the
/// classifier (usually a collaboration) that owns the interaction.
fn add_lifeline_role(e: &mut Editor, interaction: &str, lifeline: &str) {
    let Some(classifier) = e.doc().get(interaction).and_then(|i| i.parent.clone()) else {
        return;
    };
    let is_classifier = e
        .doc()
        .get(&classifier)
        .is_some_and(|c| !matches!(c.type_name.as_str(), "UMLModel" | "UMLPackage" | "Project"));
    if !is_classifier {
        return;
    }
    let taken = e
        .doc()
        .get(&classifier)
        .map(|c| c.children("attributes").len())
        .unwrap_or(0);
    let role = Element::new(e.new_id(), "UMLAttribute")
        .with("name", Field::str(format!("Role{}", taken + 1)));
    let role = e.append(&classifier, "attributes", vec![role]);
    e.set(lifeline, "represent", Field::Ref(role));
}

/// What an interaction-overview action calls, as StarUML creates it: a new
/// interaction next to the one that owns the overview's activity, and
/// either an interaction use referring to it (Interaction Use; the use goes
/// in the outer interaction's `fragments`) or a sequence diagram inside it
/// (Interaction (Inline)). The action's `target` points at the use or the
/// diagram.
fn add_overview_target(e: &mut Editor, activity: &str, action: &str, tool: &str) {
    let doc = e.doc();
    let outer = doc.get(activity).and_then(|a| a.parent.clone());
    let home = outer
        .as_deref()
        .and_then(|o| doc.get(o))
        .and_then(|o| o.parent.clone())
        .or_else(|| outer.clone())
        .unwrap_or_else(|| doc.root().to_owned());
    let outer_interaction =
        outer.filter(|o| doc.get(o).is_some_and(|o| o.type_name == "UMLInteraction"));
    let name = unique_name(doc, &home, "Interaction");
    let mut tree =
        Tree::new(Element::new(e.new_id(), "UMLInteraction").with("name", Field::str(name)));
    if tool == "UMLInteractionInOverview" {
        let diagram = Element::new(e.new_id(), "UMLSequenceDiagram")
            .with("name", Field::str("SequenceDiagram1"));
        let (_, diagram) = tree.child(0, "ownedElements", diagram);
        e.append(&home, "ownedElements", tree.elements);
        e.set(action, "target", Field::Ref(diagram));
        return;
    }
    let interaction = e.append(&home, "ownedElements", tree.elements);
    let fragments_of = outer_interaction.unwrap_or_else(|| interaction.clone());
    let use_name = unique_name(e.doc(), &fragments_of, "InteractionUse");
    let usage = Element::new(e.new_id(), "UMLInteractionUse")
        .with("name", Field::str(use_name))
        .with("refersTo", Field::Ref(interaction));
    let usage = e.append(&fragments_of, "fragments", vec![usage]);
    e.set(action, "target", Field::Ref(usage));
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
                match item_view {
                    // Regions are boxes, not text rows.
                    "UMLRegionView" => {
                        let v = sub_view(e, item_view).with("model", Field::Ref(item));
                        t.child(ci, "subViews", v);
                    }
                    // An operand is a box with a guard label.
                    "UMLInteractionOperandView" => {
                        let v = sub_view(e, item_view).with("model", Field::Ref(item));
                        let (vi, _) = t.child(ci, "subViews", v);
                        let (_, gid) = t.child(vi, "subViews", sub_view(e, "LabelView"));
                        t.set(vi, "guardLabel", Field::Ref(gid));
                    }
                    _ => {
                        let v = row_view(e, item_view, &item);
                        t.child(ci, "subViews", v);
                    }
                }
            }
        }
        t.set(0, key, Field::Ref(cid));
    }

    if spec.view_type == "UMLSeqLifelineView" {
        // The dashed line below the 40 px head, at the centre (as StarUML arranges it).
        let line = sub_view(e, "UMLLinePartView")
            .with("left", Field::int((x + spec.size.0 / 2.0).round() as i64))
            .with("top", Field::int((y + LIFELINE_HEAD + 1.0).round() as i64))
            .with("width", Field::int(1))
            .with(
                "height",
                Field::int((spec.size.1 - LIFELINE_HEAD - 1.0).round() as i64),
            );
        let (_, lid) = t.child(0, "subViews", line);
        t.set(0, "linePart", Field::Ref(lid));
    }

    let id = e.append(diagram, "ownedViews", t.elements);
    if let Some(c) = container {
        add_contained(e, c, &id);
    }
    id
}

/// Height of a sequence lifeline's head box (StarUML: `SEQ_OBJECT_MINHEIGHT`).
pub const LIFELINE_HEAD: f64 = 40.0;

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
        "UMLInteractionOperandCompartmentView" => Some(("operands", "UMLInteractionOperandView")),
        "ERDColumnCompartmentView" => Some(("columns", "ERDColumnView")),
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
    create_edge_at(e, diagram, spec, tail_view, head_view, None)
}

/// [`create_edge`], with the height where the drag started: sequence messages
/// are horizontal lines at that height.
pub fn create_edge_at(
    e: &mut Editor,
    diagram: &str,
    spec: &EdgeSpec,
    tail_view: &str,
    head_view: &str,
    at_y: Option<f64>,
) -> Result<(Option<Id>, Id), String> {
    // Self transitions and self messages always loop back to where they start.
    let head_view = if matches!(
        spec.id,
        "UMLSelfTransition" | "UMLSelfMessage" | "UMLSelfConnector"
    ) {
        tail_view
    } else {
        head_view
    };
    if spec.view_type == "UMLSeqMessageView" {
        return create_message(e, diagram, spec, tail_view, head_view, at_y);
    }
    if spec.view_type == "UMLCommMessageView" {
        return create_comm_message(e, diagram, spec, tail_view, head_view);
    }
    if spec.view_type == "UMLTimingMessageView" {
        return create_timing_message(e, diagram, spec, tail_view, head_view);
    }
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
            let is_node = |m: &str| e.doc().get(m).is_some_and(|el| el.type_name == "MMNode");
            if model_type == "MMEdge" && !(is_node(&source) && is_node(&target)) {
                return Err(format!("An {} joins two mind map nodes.", spec.label));
            }
            let mut rel = Element::new(e.new_id(), model_type)
                .with("source", Field::Ref(source.clone()))
                .with("target", Field::Ref(target));
            for (k, v) in spec.model_fields {
                rel = rel.with(k, init(*v));
            }
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
            let is_entity = |m: &str| e.doc().get(m).is_some_and(|el| el.type_name == "ERDEntity");
            if model_type == "ERDRelationship" && !(is_entity(&tail) && is_entity(&head)) {
                return Err(format!("A {} joins two entities.", spec.label));
            }
            let (tail, head) = if spec.owner == EdgeOwner::TailRole {
                match (role_of(e.doc(), &tail), role_of(e.doc(), &head)) {
                    (Some(t), Some(h)) => (t, h),
                    _ => return Err(format!("A {} joins two lifelines.", spec.label)),
                }
            } else {
                (tail, head)
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
            for (k, v) in spec.end_fields[0] {
                end1 = end1.with(k, init(*v));
            }
            for (k, v) in spec.end_fields[1] {
                end2 = end2.with(k, init(*v));
            }
            rel.single(0, "end1", end1);
            rel.single(0, "end2", end2);
            Some(e.append(&tail, "ownedElements", rel.elements))
        }
    };
    let view = add_edge_view(e, diagram, spec, model_id.as_deref(), tail_view, head_view);
    Ok((model_id, view))
}

/// The role a lifeline represents (`represent`).
fn role_of(doc: &Document, lifeline: &str) -> Option<Id> {
    doc.get(lifeline)
        .filter(|l| l.type_name == "UMLLifeline")
        .and_then(|l| l.ref_id("represent"))
        .map(str::to_owned)
}

/// A communication-diagram message: a short arrow riding on a connector.
/// Drawn between two lifelines it creates that connector too, as StarUML
/// does; dropped on a connector it rides on that one. A reverse message runs
/// from the connector's head to its tail.
fn create_comm_message(
    e: &mut Editor,
    diagram: &str,
    spec: &EdgeSpec,
    tail_view: &str,
    head_view: &str,
) -> Result<(Option<Id>, Id), String> {
    let on_connector = e
        .doc()
        .get(head_view)
        .filter(|v| v.type_name == "UMLConnectorView")
        .and_then(|v| Some((v.ref_id("tail")?.to_owned(), v.ref_id("head")?.to_owned())));
    let (connector_view, ends) = match on_connector {
        Some(ends) => (head_view.to_owned(), ends),
        None => {
            let connector = edge_spec_on("UMLConnector", "UMLCommunicationDiagram")
                .ok_or("no connector tool")?;
            let (_, cv) = create_edge_at(e, diagram, connector, tail_view, head_view, None)?;
            (cv, (tail_view.to_owned(), head_view.to_owned()))
        }
    };
    let doc = e.doc();
    let model_of = |v: &str| {
        doc.get(v)
            .and_then(|el| el.ref_id("model"))
            .map(str::to_owned)
    };
    let (Some(a), Some(b)) = (model_of(&ends.0), model_of(&ends.1)) else {
        return Err("Messages connect two lifelines, or ride on a connector between them.".into());
    };
    let (source, target) = if spec.id == "UMLReverseMessage" {
        (b, a)
    } else {
        (a, b)
    };
    let (owner, list) = edge_owner(doc, spec, &source)?;
    let name = unique_name(doc, &owner, "Message");
    let connector = model_of(&connector_view);
    // Where the arrow goes: 40 px long, parallel to the line between the
    // lifelines' centres, `distance` to its left. StarUML (and our renderer)
    // place it from the connector when drawing; this is a starting value.
    let centre = |v: &str| {
        doc.get(v).map_or((0.0, 0.0), |el| {
            (
                el.num("left", 0.0) + el.num("width", 0.0) / 2.0,
                el.num("top", 0.0) + el.num("height", 0.0) / 2.0,
            )
        })
    };
    // Messages already on this connector: stack the new one further out so
    // their arrows and labels don't overlap (StarUML would put all at 10 px).
    let stacked = doc
        .get(diagram)
        .map(|d| d.children("ownedViews").to_vec())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| doc.get(v))
        .filter(|v| {
            v.type_name == "UMLCommMessageView"
                && v.ref_id("hostEdge") == Some(connector_view.as_str())
        })
        .count();
    let distance = COMM_MESSAGE_DISTANCE + COMM_MESSAGE_STACK * stacked as f64;
    let ((x1, y1), (x2, y2)) = (centre(&ends.0), centre(&ends.1));
    let len = (x2 - x1).hypot(y2 - y1).max(1.0);
    let (ux, uy) = ((x2 - x1) / len, (y2 - y1) / len);
    let (mx, my) = (
        (x1 + x2) / 2.0 + uy * distance,
        (y1 + y2) / 2.0 - ux * distance,
    );
    let (dx, dy) = ((ux * 20.0).abs(), (uy * 20.0).abs());
    let mut msg = Element::new(e.new_id(), "UMLMessage")
        .with("name", Field::str(name))
        .with("source", Field::Ref(source))
        .with("target", Field::Ref(target));
    if let Some(c) = connector {
        msg = msg.with("connector", Field::Ref(c));
    }

    let model = e.append(&owner, list, vec![msg]);
    let root = view(e, spec.view_type)
        .with("model", Field::Ref(model.clone()))
        .with("left", Field::int((mx - dx).round() as i64))
        .with("top", Field::int((my - dy).round() as i64))
        .with("width", Field::int((2.0 * dx).round() as i64))
        .with("height", Field::int((2.0 * dy).round() as i64))
        // No metamodel defaults for alpha and distance; StarUML's middle
        // position isn't the default (head) either: write all three.
        .with("alpha", Field::Value((PI / 2.0).into()))
        .with("distance", Field::Value(distance.into()))
        .with("hostEdge", Field::Ref(connector_view))
        .with("edgePosition", Field::int(edge_position::MIDDLE));
    let root_id = root.id.clone();
    let mut t = Tree::new(root);
    for (key, distance) in [
        ("nameLabel", 10.0),
        ("stereotypeLabel", 25.0),
        ("propertyLabel", 10.0),
    ] {
        let label = sub_view(e, "NodeLabelView")
            .with("model", Field::Ref(model.clone()))
            .with("hostNode", Field::Ref(root_id.clone()))
            .with("alpha", Field::Value((PI / 2.0).into()))
            .with("distance", Field::Value(distance.into()));
        let (_, lid) = t.child(0, "subViews", label);
        t.set(0, key, Field::Ref(lid));
    }
    let view = e.append(diagram, "ownedViews", t.elements);
    Ok((Some(model), view))
}

/// A timing-diagram message between two time segments, from the lifeline
/// the tail segment covers to the one the head segment covers. Drawn from
/// the end of the tail segment to the start of the head segment.
fn create_timing_message(
    e: &mut Editor,
    diagram: &str,
    spec: &EdgeSpec,
    tail_view: &str,
    head_view: &str,
) -> Result<(Option<Id>, Id), String> {
    let doc = e.doc();
    let segment = |v: &str| doc.get(v).filter(|v| v.type_name == "UMLTimeSegmentView");
    let covered = |v: &str| {
        segment(v)
            .and_then(|s| s.ref_id("model"))
            .and_then(|m| doc.get(m))
            .and_then(|m| m.ref_id("covered"))
            .map(str::to_owned)
    };
    let (Some(source), Some(target)) = (covered(tail_view), covered(head_view)) else {
        return Err("Timing messages connect two time segments.".into());
    };
    let point = |v: &str, at_end: bool| {
        segment(v).map_or((0.0, 0.0), |s| {
            let x = s.num("left", 0.0) + if at_end { s.num("width", 0.0) } else { 0.0 };
            (x, s.num("top", 0.0) + s.num("height", 0.0) / 2.0)
        })
    };
    let points = [point(tail_view, true), point(head_view, false)];
    let (owner, list) = edge_owner(doc, spec, &source)?;
    let name = unique_name(doc, &owner, "Message");
    let msg = Element::new(e.new_id(), "UMLMessage")
        .with("name", Field::str(name))
        .with("source", Field::Ref(source))
        .with("target", Field::Ref(target));
    let model = e.append(&owner, list, vec![msg]);
    let view = add_edge_view(e, diagram, spec, Some(&model), tail_view, head_view);
    e.set(&view, "points", Field::str(format_points(&points)));
    Ok((Some(model), view))
}

/// How far a communication message's arrow sits from its connector
/// (StarUML's default), and how much further each extra message on the same
/// connector goes.
const COMM_MESSAGE_DISTANCE: f64 = 10.0;
const COMM_MESSAGE_STACK: f64 = 35.0;

/// The line part of a lifeline view (messages attach to it), or `view` itself.
fn line_part_of(doc: &Document, view: &str) -> Id {
    doc.get(view)
        .and_then(|v| v.ref_id("linePart"))
        .unwrap_or(view)
        .to_owned()
}

/// Vertical gap between consecutive messages placed without a height.
const MESSAGE_GAP: f64 = 30.0;

/// A sequence message between two lifelines, drawn at `at_y` (or below the
/// lowest message so far). Calls get an activation bar on the receiver.
fn create_message(
    e: &mut Editor,
    diagram: &str,
    spec: &EdgeSpec,
    tail_view: &str,
    head_view: &str,
    at_y: Option<f64>,
) -> Result<(Option<Id>, Id), String> {
    let doc = e.doc();
    let (tail, head) = (line_part_of(doc, tail_view), line_part_of(doc, head_view));
    let lifeline_of = |line: &str| {
        doc.get(line)
            .and_then(|l| l.parent.as_deref())
            .and_then(|lv| doc.get(lv))
            .and_then(|lv| lv.ref_id("model"))
            .map(str::to_owned)
    };
    let (Some(source), Some(target)) = (lifeline_of(&tail), lifeline_of(&head)) else {
        return Err("Messages connect two lifelines.".into());
    };
    let line_x = |line: &str| doc.get(line).map_or(0.0, |l| l.num("left", 0.0));
    let line_top = |line: &str| doc.get(line).map_or(0.0, |l| l.num("top", 0.0));
    let y = at_y.unwrap_or_else(|| {
        // Below every message already on the diagram.
        let lowest = doc
            .get(diagram)
            .map(|d| d.children("ownedViews").to_vec())
            .unwrap_or_default()
            .iter()
            .filter_map(|v| doc.get(v))
            .filter(|v| v.type_name == "UMLSeqMessageView")
            .filter_map(|v| {
                mdj::values::parse_points(v.str("points").unwrap_or(""))
                    .last()
                    .map(|p| p.1)
            })
            .fold(line_top(&tail).max(line_top(&head)), f64::max);
        lowest + MESSAGE_GAP
    });
    let (x1, x2) = (line_x(&tail), line_x(&head));
    let points = if tail == head {
        vec![
            (x1, y),
            (x1 + 40.0, y),
            (x1 + 40.0, y + 20.0),
            (x1, y + 20.0),
        ]
    } else {
        vec![(x1, y), (x2, y)]
    };

    let mut msg = Element::new(e.new_id(), "UMLMessage")
        .with("source", Field::Ref(source.clone()))
        .with("target", Field::Ref(target));
    for (k, v) in spec.model_fields {
        msg = msg.with(k, init(*v));
    }
    let (owner, list) = edge_owner(e.doc(), spec, &source)?;
    let model = e.append(&owner, list, vec![msg]);

    let view = add_edge_view(e, diagram, spec, Some(&model), &tail, &head);
    e.set(&view, "points", Field::str(format_points(&points)));
    let is_call = !spec.model_fields.iter().any(|(k, v)| {
        *k == "messageSort" && matches!(v, Init::Str("reply" | "createMessage" | "deleteMessage"))
    });
    if is_call {
        let ax = x2 - ACTIVATION_W / 2.0;
        let ay = if tail == head { y + 20.0 } else { y };
        let activation = sub_view(e, "UMLActivationView")
            .with("model", Field::Ref(model.clone()))
            .with("left", Field::int(ax.round() as i64))
            .with("top", Field::int(ay.round() as i64))
            .with("width", Field::int(ACTIVATION_W as i64))
            .with("height", Field::int(ACTIVATION_H as i64));
        let aid = e.append(&view, "subViews", vec![activation]);
        e.set(&view, "activation", Field::Ref(aid));
    }
    Ok((Some(model), view))
}

/// Activation bar size (StarUML minimum: 14 × 25).
const ACTIVATION_W: f64 = 14.0;
const ACTIVATION_H: f64 = 30.0;

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
        // Most relationships live with their source, as in StarUML; a
        // communication connector's source is already the tail's role.
        EdgeOwner::Source | EdgeOwner::TailRole => Ok((source.to_owned(), "ownedElements")),
        EdgeOwner::SourceRegion => parent_of_type("UMLRegion")
            .map(|r| (r, "transitions"))
            .ok_or_else(|| "Transitions start from a state or pseudostate.".to_owned()),
        EdgeOwner::SourceInteraction => parent_of_type("UMLInteraction")
            .map(|i| (i, "messages"))
            .ok_or_else(|| "Messages connect lifelines.".to_owned()),
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
        .with("lineStyle", Field::int(spec.line_style))
        .with("points", Field::str(format_points(&[])));
    let root_id = root.id.clone();
    let mut t = Tree::new(root);

    let mut labels: Vec<(&str, i64, f64, f64)> = Vec::new();
    if matches!(spec.labels, EdgeLabels::Name | EdgeLabels::NameAndEndNames) {
        labels.push(("nameLabel", edge_position::MIDDLE, PI / 2.0, 15.0));
    }
    if spec.labels == EdgeLabels::NameAndEndNames {
        labels.extend([
            ("tailNameLabel", edge_position::TAIL, PI / 6.0, 30.0),
            ("headNameLabel", edge_position::HEAD, -PI / 6.0, 30.0),
        ]);
    } else if !matches!(spec.labels, EdgeLabels::None | EdgeLabels::Name) {
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
        // Told apart by cardinality, as the toolbox creates them.
        "ERDRelationshipView" => {
            let many = |k: &str| {
                end(k)
                    .and_then(|e| e.str("cardinality"))
                    .is_some_and(|c| c.ends_with('*'))
            };
            match (many("end1"), many("end2")) {
                (true, true) => "ERDRelationshipManyToMany",
                (_, true) | (true, _) => "ERDRelationshipOneToMany",
                _ => "ERDRelationship",
            }
        }
        "UMLLinkView" if nav("end1") == "navigable" || nav("end2") == "navigable" => {
            "UMLDirectedLink"
        }
        "UMLTransitionView" if edge_view.ref_id("tail") == edge_view.ref_id("head") => {
            "UMLSelfTransition"
        }
        "UMLConnectorView" if edge_view.ref_id("tail") == edge_view.ref_id("head") => {
            "UMLSelfConnector"
        }
        // Reverse when the message runs from the connector's head end.
        "UMLCommMessageView" => {
            let host_tail_model = edge_view
                .ref_id("hostEdge")
                .and_then(|h| doc.get(h))
                .and_then(|h| h.ref_id("tail"))
                .and_then(|t| doc.get(t))
                .and_then(|t| t.ref_id("model"));
            match (model.and_then(|m| m.ref_id("source")), host_tail_model) {
                (Some(source), Some(tail)) if source != tail => "UMLReverseMessage",
                _ => "UMLForwardMessage",
            }
        }
        "UMLSeqMessageView" => match model
            .and_then(|m| m.str("messageSort"))
            .unwrap_or("synchCall")
        {
            _ if edge_view.ref_id("tail") == edge_view.ref_id("head") => "UMLSelfMessage",
            "asynchCall" => "UMLAsyncMessage",
            "asynchSignal" => "UMLAsyncSignalMessage",
            "reply" => "UMLReplyMessage",
            "createMessage" => "UMLCreateMessage",
            "deleteMessage" => "UMLDeleteMessage",
            _ => "UMLMessage",
        },
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
    /// An operand of a combined fragment (alt/par/…).
    Operand,
    /// A column of an ERD entity.
    Column,
}

impl FeatureKind {
    pub fn list_key(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attributes",
            FeatureKind::Operation => "operations",
            FeatureKind::Literal => "literals",
            FeatureKind::Slot => "slots",
            FeatureKind::Operand => "operands",
            FeatureKind::Column => "columns",
        }
    }
    fn model_type(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "UMLAttribute",
            FeatureKind::Operation => "UMLOperation",
            FeatureKind::Literal => "UMLEnumerationLiteral",
            FeatureKind::Slot => "UMLSlot",
            FeatureKind::Operand => "UMLInteractionOperand",
            FeatureKind::Column => "ERDColumn",
        }
    }
    fn compartment_key(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attributeCompartment",
            FeatureKind::Operation => "operationCompartment",
            FeatureKind::Literal => "enumerationLiteralCompartment",
            FeatureKind::Slot => "slotCompartment",
            FeatureKind::Operand => "operandCompartment",
            FeatureKind::Column => "columnCompartment",
        }
    }
    fn row_view(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "UMLAttributeView",
            FeatureKind::Operation => "UMLOperationView",
            FeatureKind::Literal => "UMLEnumerationLiteralView",
            FeatureKind::Slot => "UMLSlotView",
            FeatureKind::Operand => "UMLInteractionOperandView",
            FeatureKind::Column => "ERDColumnView",
        }
    }
    fn base_name(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attribute",
            FeatureKind::Operation => "operation",
            FeatureKind::Literal => "literal",
            FeatureKind::Slot => "slot",
            FeatureKind::Operand => "operand",
            // StarUML: the type name without `ERD`.
            FeatureKind::Column => "Column",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "attribute" => FeatureKind::Attribute,
            "operation" => FeatureKind::Operation,
            "literal" => FeatureKind::Literal,
            "slot" => FeatureKind::Slot,
            "operand" => FeatureKind::Operand,
            "column" => FeatureKind::Column,
            _ => return None,
        })
    }

    /// Whether elements of `model_type` can have this kind of feature.
    pub fn allowed_on(self, model_type: &str) -> bool {
        match self {
            FeatureKind::Literal => model_type == "UMLEnumeration",
            FeatureKind::Operand => model_type == "UMLCombinedFragment",
            FeatureKind::Column => model_type == "ERDEntity",
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
        if kind == FeatureKind::Operand {
            // An operand is a box with a guard label, not a text row.
            let v = sub_view(e, kind.row_view()).with("model", Field::Ref(feature_id.clone()));
            let mut t = Tree::new(v);
            let (_, gid) = t.child(0, "subViews", sub_view(e, "LabelView"));
            t.set(0, "guardLabel", Field::Ref(gid));
            e.append(&comp, "subViews", t.elements);
        } else {
            let row = row_view(e, kind.row_view(), &feature_id);
            e.append(&comp, "subViews", vec![row]);
        }
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
/// an activity diagram in an activity, a profile diagram in a profile, a
/// sequence or communication diagram in an interaction, an interaction
/// overview diagram in an activity inside an interaction — unless `parent`
/// already is the right kind. Returns the diagram's id.
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
    const INTERACTION: (&str, &str) = ("UMLInteraction", "Interaction");
    const ACTIVITY: (&str, &str) = ("UMLActivity", "Activity");
    // Wrappers to create, outermost first.
    let wrappers: &[(&str, &str)] = match (diagram_type, parent_type.as_str()) {
        ("UMLStatechartDiagram", p) if p != "UMLStateMachine" => {
            &[("UMLStateMachine", "StateMachine")]
        }
        ("UMLActivityDiagram", p) if p != "UMLActivity" => &[ACTIVITY],
        ("UMLProfileDiagram", p) if p != "UMLProfile" => &[("UMLProfile", "Profile")],
        ("UMLSequenceDiagram" | "UMLCommunicationDiagram" | "UMLTimingDiagram", p)
            if p != "UMLInteraction" =>
        {
            &[INTERACTION]
        }
        ("FCFlowchartDiagram", p) if p != "FCFlowchart" => &[("FCFlowchart", "Flowchart")],
        ("ERDDiagram", p) if p != "ERDDataModel" => &[("ERDDataModel", "Data Model")],
        ("MMMindmapDiagram", p) if p != "MMMindmap" => &[("MMMindmap", "Mindmap")],
        ("UMLInteractionOverviewDiagram", "UMLActivity") => &[],
        ("UMLInteractionOverviewDiagram", "UMLInteraction") => &[ACTIVITY],
        ("UMLInteractionOverviewDiagram", _) => &[INTERACTION, ACTIVITY],
        _ => &[],
    };
    // An interaction sits in a collaboration unless added to a classifier.
    let mut owner = if wrappers.first() == Some(&INTERACTION)
        && matches!(
            parent_type.as_str(),
            "UMLModel" | "UMLPackage" | "Project" | "UMLSubsystem"
        ) {
        let name = unique_name(e.doc(), parent, "Collaboration");
        let collab = Element::new(e.new_id(), "UMLCollaboration").with("name", Field::str(name));
        e.append(parent, "ownedElements", vec![collab])
    } else {
        parent.to_owned()
    };
    for &(wrapper_type, wrapper_base) in wrappers {
        let name = unique_name(e.doc(), &owner, wrapper_base);
        let mut tree =
            Tree::new(Element::new(e.new_id(), wrapper_type).with("name", Field::str(name)));
        if wrapper_type == "UMLStateMachine" {
            let region = Element::new(e.new_id(), "UMLRegion");
            tree.child(0, "regions", region);
        }
        owner = e.append(&owner, "ownedElements", tree.elements);
    }
    let name = unique_name(e.doc(), &owner, base);
    let diagram = Element::new(e.new_id(), diagram_type).with("name", Field::str(name));
    let id = e.append(&owner, "ownedElements", vec![diagram]);
    match diagram_type {
        "UMLSequenceDiagram" | "UMLCommunicationDiagram" | "UMLInteractionOverviewDiagram" => {
            add_frame(e, &id, "UMLFrameView", (8, 8, 700, 600));
        }
        // StarUML's _addTimingFrame: initialised from (5, 5) to (700, 600).
        "UMLTimingDiagram" => add_frame(e, &id, "UMLTimingFrameView", (5, 5, 695, 595)),
        _ => {}
    }
    Ok(id)
}

/// The frame StarUML puts on a new interaction diagram: a `view_type` view
/// whose model is the diagram itself, at the given place and size, with
/// name and frame-type labels.
fn add_frame(e: &mut Editor, diagram: &str, view_type: &str, (x, y, w, h): (i64, i64, i64, i64)) {
    let frame = view(e, view_type)
        .with("model", Field::Ref(diagram.to_owned()))
        .with("left", Field::int(x))
        .with("top", Field::int(y))
        .with("width", Field::int(w))
        .with("height", Field::int(h));
    let mut t = Tree::new(frame);
    for key in ["nameLabel", "frameTypeLabel"] {
        let (_, lid) = t.child(0, "subViews", sub_view(e, "LabelView"));
        t.set(0, key, Field::Ref(lid));
    }
    e.append(diagram, "ownedViews", t.elements);
}
