//! Creating and querying UML class-diagram elements in the shape StarUML
//! stores them (see docs/evidence.md §3).

use std::collections::HashSet;

use mdj::values::{DEFAULT_FONT, align, edge_position, format_points, line_style};
use mdj::{Document, Editor, Element, Field, Id};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Class,
    Interface,
    Enumeration,
    DataType,
    Package,
}

impl NodeKind {
    pub const ALL: [NodeKind; 5] = [
        NodeKind::Class,
        NodeKind::Interface,
        NodeKind::Enumeration,
        NodeKind::DataType,
        NodeKind::Package,
    ];

    pub fn model_type(self) -> &'static str {
        match self {
            NodeKind::Class => "UMLClass",
            NodeKind::Interface => "UMLInterface",
            NodeKind::Enumeration => "UMLEnumeration",
            NodeKind::DataType => "UMLDataType",
            NodeKind::Package => "UMLPackage",
        }
    }

    pub fn view_type(self) -> &'static str {
        match self {
            NodeKind::Class => "UMLClassView",
            NodeKind::Interface => "UMLInterfaceView",
            NodeKind::Enumeration => "UMLEnumerationView",
            NodeKind::DataType => "UMLDataTypeView",
            NodeKind::Package => "UMLPackageView",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            NodeKind::Class => "Class",
            NodeKind::Interface => "Interface",
            NodeKind::Enumeration => "Enumeration",
            NodeKind::DataType => "DataType",
            NodeKind::Package => "Package",
        }
    }

    pub fn from_view_type(t: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.view_type() == t)
    }

    pub fn is_classifier(self) -> bool {
        self != NodeKind::Package
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Association,
    DirectedAssociation,
    Aggregation,
    Composition,
    Generalization,
    Dependency,
    InterfaceRealization,
}

impl EdgeKind {
    pub const ALL: [EdgeKind; 7] = [
        EdgeKind::Association,
        EdgeKind::DirectedAssociation,
        EdgeKind::Aggregation,
        EdgeKind::Composition,
        EdgeKind::Generalization,
        EdgeKind::Dependency,
        EdgeKind::InterfaceRealization,
    ];

    pub fn model_type(self) -> &'static str {
        match self {
            EdgeKind::Association
            | EdgeKind::DirectedAssociation
            | EdgeKind::Aggregation
            | EdgeKind::Composition => "UMLAssociation",
            EdgeKind::Generalization => "UMLGeneralization",
            EdgeKind::Dependency => "UMLDependency",
            EdgeKind::InterfaceRealization => "UMLInterfaceRealization",
        }
    }

    pub fn view_type(self) -> &'static str {
        match self {
            EdgeKind::Association
            | EdgeKind::DirectedAssociation
            | EdgeKind::Aggregation
            | EdgeKind::Composition => "UMLAssociationView",
            EdgeKind::Generalization => "UMLGeneralizationView",
            EdgeKind::Dependency => "UMLDependencyView",
            EdgeKind::InterfaceRealization => "UMLInterfaceRealizationView",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EdgeKind::Association => "Association",
            EdgeKind::DirectedAssociation => "Directed Association",
            EdgeKind::Aggregation => "Aggregation",
            EdgeKind::Composition => "Composition",
            EdgeKind::Generalization => "Generalization",
            EdgeKind::Dependency => "Dependency",
            EdgeKind::InterfaceRealization => "Interface Realization",
        }
    }
}

// ---------------------------------------------------------------------------
// Building subtrees

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
    /// Returns the child's index and id.
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

fn view(e: &mut Editor, type_name: &str) -> Element {
    Element::new(e.new_id(), type_name).with("font", Field::str(DEFAULT_FONT))
}

fn sub_view(e: &mut Editor, type_name: &str) -> Element {
    view(e, type_name).with("parentStyle", Field::bool(true))
}

/// One row of a list compartment (an attribute, operation or literal).
/// StarUML draws rows left-aligned but would load a missing alignment as
/// centred (see `mdj::values::align`), so store it.
fn row_view(e: &mut Editor, type_name: &str, model: &str) -> Element {
    sub_view(e, type_name)
        .with("model", Field::Ref(model.to_owned()))
        .with("horizontalAlignment", Field::int(align::LEFT))
}

/// The diagram's owner is where new model elements created on it go.
pub fn owner_for_new_elements(doc: &Document, diagram: &str) -> Id {
    doc.get(diagram)
        .and_then(|d| d.parent.clone())
        .unwrap_or_else(|| doc.root().to_owned())
}

/// Next free "Base1", "Base2", … name among `parent`'s owned elements.
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

/// Create a model element and a view of it on `diagram` at `(x, y)`.
/// Returns `(model_id, view_id)`.
pub fn create_node(e: &mut Editor, diagram: &str, kind: NodeKind, x: f64, y: f64) -> (Id, Id) {
    let owner = owner_for_new_elements(e.doc(), diagram);
    let name = unique_name(e.doc(), &owner, kind.label());
    let model = Element::new(e.new_id(), kind.model_type()).with("name", Field::str(name));
    let model_id = e.append(&owner, "ownedElements", vec![model]);
    let view_id = add_node_view(e, diagram, kind, &model_id, x, y);
    (model_id, view_id)
}

/// Add a view of an existing model element to `diagram`.
pub fn add_node_view(
    e: &mut Editor,
    diagram: &str,
    kind: NodeKind,
    model: &str,
    x: f64,
    y: f64,
) -> Id {
    let mut root = view(e, kind.view_type())
        .with("model", Field::Ref(model.to_owned()))
        .with("left", Field::int(x.round() as i64))
        .with("top", Field::int(y.round() as i64))
        .with(
            "width",
            Field::int(if kind == NodeKind::Package { 120 } else { 140 }),
        )
        .with(
            "height",
            Field::int(if kind == NodeKind::Package { 80 } else { 60 }),
        );
    if kind == NodeKind::Interface {
        // StarUML draws interfaces as a circle unless told otherwise; this
        // renderer draws the boxed «interface» form, so pin that.
        root = root.with("stereotypeDisplay", Field::str("label"));
    }
    let mut t = Tree::new(root);

    let name_comp = sub_view(e, "UMLNameCompartmentView").with("model", Field::Ref(model.into()));
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
            label = label.with(
                "text",
                Field::str(e.doc().get(model).map(Element::name).unwrap_or("")),
            );
        }
        let (_, lid) = t.child(nc, "subViews", label);
        t.set(nc, key, Field::Ref(lid));
    }
    t.set(0, "nameCompartment", Field::Ref(nc_id));

    if kind.is_classifier() {
        let mut compartments = vec![
            ("attributeCompartment", "UMLAttributeCompartmentView"),
            ("operationCompartment", "UMLOperationCompartmentView"),
            ("receptionCompartment", "UMLReceptionCompartmentView"),
            (
                "templateParameterCompartment",
                "UMLTemplateParameterCompartmentView",
            ),
        ];
        if kind == NodeKind::Enumeration {
            compartments.push((
                "enumerationLiteralCompartment",
                "UMLEnumerationLiteralCompartmentView",
            ));
        }
        for (key, type_name) in compartments {
            let comp = sub_view(e, type_name).with("model", Field::Ref(model.into()));
            let (ci, cid) = t.child(0, "subViews", comp);
            if let Some((list_key, item_view)) = compartment_items(type_name) {
                let items = e
                    .doc()
                    .get(model)
                    .map(|m| m.children(list_key).to_vec())
                    .unwrap_or_default();
                for item in items {
                    let v = row_view(e, item_view, &item);
                    t.child(ci, "subViews", v);
                }
            }
            t.set(0, key, Field::Ref(cid));
        }
    }
    e.append(diagram, "ownedViews", t.elements)
}

/// For a list compartment type: the model field it lists and the view type of each row.
fn compartment_items(compartment_type: &str) -> Option<(&'static str, &'static str)> {
    match compartment_type {
        "UMLAttributeCompartmentView" => Some(("attributes", "UMLAttributeView")),
        "UMLOperationCompartmentView" => Some(("operations", "UMLOperationView")),
        "UMLEnumerationLiteralCompartmentView" => Some(("literals", "UMLEnumerationLiteralView")),
        _ => None,
    }
}

/// Create a relationship between the models shown by two views, plus its edge
/// view. `tail` is where the drag started. Returns `(model_id, view_id)`, or
/// `None` if either view has no model.
pub fn create_edge(
    e: &mut Editor,
    diagram: &str,
    kind: EdgeKind,
    tail_view: &str,
    head_view: &str,
) -> Option<(Id, Id)> {
    let tail_model = e.doc().get(tail_view)?.ref_id("model")?.to_owned();
    let head_model = e.doc().get(head_view)?.ref_id("model")?.to_owned();

    let mut rel = Tree::new(Element::new(e.new_id(), kind.model_type()));
    match kind {
        EdgeKind::Generalization | EdgeKind::Dependency | EdgeKind::InterfaceRealization => {
            rel.set(0, "source", Field::Ref(tail_model.clone()));
            rel.set(0, "target", Field::Ref(head_model.clone()));
        }
        _ => {
            let (tail_agg, head_nav) = match kind {
                EdgeKind::Association => ("none", "unspecified"),
                EdgeKind::DirectedAssociation => ("none", "navigable"),
                EdgeKind::Aggregation => ("shared", "navigable"),
                _ => ("composite", "navigable"),
            };
            let end1 = Element::new(e.new_id(), "UMLAssociationEnd")
                .with("reference", Field::Ref(tail_model.clone()))
                .with("aggregation", Field::str(tail_agg));
            let end2 = Element::new(e.new_id(), "UMLAssociationEnd")
                .with("reference", Field::Ref(head_model.clone()))
                .with("navigable", Field::str(head_nav));
            rel.single(0, "end1", end1);
            rel.single(0, "end2", end2);
        }
    }
    // Relationships live with their source, as in StarUML.
    let model_id = e.append(&tail_model, "ownedElements", rel.elements);
    let view_id = add_edge_view(e, diagram, kind, &model_id, tail_view, head_view);
    Some((model_id, view_id))
}

pub fn add_edge_view(
    e: &mut Editor,
    diagram: &str,
    kind: EdgeKind,
    model: &str,
    tail_view: &str,
    head_view: &str,
) -> Id {
    let root = view(e, kind.view_type())
        .with("model", Field::Ref(model.to_owned()))
        .with("head", Field::Ref(head_view.to_owned()))
        .with("tail", Field::Ref(tail_view.to_owned()))
        .with("lineStyle", Field::int(line_style::OBLIQUE))
        .with("points", Field::str(format_points(&[])));
    let root_id = root.id.clone();
    let mut t = Tree::new(root);

    use std::f64::consts::PI;
    let mut labels = vec![
        ("nameLabel", edge_position::MIDDLE, PI / 2.0, 15.0),
        ("stereotypeLabel", edge_position::MIDDLE, PI / 2.0, 30.0),
        ("propertyLabel", edge_position::MIDDLE, -PI / 2.0, 15.0),
    ];
    if kind.view_type() == "UMLAssociationView" {
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
        let label = sub_view(e, "EdgeLabelView")
            .with("model", Field::Ref(model.to_owned()))
            .with("hostEdge", Field::Ref(root_id.clone()))
            .with("edgePosition", Field::int(pos))
            .with("alpha", Field::Value(alpha.into()))
            .with("distance", Field::Value(distance.into()));
        let (_, lid) = t.child(0, "subViews", label);
        t.set(0, key, Field::Ref(lid));
    }
    if kind.view_type() == "UMLAssociationView" {
        for key in ["tailQualifiersCompartment", "headQualifiersCompartment"] {
            let comp =
                sub_view(e, "UMLQualifierCompartmentView").with("model", Field::Ref(model.into()));
            let (_, cid) = t.child(0, "subViews", comp);
            t.set(0, key, Field::Ref(cid));
        }
    }
    e.append(diagram, "ownedViews", t.elements)
}

/// Which `EdgeKind` a relationship model + edge view represents.
pub fn edge_kind_of(doc: &Document, edge_view: &Element) -> Option<EdgeKind> {
    let model = doc.get(edge_view.ref_id("model")?)?;
    Some(match model.type_name.as_str() {
        "UMLGeneralization" => EdgeKind::Generalization,
        "UMLDependency" => EdgeKind::Dependency,
        "UMLInterfaceRealization" => EdgeKind::InterfaceRealization,
        "UMLAssociation" => {
            let end = |k: &str| {
                model
                    .get(k)
                    .and_then(|f| f.children().first())
                    .and_then(|id| doc.get(id))
            };
            let agg = |k: &str| end(k).and_then(|e| e.str("aggregation")).unwrap_or("none");
            let nav = |k: &str| {
                end(k)
                    .and_then(|e| e.str("navigable"))
                    .unwrap_or("unspecified")
            };
            if agg("end1") == "composite" || agg("end2") == "composite" {
                EdgeKind::Composition
            } else if agg("end1") == "shared" || agg("end2") == "shared" {
                EdgeKind::Aggregation
            } else if nav("end1") == "navigable" || nav("end2") == "navigable" {
                EdgeKind::DirectedAssociation
            } else {
                EdgeKind::Association
            }
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Features: attributes, operations, literals

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureKind {
    Attribute,
    Operation,
    Literal,
}

impl FeatureKind {
    fn list_key(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attributes",
            FeatureKind::Operation => "operations",
            FeatureKind::Literal => "literals",
        }
    }
    fn model_type(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "UMLAttribute",
            FeatureKind::Operation => "UMLOperation",
            FeatureKind::Literal => "UMLEnumerationLiteral",
        }
    }
    fn compartment_key(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attributeCompartment",
            FeatureKind::Operation => "operationCompartment",
            FeatureKind::Literal => "enumerationLiteralCompartment",
        }
    }
    fn row_view(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "UMLAttributeView",
            FeatureKind::Operation => "UMLOperationView",
            FeatureKind::Literal => "UMLEnumerationLiteralView",
        }
    }
    fn base_name(self) -> &'static str {
        match self {
            FeatureKind::Attribute => "attribute",
            FeatureKind::Operation => "operation",
            FeatureKind::Literal => "literal",
        }
    }
}

/// Add an attribute/operation/literal to `classifier`, and a row for it in
/// every view of that classifier. Returns the new feature's id.
pub fn add_feature(e: &mut Editor, classifier: &str, kind: FeatureKind) -> Id {
    let existing = e
        .doc()
        .get(classifier)
        .map(|c| c.children(kind.list_key()).len())
        .unwrap_or(0);
    let name = format!("{}{}", kind.base_name(), existing + 1);
    let feature = Element::new(e.new_id(), kind.model_type()).with("name", Field::str(name));
    let feature_id = e.append(classifier, kind.list_key(), vec![feature]);

    let compartments: Vec<Id> = views_of(e.doc(), classifier)
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

/// Add a parameter to an operation. A `return` parameter replaces nothing:
/// callers should check [`return_parameter`] first.
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

// ---------------------------------------------------------------------------
// Deleting

/// Delete views only (StarUML's "Delete"), plus edges left without an end.
pub fn delete_views(e: &mut Editor, views: &[Id]) {
    let mut gone: HashSet<Id> = views.iter().cloned().collect();
    loop {
        let before = gone.len();
        for el in e.doc().iter() {
            let dangling = ["head", "tail", "hostEdge"]
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
/// every view showing them, relationships that would lose an end, and edges
/// that would lose a node. Remaining references to deleted elements are
/// cleared; a deleted type reference becomes its plain name.
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
                || hit("head")
                || hit("tail")
                || hit("hostEdge")
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
                    let replacement = (key == "type")
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

// ---------------------------------------------------------------------------
// Text shown for elements

pub fn visibility_symbol(v: &str) -> &'static str {
    match v {
        "private" => "-",
        "protected" => "#",
        "package" => "~",
        _ => "+",
    }
}

/// A `var` field: either a reference (show its name) or plain text.
pub fn var_text<'a>(doc: &'a Document, el: &'a Element, key: &str) -> &'a str {
    match el.get(key) {
        Some(Field::Ref(id)) => doc.get(id).map(Element::name).unwrap_or(""),
        Some(f) => f.as_str().unwrap_or(""),
        None => "",
    }
}

pub fn attribute_text(doc: &Document, attr: &Element) -> String {
    let mut s = format!(
        "{}{}",
        visibility_symbol(attr.str("visibility").unwrap_or("public")),
        attr.name()
    );
    let ty = var_text(doc, attr, "type");
    if !ty.is_empty() {
        s += &format!(": {ty}");
    }
    let mult = attr.str("multiplicity").unwrap_or("");
    if !mult.is_empty() {
        s += &format!("[{mult}]");
    }
    let default = attr.str("defaultValue").unwrap_or("");
    if !default.is_empty() {
        s += &format!(" = {default}");
    }
    s
}

pub fn operation_text(doc: &Document, op: &Element) -> String {
    let mut params = Vec::new();
    let mut ret = String::new();
    for p in op
        .children("parameters")
        .iter()
        .filter_map(|id| doc.get(id))
    {
        let ty = var_text(doc, p, "type");
        match p.str("direction").unwrap_or("in") {
            "return" => ret = ty.to_owned(),
            dir => {
                let prefix = if dir == "in" {
                    String::new()
                } else {
                    format!("{dir} ")
                };
                params.push(if ty.is_empty() {
                    format!("{prefix}{}", p.name())
                } else {
                    format!("{prefix}{}: {ty}", p.name())
                });
            }
        }
    }
    let mut s = format!(
        "{}{}({})",
        visibility_symbol(op.str("visibility").unwrap_or("public")),
        op.name(),
        params.join(", ")
    );
    if !ret.is_empty() {
        s += &format!(": {ret}");
    }
    s
}

/// `«stereotype»` text for an element, or `None`.
pub fn stereotype_text(doc: &Document, el: &Element) -> Option<String> {
    let s = var_text(doc, el, "stereotype");
    (!s.is_empty()).then(|| format!("«{s}»"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Document, Id) {
        let doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let diagram = doc.get(&model).unwrap().children("ownedElements")[0].clone();
        (doc, diagram)
    }

    #[test]
    fn class_view_has_staruml_compartments() {
        let (mut doc, dgm) = setup();
        let (class, view) = doc.edit(|e| create_node(e, &dgm, NodeKind::Class, 10.0, 20.0));
        doc.edit(|e| add_feature(e, &class, FeatureKind::Attribute));

        let v = doc.get(&view).unwrap();
        assert_eq!(v.ref_id("model"), Some(class.as_str()));
        let nc = doc.get(v.ref_id("nameCompartment").unwrap()).unwrap();
        assert_eq!(nc.type_name, "UMLNameCompartmentView");
        assert_eq!(nc.parent.as_deref(), Some(view.as_str()));
        let ac = doc.get(v.ref_id("attributeCompartment").unwrap()).unwrap();
        assert_eq!(ac.children("subViews").len(), 1, "new attribute gets a row");
        // StarUML would load a missing alignment as centred; rows must say left.
        let row = doc.get(&ac.children("subViews")[0]).unwrap();
        assert_eq!(row.num("horizontalAlignment", -1.0), align::LEFT as f64);
        assert_eq!(doc.get(&class).unwrap().name(), "Class1");
    }

    #[test]
    fn delete_from_model_cascades_and_undoes() {
        let (mut doc, dgm) = setup();
        let (a, va) = doc.edit(|e| create_node(e, &dgm, NodeKind::Class, 0.0, 0.0));
        let (b, vb) = doc.edit(|e| create_node(e, &dgm, NodeKind::Class, 300.0, 0.0));
        let attr = doc.edit(|e| add_feature(e, &a, FeatureKind::Attribute));
        doc.edit(|e| e.set(&attr, "type", Field::Ref(b.clone())));
        let (rel, ev) = doc
            .edit(|e| create_edge(e, &dgm, EdgeKind::Generalization, &va, &vb))
            .unwrap();
        let before = doc.to_value();

        doc.edit(|e| delete_elements(e, std::slice::from_ref(&b)));
        for gone in [&b, &vb, &rel, &ev] {
            assert!(doc.get(gone).is_none(), "{gone} should be deleted");
        }
        assert!(doc.get(&va).is_some());
        assert_eq!(doc.get(&attr).unwrap().str("type"), Some("Class2"));

        doc.undo();
        assert_eq!(doc.to_value(), before);
    }

    #[test]
    fn association_kind_survives_round_trip() {
        let (mut doc, dgm) = setup();
        let (_, va) = doc.edit(|e| create_node(e, &dgm, NodeKind::Class, 0.0, 0.0));
        let (_, vb) = doc.edit(|e| create_node(e, &dgm, NodeKind::Class, 300.0, 0.0));
        for kind in EdgeKind::ALL {
            let (_, ev) = doc.edit(|e| create_edge(e, &dgm, kind, &va, &vb)).unwrap();
            let reloaded = Document::from_json_str(&doc.to_json_string().unwrap()).unwrap();
            assert_eq!(
                edge_kind_of(&reloaded, reloaded.get(&ev).unwrap()),
                Some(kind)
            );
        }
    }

    #[test]
    fn feature_text() {
        let (mut doc, dgm) = setup();
        let (c, _) = doc.edit(|e| create_node(e, &dgm, NodeKind::Class, 0.0, 0.0));
        let op = doc.edit(|e| add_feature(e, &c, FeatureKind::Operation));
        doc.edit(|e| {
            e.set(&op, "visibility", Field::str("private"));
            let p = Element::new(e.new_id(), "UMLParameter")
                .with("name", Field::str("n"))
                .with("type", Field::str("int"));
            let r = Element::new(e.new_id(), "UMLParameter")
                .with("type", Field::str("bool"))
                .with("direction", Field::str("return"));
            e.append(&op, "parameters", vec![p]);
            e.append(&op, "parameters", vec![r]);
        });
        assert_eq!(
            operation_text(&doc, doc.get(&op).unwrap()),
            "-operation1(n: int): bool"
        );
    }
}
