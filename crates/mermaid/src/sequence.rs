//! `sequenceDiagram`.

use crate::lines::{Line, is_styling};
use crate::{ParseError, breaks, err, unquote};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SequenceDiagram {
    /// In order of first mention.
    pub participants: Vec<Participant>,
    pub events: Vec<Event>,
    /// `autonumber` was given.
    pub autonumber: bool,
    /// `activate`/`deactivate` or `+`/`-` were used.
    pub activations: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Participant {
    pub id: String,
    /// `participant A as Alice`
    pub label: Option<String>,
    pub actor: bool,
}

impl Participant {
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }
}

/// The arrow of a message, as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    /// `->`
    Solid,
    /// `-->`
    Dotted,
    /// `->>`
    SolidHead,
    /// `-->>`
    DottedHead,
    /// `-x`
    SolidCross,
    /// `--x`
    DottedCross,
    /// `-)`
    SolidAsync,
    /// `--)`
    DottedAsync,
    /// `<<->>`
    BothSolid,
    /// `<<-->>`
    BothDotted,
}

/// Longest first, so `-->>` isn't read as `-->`.
const ARROWS: &[(&str, Arrow)] = &[
    ("<<-->>", Arrow::BothDotted),
    ("<<->>", Arrow::BothSolid),
    ("-->>", Arrow::DottedHead),
    ("->>", Arrow::SolidHead),
    ("--x", Arrow::DottedCross),
    ("--)", Arrow::DottedAsync),
    ("-->", Arrow::Dotted),
    ("-x", Arrow::SolidCross),
    ("-)", Arrow::SolidAsync),
    ("->", Arrow::Solid),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
}

impl Block {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "loop" => Self::Loop,
            "alt" => Self::Alt,
            "opt" => Self::Opt,
            "par" => Self::Par,
            "critical" => Self::Critical,
            "break" => Self::Break,
            _ => return None,
        })
    }

    /// The word that starts another section: `else`, `and`, `option`.
    fn section_word(self) -> Option<&'static str> {
        match self {
            Self::Alt => Some("else"),
            Self::Par => Some("and"),
            Self::Critical => Some("option"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePlace {
    LeftOf,
    RightOf,
    Over,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Message {
        from: String,
        to: String,
        arrow: Arrow,
        text: String,
        /// `create participant` came just before for `to`.
        creates: bool,
        /// `destroy` came just before for `to`.
        destroys: bool,
    },
    Note {
        place: NotePlace,
        /// One participant, or two for `note over A,B`.
        of: Vec<String>,
        text: String,
    },
    /// `loop …`, `alt …`, …
    Begin {
        block: Block,
        label: String,
    },
    /// `else …`, `and …`, `option …`
    Section {
        label: String,
    },
    End,
}

/// Blocks Mermaid draws but UML has no counterpart for: their contents count,
/// the block itself is dropped.
const TRANSPARENT: &[&str] = &["rect", "box"];

pub(crate) fn parse(
    lines: &[Line],
    warnings: &mut Vec<String>,
) -> Result<(SequenceDiagram, Option<String>), ParseError> {
    let mut d = SequenceDiagram::default();
    let mut title = None;
    // What each open `end` closes: a block (true) or a transparent one (false).
    let mut open: Vec<(usize, Option<Block>)> = Vec::new();
    let mut pending_create: Option<String> = None;
    let mut pending_destroy: Vec<String> = Vec::new();
    for line in lines {
        let (t, n) = (line.text.as_str(), line.number);
        let first = t.split_whitespace().next().unwrap_or_default();
        let rest = t[first.len()..].trim();
        if is_styling(t) {
            continue;
        }
        match first {
            "title" | "title:" => title = Some(unquote(rest.trim_start_matches(':')).to_owned()),
            "autonumber" => d.autonumber = true,
            "participant" | "actor" => {
                participant(&mut d, rest, first == "actor");
            }
            "create" => {
                let mut words = rest.splitn(2, char::is_whitespace);
                let kind = words.next().unwrap_or_default();
                let decl = words.next().unwrap_or_default();
                if !["participant", "actor"].contains(&kind) || decl.is_empty() {
                    return Err(err(n, "write `create participant Name`"));
                }
                let id = participant(&mut d, decl, kind == "actor");
                pending_create = Some(id);
            }
            "destroy" => pending_destroy.push(rest.to_owned()),
            "activate" | "deactivate" => d.activations = true,
            "note" | "Note" => match note(rest) {
                Some(ev) => {
                    if let Event::Note { of, .. } = &ev {
                        for id in of {
                            ensure(&mut d, id);
                        }
                    }
                    d.events.push(ev);
                }
                None => warnings.push(format!("line {n}: skipped `{t}`")),
            },
            "end" if rest.is_empty() => {
                let Some((_, block)) = open.pop() else {
                    return Err(err(n, "`end` without an open block"));
                };
                if block.is_some() {
                    d.events.push(Event::End);
                }
            }
            w if TRANSPARENT.contains(&w) => open.push((n, None)),
            w if Block::parse(w).is_some() => {
                let block = Block::parse(w).unwrap_or(Block::Opt);
                open.push((n, Some(block)));
                d.events.push(Event::Begin {
                    block,
                    label: breaks(rest),
                });
            }
            w if ["else", "and", "option"].contains(&w) => {
                let inner = open.iter().rev().find_map(|(_, b)| *b);
                if inner.and_then(Block::section_word) != Some(w) {
                    return Err(err(n, format!("`{w}` outside a matching block")));
                }
                d.events.push(Event::Section {
                    label: breaks(rest),
                });
            }
            _ => match message(t) {
                Some((from, to, arrow, activate, text)) => {
                    if activate {
                        d.activations = true;
                    }
                    ensure(&mut d, &from);
                    ensure(&mut d, &to);
                    let creates = pending_create.as_deref() == Some(to.as_str());
                    if creates {
                        pending_create = None;
                    }
                    let destroys = match pending_destroy.iter().position(|x| *x == to) {
                        Some(i) => {
                            pending_destroy.remove(i);
                            true
                        }
                        None => false,
                    };
                    d.events.push(Event::Message {
                        from,
                        to,
                        arrow,
                        text,
                        creates,
                        destroys,
                    });
                }
                None => warnings.push(format!("line {n}: skipped `{t}`")),
            },
        }
    }
    if let Some((n, _)) = open.last() {
        return Err(err(*n, "this block is missing its `end`"));
    }
    Ok((d, title))
}

/// Declare a participant (`A`, `A as Alice`); returns its id.
fn participant(d: &mut SequenceDiagram, decl: &str, actor: bool) -> String {
    // `participant A@{ "type": "database" }` – drop the config.
    let decl = decl.split("@{").next().unwrap_or(decl).trim();
    let (id, label) = match decl.split_once(" as ") {
        Some((id, label)) => (id.trim(), Some(breaks(unquote(label)))),
        None => (decl, None),
    };
    let id = unquote(id).to_owned();
    match d.participants.iter_mut().find(|p| p.id == id) {
        Some(p) => {
            if label.is_some() {
                p.label = label;
            }
            p.actor |= actor;
        }
        None => d.participants.push(Participant {
            id: id.clone(),
            label,
            actor,
        }),
    }
    id
}

fn ensure(d: &mut SequenceDiagram, id: &str) {
    if !d.participants.iter().any(|p| p.id == id) {
        d.participants.push(Participant {
            id: id.to_owned(),
            ..Participant::default()
        });
    }
}

/// `A->>+B: text` → (from, to, arrow, uses activation, text).
fn message(t: &str) -> Option<(String, String, Arrow, bool, String)> {
    let (head, text) = match t.split_once(':') {
        Some((h, x)) => (h.trim(), breaks(x.trim())),
        None => (t.trim(), String::new()),
    };
    let (at, (sym, arrow)) = (0..head.len())
        .filter(|i| head.is_char_boundary(*i))
        .find_map(|i| {
            ARROWS
                .iter()
                .find(|(s, _)| head[i..].starts_with(s))
                .map(|a| (i, *a))
        })?;
    let from = head[..at].trim();
    let mut to = head[at + sym.len()..].trim();
    let mut activate = false;
    if let Some(r) = to.strip_prefix(['+', '-']) {
        activate = true;
        to = r.trim();
    }
    if from.is_empty() || to.is_empty() || to.contains(char::is_whitespace) {
        return None;
    }
    Some((
        unquote(from).to_owned(),
        unquote(to).to_owned(),
        arrow,
        activate,
        text,
    ))
}

/// `right of A: text`, `over A,B: text`.
fn note(rest: &str) -> Option<Event> {
    let (place, rest) = if let Some(r) = rest.strip_prefix("left of ") {
        (NotePlace::LeftOf, r)
    } else if let Some(r) = rest.strip_prefix("right of ") {
        (NotePlace::RightOf, r)
    } else {
        (NotePlace::Over, rest.strip_prefix("over ")?)
    };
    let (who, text) = rest.split_once(':')?;
    let of: Vec<String> = who
        .split(',')
        .map(|w| unquote(w).to_owned())
        .filter(|w| !w.is_empty())
        .collect();
    if of.is_empty() || of.len() > 2 {
        return None;
    }
    Some(Event::Note {
        place,
        of,
        text: breaks(text.trim()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse as parse_text};

    fn seq(text: &str) -> (SequenceDiagram, Vec<String>) {
        let p = parse_text(text).unwrap();
        match p.diagram {
            Diagram::Sequence(d) => (d, p.warnings),
            _ => panic!("not a sequence diagram"),
        }
    }

    #[test]
    fn participants_messages_and_arrows() {
        let (d, w) = seq("sequenceDiagram
    autonumber
    participant A as Alice
    actor J as John
    A->>+J: Hello John, how are you?
    J-->>-A: Great!
    A-)J: async
    A--)J: dotted async
    A-xJ: cross
    A->A: think
    B->>A: implicit");
        assert!(w.is_empty(), "{w:?}");
        assert!(d.autonumber && d.activations);
        let names: Vec<&str> = d.participants.iter().map(Participant::name).collect();
        assert_eq!(names, ["Alice", "John", "B"]);
        assert!(d.participants[1].actor);
        let arrows: Vec<Arrow> = d
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Message { arrow, .. } => Some(*arrow),
                _ => None,
            })
            .collect();
        use Arrow::*;
        assert_eq!(
            arrows,
            [
                SolidHead,
                DottedHead,
                SolidAsync,
                DottedAsync,
                SolidCross,
                Solid,
                SolidHead
            ]
        );
        let Event::Message { from, to, text, .. } = &d.events[0] else {
            panic!()
        };
        assert_eq!((from.as_str(), to.as_str()), ("A", "J"));
        assert_eq!(text, "Hello John, how are you?");
    }

    #[test]
    fn blocks_notes_create_destroy() {
        let (d, w) = seq("sequenceDiagram
    alt is sick
        Bob->>Alice: Not so good :(
    else is well
        Bob->>Alice: Feeling fresh
    end
    rect rgb(0, 0, 255)
    loop Every minute
        Alice->>Bob: ping
    end
    end
    note over Alice,Bob: A typical interaction
    Note right of Bob: thinks
    create participant Carl
    Alice->>Carl: Hi Carl!
    destroy Carl
    Alice-xCarl: We are too many
    gibberish line");
        assert_eq!(w.len(), 1);
        let kinds: Vec<&str> = d
            .events
            .iter()
            .map(|e| match e {
                Event::Message { .. } => "msg",
                Event::Note { .. } => "note",
                Event::Begin { .. } => "begin",
                Event::Section { .. } => "section",
                Event::End => "end",
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "begin", "msg", "section", "msg", "end", "begin", "msg", "end", "note", "note",
                "msg", "msg"
            ]
        );
        let Event::Message { text, .. } = &d.events[1] else {
            panic!()
        };
        assert_eq!(text, "Not so good :(");
        let Event::Message { creates, .. } = &d.events[10] else {
            panic!()
        };
        assert!(*creates);
        let Event::Message { destroys, .. } = &d.events[11] else {
            panic!()
        };
        assert!(*destroys);
        let Event::Note { of, place, .. } = &d.events[8] else {
            panic!()
        };
        assert_eq!((of.len(), *place), (2, NotePlace::Over));
    }

    #[test]
    fn structural_mistakes_are_errors() {
        assert!(parse_text("sequenceDiagram\nloop x\nA->>B: y").is_err());
        assert!(parse_text("sequenceDiagram\nend").is_err());
        assert!(parse_text("sequenceDiagram\nelse x").is_err());
        assert!(parse_text("sequenceDiagram\nloop x\nelse y\nend").is_err());
    }
}
