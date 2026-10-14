//! `mindmap`: one node per line, nested by indentation.

use crate::lines::Line;
use crate::{ParseError, breaks, err, unquote};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mindmap {
    /// Every node, parents before their children; the first is the root.
    pub nodes: Vec<Node>,
}

/// Mermaid's node shapes. Mind map nodes in rust-uml (as in StarUML) are
/// all rounded boxes; the shape is kept so the import can say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// Plain text.
    #[default]
    Default,
    /// `id[text]`
    Square,
    /// `id(text)`
    Rounded,
    /// `id((text))`
    Circle,
    /// `id))text((`
    Bang,
    /// `id)text(`
    Cloud,
    /// `id{{text}}`
    Hexagon,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub id: String,
    pub text: String,
    pub shape: Shape,
    /// Index of the parent node; `None` for the root.
    pub parent: Option<usize>,
}

pub(crate) fn parse(lines: &[Line], warnings: &mut Vec<String>) -> Result<Mindmap, ParseError> {
    let mut d = Mindmap::default();
    // Open ancestors: (indentation, node index), innermost last.
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for line in lines {
        let (t, n) = (line.text.as_str(), line.number);
        // Icons and classes decorate the node above; skipped like styling.
        if t.starts_with("::icon(") || t.starts_with(":::") {
            continue;
        }
        let Some(mut node) = node(t) else {
            warnings.push(format!("line {n}: skipped `{t}`"));
            continue;
        };
        while stack
            .last()
            .is_some_and(|(indent, _)| *indent >= line.indent)
        {
            stack.pop();
        }
        node.parent = stack.last().map(|(_, i)| *i);
        if node.parent.is_none() && !d.nodes.is_empty() {
            return Err(err(n, "a mind map has one root: indent this line under it"));
        }
        stack.push((line.indent, d.nodes.len()));
        d.nodes.push(node);
    }
    if d.nodes.is_empty() {
        return Err(err(0, "the mind map has no nodes"));
    }
    Ok(d)
}

/// Bracket pairs, longest openers first.
const SHAPES: &[(&str, &str, Shape)] = &[
    ("((", "))", Shape::Circle),
    ("))", "((", Shape::Bang),
    ("{{", "}}", Shape::Hexagon),
    ("(", ")", Shape::Rounded),
    (")", "(", Shape::Cloud),
    ("[", "]", Shape::Square),
];

/// `id[text]` and the other shapes, or plain text (its own id).
fn node(t: &str) -> Option<Node> {
    // `A:::urgent` only styles the node.
    let t = t.split(":::").next().unwrap_or(t).trim();
    if t.is_empty() {
        return None;
    }
    let id_len: usize = t
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .map(char::len_utf8)
        .sum();
    let (id, rest) = t.split_at(id_len);
    for (open, close, shape) in SHAPES {
        if let Some(inner) = rest.strip_prefix(open).and_then(|r| r.strip_suffix(close)) {
            let text = label(inner);
            return Some(Node {
                id: if id.is_empty() {
                    text.clone()
                } else {
                    id.to_owned()
                },
                text,
                shape: *shape,
                parent: None,
            });
        }
    }
    let text = label(t);
    Some(Node {
        id: text.clone(),
        text,
        shape: Shape::Default,
        parent: None,
    })
}

/// Quotes and markdown backticks removed, `<br>` and `\n` as line breaks.
fn label(s: &str) -> String {
    breaks(unquote(unquote(s)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse as parse_text};

    fn mindmap(text: &str) -> (Mindmap, Vec<String>) {
        let p = parse_text(text).unwrap();
        match p.diagram {
            Diagram::Mindmap(d) => (d, p.warnings),
            _ => panic!("not a mind map"),
        }
    }

    #[test]
    fn mermaid_docs_example() {
        let (d, w) = mindmap(
            "mindmap
  root((mindmap))
    Origins
      Long history
      ::icon(fa fa-book)
      Popularisation
        British popular psychology author Tony Buzan
    Research
      On effectiveness<br/>and features
      On Automatic creation
        Uses
            Creative techniques
            Strategic planning
    Tools
      Pen and paper
      Mermaid
",
        );
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(d.nodes.len(), 14);
        let find = |text: &str| d.nodes.iter().position(|n| n.text == text).unwrap();
        assert_eq!(d.nodes[0].text, "mindmap");
        assert_eq!(d.nodes[0].id, "root");
        assert_eq!(d.nodes[0].shape, Shape::Circle);
        assert_eq!(d.nodes[find("Origins")].parent, Some(0));
        assert_eq!(
            d.nodes[find("Popularisation")].parent,
            Some(find("Origins"))
        );
        assert_eq!(
            d.nodes[find("Strategic planning")].parent,
            Some(find("Uses"))
        );
        assert_eq!(d.nodes[find("Tools")].parent, Some(0));
        assert!(
            d.nodes
                .iter()
                .any(|n| n.text == "On effectiveness\nand features")
        );
    }

    #[test]
    fn shapes_and_quotes() {
        let (d, _) = mindmap(
            "mindmap\nr\n  a[Square]\n  b(Rounded)\n  c))Bang((\n  d)Cloud(\n  e{{Hex}}\n  f[\"`**md**`\"]\n  plain (with parens)",
        );
        let shapes: Vec<Shape> = d.nodes.iter().map(|n| n.shape).collect();
        use Shape::*;
        assert_eq!(
            shapes,
            [
                Default, Square, Rounded, Bang, Cloud, Hexagon, Square, Default
            ]
        );
        assert_eq!(d.nodes[6].text, "**md**");
        assert_eq!(d.nodes[7].text, "plain (with parens)");
    }

    #[test]
    fn one_root_only() {
        assert!(parse_text("mindmap\n  a\n  b").is_err());
        assert!(parse_text("mindmap\n  a\n    b\n  c").is_err());
        assert!(parse_text("mindmap\n%% nothing").is_err());
        // Less indented than the root: Mermaid rejects it too.
        assert!(parse_text("mindmap\n    a\n  b").is_err());
        // Uneven indentation: the nearest node indented less is the parent.
        let (d, _) = mindmap("mindmap\na\n    b\n  c\n      d");
        assert_eq!(d.nodes[1].parent, Some(0));
        assert_eq!(d.nodes[2].parent, Some(0));
        assert_eq!(d.nodes[3].parent, Some(2));
    }
}
