//! `stateDiagram` and `stateDiagram-v2`.

use crate::lines::{Line, is_styling};
use crate::{Direction, ParseError, breaks, err, unquote};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateDiagram {
    pub direction: Direction,
    /// Every state, in order of first mention. Composite states' children
    /// name them as `parent`.
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Simple,
    Composite,
    Choice,
    Fork,
    Join,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub id: String,
    /// `state "Waiting for payment" as Waiting`
    pub label: Option<String>,
    pub kind: Kind,
    /// The composite state it sits in.
    pub parent: Option<String>,
    /// `Waiting : text` lines.
    pub descriptions: Vec<String>,
}

impl State {
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }
}

/// A transition end: a state, or the start/end (`[*]`) of a scope (the
/// whole diagram or one composite state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Point {
    State(String),
    Start(Option<String>),
    End(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub from: Point,
    pub to: Point,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub state: String,
    pub text: String,
}

pub(crate) fn parse(
    lines: &[Line],
    warnings: &mut Vec<String>,
) -> Result<(StateDiagram, Option<String>), ParseError> {
    let mut d = StateDiagram::default();
    let mut title = None;
    // Open composite states, innermost last, with the line that opened them.
    let mut scopes: Vec<(usize, String)> = Vec::new();
    // An open multi-line note: (line, state, text so far).
    let mut note: Option<(usize, String, Vec<String>)> = None;
    let mut warned_regions = false;
    for line in lines {
        let (t, n) = (line.text.as_str(), line.number);
        if let Some((_, state, text)) = &mut note {
            if t == "end note" {
                d.notes.push(Note {
                    state: state.clone(),
                    text: text.join("\n"),
                });
                note = None;
            } else {
                text.push(t.to_owned());
            }
            continue;
        }
        let scope = scopes.last().map(|(_, s)| s.clone());
        if is_styling(t) || t.starts_with("class ") {
            continue;
        }
        if let Some(dir) = t.strip_prefix("direction ") {
            match Direction::parse(dir) {
                Some(dir) if scopes.is_empty() => d.direction = dir,
                Some(_) => {}
                None => warnings.push(format!("line {n}: skipped `{t}`")),
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("title ") {
            title = Some(unquote(rest).to_owned());
            continue;
        }
        if t == "}" {
            if scopes.pop().is_none() {
                return Err(err(n, "`}` without an open state"));
            }
            continue;
        }
        if t == "--" {
            if !warned_regions {
                warnings.push(format!(
                    "line {n}: concurrent regions (`--`) were merged into one region"
                ));
                warned_regions = true;
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("state ") {
            state_declaration(&mut d, rest, n, &scope, &mut scopes)?;
            continue;
        }
        if let Some(rest) = t.strip_prefix("note ") {
            match note_start(rest) {
                Some((state, Some(text))) => {
                    ensure(&mut d, &state, &scope);
                    d.notes.push(Note { state, text });
                }
                Some((state, None)) => {
                    ensure(&mut d, &state, &scope);
                    note = Some((n, state, Vec::new()));
                }
                None => warnings.push(format!("line {n}: skipped `{t}`")),
            }
            continue;
        }
        if let Some((from, to, label)) = transition(t) {
            let from = point(&mut d, from, &scope, true);
            let to = point(&mut d, to, &scope, false);
            d.transitions.push(Transition { from, to, label });
            continue;
        }
        // `Crash:::bad` only styles a state.
        if let Some((id, _)) = t.split_once(":::")
            && is_id(id.trim())
        {
            ensure(&mut d, id.trim(), &scope);
            continue;
        }
        if let Some((id, text)) = t.split_once(':') {
            let id = id.trim();
            if is_id(id) {
                ensure(&mut d, id, &scope);
                if let Some(s) = d.states.iter_mut().find(|s| s.id == id) {
                    s.descriptions.push(breaks(text.trim()));
                }
                continue;
            }
        }
        if is_id(t) {
            ensure(&mut d, t, &scope);
            continue;
        }
        warnings.push(format!("line {n}: skipped `{t}`"));
    }
    if let Some((n, _, _)) = note {
        return Err(err(n, "this note is missing its `end note`"));
    }
    if let Some((n, id)) = scopes.last() {
        return Err(err(*n, format!("state `{id}` is missing its closing `}}`")));
    }
    Ok((d, title))
}

fn state_declaration(
    d: &mut StateDiagram,
    rest: &str,
    n: usize,
    scope: &Option<String>,
    scopes: &mut Vec<(usize, String)>,
) -> Result<(), ParseError> {
    let rest = rest.trim();
    let (rest, opens) = match rest.strip_suffix('{') {
        Some(r) => (r.trim(), true),
        None => (rest, false),
    };
    // `state "Long name" as Id`, `state Id <<choice>>`, `state Id`
    let (label, rest) = if let Some(quoted) = rest.strip_prefix('"') {
        let (label, after) = quoted
            .split_once('"')
            .ok_or_else(|| err(n, "unclosed quote"))?;
        let label = breaks(label);
        let after = after.trim();
        let after = after
            .strip_prefix("as ")
            .ok_or_else(|| err(n, "write `state \"Label\" as Id`"))?;
        (Some(label), after.trim())
    } else {
        (None, rest)
    };
    let (id, stereo) = match rest.find("<<") {
        Some(at) => (
            rest[..at].trim(),
            Some(rest[at + 2..].trim_end_matches(">>").trim()),
        ),
        None => (rest, None),
    };
    // `state Id : description`
    let (id, description) = match id.split_once(':') {
        Some((i, desc)) => (i.trim(), Some(breaks(desc.trim()))),
        None => (id, None),
    };
    let id = id.split(":::").next().unwrap_or(id).trim();
    if !is_id(id) {
        return Err(err(n, format!("`{id}` is not a state name")));
    }
    ensure(d, id, scope);
    let Some(s) = d.states.iter_mut().find(|s| s.id == id) else {
        return Ok(());
    };
    if label.is_some() {
        s.label = label;
    }
    s.descriptions.extend(description);
    s.kind = match (stereo, opens) {
        (_, true) => Kind::Composite,
        (Some("choice"), _) => Kind::Choice,
        (Some("fork"), _) => Kind::Fork,
        (Some("join"), _) => Kind::Join,
        _ => s.kind,
    };
    if opens {
        scopes.push((n, id.to_owned()));
    }
    Ok(())
}

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

fn ensure(d: &mut StateDiagram, id: &str, scope: &Option<String>) {
    if !d.states.iter().any(|s| s.id == id) {
        d.states.push(State {
            id: id.to_owned(),
            parent: scope.clone(),
            ..State::default()
        });
    }
}

fn point(d: &mut StateDiagram, word: &str, scope: &Option<String>, from: bool) -> Point {
    if word == "[*]" {
        if from {
            Point::Start(scope.clone())
        } else {
            Point::End(scope.clone())
        }
    } else {
        ensure(d, word, scope);
        Point::State(word.to_owned())
    }
}

/// `A --> B : label`
fn transition(t: &str) -> Option<(&str, &str, Option<String>)> {
    let (body, label) = match t.split_once(':') {
        Some((b, l)) => (b, Some(breaks(l.trim())).filter(|l| !l.is_empty())),
        None => (t, None),
    };
    let (from, to) = body.split_once("-->")?;
    let (from, to) = (strip_class(from.trim()), strip_class(to.trim()));
    let ok = |w: &str| w == "[*]" || is_id(w);
    (ok(from) && ok(to)).then_some((from, to, label))
}

/// `A:::cls` → `A`.
fn strip_class(w: &str) -> &str {
    w.split(":::").next().unwrap_or(w).trim()
}

/// `right of A : text` (one line) or `right of A` (until `end note`).
fn note_start(rest: &str) -> Option<(String, Option<String>)> {
    let rest = rest
        .strip_prefix("left of ")
        .or_else(|| rest.strip_prefix("right of "))?;
    match rest.split_once(':') {
        Some((state, text)) => Some((state.trim().to_owned(), Some(breaks(text.trim())))),
        None => Some((rest.trim().to_owned(), None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse as parse_text};

    fn states(text: &str) -> (StateDiagram, Vec<String>) {
        let p = parse_text(text).unwrap();
        match p.diagram {
            Diagram::State(d) => (d, p.warnings),
            _ => panic!("not a state diagram"),
        }
    }

    #[test]
    fn starts_ends_labels_and_composites() {
        let (d, w) = states(
            r#"stateDiagram-v2
    direction LR
    [*] --> Still
    Still --> [*]
    Still --> Moving : push
    Moving --> Still
    Moving --> Crash
    Crash --> [*]
    state "Waiting for payment" as Waiting
    Waiting : customer pays
    state Active {
        [*] --> Working
        Working --> Paused : pause
        Paused --> [*]
    }
    state if_state <<choice>>
    state fork_state <<fork>>
    note right of Still : idle
    note left of Moving
        goes fast
        and far
    end note
    Crash:::bad
"#,
        );
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(d.direction, Direction::LeftRight);
        let get = |id: &str| d.states.iter().find(|s| s.id == id).unwrap();
        assert_eq!(get("Waiting").name(), "Waiting for payment");
        assert_eq!(get("Waiting").descriptions, ["customer pays"]);
        assert_eq!(get("Active").kind, Kind::Composite);
        assert_eq!(get("Working").parent.as_deref(), Some("Active"));
        assert_eq!(get("Still").parent, None);
        assert_eq!(get("if_state").kind, Kind::Choice);
        assert_eq!(get("fork_state").kind, Kind::Fork);
        assert_eq!(d.transitions[0].from, Point::Start(None));
        assert_eq!(d.transitions[1].to, Point::End(None));
        assert_eq!(d.transitions[2].label.as_deref(), Some("push"));
        let inner = d
            .transitions
            .iter()
            .find(|t| t.to == Point::State("Working".into()))
            .unwrap();
        assert_eq!(inner.from, Point::Start(Some("Active".into())));
        assert_eq!(d.notes.len(), 2);
        assert_eq!(d.notes[1].text, "goes fast\nand far");
    }

    #[test]
    fn regions_warn_and_mistakes_fail() {
        let (_, w) = states("stateDiagram\nstate A {\nB --> C\n--\nD --> E\n}");
        assert_eq!(w.len(), 1);
        assert!(parse_text("stateDiagram\nstate A {\nB").is_err());
        assert!(parse_text("stateDiagram\n}").is_err());
        assert!(parse_text("stateDiagram\nnote left of A\ntext").is_err());
    }
}
