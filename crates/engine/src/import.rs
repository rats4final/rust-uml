//! `import.mermaid`: Mermaid text → a new diagram, built from the ordinary
//! commands (so it is exactly what a user could have drawn), laid out
//! automatically, and undone as one step.
//!
//! Mappings follow what StarUML's own Mermaid import produces (evidence:
//! `extensions/default/mermaid`, behaviour only), except that composite
//! states keep their nesting and namespaces become package boxes.

use std::collections::HashMap;

use mdj::{Field, Id};
use mermaid::class::{ClassDiagram, End};
use mermaid::er::{Cardinality, ErDiagram};
use mermaid::flowchart::{self, Flowchart, Shape, Stroke};
use mermaid::sequence::{Arrow, Block, Event, NotePlace, SequenceDiagram};
use mermaid::state::{Kind, Point, StateDiagram};
use mermaid::{Diagram, Direction};
use render::autolayout::{self, Flow, Spacing};
use serde_json::{Value, json};

use crate::commands::{
    CommandError, add_diagram, add_feature, add_parameter, create_edge, create_node, move_views,
    resize_view, set_property,
};
use crate::params;
use crate::session::Session;

/// Where the first box goes.
const ORIGIN: (f64, f64) = (40.0, 40.0);
/// Space inside a package or composite state around its contents.
const PAD: f64 = 20.0;
/// Room for a container's name above its contents.
const HEADER: f64 = 40.0;

pub(crate) fn import_mermaid(s: &mut Session, p: &Value) -> Result<Value, CommandError> {
    let text = params::str(p, "text")?;
    let parsed = mermaid::parse(text).map_err(|e| CommandError::Param(format!("Mermaid {e}")))?;
    let name = match params::opt_str(p, "name") {
        Some(n) => Some(n.to_owned()),
        None => parsed.title.clone(),
    };
    let parent = params::opt_str(p, "parent").map(str::to_owned);

    let saved = (s.doc.clone(), s.active_diagram.clone(), s.selection.clone());
    let depth = s.doc.undo_depth();
    let mut b = Builder {
        s,
        warnings: parsed.warnings,
    };
    let built = match &parsed.diagram {
        Diagram::Class(d) => b.class(d, name, parent),
        Diagram::Sequence(d) => b.sequence(d, name, parent),
        Diagram::State(d) => b.state(d, name, parent),
        Diagram::Flowchart(d) => b.flowchart(d, name, parent),
        Diagram::Er(d) => b.er(d, name, parent),
    };
    let warnings = b.warnings;
    match built {
        Ok(diagram) => {
            s.doc.squash_undo_since(depth);
            s.active_diagram = Some(diagram.clone());
            s.selection.clear();
            Ok(json!({ "diagram": diagram, "warnings": warnings }))
        }
        Err(e) => {
            (s.doc, s.active_diagram, s.selection) = saved;
            Err(e)
        }
    }
}

type CommandFn = fn(&mut Session, &Value) -> Result<Value, CommandError>;

struct Builder<'a> {
    s: &'a mut Session,
    warnings: Vec<String>,
}

/// A line to draw once its boxes are in place (drawn earlier, it would be
/// routed around boxes that are all still piled at the origin).
struct Line {
    kind: &'static str,
    tail: Id,
    head: Id,
    label: Option<String>,
    /// Association end fields: (`end1`/`end2`, values).
    ends: Vec<(&'static str, Value)>,
    /// Fields of the relationship itself.
    values: Value,
}

impl Line {
    fn plain(kind: &'static str, tail: Id, head: Id, label: Option<String>) -> Self {
        Self {
            kind,
            tail,
            head,
            label,
            ends: Vec::new(),
            values: json!({}),
        }
    }
}

/// A box to lay out: its view and the box it sits in (if any).
struct Item {
    view: Id,
    within: Option<usize>,
}

fn flow(d: Direction) -> Flow {
    match d {
        Direction::TopDown => Flow::Down,
        Direction::BottomUp => Flow::Up,
        Direction::LeftRight => Flow::Right,
        Direction::RightLeft => Flow::Left,
    }
}

fn id_of(v: &Value, key: &str) -> Result<Id, CommandError> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| CommandError::Failed(format!("import: no `{key}` in {v}")))
}

impl Builder<'_> {
    fn run(&mut self, f: CommandFn, p: Value) -> Result<Value, CommandError> {
        f(self.s, &p)
    }

    fn diagram(
        &mut self,
        ty: &str,
        name: Option<String>,
        parent: Option<String>,
    ) -> Result<Id, CommandError> {
        let r = self.run(
            add_diagram,
            json!({ "type": ty, "name": name, "parent": parent }),
        )?;
        id_of(&r, "id")
    }

    /// Create a box; returns (model, view).
    fn node(
        &mut self,
        kind: &str,
        name: Option<&str>,
        container: Option<&str>,
        at: (f64, f64),
    ) -> Result<(Option<Id>, Id), CommandError> {
        let r = self.run(
            create_node,
            json!({ "kind": kind, "x": at.0, "y": at.1, "name": name, "container": container }),
        )?;
        let model = r.get("model").and_then(Value::as_str).map(str::to_owned);
        Ok((model, id_of(&r, "view")?))
    }

    fn edge(
        &mut self,
        kind: &str,
        tail: &str,
        head: &str,
        name: Option<&str>,
        y: Option<f64>,
    ) -> Result<(Option<Id>, Id), CommandError> {
        let r = self.run(
            create_edge,
            json!({ "kind": kind, "tail": tail, "head": head, "name": name, "y": y }),
        )?;
        let model = r.get("model").and_then(Value::as_str).map(str::to_owned);
        Ok((model, id_of(&r, "view")?))
    }

    fn set(&mut self, id: &str, values: Value) -> Result<(), CommandError> {
        let empty = values.as_object().is_none_or(|o| o.is_empty());
        if !empty {
            self.run(set_property, json!({ "id": id, "values": values }))?;
        }
        Ok(())
    }

    fn note(&mut self, text: &str, container: Option<&str>) -> Result<Id, CommandError> {
        let (_, view) = self.node("Note", None, container, ORIGIN)?;
        self.set(&view, json!({ "text": text }))?;
        Ok(view)
    }

    fn rect(&self, view: &str) -> render::Rect {
        self.s
            .doc
            .get(view)
            .and_then(|v| render::node_layout(&self.s.doc, v, self.s.fonts()))
            .map(|l| l.rect)
            .unwrap_or_default()
    }

    fn child(&self, id: &str, key: &str) -> Option<Id> {
        match self.s.doc.get(id)?.get(key)? {
            Field::Child(c) => Some(c.clone()),
            _ => None,
        }
    }

    /// Lay out `items` (boxes inside boxes allowed) with `edges` between
    /// them: contents first, each container grown around its contents,
    /// then everything moved into place, outermost first.
    fn arrange(
        &mut self,
        items: &[Item],
        edges: &[(usize, usize)],
        flow: Flow,
        gap: Spacing,
    ) -> Result<(), CommandError> {
        let n = items.len();
        let mut size: Vec<(f64, f64)> = items
            .iter()
            .map(|it| {
                let r = self.rect(&it.view);
                (r.w, r.h)
            })
            .collect();
        // Offsets of each item inside its container (or the diagram).
        let mut offset = vec![(0.0, 0.0); n];
        // The chain of containers of each item, innermost first.
        let chain = |mut i: usize| {
            let mut out = vec![i];
            while let Some(p) = items[i].within {
                if out.len() > n {
                    break;
                }
                out.push(p);
                i = p;
            }
            out
        };
        let depth = |i: usize| chain(i).len();
        let mut scopes: Vec<Option<usize>> = vec![None];
        for it in items {
            if !scopes.contains(&it.within) {
                scopes.push(it.within);
            }
        }
        // Innermost containers first, so each is sized before its container.
        scopes.sort_by_key(|sc| std::cmp::Reverse(sc.map_or(0, depth)));
        for scope in scopes {
            let members: Vec<usize> = (0..n).filter(|&i| items[i].within == scope).collect();
            if members.is_empty() {
                continue;
            }
            // An edge counts in this scope between the members that contain
            // its two ends.
            let rep = |i: usize| chain(i).into_iter().find(|c| items[*c].within == scope);
            let local: Vec<(usize, usize)> = edges
                .iter()
                .filter_map(|&(a, b)| {
                    let (ra, rb) = (rep(a)?, rep(b)?);
                    let ia = members.iter().position(|m| *m == ra)?;
                    let ib = members.iter().position(|m| *m == rb)?;
                    (ia != ib).then_some((ia, ib))
                })
                .collect();
            let sizes: Vec<(f64, f64)> = members.iter().map(|&m| size[m]).collect();
            let pos = autolayout::layered(&sizes, &local, flow, gap);
            for (k, &m) in members.iter().enumerate() {
                offset[m] = pos[k];
            }
            if let Some(c) = scope {
                let w = members
                    .iter()
                    .zip(&pos)
                    .map(|(&m, p)| p.0 + size[m].0)
                    .fold(0.0, f64::max);
                let h = members
                    .iter()
                    .zip(&pos)
                    .map(|(&m, p)| p.1 + size[m].1)
                    .fold(0.0, f64::max);
                size[c] = (
                    (w + 2.0 * PAD).max(size[c].0),
                    (h + HEADER + PAD).max(size[c].1),
                );
                self.run(
                    resize_view,
                    json!({ "view": items[c].view, "width": size[c].0, "height": size[c].1 }),
                )?;
            }
        }
        // Absolute positions, outermost first so moving a container (which
        // carries its contents) happens before its contents are placed.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| depth(i));
        let mut at = vec![(0.0, 0.0); n];
        for &i in &order {
            at[i] = match items[i].within {
                None => (ORIGIN.0 + offset[i].0, ORIGIN.1 + offset[i].1),
                Some(c) => (at[c].0 + PAD + offset[i].0, at[c].1 + HEADER + offset[i].1),
            };
            let r = self.rect(&items[i].view);
            let (dx, dy) = (at[i].0 - r.x, at[i].1 - r.y);
            if dx != 0.0 || dy != 0.0 {
                self.run(
                    move_views,
                    json!({ "views": [items[i].view], "dx": dx, "dy": dy }),
                )?;
            }
        }
        Ok(())
    }

    fn connect(&mut self, lines: Vec<Line>) -> Result<(), CommandError> {
        for l in lines {
            let (model, _) = self.edge(l.kind, &l.tail, &l.head, l.label.as_deref(), None)?;
            let Some(model) = model else { continue };
            self.set(&model, l.values)?;
            for (key, values) in l.ends {
                if let Some(end) = self.child(&model, key) {
                    self.set(&end, values)?;
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------

    fn class(
        &mut self,
        d: &ClassDiagram,
        name: Option<String>,
        parent: Option<String>,
    ) -> Result<Id, CommandError> {
        let diagram = self.diagram("UMLClassDiagram", name, parent)?;
        let mut items: Vec<Item> = Vec::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut lines: Vec<Line> = Vec::new();

        // Namespaces become packages, in order of first use.
        let mut packages: HashMap<&str, usize> = HashMap::new();
        for c in &d.classes {
            if let Some(ns) = c.namespace.as_deref()
                && !packages.contains_key(ns)
            {
                let (_, view) = self.node("UMLPackage", Some(ns), None, ORIGIN)?;
                packages.insert(ns, items.len());
                items.push(Item { view, within: None });
            }
        }

        let mut classes: HashMap<&str, (Id, Id, usize)> = HashMap::new();
        for c in &d.classes {
            let kind = if c.has_annotation("interface") {
                "UMLInterface"
            } else if c.has_annotation("enumeration") || c.has_annotation("enum") {
                "UMLEnumeration"
            } else {
                "UMLClass"
            };
            let within = c
                .namespace
                .as_deref()
                .and_then(|ns| packages.get(ns).copied());
            let container = within.map(|i| items[i].view.clone());
            let (model, view) = self.node(kind, Some(c.name()), container.as_deref(), ORIGIN)?;
            let model = model.ok_or_else(|| CommandError::Failed("a class has no model".into()))?;
            if c.has_annotation("abstract") {
                self.set(&model, json!({ "isAbstract": true }))?;
            }
            let other: Vec<&String> = c
                .annotations
                .iter()
                .filter(|a| {
                    !["interface", "enumeration", "enum", "abstract"]
                        .contains(&a.to_ascii_lowercase().as_str())
                })
                .collect();
            if let Some(st) = other.first() {
                self.set(&model, json!({ "stereotype": st }))?;
            }
            if let Some(g) = &c.generic {
                self.warnings.push(format!(
                    "`{}`: the template parameter `{g}` was not imported",
                    c.id
                ));
            }
            for m in &c.members {
                if kind == "UMLEnumeration" {
                    self.run(
                        add_feature,
                        json!({ "classifier": model, "kind": "literal", "name": m.name }),
                    )?;
                    continue;
                }
                let is_op = m.params.is_some();
                let r = self.run(
                    add_feature,
                    json!({
                        "classifier": model,
                        "kind": if is_op { "operation" } else { "attribute" },
                        "name": m.name,
                    }),
                )?;
                let f = id_of(&r, "id")?;
                let mut values = serde_json::Map::new();
                if let Some(v) = m.visibility {
                    let word = match v {
                        '-' => "private",
                        '#' => "protected",
                        '~' => "package",
                        _ => "public",
                    };
                    values.insert("visibility".into(), json!(word));
                }
                if m.is_static {
                    values.insert("isStatic".into(), json!(true));
                }
                if m.is_abstract && is_op {
                    values.insert("isAbstract".into(), json!(true));
                }
                if !is_op && let Some(t) = &m.ty {
                    values.insert("type".into(), json!(t));
                }
                self.set(&f, Value::Object(values))?;
                for param in m.params.iter().flatten() {
                    let r =
                        self.run(add_parameter, json!({ "operation": f, "direction": "in" }))?;
                    let pid = id_of(&r, "id")?;
                    self.set(&pid, json!({ "name": param.name }))?;
                    if let Some(t) = &param.ty {
                        self.set(&pid, json!({ "type": t }))?;
                    }
                }
                if is_op && let Some(t) = &m.ty {
                    let r = self.run(
                        add_parameter,
                        json!({ "operation": f, "direction": "return" }),
                    )?;
                    self.set(&id_of(&r, "id")?, json!({ "type": t }))?;
                }
            }
            classes.insert(c.id.as_str(), (model, view.clone(), items.len()));
            items.push(Item { view, within });
        }

        for r in &d.relations {
            let (Some(left), Some(right)) =
                (classes.get(r.left.as_str()), classes.get(r.right.as_str()))
            else {
                continue;
            };
            let (left, right) = (left.clone(), right.clone());
            let label = r.label.as_deref();
            // (kind, tail, head, which is drawn above for layout)
            let (kind, tail, head) = if r.left_end == End::Triangle || r.right_end == End::Triangle
            {
                if r.left_end == End::Triangle && r.right_end == End::Triangle {
                    self.warnings.push(format!(
                        "`{} … {}`: two-way inheritance was read as {} inheriting from {}",
                        r.left, r.right, r.left, r.right
                    ));
                }
                let kind = if r.dotted {
                    "UMLInterfaceRealization"
                } else {
                    "UMLGeneralization"
                };
                if r.right_end == End::Triangle {
                    (kind, &left, &right)
                } else {
                    (kind, &right, &left)
                }
            } else if r.left_end == End::Lollipop || r.right_end == End::Lollipop {
                if r.left_end == End::Lollipop {
                    ("UMLInterfaceRealization", &right, &left)
                } else {
                    ("UMLInterfaceRealization", &left, &right)
                }
            } else if r.dotted {
                if r.left_end == End::Arrow && r.right_end != End::Arrow {
                    ("UMLDependency", &right, &left)
                } else {
                    ("UMLDependency", &left, &right)
                }
            } else {
                ("UMLAssociation", &left, &right)
            };
            // Parents and the things pointed at go above.
            edges.push(if kind == "UMLAssociation" || kind == "UMLDependency" {
                (tail.2, head.2)
            } else {
                (head.2, tail.2)
            });
            let mut ends = Vec::new();
            if kind == "UMLAssociation" {
                for (key, end, card) in [
                    ("end1", r.left_end, &r.left_card),
                    ("end2", r.right_end, &r.right_card),
                ] {
                    let mut values = serde_json::Map::new();
                    match end {
                        End::Composition => {
                            values.insert("aggregation".into(), json!("composite"));
                        }
                        End::Aggregation => {
                            values.insert("aggregation".into(), json!("shared"));
                        }
                        End::Arrow => {
                            values.insert("navigable".into(), json!("navigable"));
                        }
                        _ => {}
                    }
                    if let Some(c) = card {
                        values.insert("multiplicity".into(), json!(c));
                    }
                    ends.push((key, Value::Object(values)));
                }
            }
            lines.push(Line {
                kind,
                tail: tail.1.clone(),
                head: head.1.clone(),
                label: label.map(str::to_owned),
                ends,
                values: json!({}),
            });
        }

        for note in &d.notes {
            let target = note.class.as_deref().and_then(|c| classes.get(c)).cloned();
            let within = target.as_ref().and_then(|t| items[t.2].within);
            let container = within.map(|i| items[i].view.clone());
            let view = self.note(&note.text, container.as_deref())?;
            let idx = items.len();
            items.push(Item {
                view: view.clone(),
                within,
            });
            if let Some(t) = target {
                lines.push(Line::plain("NoteLink", view, t.1.clone(), None));
                edges.push((t.2, idx));
            }
        }

        self.arrange(&items, &edges, flow(d.direction), Spacing::default())?;
        self.connect(lines)?;
        Ok(diagram)
    }

    // -----------------------------------------------------------------------

    fn state(
        &mut self,
        d: &StateDiagram,
        name: Option<String>,
        parent: Option<String>,
    ) -> Result<Id, CommandError> {
        let diagram = self.diagram("UMLStatechartDiagram", name, parent)?;
        let mut items: Vec<Item> = Vec::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut lines: Vec<Line> = Vec::new();
        let mut states: HashMap<&str, usize> = HashMap::new();
        // Start and end pseudostates per scope (None = the whole diagram).
        let mut starts: HashMap<Option<String>, usize> = HashMap::new();
        let mut ends: HashMap<Option<String>, usize> = HashMap::new();

        for st in &d.states {
            let kind = match st.kind {
                Kind::Simple => "UMLState",
                Kind::Composite => "UMLCompositeState",
                Kind::Choice => "UMLChoice",
                Kind::Fork => "UMLFork",
                Kind::Join => "UMLJoin",
            };
            let within = st.parent.as_deref().and_then(|p| states.get(p).copied());
            let container = within.map(|i| items[i].view.clone());
            let (model, view) = self.node(kind, Some(st.name()), container.as_deref(), ORIGIN)?;
            if let Some(m) = &model
                && !st.descriptions.is_empty()
            {
                self.set(m, json!({ "documentation": st.descriptions.join("\n") }))?;
            }
            states.insert(st.id.as_str(), items.len());
            items.push(Item { view, within });
        }

        for t in &d.transitions {
            let mut ends_of = Vec::new();
            for point in [&t.from, &t.to] {
                let idx = match point {
                    Point::State(id) => states.get(id.as_str()).copied(),
                    Point::Start(scope) | Point::End(scope) => {
                        let start = matches!(point, Point::Start(_));
                        let known = if start { &starts } else { &ends }.get(scope).copied();
                        match known {
                            Some(i) => Some(i),
                            None => {
                                let within = scope.as_deref().and_then(|p| states.get(p).copied());
                                let container = within.map(|i| items[i].view.clone());
                                let kind = if start {
                                    "UMLInitialState"
                                } else {
                                    "UMLFinalState"
                                };
                                let (_, view) =
                                    self.node(kind, None, container.as_deref(), ORIGIN)?;
                                let i = items.len();
                                items.push(Item { view, within });
                                if start { &mut starts } else { &mut ends }
                                    .insert(scope.clone(), i);
                                Some(i)
                            }
                        }
                    }
                };
                ends_of.push(idx);
            }
            let (Some(a), Some(b)) = (ends_of[0], ends_of[1]) else {
                continue;
            };
            let kind = if a == b {
                "UMLSelfTransition"
            } else {
                "UMLTransition"
            };
            let (tail, head) = (items[a].view.clone(), items[b].view.clone());
            lines.push(Line::plain(kind, tail, head, t.label.clone()));
            if a != b {
                edges.push((a, b));
            }
        }

        for note in &d.notes {
            let Some(&target) = states.get(note.state.as_str()) else {
                continue;
            };
            let within = items[target].within;
            let container = within.map(|i| items[i].view.clone());
            let view = self.note(&note.text, container.as_deref())?;
            let target_view = items[target].view.clone();
            lines.push(Line::plain("NoteLink", view.clone(), target_view, None));
            edges.push((target, items.len()));
            items.push(Item { view, within });
        }

        self.arrange(
            &items,
            &edges,
            flow(d.direction),
            Spacing {
                node: 50.0,
                layer: 60.0,
            },
        )?;
        self.connect(lines)?;
        Ok(diagram)
    }

    // -----------------------------------------------------------------------

    /// Shapes become the closest flowchart symbol (StarUML's mapping for
    /// the bracket shapes, extended to the named ones); links become flows.
    fn flowchart(
        &mut self,
        d: &Flowchart,
        name: Option<String>,
        parent: Option<String>,
    ) -> Result<Id, CommandError> {
        let diagram = self.diagram("FCFlowchartDiagram", name, parent)?;
        let mut items: Vec<Item> = Vec::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut lines: Vec<Line> = Vec::new();
        let mut nodes: HashMap<&str, usize> = HashMap::new();

        for node in &d.nodes {
            let kind = symbol(&node.shape).unwrap_or_else(|| {
                let shape = match &node.shape {
                    Shape::Other(s) => s.clone(),
                    s => format!("{s:?}").to_lowercase(),
                };
                self.warnings.push(format!(
                    "`{}`: the `{shape}` shape has no flowchart symbol; drawn as a process",
                    node.id
                ));
                "FCProcess"
            });
            let (_, view) = self.node(kind, Some(node.name()), None, ORIGIN)?;
            nodes.insert(node.id.as_str(), items.len());
            items.push(Item { view, within: None });
        }

        let mut restyled = false;
        for link in &d.links {
            let (Some(&a), Some(&b)) = (nodes.get(link.from.as_str()), nodes.get(link.to.as_str()))
            else {
                continue;
            };
            if a != b {
                edges.push((a, b));
            }
            if link.stroke == Stroke::Invisible {
                continue;
            }
            restyled |= link.stroke != Stroke::Solid
                || link.tail != flowchart::End::None
                || link.head != flowchart::End::Arrow;
            let (tail, head) = (items[a].view.clone(), items[b].view.clone());
            lines.push(Line::plain("FCFlow", tail, head, link.label.clone()));
        }
        if restyled {
            self.warnings.push(
                "flows are solid lines with one arrowhead: dotted, thick, two-way and arrowless link styles were not kept"
                    .into(),
            );
        }

        self.arrange(
            &items,
            &edges,
            flow(d.direction),
            Spacing {
                node: 50.0,
                layer: 50.0,
            },
        )?;
        self.connect(lines)?;
        Ok(diagram)
    }

    // -----------------------------------------------------------------------

    /// Entities with their attributes as columns; relationships with their
    /// cardinalities, as StarUML maps them (but `..` stays non-identifying).
    fn er(
        &mut self,
        d: &ErDiagram,
        name: Option<String>,
        parent: Option<String>,
    ) -> Result<Id, CommandError> {
        let diagram = self.diagram("ERDDiagram", name, parent)?;
        let mut items: Vec<Item> = Vec::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut lines: Vec<Line> = Vec::new();
        let mut entities: HashMap<&str, usize> = HashMap::new();

        for entity in &d.entities {
            let (model, view) = self.node("ERDEntity", Some(entity.name()), None, ORIGIN)?;
            let model =
                model.ok_or_else(|| CommandError::Failed("an entity has no model".into()))?;
            for a in &entity.attributes {
                let r = self.run(
                    add_feature,
                    json!({ "classifier": model, "kind": "column", "name": a.name }),
                )?;
                // `varchar(255)` is a type and a length.
                let (ty, length) = match a.ty.split_once('(') {
                    Some((t, l)) if l.ends_with(')') => (t, Some(&l[..l.len() - 1])),
                    _ => (a.ty.as_str(), None),
                };
                let mut values = serde_json::Map::new();
                values.insert("type".into(), json!(ty));
                if let Some(l) = length {
                    values.insert("length".into(), json!(l));
                }
                for (key, field) in [("PK", "primaryKey"), ("FK", "foreignKey"), ("UK", "unique")] {
                    if a.has_key(key) {
                        values.insert(field.into(), json!(true));
                    }
                }
                if let Some(c) = &a.comment {
                    values.insert("documentation".into(), json!(c));
                }
                self.set(&id_of(&r, "id")?, Value::Object(values))?;
            }
            entities.insert(entity.id.as_str(), items.len());
            items.push(Item { view, within: None });
        }

        let cardinality = |c: Cardinality| match c {
            Cardinality::ExactlyOne => json!({}),
            Cardinality::ZeroOrOne => json!({ "cardinality": "0..1" }),
            Cardinality::ZeroOrMore => json!({ "cardinality": "0..*" }),
            Cardinality::OneOrMore => json!({ "cardinality": "1..*" }),
        };
        for r in &d.relationships {
            let (Some(&a), Some(&b)) = (entities.get(r.a.as_str()), entities.get(r.b.as_str()))
            else {
                continue;
            };
            if a != b {
                edges.push((a, b));
            }
            lines.push(Line {
                kind: "ERDRelationship",
                tail: items[a].view.clone(),
                head: items[b].view.clone(),
                label: r.label.clone(),
                ends: vec![
                    ("end1", cardinality(r.a_card)),
                    ("end2", cardinality(r.b_card)),
                ],
                values: if r.identifying {
                    json!({})
                } else {
                    json!({ "identifying": false })
                },
            });
        }

        // Wider than StarUML's 50 between layers: room for a crow's foot at
        // each end and the name between them.
        self.arrange(
            &items,
            &edges,
            flow(d.direction),
            Spacing {
                node: 50.0,
                layer: 110.0,
            },
        )?;
        self.connect(lines)?;
        Ok(diagram)
    }

    // -----------------------------------------------------------------------

    fn sequence(
        &mut self,
        d: &SequenceDiagram,
        name: Option<String>,
        parent: Option<String>,
    ) -> Result<Id, CommandError> {
        let diagram = self.diagram("UMLSequenceDiagram", name, parent)?;
        let frame = self
            .s
            .doc
            .get(&diagram)
            .map(|dg| dg.children("ownedViews").to_vec())
            .unwrap_or_default()
            .into_iter()
            .find(|v| {
                self.s
                    .doc
                    .get(v)
                    .is_some_and(|e| e.type_name == "UMLFrameView")
            });
        let frame_rect = frame.as_deref().map(|f| self.rect(f));
        let (left0, top) = match frame_rect {
            Some(r) => (r.x + 30.0, r.y + 40.0),
            None => ORIGIN,
        };

        // Room between neighbouring lifelines for the longest label between them.
        let index: HashMap<&str, usize> = d
            .participants
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.as_str(), i))
            .collect();
        let mut room = vec![0.0f64; d.participants.len()];
        for ev in &d.events {
            if let Event::Message { from, to, text, .. } = ev
                && let (Some(&a), Some(&b)) = (index.get(from.as_str()), index.get(to.as_str()))
            {
                let (lo, hi) = (a.min(b), a.max(b));
                let need = text.lines().map(str::len).max().unwrap_or(0) as f64 * 7.0 + 40.0;
                let per = need / (hi - lo).max(1) as f64;
                for r in room.iter_mut().take(hi.max(lo + 1)).skip(lo) {
                    *r = r.max(per);
                }
            }
        }

        let mut lifelines: Vec<(Id, render::Rect)> = Vec::new();
        let mut x = left0;
        for (i, p) in d.participants.iter().enumerate() {
            let (_, view) = self.node("UMLLifeline", Some(p.name()), None, (x, top))?;
            let r = self.rect(&view);
            lifelines.push((view, r));
            let centre_gap = (r.w + 60.0).max(room[i]);
            x = r.x + r.w / 2.0 + centre_gap - 50.0;
        }
        let lifelines: Vec<(Id, render::Rect)> = lifelines
            .into_iter()
            .map(|(v, _)| {
                let r = self.rect(&v);
                (v, r)
            })
            .collect();

        struct Open {
            block: Block,
            labels: Vec<String>,
            top: f64,
            involved: Vec<usize>,
            nested: usize,
        }
        let mut open: Vec<Open> = Vec::new();
        let mut y = top + 80.0;
        for ev in &d.events {
            match ev {
                Event::Message {
                    from,
                    to,
                    arrow,
                    text,
                    creates,
                    destroys,
                } => {
                    let (Some(&a), Some(&b)) = (index.get(from.as_str()), index.get(to.as_str()))
                    else {
                        continue;
                    };
                    for o in &mut open {
                        o.involved.extend([a, b]);
                    }
                    // StarUML's mapping of Mermaid arrows.
                    let kind = if a == b {
                        "UMLSelfMessage"
                    } else if *creates {
                        "UMLCreateMessage"
                    } else if *destroys {
                        "UMLDeleteMessage"
                    } else {
                        match arrow {
                            Arrow::DottedHead => "UMLAsyncSignalMessage",
                            Arrow::SolidAsync => "UMLAsyncMessage",
                            Arrow::DottedAsync => "UMLReplyMessage",
                            _ => "UMLMessage",
                        }
                    };
                    // `charge(total)` is a name and its arguments.
                    let (msg_name, arguments) = match text.split_once('(') {
                        Some((n, args)) if args.ends_with(')') && !n.trim().is_empty() => {
                            (n.trim(), Some(args.trim_end_matches(')').trim()))
                        }
                        _ => (text.as_str(), None),
                    };
                    let msg_name = (!msg_name.is_empty()).then_some(msg_name);
                    let (tail, head) = (lifelines[a].0.clone(), lifelines[b].0.clone());
                    let (model, _) = self.edge(kind, &tail, &head, msg_name, Some(y))?;
                    if let (Some(m), Some(args)) = (model, arguments.filter(|a| !a.is_empty())) {
                        self.set(&m, json!({ "arguments": args }))?;
                    }
                    y += if a == b { 60.0 } else { 40.0 };
                }
                Event::Note { place, of, text } => {
                    let idx: Vec<usize> = of
                        .iter()
                        .filter_map(|p| index.get(p.as_str()).copied())
                        .collect();
                    let Some(&first) = idx.first() else { continue };
                    let last = idx.last().copied().unwrap_or(first);
                    let view = self.note(text, None)?;
                    let nr = self.rect(&view);
                    let (lr, rr) = (lifelines[first].1, lifelines[last].1);
                    let nx = match place {
                        NotePlace::LeftOf => lr.x + lr.w / 2.0 - 20.0 - nr.w,
                        NotePlace::RightOf => lr.x + lr.w / 2.0 + 20.0,
                        NotePlace::Over => {
                            let mid = (lr.x + lr.w / 2.0 + rr.x + rr.w / 2.0) / 2.0;
                            mid - nr.w / 2.0
                        }
                    };
                    if matches!(place, NotePlace::Over) && first != last {
                        let w = (rr.x + rr.w / 2.0) - (lr.x + lr.w / 2.0) + 40.0;
                        self.run(
                            resize_view,
                            json!({ "view": view, "width": w.max(nr.w), "height": nr.h }),
                        )?;
                    }
                    let nr = self.rect(&view);
                    let nx = if matches!(place, NotePlace::Over) && first != last {
                        lr.x + lr.w / 2.0 - 20.0
                    } else {
                        nx
                    };
                    self.run(
                        move_views,
                        json!({ "views": [view], "dx": nx - nr.x, "dy": y - nr.y }),
                    )?;
                    y += nr.h + 15.0;
                }
                Event::Begin { block, label } => {
                    open.push(Open {
                        block: *block,
                        labels: vec![label.clone()],
                        top: y - 10.0,
                        involved: Vec::new(),
                        nested: 0,
                    });
                    y += 30.0;
                }
                Event::Section { label } => {
                    if let Some(o) = open.last_mut() {
                        o.labels.push(label.clone());
                    }
                    y += 30.0;
                }
                Event::End => {
                    let Some(o) = open.pop() else { continue };
                    if let Some(outer) = open.last_mut() {
                        outer.nested = outer.nested.max(o.nested + 1);
                    }
                    let (lo, hi) = match (o.involved.iter().min(), o.involved.iter().max()) {
                        (Some(&lo), Some(&hi)) => (lo, hi),
                        _ => (0, lifelines.len().saturating_sub(1)),
                    };
                    let Some(((_, lr), (_, rr))) = lifelines.get(lo).zip(lifelines.get(hi)) else {
                        continue;
                    };
                    let margin = 30.0 + 10.0 * o.nested as f64;
                    let fx = lr.x + lr.w / 2.0 - margin - 40.0;
                    let fw = (rr.x + rr.w / 2.0 + margin + 40.0) - fx;
                    let fh = y - o.top;
                    let (model, view) =
                        self.node("UMLCombinedFragment", None, None, (fx, o.top))?;
                    self.run(
                        resize_view,
                        json!({ "view": view, "width": fw, "height": fh.max(40.0) }),
                    )?;
                    let r = self.rect(&view);
                    self.run(
                        move_views,
                        json!({ "views": [view], "dx": fx - r.x, "dy": o.top - r.y }),
                    )?;
                    let Some(model) = model else { continue };
                    let op = match o.block {
                        Block::Loop => "loop",
                        Block::Alt => "alt",
                        Block::Opt => "opt",
                        Block::Par => "par",
                        Block::Critical => "critical",
                        Block::Break => "break",
                    };
                    self.set(&model, json!({ "interactionOperator": op }))?;
                    let have = self
                        .s
                        .doc
                        .get(&model)
                        .map(|m| m.children("operands").len())
                        .unwrap_or(0);
                    for _ in have..o.labels.len() {
                        self.run(
                            add_feature,
                            json!({ "classifier": model, "kind": "operand" }),
                        )?;
                    }
                    let operands: Vec<Id> = self
                        .s
                        .doc
                        .get(&model)
                        .map(|m| m.children("operands").to_vec())
                        .unwrap_or_default();
                    for (operand, label) in operands.iter().zip(&o.labels) {
                        if !label.is_empty() {
                            self.set(operand, json!({ "guard": label }))?;
                        }
                    }
                    y += 20.0;
                }
            }
        }

        // Lifelines reach below the last message; the frame holds everything.
        let bottom = y + 30.0;
        for (view, _) in &lifelines {
            let r = self.rect(view);
            self.run(
                resize_view,
                json!({ "view": view, "width": r.w, "height": (bottom - r.y).max(r.h) }),
            )?;
        }
        if let (Some(f), Some(fr)) = (&frame, frame_rect) {
            let right = lifelines
                .iter()
                .map(|(v, _)| {
                    let r = self.rect(v);
                    r.x + r.w
                })
                .fold(fr.x + 100.0, f64::max);
            self.run(
                resize_view,
                json!({ "view": f, "width": (right + 40.0 - fr.x).max(fr.w), "height": (bottom + 30.0 - fr.y).max(100.0) }),
            )?;
        }
        // Both default to on in StarUML; Mermaid shows them only when asked.
        self.set(
            &diagram,
            json!({ "showSequenceNumber": d.autonumber, "showActivation": d.activations }),
        )?;
        Ok(diagram)
    }
}

/// The flowchart symbol for a Mermaid shape, if one is close enough.
fn symbol(shape: &Shape) -> Option<&'static str> {
    Some(match shape {
        Shape::Rect => "FCProcess",
        Shape::Rounded => "FCAlternateProcess",
        Shape::Stadium => "FCTerminator",
        Shape::Subroutine => "FCPredefinedProcess",
        Shape::Cylinder => "FCDatabase",
        Shape::HorizontalCylinder => "FCDirectAccessStorage",
        Shape::Circle | Shape::DoubleCircle | Shape::SmallCircle => "FCConnector",
        Shape::Diamond => "FCDecision",
        Shape::Hexagon => "FCPreparation",
        Shape::LeanRight | Shape::LeanLeft => "FCData",
        Shape::InvTrapezoid | Shape::Trapezoid => "FCManualOperation",
        Shape::Document => "FCDocument",
        Shape::Documents => "FCMultiDocument",
        Shape::Delay => "FCDelay",
        Shape::Display => "FCDisplay",
        Shape::ManualInput => "FCManualInput",
        Shape::PaperTape => "FCPunchedTape",
        Shape::StoredData => "FCStoredData",
        Shape::InternalStorage => "FCInternalStorage",
        Shape::Extract => "FCExtract",
        Shape::FlippedTriangle => "FCMerge",
        Shape::Collate => "FCCollate",
        Shape::CrossedCircle => "FCSummingJunction",
        Shape::Card => "FCCard",
        Shape::Odd | Shape::Other(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::{Registry, Session};

    fn import(s: &mut Session, text: &str) -> Value {
        Registry::default()
            .run(s, "import.mermaid", &json!({ "text": text }))
            .unwrap()
    }

    fn inspect(s: &mut Session) -> Value {
        Registry::default()
            .run(s, "diagram.inspect", &json!({}))
            .unwrap()
    }

    fn overlapping(boxes: &[Value]) -> Option<(String, String)> {
        let r = |b: &Value| {
            (
                b["x"].as_f64().unwrap(),
                b["y"].as_f64().unwrap(),
                b["width"].as_f64().unwrap(),
                b["height"].as_f64().unwrap(),
            )
        };
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                // Only siblings can't overlap; contents sit inside containers.
                if a["container"] != b["container"] {
                    continue;
                }
                let (ax, ay, aw, ah) = r(a);
                let (bx, by, bw, bh) = r(b);
                if ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah {
                    return Some((a["name"].to_string(), b["name"].to_string()));
                }
            }
        }
        None
    }

    #[test]
    fn class_diagram_with_members_relations_namespaces_and_notes() {
        let mut s = Session::default();
        let r = import(
            &mut s,
            r#"---
title: Shop
---
classDiagram
direction LR
class Order {
  +String id
  -int qty$
  +total(discount: Money) Money
}
class Shape { <<interface>> }
<<enumeration>> Status
Status : OPEN
Status : PAID
namespace Billing {
  class Invoice
}
Order "1" *-- "many" Line : has
Square ..|> Shape
Order --> Status
Invoice ..> Order
note for Order "the aggregate root"
class Gen~T~"#,
        );
        assert_eq!(r["warnings"].as_array().unwrap().len(), 1, "{r}");
        let d = inspect(&mut s);
        assert_eq!(d["name"], "Shop");
        let boxes = d["boxes"].as_array().unwrap();
        let find = |n: &str| boxes.iter().find(|b| b["name"] == n).unwrap().clone();
        assert_eq!(find("Shape")["modelType"], "UMLInterface");
        assert_eq!(find("Status")["modelType"], "UMLEnumeration");
        assert_eq!(find("Billing")["modelType"], "UMLPackage");
        assert_eq!(find("Invoice")["container"], find("Billing")["view"]);
        let note = boxes.iter().find(|b| b["type"] == "UMLNoteView").unwrap();
        assert_eq!(note["name"], "the aggregate root");
        assert_eq!(note["modelType"], Value::Null, "notes have no model");
        assert!(overlapping(boxes).is_none(), "{:?}", overlapping(boxes));
        // LR: the composition's whole is left of its part.
        assert!(find("Order")["x"].as_f64() < find("Line")["x"].as_f64());

        let lines = d["lines"].as_array().unwrap();
        let kinds: Vec<&str> = lines
            .iter()
            .map(|l| l["modelType"].as_str().unwrap_or("note"))
            .collect();
        for k in [
            "UMLAssociation",
            "UMLInterfaceRealization",
            "UMLDependency",
            "note",
        ] {
            assert!(kinds.contains(&k), "{k} in {kinds:?}");
        }
        let order = find("Order")["model"].as_str().unwrap().to_owned();
        let model = s.doc.get(&order).unwrap();
        let attrs = model.children("attributes");
        assert_eq!(attrs.len(), 2);
        let qty = s.doc.get(&attrs[1]).unwrap();
        assert_eq!(qty.str("visibility"), Some("private"));
        assert!(qty.get("isStatic").is_some());
        let op = s.doc.get(&model.children("operations")[0]).unwrap();
        assert_eq!(op.children("parameters").len(), 2, "discount + return");
        assert_eq!(
            uml::operation_text(&s.doc, op),
            "+total(discount: Money): Money"
        );

        // The association's ends carry the composition and multiplicities.
        let assoc = lines
            .iter()
            .find(|l| l["modelType"] == "UMLAssociation")
            .unwrap();
        let a = s.doc.get(assoc["model"].as_str().unwrap()).unwrap();
        let end = |k: &str| match a.get(k) {
            Some(mdj::Field::Child(c)) => s.doc.get(c).unwrap(),
            _ => panic!(),
        };
        assert_eq!(end("end1").str("aggregation"), Some("composite"));
        assert_eq!(end("end1").str("multiplicity"), Some("1"));
        assert_eq!(end("end2").str("multiplicity"), Some("many"));
        assert_eq!(a.name(), "has");
        // Lines are drawn between the final positions: two points each.
        for l in lines {
            let v = s.doc.get(l["view"].as_str().unwrap()).unwrap();
            assert_eq!(
                v.str("points").unwrap().split(';').count(),
                2,
                "{}",
                l["name"]
            );
        }
    }

    #[test]
    fn one_undo_step_and_failures_change_nothing() {
        let mut s = Session::default();
        let before = s.to_json().unwrap();
        import(&mut s, "classDiagram\nA <|-- B\nA : +x");
        let reg = Registry::default();
        reg.run(&mut s, "edit.undo", &json!({})).unwrap();
        assert_eq!(s.to_json().unwrap(), before);
        assert!(!s.doc.can_undo());

        for bad in [
            "mindmap\n  root",
            "erDiagram\nA {\nint id",
            "flowchart TD\nsubgraph x\nA-->B",
            "classDiagram\nclass A {",
            "",
            "sequenceDiagram\nloop forever\nA->>B: x",
        ] {
            let e = reg.run(&mut s, "import.mermaid", &json!({ "text": bad }));
            assert!(e.is_err(), "{bad}");
        }
        assert_eq!(s.to_json().unwrap(), before);
        assert!(
            reg.run(&mut s, "import.mermaid", &json!({ "text": 5 }))
                .is_err()
        );
    }

    #[test]
    fn sequence_diagram_with_fragments_and_notes() {
        let mut s = Session::default();
        import(
            &mut s,
            "sequenceDiagram
    autonumber
    participant C as Customer
    C->>S: checkout(cart)
    alt ok
        S--)C: done
    else failed
        S-)C: retry
    end
    S->>S: log
    Note over C,S: end of story",
        );
        let d = inspect(&mut s);
        assert_eq!(d["type"], "UMLSequenceDiagram");
        let boxes = d["boxes"].as_array().unwrap();
        let types: Vec<&str> = boxes.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(
            types.iter().filter(|t| **t == "UMLSeqLifelineView").count(),
            2
        );
        assert!(types.contains(&"UMLCombinedFragmentView"));
        assert!(types.contains(&"UMLNoteView"));
        let lines = d["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 4);
        let msg = s.doc.get(lines[0]["model"].as_str().unwrap()).unwrap();
        assert_eq!(
            (msg.name(), msg.str("arguments")),
            ("checkout", Some("cart"))
        );
        let sorts: Vec<String> = lines
            .iter()
            .map(|l| {
                let m = s.doc.get(l["model"].as_str().unwrap()).unwrap();
                m.str("messageSort").unwrap_or("synchCall").to_owned()
            })
            .collect();
        assert_eq!(sorts[1], "reply");
        assert_eq!(sorts[2], "asynchCall");
        let frag = boxes
            .iter()
            .find(|b| b["type"] == "UMLCombinedFragmentView")
            .unwrap();
        let fm = s.doc.get(frag["model"].as_str().unwrap()).unwrap();
        assert_eq!(fm.str("interactionOperator"), Some("alt"));
        let guards: Vec<&str> = fm
            .children("operands")
            .iter()
            .map(|o| s.doc.get(o).unwrap().str("guard").unwrap_or(""))
            .collect();
        assert_eq!(guards, ["ok", "failed"]);
        let dg = s.doc.get(d["id"].as_str().unwrap()).unwrap();
        assert_eq!(
            dg.get("showSequenceNumber"),
            Some(&mdj::Field::Value(json!(true)))
        );
        assert_eq!(
            dg.get("showActivation"),
            Some(&mdj::Field::Value(json!(false)))
        );
    }

    #[test]
    fn state_diagram_with_composites_choices_and_notes() {
        let mut s = Session::default();
        import(
            &mut s,
            "stateDiagram-v2
    [*] --> New
    New --> Paying : checkout
    state Paying {
        [*] --> Authorising
        Authorising --> [*]
    }
    Paying --> check
    state check <<choice>>
    check --> Done : ok
    check --> New : retry
    Done --> [*]
    Done --> Done : poll
    note right of Done : finished",
        );
        let d = inspect(&mut s);
        let boxes = d["boxes"].as_array().unwrap();
        assert!(overlapping(boxes).is_none(), "{:?}", overlapping(boxes));
        let find = |n: &str| boxes.iter().find(|b| b["name"] == n).unwrap().clone();
        assert_eq!(find("Authorising")["container"], find("Paying")["view"]);
        // The contents sit inside the composite.
        let (p, a) = (find("Paying"), find("Authorising"));
        assert!(a["x"].as_f64() > p["x"].as_f64() && a["y"].as_f64() > p["y"].as_f64());
        let models: Vec<&str> = boxes
            .iter()
            .filter_map(|b| b["modelType"].as_str())
            .collect();
        assert_eq!(
            models.iter().filter(|t| **t == "UMLPseudostate").count(),
            3,
            "{models:?}"
        );
        assert_eq!(models.iter().filter(|t| **t == "UMLFinalState").count(), 2);
        // The cycle (check → New) still flows top-down for the rest.
        assert!(find("New")["y"].as_f64() < find("Done")["y"].as_f64());
        let lines = d["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 10, "9 transitions and a note link");
    }

    #[test]
    fn flowchart_shapes_become_symbols_and_links_become_flows() {
        let mut s = Session::default();
        let r = import(
            &mut s,
            r#"flowchart TD
    start([Start]) --> input[/Enter password/]
    users[(Users)] -. lookup .-> check{Correct?}
    input --> check
    check -->|yes| open[Open session] --> stop([End])
    check -- no --> err@{ shape: display, label: "Show error" }
    err --> input
    open --> open
    open ~~~ odd>Odd one]
    subgraph audit
      log@{ shape: doc, label: Log }
    end
    open --> log"#,
        );
        let warnings: Vec<String> = r["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("`odd`")));
        assert!(warnings.iter().any(|w| w.contains("subgraphs")));
        assert!(warnings.iter().any(|w| w.contains("dotted")));

        let d = inspect(&mut s);
        assert_eq!(d["type"], "FCFlowchartDiagram");
        let boxes = d["boxes"].as_array().unwrap();
        assert!(overlapping(boxes).is_none(), "{:?}", overlapping(boxes));
        let find = |n: &str| boxes.iter().find(|b| b["name"] == n).unwrap().clone();
        let ty = |n: &str| find(n)["type"].as_str().unwrap().to_owned();
        assert_eq!(ty("Start"), "FCTerminatorView");
        assert_eq!(ty("Enter password"), "FCDataView");
        assert_eq!(ty("Users"), "FCDatabaseView");
        assert_eq!(ty("Correct?"), "FCDecisionView");
        assert_eq!(ty("Open session"), "FCProcessView");
        assert_eq!(ty("Show error"), "FCDisplayView");
        assert_eq!(ty("Odd one"), "FCProcessView");
        assert_eq!(ty("Log"), "FCDocumentView");
        // Top-down: the start is above the decision, the decision above the end.
        let y = |n: &str| find(n)["y"].as_f64().unwrap();
        assert!(y("Start") < y("Correct?") && y("Correct?") < y("End"));

        let lines = d["lines"].as_array().unwrap();
        // 10 links; the invisible one isn't drawn.
        assert_eq!(lines.len(), 9, "{lines:?}");
        assert!(lines.iter().all(|l| l["type"] == "FCFlowView"));
        let named: Vec<&str> = lines.iter().filter_map(|l| l["name"].as_str()).collect();
        for label in ["lookup", "yes", "no"] {
            assert!(named.contains(&label), "{named:?}");
        }
        let open = find("Open session")["view"].clone();
        assert!(lines.iter().any(|l| l["tail"] == open && l["head"] == open));

        Registry::default()
            .run(&mut s, "edit.undo", &json!({}))
            .unwrap();
        assert!(!s.doc.can_undo(), "one undo step");
    }

    #[test]
    fn er_diagram_entities_columns_and_relationships() {
        let mut s = Session::default();
        let r = import(
            &mut s,
            r#"erDiagram
    direction LR
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
    CUSTOMER }|..|{ ADDRESS : uses
    PART |o--o| PART : replaces
    CUSTOMER {
        int id PK
        varchar(120) email UK "login"
    }
    ORDER {
        int customer_id FK
    }"#,
        );
        assert!(r["warnings"].as_array().unwrap().is_empty(), "{r}");
        let d = inspect(&mut s);
        assert_eq!(d["type"], "ERDDiagram");
        let boxes = d["boxes"].as_array().unwrap();
        assert!(overlapping(boxes).is_none(), "{:?}", overlapping(boxes));
        let find = |n: &str| boxes.iter().find(|b| b["name"] == n).unwrap().clone();
        let x = |n: &str| find(n)["x"].as_f64().unwrap();
        assert!(
            x("CUSTOMER") < x("ORDER") && x("ORDER") < x("LINE-ITEM"),
            "left to right"
        );

        let doc = &s.doc;
        let model = |n: &str| doc.get(find(n)["model"].as_str().unwrap()).unwrap();
        let columns: Vec<&mdj::Element> = model("CUSTOMER")
            .children("columns")
            .iter()
            .map(|c| doc.get(c).unwrap())
            .collect();
        assert_eq!(columns.len(), 2);
        assert!(columns[0].bool("primaryKey", false));
        let email = columns[1];
        assert_eq!(
            (email.str("type"), email.str("length")),
            (Some("varchar"), Some("120"))
        );
        assert!(email.bool("unique", false));
        assert_eq!(email.str("documentation"), Some("login"));

        let lines = d["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 4);
        let rel = |label: &str| {
            let l = lines.iter().find(|l| l["name"] == label).unwrap();
            doc.get(l["model"].as_str().unwrap()).unwrap()
        };
        let card = |r: &mdj::Element, k: &str| {
            doc.get(&r.children(k)[0])
                .unwrap()
                .str("cardinality")
                .unwrap_or("1")
                .to_owned()
        };
        let places = rel("places");
        assert_eq!(
            (card(places, "end1"), card(places, "end2")),
            ("1".into(), "0..*".into())
        );
        assert!(places.bool("identifying", true));
        let uses = rel("uses");
        assert_eq!(
            (card(uses, "end1"), card(uses, "end2")),
            ("1..*".into(), "1..*".into())
        );
        assert!(!uses.bool("identifying", true), "`..` is non-identifying");
        let replaces = rel("replaces");
        assert_eq!(card(replaces, "end2"), "0..1");
    }
}
