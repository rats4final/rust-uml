//! What can be created, as data: one row per toolbox tool, plus which tools
//! each diagram type offers. Ids match StarUML's toolbox ids (e.g. `UMLPart`)
//! so behaviour can be compared tool by tool. Field names and type names are
//! the file format (see docs/evidence.md); notation lives in `render`.

/// Where a new model element goes in its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// `ownedElements` of the diagram's owner, or of the container's model.
    OwnedElements,
    /// `attributes` of the container's classifier (parts, ports).
    Attributes,
    /// `vertices` of a region: of the state the box is dropped on, else of
    /// the diagram's state machine.
    Vertices,
    /// `nodes` of the diagram's activity.
    ActivityNodes,
    /// `groups` of the diagram's activity (swimlanes).
    ActivityGroups,
}

/// Whether a node can or must be dropped onto another box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Anywhere; inside a box that accepts it, it becomes a child of that box.
    Free,
    /// Must be dropped onto a classifier box (class, component, node, …).
    OnClassifier,
}

/// A value written into a new element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Init {
    Str(&'static str),
    Bool(bool),
    Int(i64),
}

pub const CLASSIFIER_COMPARTMENTS: &[(&str, &str)] = &[
    ("attributeCompartment", "UMLAttributeCompartmentView"),
    ("operationCompartment", "UMLOperationCompartmentView"),
    ("receptionCompartment", "UMLReceptionCompartmentView"),
    (
        "templateParameterCompartment",
        "UMLTemplateParameterCompartmentView",
    ),
];
const ENUM_COMPARTMENTS: &[(&str, &str)] = &[
    ("attributeCompartment", "UMLAttributeCompartmentView"),
    ("operationCompartment", "UMLOperationCompartmentView"),
    ("receptionCompartment", "UMLReceptionCompartmentView"),
    (
        "templateParameterCompartment",
        "UMLTemplateParameterCompartmentView",
    ),
    (
        "enumerationLiteralCompartment",
        "UMLEnumerationLiteralCompartmentView",
    ),
];
const OBJECT_COMPARTMENTS: &[(&str, &str)] = &[("slotCompartment", "UMLSlotCompartmentView")];
const COLLABORATION_COMPARTMENTS: &[(&str, &str)] = &[(
    "templateParameterCompartment",
    "UMLTemplateParameterCompartmentView",
)];

/// StarUML's preferences start these classifiers with their attribute and
/// operation compartments hidden (`uml.<kind>.suppress*`). The metamodel
/// default is "shown", so the value must be written.
const SUPPRESSED: &[(&str, Init)] = &[
    ("suppressAttributes", Init::Bool(true)),
    ("suppressOperations", Init::Bool(true)),
];
const ARTIFACT_VIEW: &[(&str, Init)] = &[
    ("stereotypeDisplay", Init::Str("icon")),
    ("suppressAttributes", Init::Bool(true)),
    ("suppressOperations", Init::Bool(true)),
];
/// Interfaces are drawn as boxes here (StarUML's preference is the
/// lollipop icon); pin it so StarUML shows the same.
const INTERFACE_VIEW: &[(&str, Init)] = &[("stereotypeDisplay", Init::Str("label"))];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeSpec {
    /// Toolbox id, as in StarUML.
    pub id: &'static str,
    pub label: &'static str,
    /// `None` for view-only shapes (notes, text).
    pub model_type: Option<&'static str>,
    pub view_type: &'static str,
    pub slot: Slot,
    pub placement: Placement,
    pub base_name: &'static str,
    /// Has a `UMLNameCompartmentView` (`nameCompartment`).
    pub name_compartment: bool,
    /// Floating name/stereotype/property labels instead (ports).
    pub floating_labels: bool,
    /// `(view field, compartment view type)`, in StarUML's order.
    pub compartments: &'static [(&'static str, &'static str)],
    pub size: (f64, f64),
    pub model_fields: &'static [(&'static str, Init)],
    pub view_fields: &'static [(&'static str, Init)],
    /// Children created with the model: `(list field, element type)`, e.g.
    /// the regions of a composite state.
    pub model_children: &'static [(&'static str, &'static str)],
    /// `LabelView` sub-views referenced by these view fields (swimlane name).
    pub labels: &'static [&'static str],
}

const fn node(
    id: &'static str,
    label: &'static str,
    model_type: &'static str,
    view_type: &'static str,
    base_name: &'static str,
    size: (f64, f64),
) -> NodeSpec {
    NodeSpec {
        id,
        label,
        model_type: Some(model_type),
        view_type,
        slot: Slot::OwnedElements,
        placement: Placement::Free,
        base_name,
        name_compartment: true,
        floating_labels: false,
        compartments: &[],
        size,
        model_fields: &[],
        view_fields: &[],
        model_children: &[],
        labels: &[],
    }
}

const fn classifier(
    id: &'static str,
    label: &'static str,
    view_type: &'static str,
    base_name: &'static str,
) -> NodeSpec {
    NodeSpec {
        compartments: CLASSIFIER_COMPARTMENTS,
        ..node(id, label, id, view_type, base_name, (140.0, 60.0))
    }
}

const STATE_COMPARTMENTS: &[(&str, &str)] = &[
    (
        "internalActivityCompartment",
        "UMLInternalActivityCompartmentView",
    ),
    (
        "internalTransitionCompartment",
        "UMLInternalTransitionCompartmentView",
    ),
    (
        "decompositionCompartment",
        "UMLDecompositionCompartmentView",
    ),
];
const USECASE_COMPARTMENTS: &[(&str, &str)] = &[
    ("attributeCompartment", "UMLAttributeCompartmentView"),
    ("operationCompartment", "UMLOperationCompartmentView"),
    ("receptionCompartment", "UMLReceptionCompartmentView"),
    (
        "templateParameterCompartment",
        "UMLTemplateParameterCompartmentView",
    ),
    (
        "extensionPointCompartment",
        "UMLExtensionPointCompartmentView",
    ),
];

/// A state-machine vertex: an element in a region's `vertices`.
const fn vertex(
    id: &'static str,
    label: &'static str,
    model_type: &'static str,
    view_type: &'static str,
    base_name: &'static str,
    size: (f64, f64),
) -> NodeSpec {
    NodeSpec {
        slot: Slot::Vertices,
        ..node(id, label, model_type, view_type, base_name, size)
    }
}

/// A pseudostate (initial, choice, fork, history, …): a small symbol with
/// floating labels; `kind` says which.
const fn pseudostate(
    id: &'static str,
    label: &'static str,
    kind: &'static [(&'static str, Init)],
    size: (f64, f64),
) -> NodeSpec {
    NodeSpec {
        name_compartment: false,
        floating_labels: true,
        model_fields: kind,
        ..vertex(id, label, "UMLPseudostate", "UMLPseudostateView", "", size)
    }
}

/// An activity node (action, control node, object node) in the activity's `nodes`.
const fn activity_node(
    id: &'static str,
    label: &'static str,
    model_type: &'static str,
    view_type: &'static str,
    base_name: &'static str,
    size: (f64, f64),
) -> NodeSpec {
    NodeSpec {
        slot: Slot::ActivityNodes,
        ..node(id, label, model_type, view_type, base_name, size)
    }
}

/// A control node (initial, final, fork, join, merge, decision): one view
/// type, drawn by its model's type.
const fn control_node(id: &'static str, label: &'static str, size: (f64, f64)) -> NodeSpec {
    NodeSpec {
        name_compartment: false,
        ..activity_node(id, label, id, "UMLControlNodeView", "", size)
    }
}

const ONE_REGION: &[(&str, &str)] = &[("regions", "UMLRegion")];
const TWO_REGIONS: &[(&str, &str)] = &[("regions", "UMLRegion"), ("regions", "UMLRegion")];

pub const NODES: &[NodeSpec] = &[
    // Classes
    classifier("UMLClass", "Class", "UMLClassView", "Class"),
    NodeSpec {
        view_fields: INTERFACE_VIEW,
        ..classifier("UMLInterface", "Interface", "UMLInterfaceView", "Interface")
    },
    NodeSpec {
        compartments: ENUM_COMPARTMENTS,
        view_fields: SUPPRESSED,
        ..classifier(
            "UMLEnumeration",
            "Enumeration",
            "UMLEnumerationView",
            "Enumeration",
        )
    },
    classifier("UMLDataType", "DataType", "UMLDataTypeView", "DataType"),
    classifier(
        "UMLPrimitiveType",
        "PrimitiveType",
        "UMLPrimitiveTypeView",
        "PrimitiveType",
    ),
    NodeSpec {
        view_fields: SUPPRESSED,
        ..classifier("UMLSignal", "Signal", "UMLSignalView", "Signal")
    },
    // Packages
    node(
        "UMLPackage",
        "Package",
        "UMLPackage",
        "UMLPackageView",
        "Package",
        (120.0, 80.0),
    ),
    node(
        "UMLModel",
        "Model",
        "UMLModel",
        "UMLModelView",
        "Model",
        (120.0, 80.0),
    ),
    node(
        "UMLSubsystem",
        "Subsystem",
        "UMLSubsystem",
        "UMLSubsystemView",
        "Subsystem",
        (120.0, 80.0),
    ),
    // Components and deployment
    NodeSpec {
        view_fields: SUPPRESSED,
        ..classifier("UMLComponent", "Component", "UMLComponentView", "Component")
    },
    NodeSpec {
        view_fields: ARTIFACT_VIEW,
        ..classifier("UMLArtifact", "Artifact", "UMLArtifactView", "Artifact")
    },
    NodeSpec {
        view_fields: SUPPRESSED,
        size: (140.0, 80.0),
        ..classifier("UMLNode", "Node", "UMLNodeView", "Node")
    },
    // Composite structure
    NodeSpec {
        compartments: COLLABORATION_COMPARTMENTS,
        ..node(
            "UMLCollaboration",
            "Collaboration",
            "UMLCollaboration",
            "UMLCollaborationView",
            "Collaboration",
            (140.0, 70.0),
        )
    },
    node(
        "UMLCollaborationUse",
        "CollaborationUse",
        "UMLCollaborationUse",
        "UMLCollaborationUseView",
        "CollaborationUse",
        (140.0, 50.0),
    ),
    NodeSpec {
        slot: Slot::Attributes,
        placement: Placement::OnClassifier,
        model_fields: &[("aggregation", Init::Str("composite"))],
        ..node(
            "UMLPart",
            "Part",
            "UMLAttribute",
            "UMLPartView",
            "Part",
            (100.0, 40.0),
        )
    },
    NodeSpec {
        slot: Slot::Attributes,
        placement: Placement::OnClassifier,
        name_compartment: false,
        floating_labels: true,
        ..node(
            "UMLPort",
            "Port",
            "UMLPort",
            "UMLPortView",
            "Port",
            (14.0, 14.0),
        )
    },
    // Instances
    NodeSpec {
        compartments: OBJECT_COMPARTMENTS,
        ..node(
            "UMLObject",
            "Object",
            "UMLObject",
            "UMLObjectView",
            "Object",
            (140.0, 50.0),
        )
    },
    node(
        "UMLArtifactInstance",
        "ArtifactInstance",
        "UMLArtifactInstance",
        "UMLArtifactInstanceView",
        "ArtifactInstance",
        (140.0, 50.0),
    ),
    node(
        "UMLComponentInstance",
        "ComponentInstance",
        "UMLComponentInstance",
        "UMLComponentInstanceView",
        "ComponentInstance",
        (140.0, 50.0),
    ),
    node(
        "UMLNodeInstance",
        "NodeInstance",
        "UMLNodeInstance",
        "UMLNodeInstanceView",
        "NodeInstance",
        (140.0, 80.0),
    ),
    // Profiles
    NodeSpec {
        view_fields: SUPPRESSED,
        ..classifier(
            "UMLStereotype",
            "Stereotype",
            "UMLStereotypeView",
            "Stereotype",
        )
    },
    node(
        "UMLMetaClass",
        "MetaClass",
        "UMLMetaClass",
        "UMLMetaClassView",
        "MetaClass",
        (120.0, 40.0),
    ),
    // Use cases
    NodeSpec {
        compartments: USECASE_COMPARTMENTS,
        view_fields: SUPPRESSED,
        size: (110.0, 45.0),
        ..classifier("UMLUseCase", "Use Case", "UMLUseCaseView", "UseCase")
    },
    NodeSpec {
        view_fields: SUPPRESSED,
        size: (40.0, 80.0),
        ..classifier("UMLActor", "Actor", "UMLActorView", "Actor")
    },
    node(
        "UMLUseCaseSubject",
        "Use Case Subject",
        "UMLUseCaseSubject",
        "UMLUseCaseSubjectView",
        "Subject",
        (240.0, 260.0),
    ),
    // State machines
    NodeSpec {
        compartments: STATE_COMPARTMENTS,
        ..vertex(
            "UMLState",
            "State",
            "UMLState",
            "UMLStateView",
            "State",
            (100.0, 50.0),
        )
    },
    NodeSpec {
        compartments: STATE_COMPARTMENTS,
        model_children: ONE_REGION,
        ..vertex(
            "UMLCompositeState",
            "Composite State",
            "UMLState",
            "UMLStateView",
            "State",
            (220.0, 140.0),
        )
    },
    NodeSpec {
        compartments: STATE_COMPARTMENTS,
        model_children: TWO_REGIONS,
        ..vertex(
            "UMLOrthogonalState",
            "Orthogonal State",
            "UMLState",
            "UMLStateView",
            "State",
            (220.0, 180.0),
        )
    },
    pseudostate(
        "UMLInitialState",
        "Initial State",
        &[("kind", Init::Str("initial"))],
        (20.0, 20.0),
    ),
    NodeSpec {
        name_compartment: false,
        ..vertex(
            "UMLFinalState",
            "Final State",
            "UMLFinalState",
            "UMLFinalStateView",
            "",
            (26.0, 26.0),
        )
    },
    pseudostate(
        "UMLChoice",
        "Choice",
        &[("kind", Init::Str("choice"))],
        (30.0, 24.0),
    ),
    pseudostate(
        "UMLJunction",
        "Junction",
        &[("kind", Init::Str("junction"))],
        (16.0, 16.0),
    ),
    pseudostate(
        "UMLFork",
        "Fork",
        &[("kind", Init::Str("fork"))],
        (70.0, 6.0),
    ),
    pseudostate(
        "UMLJoin",
        "Join",
        &[("kind", Init::Str("join"))],
        (70.0, 6.0),
    ),
    pseudostate(
        "UMLShallowHistory",
        "Shallow History",
        &[("kind", Init::Str("shallowHistory"))],
        (26.0, 26.0),
    ),
    pseudostate(
        "UMLDeepHistory",
        "Deep History",
        &[("kind", Init::Str("deepHistory"))],
        (26.0, 26.0),
    ),
    pseudostate(
        "UMLEntryPoint",
        "Entry Point",
        &[("kind", Init::Str("entryPoint"))],
        (20.0, 20.0),
    ),
    pseudostate(
        "UMLExitPoint",
        "Exit Point",
        &[("kind", Init::Str("exitPoint"))],
        (20.0, 20.0),
    ),
    pseudostate(
        "UMLTerminate",
        "Terminate",
        &[("kind", Init::Str("terminate"))],
        (20.0, 20.0),
    ),
    // Activities
    activity_node(
        "UMLAction",
        "Action",
        "UMLAction",
        "UMLActionView",
        "Action",
        (110.0, 45.0),
    ),
    control_node("UMLInitialNode", "Initial", (20.0, 20.0)),
    control_node("UMLActivityFinalNode", "Final", (26.0, 26.0)),
    control_node("UMLFlowFinalNode", "Flow Final", (26.0, 26.0)),
    control_node("UMLForkNode", "Fork", (70.0, 6.0)),
    control_node("UMLJoinNode", "Join", (70.0, 6.0)),
    control_node("UMLMergeNode", "Merge", (30.0, 24.0)),
    control_node("UMLDecisionNode", "Decision", (30.0, 24.0)),
    activity_node(
        "UMLObjectNode",
        "Object Node",
        "UMLObjectNode",
        "UMLObjectNodeView",
        "ObjectNode",
        (100.0, 40.0),
    ),
    NodeSpec {
        slot: Slot::ActivityGroups,
        name_compartment: false,
        labels: &["nameLabel"],
        ..node(
            "UMLSwimlaneVert",
            "Swimlane (Vertical)",
            "UMLActivityPartition",
            "UMLSwimlaneView",
            "Partition",
            (200.0, 400.0),
        )
    },
    NodeSpec {
        slot: Slot::ActivityGroups,
        name_compartment: false,
        labels: &["nameLabel"],
        // The metamodel default is vertical; a horizontal lane must say so.
        view_fields: &[("isVertical", Init::Bool(false))],
        ..node(
            "UMLSwimlaneHorz",
            "Swimlane (Horizontal)",
            "UMLActivityPartition",
            "UMLSwimlaneView",
            "Partition",
            (500.0, 150.0),
        )
    },
    // Annotations (no model element)
    NodeSpec {
        model_type: None,
        name_compartment: false,
        view_fields: &[("text", Init::Str("Note"))],
        ..node("Note", "Note", "", "UMLNoteView", "", (100.0, 50.0))
    },
    NodeSpec {
        model_type: None,
        name_compartment: false,
        view_fields: &[("text", Init::Str("Text"))],
        ..node("Text", "Text", "", "UMLTextView", "", (80.0, 25.0))
    },
];

/// How an edge's model element is shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeModel {
    /// `source` (tail) and `target` (head) references.
    Directed,
    /// `end1` (tail) and `end2` (head) child ends of `end_type`.
    Undirected {
        end_type: &'static str,
        tail_aggregation: &'static str,
        head_navigable: &'static str,
    },
    /// No model element: the line is only a view.
    ViewOnly,
}

/// Which label sub-views the edge view carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeLabels {
    None,
    /// name, stereotype, property (UMLGeneralEdgeView).
    General,
    /// General + role name, multiplicity and property at each end.
    Ends,
    /// Ends + qualifier compartments (UMLAssociationView and subclasses).
    EndsAndQualifiers,
}

/// Where a new relationship's model element goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeOwner {
    /// `ownedElements` of the source (tail) element, as for most relationships.
    Source,
    /// `transitions` of the region that holds the source vertex.
    SourceRegion,
    /// `edges` of the activity that holds the source node.
    SourceActivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub model_type: Option<&'static str>,
    pub view_type: &'static str,
    pub model: EdgeModel,
    pub labels: EdgeLabels,
    pub owner: EdgeOwner,
}

const fn directed(
    id: &'static str,
    label: &'static str,
    model: &'static str,
    view: &'static str,
) -> EdgeSpec {
    EdgeSpec {
        id,
        label,
        model_type: Some(model),
        view_type: view,
        model: EdgeModel::Directed,
        labels: EdgeLabels::General,
        owner: EdgeOwner::Source,
    }
}

const fn undirected(
    id: &'static str,
    label: &'static str,
    model: &'static str,
    view: &'static str,
    end_type: &'static str,
    tail_aggregation: &'static str,
    head_navigable: &'static str,
) -> EdgeSpec {
    EdgeSpec {
        id,
        label,
        model_type: Some(model),
        view_type: view,
        model: EdgeModel::Undirected {
            end_type,
            tail_aggregation,
            head_navigable,
        },
        labels: EdgeLabels::Ends,
        owner: EdgeOwner::Source,
    }
}

const fn association(
    id: &'static str,
    label: &'static str,
    agg: &'static str,
    nav: &'static str,
) -> EdgeSpec {
    EdgeSpec {
        labels: EdgeLabels::EndsAndQualifiers,
        ..undirected(
            id,
            label,
            "UMLAssociation",
            "UMLAssociationView",
            "UMLAssociationEnd",
            agg,
            nav,
        )
    }
}

pub const EDGES: &[EdgeSpec] = &[
    association("UMLAssociation", "Association", "none", "unspecified"),
    association(
        "UMLDirectedAssociation",
        "Directed Association",
        "none",
        "navigable",
    ),
    association("UMLAggregation", "Aggregation", "shared", "navigable"),
    association("UMLComposition", "Composition", "composite", "navigable"),
    directed(
        "UMLGeneralization",
        "Generalization",
        "UMLGeneralization",
        "UMLGeneralizationView",
    ),
    directed(
        "UMLDependency",
        "Dependency",
        "UMLDependency",
        "UMLDependencyView",
    ),
    directed(
        "UMLInterfaceRealization",
        "Interface Realization",
        "UMLInterfaceRealization",
        "UMLInterfaceRealizationView",
    ),
    directed(
        "UMLRealization",
        "Realization",
        "UMLRealization",
        "UMLRealizationView",
    ),
    directed(
        "UMLComponentRealization",
        "Component Realization",
        "UMLComponentRealization",
        "UMLComponentRealizationView",
    ),
    directed(
        "UMLDeployment",
        "Deployment",
        "UMLDeployment",
        "UMLDeploymentView",
    ),
    EdgeSpec {
        labels: EdgeLabels::EndsAndQualifiers,
        ..undirected(
            "UMLCommunicationPath",
            "Communication Path",
            "UMLCommunicationPath",
            "UMLCommunicationPathView",
            "UMLAssociationEnd",
            "none",
            "unspecified",
        )
    },
    undirected(
        "UMLConnector",
        "Connector",
        "UMLConnector",
        "UMLConnectorView",
        "UMLConnectorEnd",
        "none",
        "unspecified",
    ),
    undirected(
        "UMLLink",
        "Link",
        "UMLLink",
        "UMLLinkView",
        "UMLLinkEnd",
        "none",
        "unspecified",
    ),
    undirected(
        "UMLDirectedLink",
        "Directed Link",
        "UMLLink",
        "UMLLinkView",
        "UMLLinkEnd",
        "none",
        "navigable",
    ),
    directed(
        "UMLExtension",
        "Extension",
        "UMLExtension",
        "UMLExtensionView",
    ),
    directed("UMLInclude", "Include", "UMLInclude", "UMLIncludeView"),
    directed("UMLExtend", "Extend", "UMLExtend", "UMLExtendView"),
    EdgeSpec {
        owner: EdgeOwner::SourceRegion,
        ..directed(
            "UMLTransition",
            "Transition",
            "UMLTransition",
            "UMLTransitionView",
        )
    },
    // Same model and view as a transition; the tool connects a state to itself.
    EdgeSpec {
        owner: EdgeOwner::SourceRegion,
        ..directed(
            "UMLSelfTransition",
            "Self Transition",
            "UMLTransition",
            "UMLTransitionView",
        )
    },
    EdgeSpec {
        owner: EdgeOwner::SourceActivity,
        ..directed(
            "UMLControlFlow",
            "Control Flow",
            "UMLControlFlow",
            "UMLControlFlowView",
        )
    },
    EdgeSpec {
        owner: EdgeOwner::SourceActivity,
        ..directed(
            "UMLObjectFlow",
            "Object Flow",
            "UMLObjectFlow",
            "UMLObjectFlowView",
        )
    },
    EdgeSpec {
        id: "UMLContainment",
        label: "Containment",
        model_type: None,
        view_type: "UMLContainmentView",
        model: EdgeModel::ViewOnly,
        labels: EdgeLabels::None,
        owner: EdgeOwner::Source,
    },
    EdgeSpec {
        id: "NoteLink",
        label: "Note Link",
        model_type: None,
        view_type: "UMLNoteLinkView",
        model: EdgeModel::ViewOnly,
        labels: EdgeLabels::None,
        owner: EdgeOwner::Source,
    },
];

/// A toolbox section, shown on the listed diagram types (empty = all).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolGroup {
    pub label: &'static str,
    pub diagrams: &'static [&'static str],
    pub tools: &'static [&'static str],
}

/// Mirrors StarUML's UML toolbox groups and the diagram types they appear on.
pub const TOOL_GROUPS: &[ToolGroup] = &[
    ToolGroup {
        label: "Classes",
        diagrams: &["UMLClassDiagram", "UMLCompositeStructureDiagram"],
        tools: &[
            "UMLClass",
            "UMLInterface",
            "UMLAssociation",
            "UMLDirectedAssociation",
            "UMLAggregation",
            "UMLComposition",
            "UMLDependency",
            "UMLGeneralization",
            "UMLInterfaceRealization",
        ],
    },
    ToolGroup {
        label: "Classes (Advanced)",
        diagrams: &["UMLClassDiagram", "UMLCompositeStructureDiagram"],
        tools: &[
            "UMLSignal",
            "UMLDataType",
            "UMLPrimitiveType",
            "UMLEnumeration",
            "UMLRealization",
        ],
    },
    ToolGroup {
        label: "Packages",
        diagrams: &["UMLClassDiagram", "UMLPackageDiagram"],
        tools: &[
            "UMLPackage",
            "UMLModel",
            "UMLSubsystem",
            "UMLContainment",
            "UMLDependency",
        ],
    },
    ToolGroup {
        label: "Components",
        diagrams: &["UMLComponentDiagram", "UMLDeploymentDiagram"],
        tools: &[
            "UMLComponent",
            "UMLArtifact",
            "UMLInterface",
            "UMLDependency",
            "UMLInterfaceRealization",
            "UMLComponentRealization",
        ],
    },
    ToolGroup {
        label: "Deployment",
        diagrams: &["UMLComponentDiagram", "UMLDeploymentDiagram"],
        tools: &["UMLNode", "UMLDeployment", "UMLCommunicationPath"],
    },
    ToolGroup {
        label: "Composite Structure",
        diagrams: &[
            "UMLClassDiagram",
            "UMLCompositeStructureDiagram",
            "UMLComponentDiagram",
        ],
        tools: &[
            "UMLCollaboration",
            "UMLPort",
            "UMLPart",
            "UMLConnector",
            "UMLCollaborationUse",
            "UMLRealization",
        ],
    },
    ToolGroup {
        label: "Instances",
        diagrams: &[
            "UMLClassDiagram",
            "UMLObjectDiagram",
            "UMLComponentDiagram",
            "UMLDeploymentDiagram",
        ],
        tools: &[
            "UMLObject",
            "UMLArtifactInstance",
            "UMLComponentInstance",
            "UMLNodeInstance",
            "UMLLink",
            "UMLDirectedLink",
        ],
    },
    ToolGroup {
        label: "Profile",
        diagrams: &["UMLProfileDiagram"],
        tools: &[
            "UMLMetaClass",
            "UMLStereotype",
            "UMLEnumeration",
            "UMLExtension",
            "UMLGeneralization",
        ],
    },
    ToolGroup {
        label: "Use Cases",
        diagrams: &["UMLUseCaseDiagram"],
        tools: &[
            "UMLPackage",
            "UMLUseCaseSubject",
            "UMLUseCase",
            "UMLActor",
            "UMLAssociation",
            "UMLDirectedAssociation",
            "UMLGeneralization",
            "UMLDependency",
            "UMLInclude",
            "UMLExtend",
        ],
    },
    ToolGroup {
        label: "States",
        diagrams: &["UMLStatechartDiagram"],
        tools: &[
            "UMLState",
            "UMLInitialState",
            "UMLChoice",
            "UMLJoin",
            "UMLFork",
            "UMLFinalState",
            "UMLTransition",
            "UMLSelfTransition",
        ],
    },
    ToolGroup {
        label: "States (Advanced)",
        diagrams: &["UMLStatechartDiagram"],
        tools: &[
            "UMLCompositeState",
            "UMLOrthogonalState",
            "UMLJunction",
            "UMLShallowHistory",
            "UMLDeepHistory",
            "UMLEntryPoint",
            "UMLExitPoint",
            "UMLTerminate",
        ],
    },
    ToolGroup {
        label: "Activities",
        diagrams: &["UMLActivityDiagram"],
        tools: &[
            "UMLAction",
            "UMLInitialNode",
            "UMLActivityFinalNode",
            "UMLForkNode",
            "UMLJoinNode",
            "UMLMergeNode",
            "UMLDecisionNode",
            "UMLControlFlow",
        ],
    },
    ToolGroup {
        label: "Activities (Advanced)",
        diagrams: &["UMLActivityDiagram"],
        tools: &[
            "UMLSwimlaneVert",
            "UMLSwimlaneHorz",
            "UMLFlowFinalNode",
            "UMLObjectNode",
            "UMLObjectFlow",
        ],
    },
    ToolGroup {
        label: "Annotations",
        diagrams: &[],
        tools: &["Note", "Text", "NoteLink"],
    },
];

/// Diagram types that can be created, with their menu label and base name.
pub const DIAGRAMS: &[(&str, &str, &str)] = &[
    ("UMLClassDiagram", "Class Diagram", "ClassDiagram"),
    ("UMLPackageDiagram", "Package Diagram", "PackageDiagram"),
    ("UMLObjectDiagram", "Object Diagram", "ObjectDiagram"),
    (
        "UMLComponentDiagram",
        "Component Diagram",
        "ComponentDiagram",
    ),
    (
        "UMLDeploymentDiagram",
        "Deployment Diagram",
        "DeploymentDiagram",
    ),
    (
        "UMLCompositeStructureDiagram",
        "Composite Structure Diagram",
        "CompositeStructureDiagram",
    ),
    ("UMLProfileDiagram", "Profile Diagram", "ProfileDiagram"),
    ("UMLUseCaseDiagram", "Use Case Diagram", "UseCaseDiagram"),
    (
        "UMLStatechartDiagram",
        "Statechart Diagram",
        "StatechartDiagram",
    ),
    ("UMLActivityDiagram", "Activity Diagram", "ActivityDiagram"),
];

/// A tool, either kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Node(&'static NodeSpec),
    Edge(&'static EdgeSpec),
}

impl Tool {
    pub fn label(&self) -> &'static str {
        match self {
            Tool::Node(n) => n.label,
            Tool::Edge(e) => e.label,
        }
    }
}

fn normalize(s: &str) -> String {
    s.to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .trim_start_matches("uml")
        .to_owned()
}

/// Look a node tool up by id (`UMLClass`) or label (`class`, `Class`).
pub fn node_spec(name: &str) -> Option<&'static NodeSpec> {
    let n = normalize(name);
    NODES
        .iter()
        .find(|s| normalize(s.id) == n || normalize(s.label) == n)
}

/// Look an edge tool up by id (`UMLComposition`) or label (`Interface Realization`).
pub fn edge_spec(name: &str) -> Option<&'static EdgeSpec> {
    let n = normalize(name);
    EDGES
        .iter()
        .find(|s| normalize(s.id) == n || normalize(s.label) == n)
}

pub fn tool(id: &str) -> Option<Tool> {
    node_spec(id)
        .map(Tool::Node)
        .or_else(|| edge_spec(id).map(Tool::Edge))
}

/// Toolbox groups for a diagram type, in order.
pub fn tool_groups(diagram_type: &str) -> impl Iterator<Item = &'static ToolGroup> {
    let t = diagram_type.to_owned();
    TOOL_GROUPS
        .iter()
        .filter(move |g| g.diagrams.is_empty() || g.diagrams.contains(&t.as_str()))
}

/// The spec whose view type is `view_type` (first match).
pub fn node_spec_for_view(view_type: &str) -> Option<&'static NodeSpec> {
    NODES.iter().find(|s| s.view_type == view_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_in_a_group_exists() {
        for g in TOOL_GROUPS {
            for t in g.tools {
                assert!(tool(t).is_some(), "{} lists unknown tool {t}", g.label);
            }
        }
    }

    #[test]
    fn ids_and_labels_resolve_both_ways() {
        assert_eq!(node_spec("UMLClass").map(|s| s.id), Some("UMLClass"));
        assert_eq!(node_spec("class").map(|s| s.id), Some("UMLClass"));
        assert_eq!(
            edge_spec("interface-realization").map(|s| s.id),
            Some("UMLInterfaceRealization")
        );
        assert_eq!(
            edge_spec("UMLDirectedLink").map(|s| s.label),
            Some("Directed Link")
        );
        assert!(node_spec("nonsense").is_none());
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = NODES
            .iter()
            .map(|s| s.id)
            .chain(EDGES.iter().map(|s| s.id))
            .collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn component_diagram_offers_ports_and_annotations() {
        let tools: Vec<&str> = tool_groups("UMLComponentDiagram")
            .flat_map(|g| g.tools.iter().copied())
            .collect();
        for t in ["UMLComponent", "UMLPort", "UMLNode", "Note", "NoteLink"] {
            assert!(tools.contains(&t), "missing {t}");
        }
        assert!(!tools.contains(&"UMLMetaClass"));
    }
}
