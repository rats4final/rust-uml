//! Box geometry for node views: which shape a view draws as, what text each
//! compartment holds, and the smallest size that fits it.

use std::collections::HashMap;

use mdj::values::{DEFAULT_FONT, Font, absent, align};
use mdj::{Document, Element};

use crate::geom::{Color, Point, Rect};
use crate::text::Fonts;

const PAD_X: f64 = 6.0;
const PAD_Y: f64 = 4.0;
const EMPTY_COMPARTMENT: f64 = 10.0;
const MIN_WIDTH: f64 = 40.0;
pub(crate) const PACKAGE_TAB_H: f64 = 14.0;
/// Gap between the outer and inner side borders of an active class.
pub(crate) const ACTIVE_INSET: f64 = 4.0;
/// Depth of a node's 3-D box.
pub(crate) const NODE_DEPTH: f64 = 10.0;
/// Size of the folded corner of a note.
pub(crate) const NOTE_FOLD: f64 = 10.0;
/// Diameter of an interface's lollipop circle.
pub(crate) const LOLLIPOP: f64 = 20.0;
/// Room kept free at the top-right for a component/artifact icon.
pub(crate) const ICON_SPACE: f64 = 22.0;
/// A port is a small square (StarUML: 14 × 14).
const PORT_SIZE: f64 = 14.0;
/// Corner radius of states and actions (StarUML: 10).
pub(crate) const ROUND: f64 = 10.0;
/// Corner radius of mind map nodes (StarUML: 14).
pub(crate) const MIND_ROUND: f64 = 14.0;
/// Padding around a mind map node's name (StarUML: 8 at the sides, 6 above and below).
const MIND_PAD_X: f64 = 8.0;
const MIND_PAD_Y: f64 = 6.0;
/// Width of a swimlane's name band.
pub(crate) const LANE_HEADER: f64 = 24.0;
/// Height of an actor's stick figure (StarUML: 55).
pub(crate) const ACTOR_FIGURE_H: f64 = 55.0;
/// Height of a sequence lifeline's head box (StarUML: 40).
pub(crate) const LIFELINE_HEAD: f64 = 40.0;

/// Horizontal placement of a line inside its compartment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

impl HAlign {
    /// From a stored `horizontalAlignment` code (see `mdj::values::align`).
    fn from_code(code: i64) -> Self {
        match code {
            align::LEFT => HAlign::Left,
            align::RIGHT => HAlign::Right,
            _ => HAlign::Center,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub align: HAlign,
}

impl TextLine {
    fn centered(text: String) -> Self {
        Self {
            text,
            bold: false,
            italic: false,
            underline: false,
            align: HAlign::Center,
        }
    }

    fn aligned(text: String, align: HAlign) -> Self {
        Self {
            align,
            ..Self::centered(text)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Compartment {
    pub lines: Vec<TextLine>,
    pub rect: Rect,
}

/// The outline a node view draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Box,
    /// A folder: the tab sits at the top-left, the body below it.
    Package {
        tab: Rect,
    },
    /// UML node: a box with top and right faces.
    Node3D,
    /// Collaborations: a dashed ellipse.
    Ellipse,
    /// A note: a box with a folded top-right corner.
    Note,
    /// Free text: no outline.
    Text,
    /// A port: a small square on its owner's border; its name is drawn outside.
    Port,
    /// An interface shown as a "lollipop": a circle, name below.
    Lollipop {
        circle: Rect,
    },
    /// States and actions: a rectangle with rounded corners.
    RoundRect,
    /// A stick figure in `figure`, name below.
    Actor {
        figure: Rect,
    },
    /// A small UML symbol (initial dot, final bull's-eye, bar, diamond, …).
    Symbol(Symbol),
    /// An activity partition: a lane with a name band.
    Swimlane {
        vertical: bool,
        header: Rect,
    },
    /// A sequence lifeline: a head box with a dashed line down from its centre.
    Lifeline {
        head: Rect,
    },
    /// A frame (diagram frame, combined fragment): an outline with a
    /// pentagon name tag at the top-left; transparent inside.
    Frame {
        tag: Rect,
    },
    /// A flowchart symbol, filling the box.
    Flowchart(Flow),
    /// A mind map node: a box with well-rounded corners.
    MindNode,
}

/// The flowchart symbols (StarUML's flowchart extension, ISO 5807 shapes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Process,
    Terminator,
    Decision,
    Delay,
    PredefinedProcess,
    AlternateProcess,
    Data,
    Document,
    MultiDocument,
    Preparation,
    Display,
    ManualInput,
    ManualOperation,
    Card,
    PunchedTape,
    Connector,
    OffPageConnector,
    Or,
    SummingJunction,
    Collate,
    Sort,
    Merge,
    Extract,
    StoredData,
    Database,
    DirectAccessStorage,
    InternalStorage,
}

impl Flow {
    /// The symbol a view type (`FCProcessView`, …) draws.
    pub fn of_view(view_type: &str) -> Option<Flow> {
        use Flow::*;
        Some(match view_type.strip_prefix("FC")?.strip_suffix("View")? {
            "Process" => Process,
            "Terminator" => Terminator,
            "Decision" => Decision,
            "Delay" => Delay,
            "PredefinedProcess" => PredefinedProcess,
            "AlternateProcess" => AlternateProcess,
            "Data" => Data,
            "Document" => Document,
            "MultiDocument" => MultiDocument,
            "Preparation" => Preparation,
            "Display" => Display,
            "ManualInput" => ManualInput,
            "ManualOperation" => ManualOperation,
            "Card" => Card,
            "PunchedTape" => PunchedTape,
            "Connector" => Connector,
            "OffPageConnector" => OffPageConnector,
            "Or" => Or,
            "SummingJunction" => SummingJunction,
            "Collate" => Collate,
            "Sort" => Sort,
            "Merge" => Merge,
            "Extract" => Extract,
            "StoredData" => StoredData,
            "Database" => Database,
            "DirectAccessStorage" => DirectAccessStorage,
            "InternalStorage" => InternalStorage,
            _ => return None,
        })
    }

    /// Smallest box for a name `w` × `h`: StarUML pads the name by 10 px
    /// (a decision or merge needs twice the name; junctions are 30). Where a
    /// symbol keeps its name off a curve or edge, the box grows so the name
    /// still fits there.
    fn min_size(self, w: f64, h: f64) -> (f64, f64) {
        use Flow::*;
        let (pw, ph) = (w + 20.0, h + 20.0);
        match self {
            Decision => ((2.0 * w).max(50.0), (2.0 * h).max(30.0)),
            Merge | Extract => ((2.0 * w).max(30.0), (2.0 * h).max(25.0)),
            Connector => (w.max(30.0), h.max(30.0)),
            Or | SummingJunction | Collate | Sort => (30.0, 30.0),
            Data | Preparation | Display => (pw * 1.5, ph),
            StoredData | Delay => (pw / 0.75, ph),
            DirectAccessStorage => (pw / 0.625, ph),
            Database => (pw, ph * 1.6),
            PredefinedProcess => (pw + 5.0, ph),
            InternalStorage | MultiDocument => (pw + 10.0, ph + 10.0),
            _ => (pw, ph),
        }
    }

    /// Where the name goes in box `r`: the band's left edge and width, and
    /// the top of the text (as StarUML arranges each symbol's name).
    fn label_band(self, r: Rect, line_h: f64) -> (f64, f64, f64) {
        use Flow::*;
        let (left, width, top) = (r.x + 10.0, r.w - 20.0, r.y + 10.0);
        let middle = r.y + (r.h - line_h) / 2.0;
        let g6 = (r.w / 6.0).floor();
        let g8 = (r.w / 8.0).floor();
        match self {
            Decision => (r.x + r.w / 4.0, r.w / 2.0, middle),
            Sort | Merge | Extract | Connector | OffPageConnector | Or | SummingJunction => {
                (r.x, r.w, middle)
            }
            Data | Preparation | Display => (left + g6, width - 2.0 * g6, top),
            StoredData => (left + g8, width - 2.0 * g8, top),
            DirectAccessStorage => (left + g8, width - 3.0 * g8, top),
            Delay => (left, r.w * 0.75 - 10.0, top),
            PredefinedProcess => (left + 5.0, width - 5.0, top),
            Database => (left, width, top + 2.0 * (r.h / 8.0).floor()),
            InternalStorage | MultiDocument => (left + 10.0, width - 10.0, top + 10.0),
            _ => (left, width, top),
        }
    }
}

/// The small symbols of state machines and activities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    /// Filled circle: initial state/node.
    Dot,
    /// Ring with a filled centre: final state, activity final.
    BullsEye,
    /// Ring with an X: flow final, exit point.
    CircleX,
    /// Filled bar: fork, join.
    Bar,
    /// Hollow diamond: choice, merge, decision.
    Diamond,
    /// Small filled circle: junction.
    SmallDot,
    /// Ring with "H" or "H*": shallow / deep history.
    History { deep: bool },
    /// Hollow ring: entry point.
    Ring,
    /// An X: terminate.
    Cross,
}

/// A small symbol in the top-right corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Component,
    Artifact,
}

/// Text drawn outside the shape, centred on `center` (port names).
#[derive(Debug, Clone, PartialEq)]
pub struct OutsideLabel {
    pub line: TextLine,
    pub center: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    /// Final rectangle: the stored size, grown if the content needs more.
    pub rect: Rect,
    pub min_w: f64,
    pub min_h: f64,
    pub shape: Shape,
    pub compartments: Vec<Compartment>,
    /// Draw lines between compartments.
    pub separators: bool,
    pub font_size: f64,
    pub fill: Color,
    pub line: Color,
    pub text: Color,
    /// An active class (`isActive`): drawn with double side borders.
    pub active: bool,
    /// Dashed outline (shared parts, collaborations).
    pub dashed: bool,
    pub icon: Option<Icon>,
    pub outside: Vec<OutsideLabel>,
    /// Extra lines `(from, to, dashed)`: region dividers in composite states.
    pub dividers: Vec<(Point, Point, bool)>,
}

/// Default «keyword» shown for a model type when it has no explicit stereotype.
fn keyword(model_type: &str) -> Option<&'static str> {
    Some(match model_type {
        "UMLInterface" => "«interface»",
        "UMLEnumeration" => "«enumeration»",
        "UMLDataType" => "«dataType»",
        "UMLPrimitiveType" => "«primitiveType»",
        "UMLSignal" => "«signal»",
        "UMLInformationItem" => "«information»",
        "UMLMetaClass" => "«metaclass»",
        "UMLStereotype" => "«stereotype»",
        "UMLArtifact" => "«artifact»",
        "UMLComponent" => "«component»",
        "UMLModel" => "«model»",
        "UMLSubsystem" => "«subsystem»",
        _ => return None,
    })
}

/// `stereotypeDisplay` values that show a symbol instead of (or with) text.
fn shows_icon(display: &str) -> bool {
    matches!(
        display,
        "icon" | "icon-label" | "decoration" | "decoration-label"
    )
}

/// `stereotypeDisplay` values that show the «keyword» text.
fn shows_keyword(display: &str) -> bool {
    matches!(display, "label" | "icon-label" | "decoration-label")
}

/// The symbol a control node or pseudostate draws as, from its model.
fn symbol_of(model: Option<&Element>) -> Option<Symbol> {
    let m = model?;
    Some(match (m.type_name.as_str(), m.str("kind").unwrap_or("")) {
        ("UMLInitialNode", _) | ("UMLPseudostate", "initial") => Symbol::Dot,
        ("UMLActivityFinalNode", _) | ("UMLFinalState", _) => Symbol::BullsEye,
        ("UMLFlowFinalNode", _) | ("UMLPseudostate", "exitPoint") => Symbol::CircleX,
        ("UMLForkNode" | "UMLJoinNode", _) | ("UMLPseudostate", "fork" | "join") => Symbol::Bar,
        ("UMLMergeNode" | "UMLDecisionNode", _) | ("UMLPseudostate", "choice") => Symbol::Diamond,
        ("UMLPseudostate", "junction") => Symbol::SmallDot,
        ("UMLPseudostate", "shallowHistory") => Symbol::History { deep: false },
        ("UMLPseudostate", "deepHistory") => Symbol::History { deep: true },
        ("UMLPseudostate", "entryPoint") => Symbol::Ring,
        ("UMLPseudostate", "terminate") => Symbol::Cross,
        _ => return None,
    })
}

fn shape_of(view: &Element, model: Option<&Element>) -> Shape {
    if let Some(flow) = Flow::of_view(&view.type_name) {
        return Shape::Flowchart(flow);
    }
    let display = view.str("stereotypeDisplay").unwrap_or("label");
    if matches!(
        view.type_name.as_str(),
        "UMLControlNodeView" | "UMLPseudostateView" | "UMLFinalStateView"
    ) && let Some(symbol) = symbol_of(model)
    {
        return Shape::Symbol(symbol);
    }
    match view.type_name.as_str() {
        "UMLModelView" | "UMLSubsystemView" | "UMLProfileView" => Shape::Package {
            tab: Rect::default(),
        },
        t if t.ends_with("PackageView") => Shape::Package {
            tab: Rect::default(),
        },
        "UMLNodeView" | "UMLNodeInstanceView" => Shape::Node3D,
        "UMLCollaborationView" | "UMLCollaborationUseView" | "UMLUseCaseView" => Shape::Ellipse,
        "UMLStateView" | "UMLActionView" => Shape::RoundRect,
        "UMLActorView" => Shape::Actor {
            figure: Rect::default(),
        },
        "UMLSeqLifelineView" => Shape::Lifeline {
            head: Rect::default(),
        },
        "UMLFrameView"
        | "UMLTimingFrameView"
        | "UMLCombinedFragmentView"
        | "UMLInteractionUseView"
        | "UMLInteractionInlineView" => Shape::Frame {
            tag: Rect::default(),
        },
        "UMLSwimlaneView" => Shape::Swimlane {
            vertical: view.bool("isVertical", true),
            header: Rect::default(),
        },
        "UMLNoteView" => Shape::Note,
        "UMLTextView" => Shape::Text,
        "UMLPortView" => Shape::Port,
        "UMLInterfaceView" if matches!(display, "icon" | "icon-label") => Shape::Lollipop {
            circle: Rect::default(),
        },
        _ => Shape::Box,
    }
}

fn icon_of(view: &Element) -> Option<Icon> {
    let display = view.str("stereotypeDisplay").unwrap_or("label");
    if !shows_icon(display) {
        return None;
    }
    match view.type_name.as_str() {
        "UMLComponentView" | "UMLComponentInstanceView" => Some(Icon::Component),
        "UMLArtifactView" | "UMLArtifactInstanceView" => Some(Icon::Artifact),
        _ => None,
    }
}

/// The keyword in a frame's tag for the diagram or element it frames, as
/// StarUML chooses it; empty when there is none.
fn frame_kind(type_name: &str) -> &'static str {
    match type_name {
        "UMLClass" | "UMLClassDiagram" => "class",
        "UMLComponent" | "UMLComponentDiagram" => "cmp",
        "UMLDeploymentDiagram" => "dep",
        "UMLInteraction"
        | "UMLSequenceDiagram"
        | "UMLCommunicationDiagram"
        | "UMLTimingDiagram"
        | "UMLInteractionOverviewDiagram" => "sd",
        "UMLStateMachine" | "UMLStatechartDiagram" => "stm",
        "UMLActivity" | "UMLActivityDiagram" => "act",
        "UMLUseCase" | "UMLUseCaseDiagram" => "uc",
        "UMLPackage" | "UMLPackageDiagram" => "pkg",
        "SysMLRequirementDiagram" => "req",
        "SysMLBlockDefinitionDiagram" => "bdd",
        "SysMLParametricDiagram" => "par",
        "SysMLInternalBlockDiagram" => "ibd",
        _ => "",
    }
}

/// The heading text of a node, and whether it is underlined (instances).
pub(crate) fn heading(doc: &Document, view: &Element, m: &Element) -> (String, bool) {
    match view.type_name.as_str() {
        "UMLObjectView"
        | "UMLArtifactInstanceView"
        | "UMLComponentInstanceView"
        | "UMLNodeInstanceView" => (uml::instance_text(doc, m), true),
        "UMLPartView" => (uml::part_text(doc, m), false),
        // `name : Type`, the type coming from the role the lifeline represents.
        "UMLSeqLifelineView" | "UMLCommLifelineView" | "UMLTimingLifelineView" => {
            let ty = m
                .ref_id("represent")
                .and_then(|r| doc.get(r))
                .map(|role| uml::var_text(doc, role, "type"))
                .unwrap_or("");
            (uml::typed_name(m.name(), ty), false)
        }
        "UMLCollaborationUseView" => (
            uml::typed_name(m.name(), uml::var_text(doc, m, "type")),
            false,
        ),
        _ => (m.name().to_owned(), false),
    }
}

/// Alignment of each row of a list compartment, keyed by the row's model id.
/// A row view without the field is centred, as StarUML loads it (the
/// metamodel default). A feature with no row view at all is left-aligned,
/// as StarUML creates it.
fn row_alignments<'a>(
    doc: &'a Document,
    view: &Element,
    compartment_key: &str,
) -> HashMap<&'a str, HAlign> {
    let Some(comp) = view.ref_id(compartment_key).and_then(|c| doc.get(c)) else {
        return HashMap::new();
    };
    comp.children("subViews")
        .iter()
        .filter_map(|id| doc.get(id))
        .filter_map(|row| {
            let code = row.num("horizontalAlignment", absent::H_ALIGN as f64) as i64;
            Some((row.ref_id("model")?, HAlign::from_code(code)))
        })
        .collect()
}

/// Greedy word wrap of `text` to `width`, keeping explicit line breaks.
fn wrap(text: &str, width: f64, fonts: &Fonts, size: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && fonts.width(&candidate, size, false, false) > width {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = candidate;
            }
        }
        out.push(line);
    }
    out
}

/// Layout of a top-level node view, or `None` if `view` is not a node
/// (no position fields).
pub fn node_layout(doc: &Document, view: &Element, fonts: &Fonts) -> Option<NodeLayout> {
    let has_geometry = view.get("left").is_some() || view.get("width").is_some();
    // Edges, and views riding on an edge (communication messages), aren't boxes.
    if !has_geometry || view.get("head").is_some() || view.get("hostEdge").is_some() {
        return None;
    }
    let stored = Rect::new(
        view.num("left", 0.0),
        view.num("top", 0.0),
        view.num("width", 0.0),
        view.num("height", 0.0),
    );
    // Laid out by their timing frame (`crate::timing`), not sized by content:
    // keep the stored box, so nothing grows it.
    if crate::timing::is_arranged(&view.type_name) {
        return Some(NodeLayout {
            rect: stored,
            min_w: 0.0,
            min_h: 0.0,
            shape: Shape::Box,
            compartments: Vec::new(),
            separators: false,
            font_size: 13.0,
            fill: Color::WHITE,
            line: Color::BLACK,
            text: Color::BLACK,
            active: false,
            dashed: false,
            icon: None,
            outside: Vec::new(),
            dividers: Vec::new(),
        });
    }
    let font = Font::parse(view.str("font").unwrap_or(DEFAULT_FONT));
    let size = if font.size > 0.0 && font.size.is_finite() {
        font.size
    } else {
        13.0
    };
    let line_h = fonts.line_height(size);
    let model = view.ref_id("model").and_then(|m| doc.get(m));
    let mut shape = shape_of(view, model);
    let icon = icon_of(view);
    let display = view.str("stereotypeDisplay").unwrap_or("label");
    let colour =
        |key: &str, default: Color| view.str(key).and_then(Color::parse).unwrap_or(default);
    let (fill, line, text) = (
        colour("fillColor", Color::WHITE),
        colour("lineColor", Color::BLACK),
        colour("fontColor", Color::BLACK),
    );

    // Ports and symbols: fixed small shapes; any name is drawn outside.
    if matches!(shape, Shape::Port | Shape::Symbol(_)) {
        let min = if shape == Shape::Port { PORT_SIZE } else { 4.0 };
        let rect = Rect::new(stored.x, stored.y, stored.w.max(min), stored.h.max(min));
        let mut outside = Vec::new();
        if let Some(m) = model {
            let ty = uml::var_text(doc, m, "type");
            let label = if shape == Shape::Port && view.bool("showType", true) {
                uml::typed_name(m.name(), ty)
            } else {
                m.name().to_owned()
            };
            let host = view.ref_id("nameLabel").and_then(|l| doc.get(l));
            let alpha = host.map_or(absent::ALPHA, |l| l.num("alpha", absent::ALPHA));
            let distance = host.map_or(absent::DISTANCE, |l| l.num("distance", absent::DISTANCE));
            let c = rect.center();
            // StarUML measures label angles opposite to screen angles (see docs/evidence.md §3).
            let center = Point::new(c.x + distance * alpha.cos(), c.y - distance * alpha.sin());
            if !label.is_empty() {
                outside.push(OutsideLabel {
                    line: TextLine::centered(label),
                    center,
                });
            }
        }
        return Some(NodeLayout {
            rect,
            min_w: min,
            min_h: min,
            shape,
            compartments: Vec::new(),
            separators: false,
            font_size: size,
            fill,
            line,
            text,
            active: false,
            dashed: false,
            icon: None,
            outside,
            dividers: Vec::new(),
        });
    }

    // Mind map nodes: the name centred, wrapped to the box unless `wordWrap`
    // is off (on by default).
    if view.type_name == "MMNodeView" {
        let name = model.map(|m| m.name().to_owned()).unwrap_or_default();
        let wraps = view.bool("wordWrap", true);
        let widest_word = name
            .split(['\n', ' '])
            .map(|w| fonts.width(w, size, false, false))
            .fold(0.0, f64::max);
        let lines: Vec<String> = if wraps {
            let room = (stored.w - 2.0 * MIND_PAD_X).max(widest_word);
            wrap(&name, room, fonts, size)
        } else {
            name.split('\n').map(str::to_owned).collect()
        };
        let widest = lines
            .iter()
            .map(|l| fonts.width(l, size, false, false))
            .fold(0.0, f64::max);
        let min_w = ((if wraps { widest_word } else { widest }) + 2.0 * MIND_PAD_X).ceil();
        let text_h = lines.len() as f64 * line_h;
        let min_h = (text_h + 2.0 * MIND_PAD_Y).ceil();
        let rect = Rect::new(stored.x, stored.y, stored.w.max(min_w), stored.h.max(min_h));
        let top = rect.y + (rect.h - text_h) / 2.0;
        return Some(NodeLayout {
            rect,
            min_w,
            min_h,
            shape: Shape::MindNode,
            compartments: vec![Compartment {
                lines: lines.into_iter().map(TextLine::centered).collect(),
                rect: Rect::new(rect.x, top - PAD_Y, rect.w, text_h + 2.0 * PAD_Y),
            }],
            separators: false,
            font_size: size,
            fill,
            line,
            text,
            active: false,
            dashed: false,
            icon: None,
            outside: Vec::new(),
            dividers: Vec::new(),
        });
    }

    if view.type_name == "ERDEntityView" {
        let mut l = entity_layout(doc, view, model, stored, size, fonts);
        (l.fill, l.line, l.text) = (fill, line, text);
        return Some(l);
    }

    // Flowchart symbols: the name centred in the symbol's label band, near
    // the top (in the middle for diamonds and circles).
    if let Shape::Flowchart(flow) = shape {
        let name = model.map(|m| m.name().to_owned()).unwrap_or_default();
        let w = if name.is_empty() {
            0.0
        } else {
            fonts.width(&name, size, false, false)
        };
        let h = if name.is_empty() { 0.0 } else { line_h };
        let (min_w, min_h) = flow.min_size(w, h);
        let (min_w, min_h) = (min_w.ceil(), min_h.ceil());
        let rect = Rect::new(stored.x, stored.y, stored.w.max(min_w), stored.h.max(min_h));
        let (band_x, band_w, text_top) = flow.label_band(rect, line_h);
        let compartments = if name.is_empty() {
            Vec::new()
        } else {
            vec![Compartment {
                lines: vec![TextLine::centered(name)],
                rect: Rect::new(band_x, text_top - PAD_Y, band_w, line_h + 2.0 * PAD_Y),
            }]
        };
        return Some(NodeLayout {
            rect,
            min_w,
            min_h,
            shape,
            compartments,
            separators: false,
            font_size: size,
            fill,
            line,
            text,
            active: false,
            dashed: false,
            icon: None,
            outside: Vec::new(),
            dividers: Vec::new(),
        });
    }

    // Frames and combined fragments: a tag with the kind and name, then (for
    // fragments) one area per operand with its guard.
    if matches!(shape, Shape::Frame { .. }) {
        // An interaction-overview action frames what it calls.
        let framed = match model {
            Some(m) if m.type_name == "UMLAction" => m.ref_id("target").and_then(|t| doc.get(t)),
            other => other,
        };
        let is_ref = view.type_name == "UMLInteractionUseView";
        let tag_text = match framed {
            _ if is_ref => "ref".to_owned(),
            Some(m) if m.type_name == "UMLCombinedFragment" => {
                m.str("interactionOperator").unwrap_or("alt").to_owned()
            }
            Some(m) => match frame_kind(&m.type_name) {
                "" => m.name().to_owned(),
                kind => format!("{kind} {}", m.name()),
            },
            None => String::new(),
        };
        let tag_w = fonts.width(&tag_text, size, true, false) + 2.0 * PAD_X + 8.0;
        let tag = Rect::new(stored.x, stored.y, tag_w, line_h + 2.0 * PAD_Y);
        let min_w = (tag_w + 20.0).ceil();
        let min_h = (tag.h + 20.0).ceil();
        let rect = Rect::new(stored.x, stored.y, stored.w.max(min_w), stored.h.max(min_h));
        let mut compartments = vec![Compartment {
            lines: vec![TextLine {
                bold: true,
                ..TextLine::aligned(tag_text, HAlign::Left)
            }],
            rect: tag,
        }];
        if is_ref && let Some(usage) = framed {
            // StarUML puts the call beside the tag: `x = Called(args): result`.
            let named = |key: &str| {
                usage
                    .ref_id(key)
                    .and_then(|r| doc.get(r))
                    .map_or("", |e| e.name())
            };
            let mut call = String::new();
            if !named("returnValueRecipient").is_empty() {
                call += &format!("{} = ", named("returnValueRecipient"));
            }
            call += named("refersTo");
            let field = |k: &str| usage.str(k).unwrap_or("");
            if !field("arguments").is_empty() {
                call += &format!("({})", field("arguments"));
            }
            if !field("returnValue").is_empty() {
                call += &format!(": {}", field("returnValue"));
            }
            let left = tag.right() + PAD_X;
            compartments.push(Compartment {
                lines: vec![TextLine::aligned(call, HAlign::Left)],
                rect: Rect::new(left, tag.y, (rect.right() - left - PAD_X).max(0.0), tag.h),
            });
        }
        let mut dividers = Vec::new();
        let operands: Vec<&Element> = framed
            .map(|m| {
                m.children("operands")
                    .iter()
                    .filter_map(|id| doc.get(id))
                    .collect()
            })
            .unwrap_or_default();
        if !operands.is_empty() {
            let each = (rect.bottom() - tag.bottom()) / operands.len() as f64;
            for (i, op) in operands.iter().enumerate() {
                let top = tag.bottom() + each * i as f64;
                if i > 0 {
                    dividers.push((Point::new(rect.x, top), Point::new(rect.right(), top), true));
                }
                let guard = op.str("guard").unwrap_or("");
                if !guard.is_empty() {
                    compartments.push(Compartment {
                        lines: vec![TextLine::aligned(format!("[{guard}]"), HAlign::Left)],
                        rect: Rect::new(rect.x, top, rect.w, line_h + 2.0 * PAD_Y),
                    });
                }
            }
        }
        return Some(NodeLayout {
            rect,
            min_w,
            min_h,
            shape: Shape::Frame { tag },
            compartments,
            separators: false,
            font_size: size,
            fill,
            line,
            text,
            active: false,
            dashed: false,
            icon: None,
            outside: Vec::new(),
            dividers,
        });
    }

    // Notes and free text: their own `text`, wrapped to the box.
    if matches!(shape, Shape::Note | Shape::Text) {
        let body = view.str("text").unwrap_or("");
        let fold = if shape == Shape::Note { NOTE_FOLD } else { 0.0 };
        let wrap_w = (stored.w - 2.0 * PAD_X - fold).max(20.0);
        let lines: Vec<String> = if view.bool("wordWrap", true) {
            wrap(body, wrap_w, fonts, size)
        } else {
            body.split('\n').map(str::to_owned).collect()
        };
        let widest = lines
            .iter()
            .map(|l| fonts.width(l, size, false, false))
            .fold(0.0, f64::max);
        let min_w = if view.bool("wordWrap", true) {
            30.0
        } else {
            (widest + 2.0 * PAD_X + fold).ceil()
        };
        let min_h = (lines.len() as f64 * line_h + 2.0 * PAD_Y).ceil();
        let rect = Rect::new(stored.x, stored.y, stored.w.max(min_w), stored.h.max(min_h));
        return Some(NodeLayout {
            rect,
            min_w,
            min_h,
            shape,
            compartments: vec![Compartment {
                lines: lines
                    .into_iter()
                    .map(|l| TextLine::aligned(l, HAlign::Left))
                    .collect(),
                rect,
            }],
            separators: false,
            font_size: size,
            fill,
            line,
            text,
            active: false,
            dashed: false,
            icon: None,
            outside: Vec::new(),
            dividers: Vec::new(),
        });
    }

    let mut groups: Vec<Vec<TextLine>> = Vec::new();
    let mut name_lines = Vec::new();
    let structured = view.ref_id("nameCompartment").is_some();
    match model {
        Some(m) if structured => {
            if display != "none" {
                let st = uml::stereotype_text(doc, m).or_else(|| {
                    keyword(&m.type_name)
                        .filter(|_| shows_keyword(display) || icon.is_none())
                        .map(str::to_owned)
                });
                if let Some(st) = st {
                    name_lines.push(TextLine::centered(st));
                }
            }
            let (title, underline) = heading(doc, view, m);
            name_lines.push(TextLine {
                bold: true,
                italic: m.bool("isAbstract", false),
                underline,
                ..TextLine::centered(title)
            });
            if view.bool("showNamespace", false)
                && let Some(owner) = m.parent.as_deref().and_then(|p| doc.get(p))
            {
                name_lines.push(TextLine::centered(format!("(from {})", owner.name())));
            }
        }
        Some(m) => name_lines.push(TextLine::centered(m.name().to_owned())),
        None => name_lines.push(TextLine::centered(view.type_name.clone())),
    }
    groups.push(name_lines);

    if let Some(m) = model.filter(|_| structured) {
        // `key` lists the features; `compartment` is the view field holding their rows.
        let rows =
            |key: &str, compartment: &str, text: &dyn Fn(&Element) -> TextLine| -> Vec<TextLine> {
                let aligns = row_alignments(doc, view, compartment);
                m.children(key)
                    .iter()
                    .filter_map(|id| doc.get(id))
                    .map(|f| TextLine {
                        align: aligns.get(f.id.as_str()).copied().unwrap_or(HAlign::Left),
                        ..text(f)
                    })
                    .collect()
            };
        if view.ref_id("enumerationLiteralCompartment").is_some()
            && !view.bool("suppressLiterals", false)
        {
            groups.push(rows("literals", "enumerationLiteralCompartment", &|l| {
                TextLine::aligned(l.name().to_owned(), HAlign::Left)
            }));
        }
        if view.ref_id("slotCompartment").is_some() {
            groups.push(rows("slots", "slotCompartment", &|s| {
                TextLine::aligned(uml::slot_text(doc, s), HAlign::Left)
            }));
        }
        if view.ref_id("attributeCompartment").is_some() && !view.bool("suppressAttributes", false)
        {
            groups.push(rows("attributes", "attributeCompartment", &|a| TextLine {
                underline: a.bool("isStatic", false),
                ..TextLine::aligned(uml::attribute_text(doc, a), HAlign::Left)
            }));
        }
        if view.ref_id("operationCompartment").is_some() && !view.bool("suppressOperations", false)
        {
            let signature = view.bool("showOperationSignature", true);
            groups.push(rows("operations", "operationCompartment", &|o| TextLine {
                underline: o.bool("isStatic", false),
                italic: o.bool("isAbstract", false),
                ..TextLine::aligned(
                    if signature {
                        uml::operation_text(doc, o)
                    } else {
                        format!(
                            "{}{}()",
                            uml::visibility_symbol(o.str("visibility").unwrap_or("public")),
                            o.name()
                        )
                    },
                    HAlign::Left,
                )
            }));
        }
    }
    // Ellipses, lollipops, actors and lanes show only their heading; actions
    // too (an action's "compartments" are its pins, drawn elsewhere).
    if matches!(
        shape,
        Shape::Ellipse | Shape::Lollipop { .. } | Shape::Actor { .. } | Shape::Swimlane { .. }
    ) || view.type_name == "UMLActionView"
    {
        groups.truncate(1);
    }
    // States: name, then their regions (if composite); internal activities
    // are not listed yet.
    let regions = if view.type_name == "UMLStateView" {
        model.map_or(0, |m| m.children("regions").len())
    } else {
        0
    };
    if matches!(
        view.type_name.as_str(),
        "UMLStateView" | "UMLSeqLifelineView"
    ) {
        groups.truncate(1);
    }

    let mut min_w = MIN_WIDTH;
    let mut heights = Vec::with_capacity(groups.len());
    for g in &groups {
        for l in g {
            min_w = min_w.max(fonts.width(&l.text, size, l.bold, l.italic) + 2.0 * PAD_X);
        }
        heights.push(if g.is_empty() {
            EMPTY_COMPARTMENT
        } else {
            g.len() as f64 * line_h + 2.0 * PAD_Y
        });
    }
    let content_h: f64 = heights.iter().sum();
    let active = structured && model.is_some_and(|m| m.bool("isActive", false));
    if active {
        min_w += 2.0 * ACTIVE_INSET;
    }
    if icon.is_some() {
        min_w += ICON_SPACE;
    }

    // Where the compartments go inside the outline, and the outline's minimum size.
    let (min_w, min_h) = match shape {
        Shape::Actor { .. } => (min_w.max(26.0), content_h + ACTOR_FIGURE_H),
        Shape::Lifeline { .. } => (min_w.max(30.0), LIFELINE_HEAD.max(content_h) + 50.0),
        Shape::Swimlane { vertical: true, .. } => {
            (min_w.max(60.0), (content_h + 40.0).max(LANE_HEADER + 40.0))
        }
        Shape::Swimlane {
            vertical: false, ..
        } => ((min_w + LANE_HEADER).max(100.0), content_h.max(60.0)),
        Shape::RoundRect if regions > 0 => (min_w.max(60.0), content_h + 30.0 * regions as f64),
        Shape::RoundRect => (min_w.max(60.0), content_h.max(40.0)),
        Shape::Package { .. } => (min_w.max(60.0), (content_h + PACKAGE_TAB_H).max(50.0)),
        Shape::Node3D => (min_w + NODE_DEPTH, content_h + NODE_DEPTH),
        Shape::Ellipse => (min_w * 1.3, (content_h * 1.6).max(40.0)),
        Shape::Lollipop { .. } => (min_w.max(LOLLIPOP), content_h + LOLLIPOP + 2.0),
        _ => (min_w, content_h),
    };
    let (min_w, min_h) = (min_w.ceil(), min_h.ceil());
    let rect = Rect::new(stored.x, stored.y, stored.w.max(min_w), stored.h.max(min_h));
    let body = match shape {
        Shape::Package { .. } => Rect::new(
            rect.x,
            rect.y + PACKAGE_TAB_H,
            rect.w,
            rect.h - PACKAGE_TAB_H,
        ),
        Shape::Node3D => Rect::new(
            rect.x,
            rect.y + NODE_DEPTH,
            rect.w - NODE_DEPTH,
            rect.h - NODE_DEPTH,
        ),
        Shape::Ellipse => {
            let y = rect.y + (rect.h - content_h) / 2.0;
            Rect::new(rect.x, y, rect.w, content_h)
        }
        Shape::Lollipop { .. } => Rect::new(
            rect.x,
            rect.y + LOLLIPOP + 2.0,
            rect.w,
            rect.h - LOLLIPOP - 2.0,
        ),
        // Name below the figure.
        Shape::Actor { .. } => Rect::new(rect.x, rect.bottom() - content_h, rect.w, content_h),
        // Name centred in the head box.
        Shape::Lifeline { .. } => {
            let head_h = LIFELINE_HEAD.max(content_h);
            Rect::new(
                rect.x,
                rect.y + (head_h - content_h) / 2.0,
                rect.w,
                content_h,
            )
        }
        // Name in the band (top for vertical lanes; top-left of the lane otherwise).
        Shape::Swimlane { vertical: true, .. } => Rect::new(rect.x, rect.y, rect.w, LANE_HEADER),
        Shape::Swimlane {
            vertical: false, ..
        } => Rect::new(
            rect.x + LANE_HEADER,
            rect.y,
            rect.w - LANE_HEADER,
            content_h,
        ),
        // A simple state or action: name centred vertically.
        Shape::RoundRect if regions == 0 => Rect::new(
            rect.x,
            rect.y + (rect.h - content_h) / 2.0,
            rect.w,
            content_h,
        ),
        _ => rect,
    };
    shape = match shape {
        Shape::Package { .. } => {
            let tab_w = (rect.w * 0.4).clamp(30.0, 80.0);
            Shape::Package {
                tab: Rect::new(rect.x, rect.y, tab_w, PACKAGE_TAB_H),
            }
        }
        Shape::Lollipop { .. } => Shape::Lollipop {
            circle: Rect::new(rect.center().x - LOLLIPOP / 2.0, rect.y, LOLLIPOP, LOLLIPOP),
        },
        Shape::Actor { .. } => Shape::Actor {
            figure: Rect::new(rect.x, rect.y, rect.w, rect.h - content_h),
        },
        Shape::Lifeline { .. } => Shape::Lifeline {
            head: Rect::new(rect.x, rect.y, rect.w, LIFELINE_HEAD.max(content_h)),
        },
        Shape::Swimlane { vertical, .. } => Shape::Swimlane {
            vertical,
            header: if vertical {
                Rect::new(rect.x, rect.y, rect.w, LANE_HEADER)
            } else {
                Rect::new(rect.x, rect.y, LANE_HEADER, rect.h)
            },
        },
        other => other,
    };

    // Stack compartments; the last one takes any spare height (box-like shapes).
    let stretch = matches!(shape, Shape::Box | Shape::Package { .. } | Shape::Node3D);
    let mut compartments = Vec::with_capacity(groups.len());
    let mut y = body.y;
    let count = groups.len();
    for (i, (lines, h)) in groups.into_iter().zip(heights).enumerate() {
        let h = if stretch && i + 1 == count {
            body.bottom() - y
        } else {
            h
        };
        compartments.push(Compartment {
            lines,
            rect: Rect::new(body.x, y, body.w, h),
        });
        y += h;
    }

    // A composite state: a line under the name, dashed lines between regions.
    let mut dividers = Vec::new();
    if regions > 0
        && let Some(name) = compartments.first()
    {
        let top = name.rect.bottom();
        dividers.push((
            Point::new(rect.x, top),
            Point::new(rect.right(), top),
            false,
        ));
        let each = (rect.bottom() - top) / regions as f64;
        for i in 1..regions {
            let y = top + each * i as f64;
            dividers.push((Point::new(rect.x, y), Point::new(rect.right(), y), true));
        }
    }
    let dashed = (shape == Shape::Ellipse && view.type_name != "UMLUseCaseView")
        || (view.type_name == "UMLPartView"
            && model.is_some_and(|m| m.str("aggregation").unwrap_or("none") != "composite"));
    Some(NodeLayout {
        rect,
        min_w,
        min_h,
        shape,
        compartments,
        separators: stretch,
        font_size: size,
        fill,
        line,
        text,
        active,
        dashed,
        icon,
        outside: Vec::new(),
        dividers,
    })
}

/// Padding inside an ERD entity, around its name and its column table, and
/// between a table column and its divider (StarUML: 5).
const ERD_PAD: f64 = 5.0;
/// Space between two column rows (StarUML: 2).
const ERD_ROW_GAP: f64 = 2.0;

/// An ERD entity: its name, bold and centred, over a table of columns
/// (keys | name | type) with lines between the table's columns, laid out
/// as StarUML does.
fn entity_layout(
    doc: &Document,
    view: &Element,
    model: Option<&Element>,
    stored: Rect,
    size: f64,
    fonts: &Fonts,
) -> NodeLayout {
    let line_h = fonts.line_height(size);
    let width = |t: &str| fonts.width(t, size, false, false);
    let name = model.map(|m| m.name().to_owned()).unwrap_or_default();
    let columns: Vec<(String, String, String)> = model
        .filter(|_| !view.bool("suppressColumns", false))
        .map(|m| {
            m.children("columns")
                .iter()
                .filter_map(|id| doc.get(id))
                .map(|c| {
                    (
                        uml::column_keys(c),
                        c.name().to_owned(),
                        uml::column_type(c),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let widest = |f: fn(&(String, String, String)) -> &String| {
        columns.iter().map(|c| width(f(c))).fold(0.0, f64::max)
    };
    let (key_w, name_w, type_w) = (widest(|c| &c.0), widest(|c| &c.1), widest(|c| &c.2));
    // Where the name and type columns start, measured from the table's inside.
    let name_at = key_w + ERD_PAD;
    let type_at = name_at + ERD_PAD + name_w + ERD_PAD;

    let header_h = ERD_PAD + line_h + ERD_PAD;
    let rows = columns.len() as f64;
    let table_h = if view.bool("suppressColumns", false) {
        0.0
    } else {
        rows * line_h + (rows - 1.0).max(0.0) * ERD_ROW_GAP + 2.0 * ERD_PAD
    };
    let mut min_w = fonts.width(&name, size, true, false) + 2.0 * ERD_PAD;
    if !columns.is_empty() {
        min_w = min_w.max(type_at + ERD_PAD + type_w + 2.0 * ERD_PAD);
    }
    let (min_w, min_h) = (min_w.max(MIN_WIDTH).ceil(), (header_h + table_h).ceil());
    let rect = Rect::new(stored.x, stored.y, stored.w.max(min_w), stored.h.max(min_h));

    // A compartment whose text starts at (x, y) (compartment text is inset by the pads).
    let cell = |text: String, x: f64, y: f64, align: HAlign, w: f64| Compartment {
        lines: vec![TextLine::aligned(text, align)],
        rect: Rect::new(x - PAD_X, y - PAD_Y, w + 2.0 * PAD_X, line_h + 2.0 * PAD_Y),
    };
    let mut compartments = vec![Compartment {
        lines: vec![TextLine {
            bold: true,
            ..TextLine::centered(name)
        }],
        rect: Rect::new(
            rect.x,
            rect.y + ERD_PAD - PAD_Y,
            rect.w,
            line_h + 2.0 * PAD_Y,
        ),
    }];
    let mut dividers = Vec::new();
    if !columns.is_empty() {
        let top = rect.y + header_h;
        let left = rect.x + ERD_PAD;
        dividers.push((
            Point::new(rect.x, top),
            Point::new(rect.right(), top),
            false,
        ));
        for x in [left + name_at, left + type_at] {
            dividers.push((Point::new(x, top), Point::new(x, rect.bottom()), false));
        }
        for (i, (keys, name, ty)) in columns.into_iter().enumerate() {
            let y = top + ERD_PAD + i as f64 * (line_h + ERD_ROW_GAP);
            compartments.push(cell(keys, left, y, HAlign::Left, key_w));
            compartments.push(cell(
                name,
                left + name_at + ERD_PAD,
                y,
                HAlign::Left,
                name_w,
            ));
            compartments.push(cell(ty, left + type_at + ERD_PAD, y, HAlign::Left, type_w));
        }
    }
    NodeLayout {
        rect,
        min_w,
        min_h,
        shape: Shape::Box,
        compartments,
        separators: false,
        font_size: size,
        fill: Color::WHITE,
        line: Color::BLACK,
        text: Color::BLACK,
        active: false,
        dashed: false,
        icon: None,
        outside: Vec::new(),
        dividers,
    }
}

pub(crate) const TEXT_PAD_X: f64 = PAD_X;
pub(crate) const TEXT_PAD_Y: f64 = PAD_Y;

#[cfg(test)]
mod tests {
    use mdj::{Field, Id};

    use super::*;

    fn setup() -> (Document, Id) {
        let doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let dgm = doc.get(&model).unwrap().children("ownedElements")[0].clone();
        (doc, dgm)
    }

    fn place(doc: &mut Document, dgm: &str, tool: &str, container: Option<&str>) -> (Id, Id) {
        let spec = uml::node_spec(tool).unwrap();
        let (m, v) = doc
            .try_edit(|e| uml::create_node(e, dgm, spec, 100.0, 100.0, container))
            .unwrap();
        (m.unwrap_or_default(), v)
    }

    fn layout(doc: &Document, view: &str) -> NodeLayout {
        node_layout(doc, doc.get(view).unwrap(), &Fonts::default()).unwrap()
    }

    /// Rows follow their stored alignment the way StarUML loads it: written
    /// LEFT → left, field missing → centred (metamodel default).
    #[test]
    fn rows_follow_stored_alignment_like_staruml() {
        let (mut doc, dgm) = setup();
        let (class, view) = place(&mut doc, &dgm, "UMLClass", None);
        doc.edit(|e| uml::add_feature(e, &class, uml::FeatureKind::Attribute));
        let rows = |doc: &Document| layout(doc, &view).compartments[1].lines[0].align;
        assert_eq!(rows(&doc), HAlign::Left, "rust-uml writes LEFT");

        // A file written without the field (like rust-uml 0.1.0) shows centred, as in StarUML.
        let comp = doc
            .get(&view)
            .unwrap()
            .ref_id("attributeCompartment")
            .unwrap()
            .to_owned();
        let row = doc.get(&comp).unwrap().children("subViews")[0].clone();
        doc.edit(|e| e.unset(&row, "horizontalAlignment"));
        assert_eq!(rows(&doc), HAlign::Center);
        doc.edit(|e| e.set(&row, "horizontalAlignment", Field::int(align::RIGHT)));
        assert_eq!(rows(&doc), HAlign::Right);
    }

    #[test]
    fn active_class_is_wider_and_flagged() {
        let (mut doc, dgm) = setup();
        let (class, view) = place(&mut doc, &dgm, "UMLClass", None);
        let plain = layout(&doc, &view);
        doc.edit(|e| e.set(&class, "isActive", Field::bool(true)));
        let active = layout(&doc, &view);
        assert!(!plain.active && active.active);
        assert_eq!(active.min_w, plain.min_w + 2.0 * ACTIVE_INSET);
    }

    #[test]
    fn shapes_by_view_type() {
        let (mut doc, dgm) = setup();
        let cases = [
            ("UMLNode", "Node3D"),
            ("UMLCollaboration", "Ellipse"),
            ("Note", "Note"),
            ("Text", "Text"),
            ("UMLPackage", "Package"),
            ("UMLSubsystem", "Package"),
            ("UMLComponent", "Box"),
        ];
        for (tool, want) in cases {
            let (_, v) = place(&mut doc, &dgm, tool, None);
            let got = format!("{:?}", layout(&doc, &v).shape);
            assert!(got.starts_with(want), "{tool}: {got}");
        }
        // Artifacts are created with the icon display, as StarUML does.
        let (_, artifact) = place(&mut doc, &dgm, "UMLArtifact", None);
        assert_eq!(layout(&doc, &artifact).icon, Some(Icon::Artifact));
    }

    #[test]
    fn objects_are_underlined_and_list_slots() {
        let (mut doc, dgm) = setup();
        let (obj, view) = place(&mut doc, &dgm, "UMLObject", None);
        doc.edit(|e| {
            e.set(&obj, "classifier", Field::str("Order"));
            uml::add_feature(e, &obj, uml::FeatureKind::Slot);
        });
        let l = layout(&doc, &view);
        let title = &l.compartments[0].lines[0];
        assert_eq!(title.text, "Object1 : Order");
        assert!(title.underline);
        assert_eq!(l.compartments[1].lines[0].text, "slot1");
    }

    #[test]
    fn port_name_sits_outside_its_square() {
        let (mut doc, dgm) = setup();
        let (_, comp) = place(&mut doc, &dgm, "UMLComponent", None);
        let (_, port) = place(&mut doc, &dgm, "UMLPort", Some(&comp));
        let l = layout(&doc, &port);
        assert_eq!((l.rect.w, l.rect.h), (14.0, 14.0));
        let label = &l.outside[0];
        assert_eq!(label.line.text, "Port1");
        assert!(!l.rect.contains(label.center), "{label:?} vs {:?}", l.rect);
    }

    #[test]
    fn shared_parts_are_dashed() {
        let (mut doc, dgm) = setup();
        let (_, class) = place(&mut doc, &dgm, "UMLClass", None);
        let (part, view) = place(&mut doc, &dgm, "UMLPart", Some(&class));
        assert!(!layout(&doc, &view).dashed, "composite part: solid");
        doc.edit(|e| e.set(&part, "aggregation", Field::str("shared")));
        assert!(layout(&doc, &view).dashed);
    }

    /// A diagram of `ty` created the way the UI does (with its state machine
    /// or activity).
    fn diagram_of_type(doc: &mut Document, ty: &str) -> Id {
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        doc.try_edit(|e| uml::create_diagram(e, &model, ty))
            .unwrap()
    }

    #[test]
    fn behaviour_symbols_follow_their_model() {
        let mut doc = Document::new_project();
        let sm = diagram_of_type(&mut doc, "UMLStatechartDiagram");
        let act = diagram_of_type(&mut doc, "UMLActivityDiagram");
        let cases = [
            (&sm, "UMLInitialState", Shape::Symbol(Symbol::Dot)),
            (&sm, "UMLFinalState", Shape::Symbol(Symbol::BullsEye)),
            (&sm, "UMLChoice", Shape::Symbol(Symbol::Diamond)),
            (&sm, "UMLFork", Shape::Symbol(Symbol::Bar)),
            (
                &sm,
                "UMLDeepHistory",
                Shape::Symbol(Symbol::History { deep: true }),
            ),
            (&sm, "UMLTerminate", Shape::Symbol(Symbol::Cross)),
            (&act, "UMLInitialNode", Shape::Symbol(Symbol::Dot)),
            (&act, "UMLFlowFinalNode", Shape::Symbol(Symbol::CircleX)),
            (&act, "UMLDecisionNode", Shape::Symbol(Symbol::Diamond)),
            (&act, "UMLAction", Shape::RoundRect),
        ];
        for (dgm, tool, want) in cases {
            let (_, v) = place(&mut doc, dgm, tool, None);
            assert_eq!(layout(&doc, &v).shape, want, "{tool}");
        }
    }

    #[test]
    fn composite_states_divide_their_regions() {
        let mut doc = Document::new_project();
        let sm = diagram_of_type(&mut doc, "UMLStatechartDiagram");
        let (_, simple) = place(&mut doc, &sm, "UMLState", None);
        let (_, ortho) = place(&mut doc, &sm, "UMLOrthogonalState", None);
        assert!(layout(&doc, &simple).dividers.is_empty());
        let d = layout(&doc, &ortho).dividers;
        assert_eq!(
            d.len(),
            2,
            "a line under the name and one dashed line between two regions"
        );
        assert!(!d[0].2 && d[1].2);
    }

    #[test]
    fn actors_lanes_and_use_cases() {
        let mut doc = Document::new_project();
        let uc = diagram_of_type(&mut doc, "UMLUseCaseDiagram");
        let (_, actor) = place(&mut doc, &uc, "UMLActor", None);
        let l = layout(&doc, &actor);
        let Shape::Actor { figure } = l.shape else {
            panic!("{:?}", l.shape)
        };
        assert!(
            figure.bottom() <= l.compartments[0].rect.y + 0.01,
            "name below the figure"
        );
        let (_, case) = place(&mut doc, &uc, "UMLUseCase", None);
        let l = layout(&doc, &case);
        assert_eq!(l.shape, Shape::Ellipse);
        assert!(
            !l.dashed,
            "use cases are solid; only collaborations are dashed"
        );

        let act = diagram_of_type(&mut doc, "UMLActivityDiagram");
        let (_, lane) = place(&mut doc, &act, "UMLSwimlaneHorz", None);
        let Shape::Swimlane { vertical, header } = layout(&doc, &lane).shape else {
            panic!()
        };
        assert!(!vertical && header.w == LANE_HEADER);
    }

    #[test]
    fn notes_wrap_their_text() {
        let (mut doc, dgm) = setup();
        let (_, note) = place(&mut doc, &dgm, "Note", None);
        doc.edit(|e| {
            e.set(
                &note,
                "text",
                Field::str("one two three four five six seven eight nine ten"),
            )
        });
        let l = layout(&doc, &note);
        assert!(
            l.compartments[0].lines.len() > 1,
            "wrapped: {:?}",
            l.compartments[0].lines
        );
        assert!(l.rect.h > 50.0, "grew to fit");
    }
}
