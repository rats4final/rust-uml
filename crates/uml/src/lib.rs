//! UML elements in the shape StarUML stores them (see docs/evidence.md §3):
//! a catalog of what can be created, factories that build the exact view
//! trees, cascading delete, and the text UML notation shows.

pub mod catalog;
mod delete;
mod factory;
mod text;

pub use catalog::{
    DIAGRAMS, EDGES, EdgeSpec, NODES, NodeSpec, TOOL_GROUPS, Tool, ToolGroup, edge_spec, node_spec,
    node_spec_for_view, tool, tool_groups,
};
pub use delete::{delete_elements, delete_views};
pub use factory::{
    FeatureKind, add_edge_view, add_feature, add_node_view, add_parameter, compartment_items,
    container_owner, create_edge, create_node, edge_spec_of, owner_for_new_elements,
    return_parameter, views_of, with_contained,
};
pub use text::{
    attribute_text, instance_text, operation_text, part_text, slot_text, stereotype_text,
    typed_name, var_text, visibility_symbol,
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
        let spec = node_spec(tool).unwrap();
        let (m, v) = doc
            .try_edit(|e| create_node(e, dgm, spec, x, y, container))
            .unwrap();
        (m.unwrap_or_default(), v)
    }

    fn edge(doc: &mut Document, dgm: &str, tool: &str, tail: &str, head: &str) -> (Option<Id>, Id) {
        let spec = edge_spec(tool).unwrap();
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

    #[test]
    fn every_node_tool_builds_its_view_type() {
        let (mut doc, dgm) = setup();
        let (class, class_view) = node(&mut doc, &dgm, "UMLClass", 0.0, 0.0, None);
        for spec in NODES {
            let container =
                (spec.placement == catalog::Placement::OnClassifier).then_some(class_view.as_str());
            let (_, v) = node(&mut doc, &dgm, spec.id, 300.0, 300.0, container);
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
        }
        // Parts and ports became attributes of the class.
        let attrs: Vec<&str> = doc
            .get(&class)
            .unwrap()
            .children("attributes")
            .iter()
            .map(|a| doc.get(a).unwrap().type_name.as_str())
            .collect();
        assert_eq!(attrs, ["UMLAttribute", "UMLPort"]);
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
        let (mut doc, dgm) = setup();
        let (_, va) = node(&mut doc, &dgm, "UMLClass", 0.0, 0.0, None);
        let (_, vb) = node(&mut doc, &dgm, "UMLClass", 300.0, 0.0, None);
        for spec in EDGES.iter().filter(|s| s.id != "UMLContainment") {
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
