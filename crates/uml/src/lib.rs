//! UML elements in the shape StarUML stores them (see docs/evidence.md §3):
//! a catalog of what can be created, factories that build the exact view
//! trees, cascading delete, and the text UML notation shows.

pub mod catalog;
mod delete;
mod factory;
mod text;

pub use catalog::{
    DIAGRAMS, EDGES, EdgeSpec, NODES, NodeSpec, TOOL_GROUPS, Tool, ToolGroup, edge_spec,
    edge_spec_on, node_spec, node_spec_for_view, node_spec_on, tool, tool_groups, tool_on,
};
pub use delete::{delete_elements, delete_views};
pub use factory::{
    FeatureKind, LIFELINE_HEAD, Target, add_edge_view, add_feature, add_node_view, add_parameter,
    compartment_items, container_owner, create_diagram, create_edge, create_edge_at, create_node,
    edge_spec_of, owner_for_new_elements, return_parameter, target_for, views_of, with_contained,
};
pub use text::{
    attribute_text, flow_text, instance_text, message_text, numbered_message_text, operation_text,
    part_text, slot_text, stereotype_text, transition_text, typed_name, var_text,
    visibility_symbol,
};

#[cfg(test)]
mod tests {
    use mdj::values::align;
    use mdj::{Document, Element, Field, Id};

    use super::*;

    fn setup() -> (Document, Id) {
        let doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let diagram = doc.get(&model).unwrap().children("ownedElements")[0].clone();
        (doc, diagram)
    }

    fn node(
        doc: &mut Document,
        dgm: &str,
        tool: &str,
        x: f64,
        y: f64,
        container: Option<&str>,
    ) -> (Id, Id) {
        let ty = doc.get(dgm).unwrap().type_name.clone();
        let spec = node_spec_on(tool, &ty).unwrap();
        let (m, v) = doc
            .try_edit(|e| create_node(e, dgm, spec, x, y, container))
            .unwrap();
        (m.unwrap_or_default(), v)
    }

    fn edge(doc: &mut Document, dgm: &str, tool: &str, tail: &str, head: &str) -> (Option<Id>, Id) {
        let ty = doc.get(dgm).unwrap().type_name.clone();
        let spec = edge_spec_on(tool, &ty).unwrap();
        doc.try_edit(|e| create_edge(e, dgm, spec, tail, head))
            .unwrap()
    }

    #[test]
    fn class_view_has_staruml_compartments() {
        let (mut doc, dgm) = setup();
        let (class, view) = node(&mut doc, &dgm, "UMLClass", 10.0, 20.0, None);
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

    /// A diagram of a type that offers `tool`, created the way the UI would.
    fn diagram_offering(doc: &mut Document, tool: &str) -> Id {
        let ty = TOOL_GROUPS
            .iter()
            .find(|g| g.tools.contains(&tool) && !g.diagrams.is_empty())
            .map_or("UMLClassDiagram", |g| g.diagrams[0]);
        diagram_of_type(doc, ty)
    }

    fn diagram_of_type(doc: &mut Document, ty: &str) -> Id {
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        doc.try_edit(|e| create_diagram(e, &model, ty)).unwrap()
    }

    #[test]
    fn every_node_tool_builds_its_view_type() {
        let mut doc = Document::new_project();
        for spec in NODES {
            let dgm = match spec.diagrams.first() {
                Some(ty) => diagram_of_type(&mut doc, ty),
                None => diagram_offering(&mut doc, spec.id),
            };
            let container = if spec.placement == catalog::Placement::OnClassifier {
                Some(node(&mut doc, &dgm, "UMLClass", 0.0, 0.0, None).1)
            } else {
                None
            };
            let (_, v) = node(&mut doc, &dgm, spec.id, 300.0, 300.0, container.as_deref());
            let view = doc.get(&v).unwrap();
            assert_eq!(view.type_name, spec.view_type, "{}", spec.id);
            for (key, comp_type) in spec.compartments {
                let comp = doc.get(view.ref_id(key).unwrap()).unwrap();
                assert_eq!(comp.type_name, *comp_type, "{}.{key}", spec.id);
            }
            assert_eq!(
                view.ref_id("model").is_some(),
                spec.model_type.is_some(),
                "{}",
                spec.id
            );
            for key in spec.labels {
                assert!(view.ref_id(key).is_some(), "{}.{key}", spec.id);
            }
        }
    }

    #[test]
    fn behaviour_diagrams_get_their_owners_like_staruml() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        for (ty, wrapper) in [
            ("UMLStatechartDiagram", "UMLStateMachine"),
            ("UMLActivityDiagram", "UMLActivity"),
            ("UMLProfileDiagram", "UMLProfile"),
            ("UMLUseCaseDiagram", "UMLModel"),
        ] {
            let d = doc.try_edit(|e| create_diagram(e, &model, ty)).unwrap();
            let owner = doc
                .get(doc.get(&d).unwrap().parent.as_deref().unwrap())
                .unwrap();
            assert_eq!(owner.type_name, wrapper, "{ty}");
        }
        let machine = doc
            .iter()
            .find(|e| e.type_name == "UMLStateMachine")
            .unwrap();
        assert_eq!(
            machine.children("regions").len(),
            1,
            "state machines start with a region"
        );
    }

    #[test]
    fn states_go_in_regions_and_transitions_with_them() {
        let mut doc = Document::new_project();
        let dgm = diagram_offering(&mut doc, "UMLState");
        let (idle, idle_v) = node(&mut doc, &dgm, "UMLState", 0.0, 0.0, None);
        let (busy, busy_v) = node(&mut doc, &dgm, "UMLCompositeState", 200.0, 0.0, None);
        let region = doc.get(&idle).unwrap().parent.clone().unwrap();
        assert_eq!(doc.get(&region).unwrap().type_name, "UMLRegion");
        // A state dropped in a composite state goes in that state's region.
        let busy_region = doc.get(&busy).unwrap().children("regions")[0].clone();
        let (inner, inner_v) = node(&mut doc, &dgm, "UMLState", 220.0, 40.0, Some(&busy_v));
        assert_eq!(
            doc.get(&inner).unwrap().parent.as_deref(),
            Some(busy_region.as_str())
        );
        assert_eq!(
            doc.get(&inner_v).unwrap().ref_id("containerView"),
            Some(busy_v.as_str())
        );
        // The composite state's view shows its region.
        let decomposition = doc
            .get(
                doc.get(&busy_v)
                    .unwrap()
                    .ref_id("decompositionCompartment")
                    .unwrap(),
            )
            .unwrap();
        let region_view = doc.get(&decomposition.children("subViews")[0]).unwrap();
        assert_eq!(region_view.type_name, "UMLRegionView");
        assert!(
            region_view.get("horizontalAlignment").is_none(),
            "regions are boxes, not text rows"
        );

        let (t, _) = edge(&mut doc, &dgm, "UMLTransition", &idle_v, &busy_v);
        assert_eq!(
            doc.get(&t.unwrap()).unwrap().parent.as_deref(),
            Some(region.as_str())
        );
        assert!(doc.get(&region).unwrap().children("transitions").len() == 1);
        // A self transition loops back whatever the head.
        let (_, sv) = edge(&mut doc, &dgm, "UMLSelfTransition", &idle_v, &busy_v);
        let sv = doc.get(&sv).unwrap();
        assert_eq!(sv.ref_id("tail"), sv.ref_id("head"));
    }

    #[test]
    fn activity_nodes_and_flows_live_in_the_activity() {
        let mut doc = Document::new_project();
        let dgm = diagram_offering(&mut doc, "UMLAction");
        let activity = doc.get(&dgm).unwrap().parent.clone().unwrap();
        let (lane, lane_v) = node(&mut doc, &dgm, "UMLSwimlaneHorz", 0.0, 0.0, None);
        assert!(
            doc.get(&activity)
                .unwrap()
                .children("groups")
                .contains(&lane)
        );
        assert_eq!(
            doc.get(&lane_v)
                .unwrap()
                .get("isVertical")
                .and_then(Field::as_bool),
            Some(false)
        );
        let (a, a_v) = node(&mut doc, &dgm, "UMLAction", 20.0, 40.0, Some(&lane_v));
        assert_eq!(
            doc.get(&a).unwrap().parent.as_deref(),
            Some(activity.as_str()),
            "model in the activity"
        );
        assert_eq!(
            doc.get(&a_v).unwrap().ref_id("containerView"),
            Some(lane_v.as_str()),
            "view in the lane"
        );
        let (start, start_v) = node(&mut doc, &dgm, "UMLInitialNode", 0.0, 200.0, None);
        assert_eq!(doc.get(&start).unwrap().type_name, "UMLInitialNode");
        let (f, _) = edge(&mut doc, &dgm, "UMLControlFlow", &start_v, &a_v);
        assert_eq!(
            doc.get(&f.unwrap()).unwrap().parent.as_deref(),
            Some(activity.as_str())
        );
    }

    #[test]
    fn communication_diagrams_like_staruml() {
        let mut doc = Document::new_project();
        let dgm = diagram_of_type(&mut doc, "UMLCommunicationDiagram");
        let interaction = doc.get(&dgm).unwrap().parent.clone().unwrap();
        assert_eq!(doc.get(&interaction).unwrap().type_name, "UMLInteraction");
        let frame = doc
            .get(&doc.get(&dgm).unwrap().children("ownedViews")[0])
            .unwrap();
        assert_eq!(frame.type_name, "UMLFrameView");

        let (a, av) = node(&mut doc, &dgm, "UMLLifeline", 40.0, 40.0, None);
        let (b, bv) = node(&mut doc, &dgm, "UMLLifeline", 300.0, 40.0, None);
        let v = doc.get(&av).unwrap();
        assert_eq!(v.type_name, "UMLCommLifelineView");
        assert!(v.ref_id("linePart").is_none());
        let role = |l: &str| doc.get(l).unwrap().ref_id("represent").unwrap().to_owned();
        let (ra, rb) = (role(&a), role(&b));

        // A message drawn between lifelines brings its connector, which joins
        // their roles and belongs to the tail's role.
        let (m, mv) = edge(&mut doc, &dgm, "UMLForwardMessage", &av, &bv);
        let m = doc.get(&m.unwrap()).unwrap().clone();
        assert_eq!(m.parent.as_deref(), Some(interaction.as_str()));
        assert_eq!(m.name(), "Message1");
        assert_eq!(
            (m.ref_id("source"), m.ref_id("target")),
            (Some(a.as_str()), Some(b.as_str()))
        );
        let connector = doc.get(m.ref_id("connector").unwrap()).unwrap();
        assert_eq!(connector.type_name, "UMLConnector");
        assert_eq!(connector.parent.as_deref(), Some(ra.as_str()));
        let end = |k: &str| {
            doc.get(&connector.children(k)[0])
                .unwrap()
                .ref_id("reference")
                .unwrap()
                .to_owned()
        };
        assert_eq!((end("end1"), end("end2")), (ra.clone(), rb.clone()));

        let mv = doc.get(&mv).unwrap();
        assert_eq!(mv.type_name, "UMLCommMessageView");
        let cv = doc.get(mv.ref_id("hostEdge").unwrap()).unwrap();
        assert_eq!(cv.type_name, "UMLConnectorView");
        assert_eq!(cv.ref_id("model"), Some(connector.id.as_str()));
        assert_eq!(
            mv.num("edgePosition", -1.0),
            mdj::values::edge_position::MIDDLE as f64,
            "written: the metamodel default is the head"
        );
        assert!(mv.get("alpha").is_some() && mv.get("distance").is_some());
        let label = doc.get(mv.ref_id("nameLabel").unwrap()).unwrap();
        assert_eq!(label.type_name, "NodeLabelView");
        assert_eq!(label.ref_id("hostNode"), Some(mv.id.as_str()));

        // A reverse message dropped on the connector rides on it, backwards.
        let cv_id = cv.id.clone();
        let (r, rv) = edge(&mut doc, &dgm, "UMLReverseMessage", &bv, &cv_id);
        let r = doc.get(&r.unwrap()).unwrap();
        assert_eq!(
            (r.ref_id("source"), r.ref_id("target")),
            (Some(b.as_str()), Some(a.as_str()))
        );
        assert_eq!(r.name(), "Message2");
        assert_eq!(
            doc.get(&rv).unwrap().ref_id("hostEdge"),
            Some(cv_id.as_str())
        );
        let connectors = doc
            .get(&dgm)
            .unwrap()
            .children("ownedViews")
            .iter()
            .filter(|v| doc.get(v).unwrap().type_name == "UMLConnectorView")
            .count();
        assert_eq!(connectors, 1, "no second connector");

        let (_, self_v) = edge(&mut doc, &dgm, "UMLSelfConnector", &av, &bv);
        let self_v = doc.get(&self_v).unwrap();
        assert_eq!(self_v.ref_id("head"), self_v.ref_id("tail"));

        // Deleting the connector's view takes the messages riding on it.
        doc.edit(|e| delete_views(e, std::slice::from_ref(&cv_id)));
        assert!(doc.get(&rv).is_none());

        // Messages need lifelines at both ends.
        let class_dgm = diagram_of_type(&mut doc, "UMLClassDiagram");
        let (_, cls) = node(&mut doc, &class_dgm, "UMLClass", 0.0, 0.0, None);
        let spec = edge_spec_on("UMLForwardMessage", "UMLCommunicationDiagram").unwrap();
        assert!(
            doc.try_edit(|e| create_edge(e, &dgm, spec, &av, &cls))
                .is_err()
        );
    }

    #[test]
    fn sequence_diagrams_like_staruml() {
        let mut doc = Document::new_project();
        let dgm = diagram_offering(&mut doc, "UMLLifeline");
        // Collaboration → Interaction → diagram, plus a frame showing the diagram.
        let interaction = doc.get(&dgm).unwrap().parent.clone().unwrap();
        let collaboration = doc.get(&interaction).unwrap().parent.clone().unwrap();
        assert_eq!(doc.get(&interaction).unwrap().type_name, "UMLInteraction");
        assert_eq!(
            doc.get(&collaboration).unwrap().type_name,
            "UMLCollaboration"
        );
        let frame = doc
            .get(&doc.get(&dgm).unwrap().children("ownedViews")[0])
            .unwrap();
        assert_eq!(frame.type_name, "UMLFrameView");
        assert_eq!(frame.ref_id("model"), Some(dgm.as_str()));

        let (a, av) = node(&mut doc, &dgm, "UMLLifeline", 50.0, 40.0, None);
        let (_, bv) = node(&mut doc, &dgm, "UMLLifeline", 250.0, 40.0, None);
        // Each lifeline represents a new role of the collaboration.
        let role = doc.get(&a).unwrap().ref_id("represent").unwrap().to_owned();
        assert_eq!(
            doc.get(&role).unwrap().parent.as_deref(),
            Some(collaboration.as_str())
        );
        let line_a = doc.get(&av).unwrap().ref_id("linePart").unwrap().to_owned();
        assert_eq!(
            doc.get(&line_a).unwrap().num("left", 0.0),
            100.0,
            "centred under the head"
        );

        let spec = edge_spec("UMLMessage").unwrap();
        let (m, mv) = doc
            .try_edit(|e| create_edge_at(e, &dgm, spec, &av, &bv, Some(150.0)))
            .unwrap();
        let mv = doc.get(&mv).unwrap();
        assert_eq!(
            mv.ref_id("tail"),
            Some(line_a.as_str()),
            "messages attach to line parts"
        );
        assert_eq!(
            mdj::values::parse_points(mv.str("points").unwrap()),
            vec![(100.0, 150.0), (300.0, 150.0)]
        );
        assert!(
            mv.ref_id("activation").is_some(),
            "a call activates the receiver"
        );
        assert_eq!(
            doc.get(&m.unwrap()).unwrap().parent.as_deref(),
            Some(interaction.as_str())
        );

        let (_, reply) = edge(&mut doc, &dgm, "UMLReplyMessage", &bv, &av);
        let reply = doc.get(&reply).unwrap();
        assert!(reply.ref_id("activation").is_none());
        let y = mdj::values::parse_points(reply.str("points").unwrap())[0].1;
        assert!(y > 150.0, "placed below the last message: {y}");

        let (frag, fv) = node(&mut doc, &dgm, "UMLCombinedFragment", 30.0, 120.0, None);
        assert_eq!(
            doc.get(&frag).unwrap().str("interactionOperator"),
            Some("alt")
        );
        let comp = doc
            .get(doc.get(&fv).unwrap().ref_id("operandCompartment").unwrap())
            .unwrap();
        let operand_view = doc.get(&comp.children("subViews")[0]).unwrap();
        assert_eq!(operand_view.type_name, "UMLInteractionOperandView");
        assert!(operand_view.ref_id("guardLabel").is_some());
    }

    #[test]
    fn behaviour_elements_refuse_the_wrong_diagram() {
        let (mut doc, class_dgm) = setup();
        for tool in ["UMLState", "UMLAction", "UMLSwimlaneVert"] {
            let spec = node_spec(tool).unwrap();
            let err = doc.try_edit(|e| create_node(e, &class_dgm, spec, 0.0, 0.0, None));
            assert!(err.is_err(), "{tool} on a class diagram");
        }
    }

    #[test]
    fn parts_and_ports_need_a_classifier() {
        let (mut doc, dgm) = setup();
        let spec = node_spec("UMLPort").unwrap();
        assert!(
            doc.try_edit(|e| create_node(e, &dgm, spec, 0.0, 0.0, None))
                .is_err()
        );
        let (_, note) = node(&mut doc, &dgm, "Note", 0.0, 0.0, None);
        assert!(
            doc.try_edit(|e| create_node(e, &dgm, spec, 0.0, 0.0, Some(&note)))
                .is_err()
        );
        assert!(
            !doc.can_undo() || doc.get(&note).is_some(),
            "failed creates leave nothing behind"
        );
    }

    #[test]
    fn port_snaps_to_container_border_and_is_contained() {
        let (mut doc, dgm) = setup();
        let (_, comp_view) = node(&mut doc, &dgm, "UMLComponent", 100.0, 100.0, None);
        // Click just inside the right edge (component is 140 wide).
        let (port, port_view) = node(&mut doc, &dgm, "UMLPort", 235.0, 120.0, Some(&comp_view));
        let pv = doc.get(&port_view).unwrap();
        assert_eq!(
            pv.num("left", 0.0) + 7.0,
            240.0,
            "centred on the right border"
        );
        assert_eq!(pv.ref_id("containerView"), Some(comp_view.as_str()));
        let contained = doc
            .get(&comp_view)
            .unwrap()
            .get("containedViews")
            .unwrap()
            .refs()
            .to_vec();
        assert_eq!(contained, vec![port_view.clone()]);
        assert!(pv.ref_id("nameLabel").is_some(), "floating name label");
        assert_eq!(doc.get(&port).unwrap().type_name, "UMLPort");
        assert_eq!(
            with_contained(&doc, &comp_view),
            vec![comp_view.clone(), port_view]
        );
    }

    #[test]
    fn classes_dropped_in_packages_belong_to_them() {
        let (mut doc, dgm) = setup();
        let (pkg, pkg_view) = node(&mut doc, &dgm, "UMLPackage", 0.0, 0.0, None);
        let (class, view) = node(&mut doc, &dgm, "UMLClass", 20.0, 30.0, Some(&pkg_view));
        assert_eq!(
            doc.get(&class).unwrap().parent.as_deref(),
            Some(pkg.as_str())
        );
        assert_eq!(
            doc.get(&view).unwrap().ref_id("containerView"),
            Some(pkg_view.as_str())
        );
        // A class can't hold another class: it goes on the diagram instead.
        let (_, class_view) = node(&mut doc, &dgm, "UMLClass", 300.0, 0.0, None);
        let (other, other_view) = node(&mut doc, &dgm, "UMLClass", 310.0, 10.0, Some(&class_view));
        assert_ne!(
            doc.get(&other).unwrap().parent.as_deref(),
            Some(class.as_str())
        );
        assert!(
            doc.get(&other_view)
                .unwrap()
                .ref_id("containerView")
                .is_none()
        );
    }

    #[test]
    fn containment_link_moves_the_model_and_undoes() {
        let (mut doc, dgm) = setup();
        let (pkg, pkg_view) = node(&mut doc, &dgm, "UMLPackage", 0.0, 0.0, None);
        let (class, class_view) = node(&mut doc, &dgm, "UMLClass", 300.0, 0.0, None);
        let before = doc.to_value();
        let (m, v) = edge(&mut doc, &dgm, "UMLContainment", &class_view, &pkg_view);
        assert!(m.is_none());
        assert_eq!(doc.get(&v).unwrap().type_name, "UMLContainmentView");
        assert_eq!(
            doc.get(&class).unwrap().parent.as_deref(),
            Some(pkg.as_str())
        );
        doc.undo();
        assert_eq!(doc.to_value(), before);
    }

    #[test]
    fn deleting_a_container_view_takes_its_contents() {
        let (mut doc, dgm) = setup();
        let (_, comp_view) = node(&mut doc, &dgm, "UMLComponent", 100.0, 100.0, None);
        let (port, port_view) = node(&mut doc, &dgm, "UMLPort", 100.0, 120.0, Some(&comp_view));
        doc.edit(|e| delete_views(e, std::slice::from_ref(&comp_view)));
        assert!(doc.get(&port_view).is_none(), "port view went with its box");
        assert!(doc.get(&port).is_some(), "but the port model stays");
    }

    #[test]
    fn delete_from_model_cascades_and_undoes() {
        let (mut doc, dgm) = setup();
        let (a, va) = node(&mut doc, &dgm, "UMLClass", 0.0, 0.0, None);
        let (b, vb) = node(&mut doc, &dgm, "UMLClass", 300.0, 0.0, None);
        let attr = doc.edit(|e| add_feature(e, &a, FeatureKind::Attribute));
        doc.edit(|e| e.set(&attr, "type", Field::Ref(b.clone())));
        let (rel, ev) = edge(&mut doc, &dgm, "UMLGeneralization", &va, &vb);
        let rel = rel.unwrap();
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
    fn every_edge_tool_survives_round_trip() {
        let mut doc = Document::new_project();
        for spec in EDGES.iter().filter(|s| s.id != "UMLContainment") {
            let dgm = match spec.diagrams.first() {
                Some(ty) => diagram_of_type(&mut doc, ty),
                None => diagram_offering(&mut doc, spec.id),
            };
            let end_tool = match spec.owner {
                catalog::EdgeOwner::SourceRegion => "UMLState",
                catalog::EdgeOwner::SourceActivity => "UMLAction",
                catalog::EdgeOwner::SourceInteraction | catalog::EdgeOwner::TailRole => {
                    "UMLLifeline"
                }
                catalog::EdgeOwner::Source => "UMLClass",
            };
            let end_tool = if node_spec(end_tool)
                .is_some_and(|n| n.slot == catalog::Slot::OwnedElements)
                && doc.get(&dgm).unwrap().type_name == "UMLUseCaseDiagram"
            {
                "UMLUseCase"
            } else {
                end_tool
            };
            let (_, va) = node(&mut doc, &dgm, end_tool, 0.0, 0.0, None);
            let (_, vb) = node(&mut doc, &dgm, end_tool, 300.0, 0.0, None);
            let (_, ev) = edge(&mut doc, &dgm, spec.id, &va, &vb);
            let reloaded = Document::from_json_str(&doc.to_json_string().unwrap()).unwrap();
            let got = edge_spec_of(&reloaded, reloaded.get(&ev).unwrap()).map(|s| s.id);
            assert_eq!(got, Some(spec.id));
        }
    }

    #[test]
    fn objects_get_slot_rows() {
        let (mut doc, dgm) = setup();
        let (obj, view) = node(&mut doc, &dgm, "UMLObject", 0.0, 0.0, None);
        let slot = doc.edit(|e| add_feature(e, &obj, FeatureKind::Slot));
        doc.edit(|e| e.set(&slot, "value", Field::str("42")));
        let comp = doc
            .get(doc.get(&view).unwrap().ref_id("slotCompartment").unwrap())
            .unwrap();
        assert_eq!(comp.children("subViews").len(), 1);
        assert_eq!(slot_text(&doc, doc.get(&slot).unwrap()), "slot1 = 42");
        assert!(
            FeatureKind::Slot.allowed_on("UMLObject") && !FeatureKind::Slot.allowed_on("UMLClass")
        );
    }

    #[test]
    fn feature_text() {
        let (mut doc, dgm) = setup();
        let (c, _) = node(&mut doc, &dgm, "UMLClass", 0.0, 0.0, None);
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
