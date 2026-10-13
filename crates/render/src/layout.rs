//! Box geometry for node views: what text each compartment holds and the
//! smallest size that fits it.

use std::collections::HashMap;

use mdj::values::{DEFAULT_FONT, Font, absent, align};
use mdj::{Document, Element};

use crate::geom::{Color, Rect};
use crate::text::Fonts;

const PAD_X: f64 = 6.0;
const PAD_Y: f64 = 4.0;
const EMPTY_COMPARTMENT: f64 = 10.0;
const MIN_WIDTH: f64 = 40.0;
pub(crate) const PACKAGE_TAB_H: f64 = 14.0;
/// Gap between the outer and inner side borders of an active class.
pub(crate) const ACTIVE_INSET: f64 = 4.0;

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
    /// An active class (`isActive`): drawn with double side borders.
    pub active: bool,
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
    let active = structured && model.is_some_and(|m| m.bool("isActive", false));
    if active {
        min_w += 2.0 * ACTIVE_INSET;
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
        active,
    })
}

pub(crate) const TEXT_PAD_X: f64 = PAD_X;
pub(crate) const TEXT_PAD_Y: f64 = PAD_Y;

#[cfg(test)]
mod tests {
    use mdj::Field;

    use super::*;

    /// Rows follow their stored alignment the way StarUML loads it: written
    /// LEFT → left, field missing → centred (metamodel default).
    #[test]
    fn rows_follow_stored_alignment_like_staruml() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let dgm = doc.get(&model).unwrap().children("ownedElements")[0].clone();
        let (class, view) = doc.edit(|e| uml::create_node(e, &dgm, uml::NodeKind::Class, 0.0, 0.0));
        doc.edit(|e| uml::add_feature(e, &class, uml::FeatureKind::Attribute));
        let fonts = Fonts::default();
        let rows = |doc: &Document| {
            let l = node_layout(doc, doc.get(&view).unwrap(), &fonts).unwrap();
            l.compartments[1].lines[0].align
        };
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
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let dgm = doc.get(&model).unwrap().children("ownedElements")[0].clone();
        let (class, view) = doc.edit(|e| uml::create_node(e, &dgm, uml::NodeKind::Class, 0.0, 0.0));
        let fonts = Fonts::default();
        let plain = node_layout(&doc, doc.get(&view).unwrap(), &fonts).unwrap();
        doc.edit(|e| e.set(&class, "isActive", Field::bool(true)));
        let active = node_layout(&doc, doc.get(&view).unwrap(), &fonts).unwrap();
        assert!(!plain.active && active.active);
        assert_eq!(active.min_w, plain.min_w + 2.0 * ACTIVE_INSET);
    }
}
