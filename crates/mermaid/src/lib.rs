//! Mermaid diagram text → plain data, for importing into a model.
//!
//! Written from Mermaid's public syntax documentation. Covers what maps onto
//! UML: `classDiagram`, `sequenceDiagram` and `stateDiagram(-v2)`, and
//! `flowchart`/`graph`, `erDiagram` and `mindmap`, which map onto flowchart
//! symbols, ER diagrams and mind maps. Styling
//! (`style`, `classDef`, `:::`, `click`, …) is skipped silently; anything
//! else that isn't understood is skipped with a warning, so an import never
//! fails on one odd line. Structural mistakes (an unclosed block) are errors.

pub mod class;
pub mod er;
pub mod flowchart;
mod lines;
pub mod mindmap;
pub mod sequence;
pub mod state;

pub use lines::Line;

/// Layout direction (`direction LR`, `flowchart TD`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    TopDown,
    BottomUp,
    LeftRight,
    RightLeft,
}

impl Direction {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "TB" | "TD" => Some(Self::TopDown),
            "BT" => Some(Self::BottomUp),
            "LR" => Some(Self::LeftRight),
            "RL" => Some(Self::RightLeft),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Diagram {
    Class(class::ClassDiagram),
    Sequence(sequence::SequenceDiagram),
    State(state::StateDiagram),
    Flowchart(flowchart::Flowchart),
    Er(er::ErDiagram),
    Mindmap(mindmap::Mindmap),
}

/// A parsed diagram, its title and what was skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub diagram: Diagram,
    /// From front matter (`title: …`) or a `title` line.
    pub title: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number, 0 when it concerns the whole text.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for ParseError {}

pub(crate) fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
}

/// Largest text accepted, so hostile input can't take unbounded time.
pub const MAX_TEXT: usize = 1 << 20;

/// Parse Mermaid text.
pub fn parse(text: &str) -> Result<Parsed, ParseError> {
    if text.len() > MAX_TEXT {
        return Err(err(0, format!("text is larger than {} KB", MAX_TEXT >> 10)));
    }
    let (title, body) = lines::split(text);
    let Some((header, rest)) = body.split_first() else {
        return Err(err(0, "empty diagram"));
    };
    let kind = header
        .text
        .split(|c: char| c.is_whitespace() || c == ';')
        .next()
        .unwrap_or_default();
    let mut warnings = Vec::new();
    let (diagram, title2) = match kind {
        "classDiagram" | "classDiagram-v2" => {
            let (d, t) = class::parse(rest, &mut warnings)?;
            (Diagram::Class(d), t)
        }
        "sequenceDiagram" => {
            let (d, t) = sequence::parse(rest, &mut warnings)?;
            (Diagram::Sequence(d), t)
        }
        "stateDiagram" | "stateDiagram-v2" => {
            let (d, t) = state::parse(rest, &mut warnings)?;
            (Diagram::State(d), t)
        }
        "flowchart" | "flowchart-elk" | "graph" => (
            Diagram::Flowchart(flowchart::parse(header, rest, &mut warnings)?),
            None,
        ),
        "erDiagram" => (Diagram::Er(er::parse(rest, &mut warnings)?), None),
        "mindmap" => (Diagram::Mindmap(mindmap::parse(rest, &mut warnings)?), None),
        "requirementDiagram" => {
            return Err(err(
                header.number,
                format!(
                    "`{kind}` can't be imported yet: it needs a diagram family rust-uml doesn't have yet (class, sequence, state, flowchart, ER and mind map diagrams work)"
                ),
            ));
        }
        _ => {
            return Err(err(
                header.number,
                format!(
                    "`{kind}` is not a Mermaid diagram rust-uml can import (classDiagram, sequenceDiagram, stateDiagram, flowchart, erDiagram, mindmap)"
                ),
            ));
        }
    };
    Ok(Parsed {
        diagram,
        title: title.or(title2),
        warnings,
    })
}

/// `~T~` generics → `<T>`: a `~` between two name characters opens, any
/// other closes.
pub(crate) fn generics(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == '~' {
            let before = i > 0 && is_name_char(chars[i - 1]);
            let after = chars.get(i + 1).is_some_and(|c| is_name_char(*c));
            out.push(if before && after { '<' } else { '>' });
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Remove one pair of surrounding double quotes or backticks.
pub(crate) fn unquote(s: &str) -> &str {
    let s = s.trim();
    for q in ['"', '`'] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Mermaid writes line breaks in labels as `\n` or `<br>`.
pub(crate) fn breaks(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_kind_and_front_matter_title() {
        let p = parse("---\ntitle: Shop\n---\n%% a comment\nclassDiagram\nclass A").unwrap();
        assert_eq!(p.title.as_deref(), Some("Shop"));
        assert!(matches!(p.diagram, Diagram::Class(_)));
        let e = parse("requirementDiagram\n  requirement r { }").unwrap_err();
        assert!(e.message.contains("can't be imported yet"));
        assert!(parse("pie\n\"a\": 1").is_err());
        assert!(parse("  \n%% only a comment").is_err());
    }

    #[test]
    fn generics_and_quotes() {
        assert_eq!(generics("List~int~"), "List<int>");
        assert_eq!(generics("Map~K, List~V~~"), "Map<K, List<V>>");
        assert_eq!(unquote("\"a b\""), "a b");
        assert_eq!(unquote("`x`"), "x");
        assert_eq!(breaks("a\\nb<br>c"), "a\nb\nc");
    }
}
