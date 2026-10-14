//! What can be created, as data: one row per toolbox tool, plus which tools
//! each diagram type offers. Ids match StarUML's toolbox ids (e.g. `UMLPart`)
//! so behaviour can be compared tool by tool. Field names and type names are
//! the file format (see docs/evidence.md); notation lives in `render`.

use std::f64::consts::PI;

use mdj::values::line_style;

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
    /// `participants` of the diagram's interaction (lifelines).
    Participants,
    /// `fragments` of the diagram's interaction (combined fragments).
    Fragments,
    /// Timing diagrams. A lifeline: `participants` of the interaction, its
    /// view inside the timing frame.
    TimingLifeline,
    /// A state/condition: a constraint in the lifeline's `ownedElements`,
    /// its view inside the lifeline's (dropped on a lifeline or a state).
    TimingState,
    /// A time segment: a state invariant in the interaction's `fragments`,
    /// its view inside the state's (dropped on a state).
    TimingSegment,
    /// A time tick: a view only, inside the timing frame.
    TimingTick,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Floating labels instead (ports, pseudostates, Azure services):
    /// `(view field, distance, angle)` from the node's centre.
    pub floating_labels: &'static [(&'static str, f64, f64)],
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
    /// Diagram types this row is for, when the same tool id draws differently
    /// on different diagrams (a lifeline). Empty: any diagram.
    pub diagrams: &'static [&'static str],
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
        floating_labels: &[],
        compartments: &[],
        size,
        model_fields: &[],
        view_fields: &[],
        model_children: &[],
        labels: &[],
        diagrams: &[],
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
        floating_labels: UML_FLOATING,
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
/// A flowchart symbol: a box with a centred name label.
const fn flowchart(
    id: &'static str,
    label: &'static str,
    view_type: &'static str,
    base_name: &'static str,
    size: (f64, f64),
) -> NodeSpec {
    NodeSpec {
        name_compartment: false,
        labels: &["nameLabel"],
        ..node(id, label, id, view_type, base_name, size)
    }
}

const fn control_node(id: &'static str, label: &'static str, size: (f64, f64)) -> NodeSpec {
    NodeSpec {
        name_compartment: false,
        ..activity_node(id, label, id, "UMLControlNodeView", "", size)
    }
}

const C4_LABELS: &[&str] = &["nameLabel", "technologyLabel", "descriptionLabel"];
// StarUML's default fill colours (`c4.<kind>.fillColor` preferences).
const C4_PERSON: &[(&str, Init)] = &[("fillColor", Init::Str("#38bcff"))];
const C4_SYSTEM: &[(&str, Init)] = &[("fillColor", Init::Str("#6dceff"))];
const C4_CONTAINER: &[(&str, Init)] = &[("fillColor", Init::Str("#a3e0ff"))];
const C4_COMPONENT: &[(&str, Init)] = &[("fillColor", Init::Str("#d8f2ff"))];
// Container kinds the toolbox creates (`C4ContainerKind`; new containers are `etc`).
const ETC: &[(&str, Init)] = &[("kind", Init::Str("etc"))];
const DATABASE: &[(&str, Init)] = &[("kind", Init::Str("database"))];
const WEB_APP: &[(&str, Init)] = &[("kind", Init::Str("client-webapp"))];
const DESKTOP_APP: &[(&str, Init)] = &[("kind", Init::Str("desktop-app"))];
const MOBILE_APP: &[(&str, Init)] = &[("kind", Init::Str("mobile-app"))];

/// A C4 element (`(type, view type)`) with its three labels. New ones are
/// named after their type, as StarUML does (`C4Person1`).
const fn c4(
    (model_type, view_type): (&'static str, &'static str),
    label: &'static str,
    view_fields: &'static [(&'static str, Init)],
    size: (f64, f64),
) -> NodeSpec {
    NodeSpec {
        name_compartment: false,
        labels: C4_LABELS,
        view_fields,
        ..node(model_type, label, model_type, view_type, model_type, size)
    }
}

/// A container tool: a `C4Container` of the given kind.
const fn c4_container(
    id: &'static str,
    label: &'static str,
    kind: &'static [(&'static str, Init)],
) -> NodeSpec {
    NodeSpec {
        id,
        label,
        model_fields: kind,
        ..c4(
            ("C4Container", "C4ContainerView"),
            label,
            C4_CONTAINER,
            (160.0, 100.0),
        )
    }
}

/// A data flow diagram element (`(type, view type)`); StarUML names new
/// ones from the type without `DFD` (`Process1`).
const fn dfd(
    (id, view_type): (&'static str, &'static str),
    label: &'static str,
    base_name: &'static str,
    labels: &'static [&'static str],
    size: (f64, f64),
) -> NodeSpec {
    NodeSpec {
        name_compartment: false,
        labels,
        ..node(id, label, id, view_type, base_name, size)
    }
}

/// StarUML's floating node labels: distance and angle from the node's centre.
const UML_FLOATING: &[(&str, f64, f64)] = &[
    ("nameLabel", 20.0, 3.0 * PI / 4.0),
    ("stereotypeLabel", 35.0, 3.0 * PI / 4.0),
    ("propertyLabel", 20.0, -3.0 * PI / 4.0),
];
/// An Azure service's name, below its icon.
const AZURE_FLOATING: &[(&str, f64, f64)] = &[("nameLabel", 42.0, -PI / 2.0)];

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
        floating_labels: UML_FLOATING,
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
    // Sequence diagrams
    NodeSpec {
        slot: Slot::Participants,
        size: (100.0, 260.0),
        ..node(
            "UMLLifeline",
            "Lifeline",
            "UMLLifeline",
            "UMLSeqLifelineView",
            "Lifeline",
            (100.0, 260.0),
        )
    },
    // Communication diagrams: the same Lifeline tool draws a plain box.
    NodeSpec {
        slot: Slot::Participants,
        diagrams: COMMUNICATION,
        ..node(
            "UMLLifeline",
            "Lifeline",
            "UMLLifeline",
            "UMLCommLifelineView",
            "Lifeline",
            (100.0, 30.0),
        )
    },
    NodeSpec {
        slot: Slot::Fragments,
        name_compartment: false,
        labels: &["nameLabel", "frameTypeLabel"],
        // No metamodel default for the operator: write it.
        model_fields: &[("interactionOperator", Init::Str("alt"))],
        model_children: &[("operands", "UMLInteractionOperand")],
        compartments: &[("operandCompartment", "UMLInteractionOperandCompartmentView")],
        ..node(
            "UMLCombinedFragment",
            "Combined Fragment",
            "UMLCombinedFragment",
            "UMLCombinedFragmentView",
            "CombinedFragment",
            (260.0, 120.0),
        )
    },
    // Interaction overview diagrams: actions that call an interaction, drawn
    // as frames (StarUML's interactionUseInOverviewFn / interactionInOverviewFn).
    NodeSpec {
        slot: Slot::ActivityNodes,
        name_compartment: false,
        labels: &["nameLabel", "frameTypeLabel"],
        model_fields: &[("kind", Init::Str("callBehavior"))],
        ..node(
            "UMLInteractionUseInOverview",
            "Interaction Use",
            "UMLAction",
            "UMLInteractionUseView",
            "Action",
            (160.0, 60.0),
        )
    },
    NodeSpec {
        slot: Slot::ActivityNodes,
        name_compartment: false,
        labels: &["nameLabel", "frameTypeLabel"],
        model_fields: &[("kind", Init::Str("callBehavior"))],
        ..node(
            "UMLInteractionInOverview",
            "Interaction (Inline)",
            "UMLAction",
            "UMLInteractionInlineView",
            "Action",
            (260.0, 180.0),
        )
    },
    // Timing diagrams (StarUML's lifelineFn, timingStateFn, timeSegmentFn,
    // timeTickFn). Positions are arranged by the frame, not where dropped.
    NodeSpec {
        slot: Slot::TimingLifeline,
        name_compartment: false,
        labels: &["nameLabel"],
        diagrams: TIMING,
        ..node(
            "UMLLifeline",
            "Lifeline",
            "UMLLifeline",
            "UMLTimingLifelineView",
            "Lifeline",
            (600.0, 50.0),
        )
    },
    NodeSpec {
        slot: Slot::TimingState,
        name_compartment: false,
        labels: &["nameLabel"],
        ..node(
            "UMLTimingState",
            "State/Condition",
            "UMLConstraint",
            "UMLTimingStateView",
            "State-Condition",
            (500.0, 25.0),
        )
    },
    NodeSpec {
        slot: Slot::TimingSegment,
        name_compartment: false,
        ..node(
            "UMLTimeSegment",
            "Time Segment",
            "UMLStateInvariant",
            "UMLTimeSegmentView",
            "Time Segment",
            (60.0, 25.0),
        )
    },
    NodeSpec {
        slot: Slot::TimingTick,
        model_type: None,
        name_compartment: false,
        ..node(
            "UMLTimeTick",
            "Time Tick",
            "",
            "UMLTimeTickView",
            "",
            // As wide as a new segment, so ticks mark segment boundaries.
            (60.0, 11.0),
        )
    },
    // Information flow diagrams
    classifier(
        "UMLInformationItem",
        "Information Item",
        "UMLInformationItemView",
        "InformationItem",
    ),
    // Azure (StarUML's azure extension). A service is an icon, named by
    // its `icon` file (Microsoft's icons are not shipped: see ATTRIBUTION.md),
    // with its name floating below; a callout a numbered black dot.
    NodeSpec {
        name_compartment: false,
        labels: &["nameLabel"],
        ..node(
            "AzureGroup",
            "Azure Group",
            "AzureGroup",
            "AzureGroupView",
            "AzureGroup",
            (200.0, 120.0),
        )
    },
    NodeSpec {
        name_compartment: false,
        floating_labels: AZURE_FLOATING,
        model_fields: &[("icon", Init::Str("App-Services.svg"))],
        ..node(
            "AzureService",
            "Azure Service",
            "AzureService",
            "AzureServiceView",
            "AzureService",
            (48.0, 48.0),
        )
    },
    NodeSpec {
        name_compartment: false,
        labels: &["nameLabel"],
        // New callouts are numbered 1, white on black.
        model_fields: &[("name", Init::Str("1"))],
        view_fields: &[
            ("fillColor", Init::Str("#000000")),
            ("fontColor", Init::Str("#ffffff")),
        ],
        ..node(
            "AzureCallout",
            "Azure Callout",
            "AzureCallout",
            "AzureCalloutView",
            "",
            (24.0, 24.0),
        )
    },
    // C4 (StarUML's c4 extension): name, technology and description labels;
    // StarUML names new elements from the type ("C4Person1") and fills each
    // kind with its preference colour.
    c4(
        ("C4Person", "C4PersonView"),
        "Person",
        C4_PERSON,
        (120.0, 140.0),
    ),
    c4(
        ("C4SoftwareSystem", "C4SoftwareSystemView"),
        "Software System",
        C4_SYSTEM,
        (160.0, 100.0),
    ),
    c4_container("C4Container", "Container", ETC),
    c4_container("C4ContainerDatabase", "Container (Database)", DATABASE),
    c4_container("C4ContainerWebApp", "Container (Web App)", WEB_APP),
    c4_container(
        "C4ContainerDesktopApp",
        "Container (Desktop App)",
        DESKTOP_APP,
    ),
    c4_container("C4ContainerMobileApp", "Container (Mobile App)", MOBILE_APP),
    c4(
        ("C4Component", "C4ComponentView"),
        "Component",
        C4_COMPONENT,
        (160.0, 100.0),
    ),
    c4(
        ("C4Element", "C4ElementView"),
        "Element",
        &[],
        (160.0, 100.0),
    ),
    // Data flow diagrams (StarUML's dfd extension): each element shows its
    // `id` and name; processes and data stores keep the id in a label of its own.
    dfd(
        ("DFDExternalEntity", "DFDExternalEntityView"),
        "External Entity",
        "ExternalEntity",
        &["nameLabel"],
        (100.0, 50.0),
    ),
    dfd(
        ("DFDProcess", "DFDProcessView"),
        "Process",
        "Process",
        &["nameLabel", "idLabel"],
        (100.0, 60.0),
    ),
    dfd(
        ("DFDDataStore", "DFDDataStoreView"),
        "Data Store",
        "DataStore",
        &["nameLabel", "idLabel"],
        (120.0, 40.0),
    ),
    // Mind maps (StarUML's mindmap extension): rounded boxes with a centred
    // name; StarUML names new ones from the type ("MMNode1").
    NodeSpec {
        name_compartment: false,
        labels: &["nameLabel"],
        ..node(
            "MMNode",
            "Node",
            "MMNode",
            "MMNodeView",
            "MMNode",
            (80.0, 30.0),
        )
    },
    // Entity-relationship diagrams (StarUML's ERD extension): an entity has
    // a bold name and a table of columns.
    NodeSpec {
        name_compartment: false,
        labels: &["nameLabel"],
        compartments: &[("columnCompartment", "ERDColumnCompartmentView")],
        ..node(
            "ERDEntity",
            "Entity",
            "ERDEntity",
            "ERDEntityView",
            "Entity",
            (100.0, 40.0),
        )
    },
    // Flowcharts (StarUML's flowchart extension). Names come from the type
    // ("Process1"); connectors are named C and P; junction symbols have none.
    flowchart(
        "FCProcess",
        "Process",
        "FCProcessView",
        "Process",
        (100.0, 50.0),
    ),
    flowchart(
        "FCTerminator",
        "Terminator",
        "FCTerminatorView",
        "Terminator",
        (100.0, 40.0),
    ),
    flowchart(
        "FCDecision",
        "Decision",
        "FCDecisionView",
        "Decision",
        (100.0, 60.0),
    ),
    flowchart("FCDelay", "Delay", "FCDelayView", "Delay", (100.0, 50.0)),
    flowchart(
        "FCPredefinedProcess",
        "Predefined Process",
        "FCPredefinedProcessView",
        "PredefinedProcess",
        (140.0, 50.0),
    ),
    flowchart(
        "FCAlternateProcess",
        "Alternate Process",
        "FCAlternateProcessView",
        "AlternateProcess",
        (130.0, 50.0),
    ),
    flowchart("FCData", "Data", "FCDataView", "Data", (100.0, 50.0)),
    flowchart(
        "FCDocument",
        "Document",
        "FCDocumentView",
        "Document",
        (100.0, 60.0),
    ),
    flowchart(
        "FCMultiDocument",
        "Multi-Document",
        "FCMultiDocumentView",
        "MultiDocument",
        (120.0, 70.0),
    ),
    flowchart(
        "FCPreparation",
        "Preparation",
        "FCPreparationView",
        "Preparation",
        (110.0, 50.0),
    ),
    flowchart(
        "FCDisplay",
        "Display",
        "FCDisplayView",
        "Display",
        (100.0, 50.0),
    ),
    flowchart(
        "FCManualInput",
        "Manual Input",
        "FCManualInputView",
        "Input",
        (100.0, 50.0),
    ),
    flowchart(
        "FCManualOperation",
        "Manual Operation",
        "FCManualOperationView",
        "Operation",
        (110.0, 50.0),
    ),
    flowchart("FCCard", "Card", "FCCardView", "Card", (100.0, 50.0)),
    flowchart(
        "FCPunchedTape",
        "Punched Tape",
        "FCPunchedTapeView",
        "PunchedTape",
        (110.0, 60.0),
    ),
    flowchart(
        "FCConnector",
        "Connector",
        "FCConnectorView",
        "C",
        (30.0, 30.0),
    ),
    flowchart(
        "FCOffPageConnector",
        "Off-Page Connector",
        "FCOffPageConnectorView",
        "P",
        (30.0, 36.0),
    ),
    flowchart("FCOr", "Or", "FCOrView", "", (30.0, 30.0)),
    flowchart(
        "FCSummingJunction",
        "Summing Junction",
        "FCSummingJunctionView",
        "",
        (30.0, 30.0),
    ),
    flowchart("FCCollate", "Collate", "FCCollateView", "", (30.0, 30.0)),
    flowchart("FCSort", "Sort", "FCSortView", "", (30.0, 30.0)),
    flowchart("FCMerge", "Merge", "FCMergeView", "", (30.0, 25.0)),
    flowchart("FCExtract", "Extract", "FCExtractView", "", (30.0, 25.0)),
    flowchart(
        "FCStoredData",
        "Stored Data",
        "FCStoredDataView",
        "StoredData",
        (110.0, 50.0),
    ),
    flowchart(
        "FCDatabase",
        "Database",
        "FCDatabaseView",
        "Database",
        (90.0, 60.0),
    ),
    flowchart(
        "FCDirectAccessStorage",
        "Direct Access Storage",
        "FCDirectAccessStorageView",
        "DirectAccessStorage",
        (160.0, 50.0),
    ),
    flowchart(
        "FCInternalStorage",
        "Internal Storage",
        "FCInternalStorageView",
        "InternalStorage",
        (130.0, 50.0),
    ),
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
    /// Only a name label (flowchart flows: FCGeneralEdgeView).
    Name,
    /// A name label and a name label at each end (ERD relationships).
    NameAndEndNames,
    /// A description label above the line and a technology label below (C4).
    DescriptionAndTechnology,
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
    /// `messages` of the interaction that holds the source lifeline.
    SourceInteraction,
    /// Between lifelines on a communication diagram: the ends refer to the
    /// roles the lifelines represent, and the tail's role owns it.
    TailRole,
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
    /// Values written into the new model element (e.g. a message's sort).
    pub model_fields: &'static [(&'static str, Init)],
    /// Values written into the `end1` and `end2` of an undirected relationship
    /// (an ERD relationship's cardinalities).
    pub end_fields: [&'static [(&'static str, Init)]; 2],
    /// `lineStyle` of a new view (`mdj::values::line_style`).
    pub line_style: i64,
    /// Diagram types this row is for (see [`NodeSpec::diagrams`]). Empty: any.
    pub diagrams: &'static [&'static str],
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
        model_fields: &[],
        end_fields: [&[], &[]],
        line_style: line_style::OBLIQUE,
        diagrams: &[],
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
        model_fields: &[],
        end_fields: [&[], &[]],
        line_style: line_style::OBLIQUE,
        diagrams: &[],
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

/// A sequence-diagram message of the given `messageSort`. The view connects
/// the two lifelines' dashed line parts.
const fn message(
    id: &'static str,
    label: &'static str,
    sort: &'static [(&'static str, Init)],
) -> EdgeSpec {
    EdgeSpec {
        owner: EdgeOwner::SourceInteraction,
        model_fields: sort,
        ..directed(id, label, "UMLMessage", "UMLSeqMessageView")
    }
}

/// An ERD relationship end's `cardinality` for "many" (the metamodel default is `1`).
const MANY: &[(&str, Init)] = &[("cardinality", Init::Str("0..*"))];

/// An ERD relationship: entities at both ends, the given end cardinalities.
/// StarUML's preference draws these with rounded right angles.
const fn erd_relationship(
    id: &'static str,
    label: &'static str,
    end_fields: [&'static [(&'static str, Init)]; 2],
) -> EdgeSpec {
    EdgeSpec {
        labels: EdgeLabels::NameAndEndNames,
        end_fields,
        line_style: line_style::ROUND_RECT,
        ..undirected(
            id,
            label,
            "ERDRelationship",
            "ERDRelationshipView",
            "ERDRelationshipEnd",
            "none",
            "unspecified",
        )
    }
}

const COMMUNICATION: &[&str] = &["UMLCommunicationDiagram"];
const TIMING: &[&str] = &["UMLTimingDiagram"];

/// A connector between lifelines on a communication diagram.
const fn role_connector(id: &'static str, label: &'static str) -> EdgeSpec {
    EdgeSpec {
        owner: EdgeOwner::TailRole,
        diagrams: COMMUNICATION,
        ..undirected(
            id,
            label,
            "UMLConnector",
            "UMLConnectorView",
            "UMLConnectorEnd",
            "none",
            "unspecified",
        )
    }
}

/// A communication-diagram message: an arrow riding on a connector, which is
/// created too when the message is drawn between two lifelines.
const fn comm_message(id: &'static str, label: &'static str) -> EdgeSpec {
    EdgeSpec {
        owner: EdgeOwner::SourceInteraction,
        labels: EdgeLabels::None,
        diagrams: COMMUNICATION,
        ..directed(id, label, "UMLMessage", "UMLCommMessageView")
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
    // synchCall is the metamodel default, so a plain message writes no sort.
    message("UMLMessage", "Message", &[]),
    message("UMLSelfMessage", "Self Message", &[]),
    message(
        "UMLAsyncMessage",
        "Async Message",
        &[("messageSort", Init::Str("asynchCall"))],
    ),
    message(
        "UMLReplyMessage",
        "Reply Message",
        &[("messageSort", Init::Str("reply"))],
    ),
    message(
        "UMLCreateMessage",
        "Create Message",
        &[("messageSort", Init::Str("createMessage"))],
    ),
    message(
        "UMLDeleteMessage",
        "Delete Message",
        &[("messageSort", Init::Str("deleteMessage"))],
    ),
    message(
        "UMLAsyncSignalMessage",
        "Async Signal Message",
        &[("messageSort", Init::Str("asynchSignal"))],
    ),
    directed(
        "UMLInformationFlow",
        "Information Flow",
        "UMLInformationFlow",
        "UMLInformationFlowView",
    ),
    // Timing messages join two time segments; source and target are the
    // lifelines the segments cover.
    EdgeSpec {
        owner: EdgeOwner::SourceInteraction,
        diagrams: TIMING,
        ..directed(
            "UMLMessage",
            "Message",
            "UMLMessage",
            "UMLTimingMessageView",
        )
    },
    EdgeSpec {
        labels: EdgeLabels::Name,
        ..directed("FCFlow", "Flow", "FCFlow", "FCFlowView")
    },
    // An Azure connector: a named arrow, solid or dashed; rounded right
    // angles by StarUML's preference.
    EdgeSpec {
        labels: EdgeLabels::Name,
        line_style: line_style::ROUND_RECT,
        ..directed(
            "AzureConnector",
            "Azure Connector",
            "AzureConnector",
            "AzureConnectorView",
        )
    },
    // A C4 relationship: a dotted arrow with its description and technology.
    EdgeSpec {
        labels: EdgeLabels::DescriptionAndTechnology,
        ..directed(
            "C4Relationship",
            "Relationship",
            "C4Relationship",
            "C4RelationshipView",
        )
    },
    // A data flow: a named arrow, rounded right angles by StarUML's preference.
    EdgeSpec {
        labels: EdgeLabels::Name,
        line_style: line_style::ROUND_RECT,
        ..directed("DFDDataFlow", "Data Flow", "DFDDataFlow", "DFDDataFlowView")
    },
    // A plain line between mind map nodes (no arrowheads, no labels).
    EdgeSpec {
        labels: EdgeLabels::None,
        ..directed("MMEdge", "Edge", "MMEdge", "MMEdgeView")
    },
    erd_relationship("ERDRelationship", "One to One Relationship", [&[], &[]]),
    erd_relationship(
        "ERDRelationshipOneToMany",
        "One to Many Relationship",
        [&[], MANY],
    ),
    erd_relationship(
        "ERDRelationshipManyToMany",
        "Many to Many Relationship",
        [MANY, MANY],
    ),
    role_connector("UMLConnector", "Connector"),
    role_connector("UMLSelfConnector", "Self Connector"),
    comm_message("UMLForwardMessage", "Forward Message"),
    comm_message("UMLReverseMessage", "Reverse Message"),
    EdgeSpec {
        id: "UMLContainment",
        label: "Containment",
        model_type: None,
        view_type: "UMLContainmentView",
        model: EdgeModel::ViewOnly,
        labels: EdgeLabels::None,
        owner: EdgeOwner::Source,
        model_fields: &[],
        end_fields: [&[], &[]],
        line_style: line_style::OBLIQUE,
        diagrams: &[],
    },
    EdgeSpec {
        id: "NoteLink",
        label: "Note Link",
        model_type: None,
        view_type: "UMLNoteLinkView",
        model: EdgeModel::ViewOnly,
        labels: EdgeLabels::None,
        owner: EdgeOwner::Source,
        model_fields: &[],
        end_fields: [&[], &[]],
        line_style: line_style::OBLIQUE,
        diagrams: &[],
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
        diagrams: &[
            "UMLClassDiagram",
            "UMLCompositeStructureDiagram",
            "UMLInformationFlowDiagram",
        ],
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
        diagrams: &["UMLUseCaseDiagram", "UMLInformationFlowDiagram"],
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
        label: "Sequence",
        diagrams: &["UMLSequenceDiagram"],
        tools: &[
            "UMLLifeline",
            "UMLMessage",
            "UMLSelfMessage",
            "UMLAsyncMessage",
            "UMLReplyMessage",
            "UMLCreateMessage",
            "UMLDeleteMessage",
            "UMLAsyncSignalMessage",
        ],
    },
    ToolGroup {
        label: "Sequence (Advanced)",
        diagrams: &["UMLSequenceDiagram"],
        tools: &["UMLCombinedFragment"],
    },
    ToolGroup {
        label: "Interaction Overview",
        diagrams: &["UMLInteractionOverviewDiagram"],
        tools: &[
            "UMLInteractionUseInOverview",
            "UMLInteractionInOverview",
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
        label: "Flowchart",
        diagrams: &["FCFlowchartDiagram"],
        tools: &[
            "FCFlow",
            "FCProcess",
            "FCTerminator",
            "FCDecision",
            "FCDelay",
            "FCPredefinedProcess",
            "FCAlternateProcess",
            "FCData",
            "FCDocument",
            "FCMultiDocument",
            "FCPreparation",
            "FCDisplay",
            "FCManualInput",
            "FCManualOperation",
            "FCCard",
            "FCPunchedTape",
            "FCConnector",
            "FCOffPageConnector",
            "FCOr",
            "FCSummingJunction",
            "FCCollate",
            "FCSort",
            "FCMerge",
            "FCExtract",
            "FCStoredData",
            "FCDatabase",
            "FCDirectAccessStorage",
            "FCInternalStorage",
        ],
    },
    ToolGroup {
        label: "Azure",
        diagrams: &["AzureDiagram"],
        tools: &[
            "AzureGroup",
            "AzureService",
            "AzureCallout",
            "AzureConnector",
        ],
    },
    ToolGroup {
        label: "C4 Model",
        diagrams: &["C4Diagram"],
        tools: &[
            "C4Person",
            "C4SoftwareSystem",
            "C4Container",
            "C4ContainerDatabase",
            "C4ContainerWebApp",
            "C4ContainerDesktopApp",
            "C4ContainerMobileApp",
            "C4Component",
            "C4Element",
            "C4Relationship",
        ],
    },
    ToolGroup {
        label: "Data Flow",
        diagrams: &["DFDDiagram"],
        tools: &[
            "DFDExternalEntity",
            "DFDProcess",
            "DFDDataStore",
            "DFDDataFlow",
        ],
    },
    ToolGroup {
        label: "Mindmap",
        diagrams: &["MMMindmapDiagram"],
        tools: &["MMNode", "MMEdge"],
    },
    ToolGroup {
        label: "Entity-Relationship",
        diagrams: &["ERDDiagram"],
        tools: &[
            "ERDEntity",
            "ERDRelationship",
            "ERDRelationshipOneToMany",
            "ERDRelationshipManyToMany",
        ],
    },
    ToolGroup {
        label: "Timing",
        diagrams: TIMING,
        tools: &[
            "UMLLifeline",
            "UMLTimingState",
            "UMLTimeSegment",
            "UMLTimeTick",
            "UMLMessage",
        ],
    },
    ToolGroup {
        label: "Information Flow",
        diagrams: &["UMLInformationFlowDiagram"],
        tools: &["UMLInformationItem", "UMLInformationFlow"],
    },
    ToolGroup {
        label: "Communication",
        diagrams: COMMUNICATION,
        tools: &[
            "UMLLifeline",
            "UMLConnector",
            "UMLSelfConnector",
            "UMLForwardMessage",
            "UMLReverseMessage",
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
    ("UMLSequenceDiagram", "Sequence Diagram", "SequenceDiagram"),
    (
        "UMLCommunicationDiagram",
        "Communication Diagram",
        "CommunicationDiagram",
    ),
    ("UMLTimingDiagram", "Timing Diagram", "TimingDiagram"),
    (
        "UMLInteractionOverviewDiagram",
        "Interaction Overview Diagram",
        "InteractionOverviewDiagram",
    ),
    (
        "UMLInformationFlowDiagram",
        "Information Flow Diagram",
        "InformationFlowDiagram",
    ),
    (
        "FCFlowchartDiagram",
        "Flowchart Diagram",
        "FlowchartDiagram",
    ),
    // StarUML names these from the type: "ERDDiagram1".
    ("ERDDiagram", "ER Diagram", "ERDDiagram"),
    ("MMMindmapDiagram", "Mindmap Diagram", "MMMindmapDiagram"),
    ("DFDDiagram", "Data Flow Diagram", "DFDDiagram"),
    ("C4Diagram", "C4 Diagram", "C4Diagram"),
    ("AzureDiagram", "Azure Diagram", "AzureDiagram"),
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
    by_id(NODES, name, None).or_else(|| by_label(NODES, name, None))
}

/// Look an edge tool up by id (`UMLComposition`) or label (`Interface Realization`).
pub fn edge_spec(name: &str) -> Option<&'static EdgeSpec> {
    by_id(EDGES, name, None).or_else(|| by_label(EDGES, name, None))
}

pub fn tool(id: &str) -> Option<Tool> {
    lookup(id, None)
}

/// What a catalog row needs for look-ups.
trait Row {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn diagrams(&self) -> &'static [&'static str];
}

impl Row for NodeSpec {
    fn id(&self) -> &'static str {
        self.id
    }
    fn label(&self) -> &'static str {
        self.label
    }
    fn diagrams(&self) -> &'static [&'static str] {
        self.diagrams
    }
}

impl Row for EdgeSpec {
    fn id(&self) -> &'static str {
        self.id
    }
    fn label(&self) -> &'static str {
        self.label
    }
    fn diagrams(&self) -> &'static [&'static str] {
        self.diagrams
    }
}

/// Whether `diagram_type`'s toolbox offers the tool `id`.
fn offered(id: &str, diagram_type: &str) -> bool {
    tool_groups(diagram_type).any(|g| g.tools.contains(&id))
}

/// Of several rows answering to one name, the one meant for `diagram_type`:
/// a row limited to that type, else one its toolbox offers, else one for any
/// diagram, else the first.
fn pick<T: Row>(rows: Vec<&'static T>, diagram_type: Option<&str>) -> Option<&'static T> {
    let for_type = |r: &T| diagram_type.is_some_and(|t| r.diagrams().contains(&t));
    let on_toolbox =
        |r: &T| r.diagrams().is_empty() && diagram_type.is_some_and(|t| offered(r.id(), t));
    let any = |r: &T| r.diagrams().is_empty();
    let first_where = |f: &dyn Fn(&T) -> bool| rows.iter().copied().find(|r| f(r));
    first_where(&for_type)
        .or_else(|| first_where(&on_toolbox))
        .or_else(|| first_where(&any))
        .or_else(|| rows.first().copied())
}

/// The row whose id is `name` (ignoring case, spaces, dashes and a `UML` prefix).
fn by_id<T: Row>(rows: &'static [T], name: &str, diagram_type: Option<&str>) -> Option<&'static T> {
    let n = normalize(name);
    pick(
        rows.iter().filter(|r| normalize(r.id()) == n).collect(),
        diagram_type,
    )
}

/// The row labelled `name`. Labels repeat across families ("Decision" in
/// activities and flowcharts), so the open diagram's toolbox decides.
fn by_label<T: Row>(
    rows: &'static [T],
    name: &str,
    diagram_type: Option<&str>,
) -> Option<&'static T> {
    let n = normalize(name);
    pick(
        rows.iter().filter(|r| normalize(r.label()) == n).collect(),
        diagram_type,
    )
}

/// Ids win over labels across both kinds: `UMLConnector` is the line, even
/// though a flowchart box is labelled "Connector".
fn lookup(name: &str, diagram_type: Option<&str>) -> Option<Tool> {
    by_id(NODES, name, diagram_type)
        .map(Tool::Node)
        .or_else(|| by_id(EDGES, name, diagram_type).map(Tool::Edge))
        .or_else(|| by_label(NODES, name, diagram_type).map(Tool::Node))
        .or_else(|| by_label(EDGES, name, diagram_type).map(Tool::Edge))
}

/// [`node_spec`] for a tool used on a `diagram_type` diagram.
pub fn node_spec_on(name: &str, diagram_type: &str) -> Option<&'static NodeSpec> {
    by_id(NODES, name, Some(diagram_type)).or_else(|| by_label(NODES, name, Some(diagram_type)))
}

/// [`edge_spec`] for a tool used on a `diagram_type` diagram.
pub fn edge_spec_on(name: &str, diagram_type: &str) -> Option<&'static EdgeSpec> {
    by_id(EDGES, name, Some(diagram_type)).or_else(|| by_label(EDGES, name, Some(diagram_type)))
}

/// [`tool`] for a toolbox on a `diagram_type` diagram.
pub fn tool_on(id: &str, diagram_type: &str) -> Option<Tool> {
    lookup(id, Some(diagram_type))
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

/// The row that draws an existing element of `model_type` on a
/// `diagram_type` diagram: the tool named after the type if there is one
/// (`C4Container`, not `C4ContainerDatabase`), else any row creating that type.
pub fn node_spec_for_model(model_type: &str, diagram_type: &str) -> Option<&'static NodeSpec> {
    node_spec_on(model_type, diagram_type)
        .filter(|s| s.model_type == Some(model_type))
        .or_else(|| {
            let rows = || NODES.iter().filter(|s| s.model_type == Some(model_type));
            rows()
                .find(|s| s.diagrams.contains(&diagram_type))
                .or_else(|| rows().find(|s| s.diagrams.is_empty()))
        })
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
    fn ids_are_unique_per_diagram() {
        let mut ids: Vec<(&str, &[&str])> = NODES
            .iter()
            .map(|s| (s.id, s.diagrams))
            .chain(EDGES.iter().map(|s| (s.id, s.diagrams)))
            .collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn the_same_tool_id_can_differ_by_diagram() {
        let seq = node_spec_on("UMLLifeline", "UMLSequenceDiagram").unwrap();
        let comm = node_spec_on("Lifeline", "UMLCommunicationDiagram").unwrap();
        assert_eq!(seq.view_type, "UMLSeqLifelineView");
        assert_eq!(comm.view_type, "UMLCommLifelineView");
        assert_eq!(
            node_spec_on("UMLLifeline", "UMLClassDiagram").map(|s| s.view_type),
            Some("UMLSeqLifelineView"),
            "rows for any diagram win elsewhere"
        );
        let structure = edge_spec_on("UMLConnector", "UMLCompositeStructureDiagram").unwrap();
        assert_eq!(structure.owner, EdgeOwner::Source);
        let roles = edge_spec_on("UMLConnector", "UMLCommunicationDiagram").unwrap();
        assert_eq!(roles.owner, EdgeOwner::TailRole);
        for g in tool_groups("UMLCommunicationDiagram") {
            for t in g.tools {
                assert!(tool_on(t, "UMLCommunicationDiagram").is_some(), "{t}");
            }
        }
    }

    #[test]
    fn shared_labels_resolve_by_diagram() {
        let flow = "FCFlowchartDiagram";
        assert_eq!(
            node_spec_on("Decision", flow).map(|s| s.id),
            Some("FCDecision")
        );
        assert_eq!(
            node_spec_on("Decision", "UMLActivityDiagram").map(|s| s.id),
            Some("UMLDecisionNode")
        );
        assert_eq!(node_spec_on("Merge", flow).map(|s| s.id), Some("FCMerge"));
        // An id beats a label: the composite-structure Connector is a line.
        for t in ["UMLCompositeStructureDiagram", "UMLCommunicationDiagram"] {
            assert!(
                matches!(tool_on("UMLConnector", t), Some(Tool::Edge(_))),
                "{t}"
            );
        }
        assert!(matches!(tool_on("FCConnector", flow), Some(Tool::Node(_))));
        for g in tool_groups(flow) {
            for t in g.tools {
                let found = tool_on(t, flow).map(|tool| match tool {
                    Tool::Node(n) => n.id,
                    Tool::Edge(e) => e.id,
                });
                assert_eq!(found, Some(*t));
            }
        }
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
