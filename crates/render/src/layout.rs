//! Box geometry for node views: what text each compartment holds and the
//! smallest size that fits it.

use mdj::values::{DEFAULT_FONT, Font};
use mdj::{Document, Element};

use crate::geom::{Color, Rect};
use crate::text::Fonts;

const PAD_X: f64 = 6.0;
const PAD_Y: f64 = 4.0;
const EMPTY_COMPARTMENT: f64 = 10.0;
const MIN_WIDTH: f64 = 40.0;
pub(crate) const PACKAGE_TAB_H: f64 = 14.0;

#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub centered: bool,
}

impl TextLine {
    fn centered(text: String) -> Self {
        Self {
            text,
            bold: false,
            italic: false,
            underline: false,
            centered: true,
        }
    }

    fn row(text: String) -> Self {
        Self {
            centered: false,
            ..Self::centered(text)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Compartment {
    pub lines: Vec<TextLine>,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Box,
    /// A folder: the tab sits at the top-left, the body below it.
    Package {
        tab: Rect,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    /// Final rectangle: the stored size, grown if the content needs more.
    pub rect: Rect,
    pub min_w: f64,
    pub min_h: f64,
    pub shape: Shape,
    pub compartments: Vec<Compartment>,
    pub font_size: f64,
    pub fill: Color,
    pub line: Color,
    pub text: Color,
}

/// Default «keyword» shown for a model type when it has no explicit stereotype.
fn keyword(model_type: &str) -> Option<&'static str> {
    Some(match model_type {
        "UMLInterface" => "«interface»",
        "UMLEnumeration" => "«enumeration»",
        "UMLDataType" => "«dataType»",
        "UMLPrimitiveType" => "«primitiveType»",
        "UMLSignal" => "«signal»",
        "UMLMetaClass" => "«metaclass»",
        "UMLStereotype" => "«stereotype»",
        "UMLArtifact" => "«artifact»",
        "UMLComponent" => "«component»",
        _ => return None,
    })
}

/// Layout of a top-level node view, or `None` if `view` is not a node
/// (no position fields).
pub fn node_layout(doc: &Document, view: &Element, fonts: &Fonts) -> Option<NodeLayout> {
    let has_geometry = view.get("left").is_some() || view.get("width").is_some();
    if !has_geometry || view.get("head").is_some() {
        return None;
    }
    let stored = Rect::new(
        view.num("left", 0.0),
        view.num("top", 0.0),
        view.num("width", 0.0),
        view.num("height", 0.0),
    );
    let font = Font::parse(view.str("font").unwrap_or(DEFAULT_FONT));
    let size = if font.size > 0.0 && font.size.is_finite() {
        font.size
    } else {
        13.0
    };
    let model = view.ref_id("model").and_then(|m| doc.get(m));

    let mut groups: Vec<Vec<TextLine>> = Vec::new();
    let mut name_lines = Vec::new();
    let structured = view.ref_id("nameCompartment").is_some();
    match model {
        Some(m) if structured => {
            if view.str("stereotypeDisplay") != Some("none") {
                let st = uml::stereotype_text(doc, m)
                    .or_else(|| keyword(&m.type_name).map(str::to_owned));
                if let Some(st) = st {
                    name_lines.push(TextLine::centered(st));
                }
            }
            name_lines.push(TextLine {
                bold: true,
                italic: m.bool("isAbstract", false),
                ..TextLine::centered(m.name().to_owned())
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
        let rows = |key: &str, text: &dyn Fn(&Element) -> TextLine| -> Vec<TextLine> {
            m.children(key)
                .iter()
                .filter_map(|id| doc.get(id))
                .map(text)
                .collect()
        };
        if view.ref_id("enumerationLiteralCompartment").is_some()
            && !view.bool("suppressLiterals", false)
        {
            groups.push(rows("literals", &|l| TextLine::row(l.name().to_owned())));
        }
        if view.ref_id("attributeCompartment").is_some() && !view.bool("suppressAttributes", false)
        {
            groups.push(rows("attributes", &|a| TextLine {
                underline: a.bool("isStatic", false),
                ..TextLine::row(uml::attribute_text(doc, a))
            }));
        }
        if view.ref_id("operationCompartment").is_some() && !view.bool("suppressOperations", false)
        {
            let signature = view.bool("showOperationSignature", true);
            groups.push(rows("operations", &|o| TextLine {
                underline: o.bool("isStatic", false),
                italic: o.bool("isAbstract", false),
                ..TextLine::row(if signature {
                    uml::operation_text(doc, o)
                } else {
                    format!(
                        "{}{}()",
                        uml::visibility_symbol(o.str("visibility").unwrap_or("public")),
                        o.name()
                    )
                })
            }));
        }
    }

    let line_h = fonts.line_height(size);
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
    let is_package = view.type_name.ends_with("PackageView")
        || view.type_name == "UMLModelView"
        || view.type_name == "UMLSubsystemView";
    let tab_h = if is_package { PACKAGE_TAB_H } else { 0.0 };
    let mut min_h: f64 = heights.iter().sum::<f64>() + tab_h;
    if is_package {
        min_w = min_w.max(60.0);
        min_h = min_h.max(50.0);
    }
    min_w = min_w.ceil();
    min_h = min_h.ceil();

    let rect = Rect::new(stored.x, stored.y, stored.w.max(min_w), stored.h.max(min_h));

    // Stack compartments; the last one takes any spare height.
    let mut compartments = Vec::with_capacity(groups.len());
    let mut y = rect.y + tab_h;
    let count = groups.len();
    for (i, (lines, h)) in groups.into_iter().zip(heights).enumerate() {
        let h = if i + 1 == count { rect.bottom() - y } else { h };
        compartments.push(Compartment {
            lines,
            rect: Rect::new(rect.x, y, rect.w, h),
        });
        y += h;
    }

    let shape = if is_package {
        let tab_w = (rect.w * 0.4).clamp(30.0, 80.0);
        Shape::Package {
            tab: Rect::new(rect.x, rect.y, tab_w, tab_h),
        }
    } else {
        Shape::Box
    };
    let colour =
        |key: &str, default: Color| view.str(key).and_then(Color::parse).unwrap_or(default);
    Some(NodeLayout {
        rect,
        min_w,
        min_h,
        shape,
        compartments,
        font_size: size,
        fill: colour("fillColor", Color::WHITE),
        line: colour("lineColor", Color::BLACK),
        text: colour("fontColor", Color::BLACK),
    })
}

pub(crate) const TEXT_PAD_X: f64 = PAD_X;
pub(crate) const TEXT_PAD_Y: f64 = PAD_Y;
