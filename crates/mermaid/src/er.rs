//! `erDiagram`.
//!
//! Entities with attribute blocks (`CUSTOMER { string name PK "comment" }`),
//! and relationships `A ||--o{ B : label` with crow's-foot markers or their
//! word forms (`A 1 to zero or more B : label`).

use crate::lines::{Line, is_styling};
use crate::{Direction, ParseError, err, unquote};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ErDiagram {
    pub direction: Direction,
    /// Every entity, in order of first mention.
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entity {
    pub id: String,
    /// `p[Person]` shows as "Person".
    pub label: Option<String>,
    pub attributes: Vec<Attribute>,
}

impl Entity {
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Attribute {
    pub ty: String,
    pub name: String,
    /// `PK`, `FK`, `UK`, as written.
    pub keys: Vec<String>,
    pub comment: Option<String>,
}

impl Attribute {
    pub fn has_key(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k.eq_ignore_ascii_case(key))
    }
}

/// How many of one entity relate to one of the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMore,
    OneOrMore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub a: String,
    pub b: String,
    /// The marker next to `a`.
    pub a_card: Cardinality,
    /// The marker next to `b`.
    pub b_card: Cardinality,
    /// `--` (or `to`); `..` (or `optionally to`) is non-identifying.
    pub identifying: bool,
    pub label: Option<String>,
}

pub(crate) fn parse(lines: &[Line], warnings: &mut Vec<String>) -> Result<ErDiagram, ParseError> {
    let mut d = ErDiagram::default();
    // The entity whose `{ … }` block is open, with the line that opened it.
    let mut open: Option<(usize, String)> = None;
    for line in lines {
        let (t, n) = (line.text.as_str(), line.number);
        if let Some((_, id)) = &open {
            let (body, closes) = match t.strip_suffix('}') {
                Some(b) => (b.trim(), true),
                None => (t, false),
            };
            if !body.is_empty() {
                match attribute(body) {
                    Some(a) => {
                        if let Some(e) = d.entities.iter_mut().find(|e| e.id == *id) {
                            e.attributes.push(a);
                        }
                    }
                    None => warnings.push(format!("line {n}: skipped `{body}`")),
                }
            }
            if closes {
                open = None;
            }
            continue;
        }
        if is_styling(t) || t.starts_with("class ") {
            continue;
        }
        if let Some(dir) = t.strip_prefix("direction ") {
            match Direction::parse(dir) {
                Some(dir) => d.direction = dir,
                None => warnings.push(format!("line {n}: skipped `{t}`")),
            }
            continue;
        }
        if let Some(r) = relationship(t) {
            ensure(&mut d, &r.0);
            ensure(&mut d, &r.1);
            d.relationships.push(Relationship {
                a: r.0.id,
                b: r.1.id,
                a_card: r.2,
                b_card: r.3,
                identifying: r.4,
                label: r.5,
            });
            continue;
        }
        // `NAME {`, `NAME { type attr }`, `alias[Label] {`, or a bare `NAME`.
        let (head, block) = match t.split_once('{') {
            Some((h, rest)) => (h.trim(), Some(rest.trim())),
            None => (t, None),
        };
        let Some(entity) = entity_ref(head) else {
            warnings.push(format!("line {n}: skipped `{t}`"));
            continue;
        };
        let id = entity.id.clone();
        ensure(&mut d, &entity);
        match block {
            None => {}
            Some(rest) => match rest.strip_suffix('}') {
                Some(one_line) => {
                    let one_line = one_line.trim();
                    if !one_line.is_empty() {
                        match attribute(one_line) {
                            Some(a) => {
                                if let Some(e) = d.entities.iter_mut().find(|e| e.id == id) {
                                    e.attributes.push(a);
                                }
                            }
                            None => warnings.push(format!("line {n}: skipped `{one_line}`")),
                        }
                    }
                }
                None if rest.is_empty() => open = Some((n, id)),
                None => return Err(err(n, "write each attribute on its own line")),
            },
        }
    }
    if let Some((n, id)) = open {
        return Err(err(n, format!("entity `{id}` is missing its closing `}}`")));
    }
    Ok(d)
}

/// Add the entity, or give a known one the label it now has.
fn ensure(d: &mut ErDiagram, r: &Entity) {
    match d.entities.iter_mut().find(|e| e.id == r.id) {
        Some(e) => {
            if r.label.is_some() {
                e.label.clone_from(&r.label);
            }
        }
        None => d.entities.push(Entity {
            id: r.id.clone(),
            label: r.label.clone(),
            attributes: Vec::new(),
        }),
    }
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// `CUSTOMER`, `"Customer Account"`, `p[Person]`, `p["A person"]`.
fn entity_ref(s: &str) -> Option<Entity> {
    let s = s.trim();
    let s = s.split(":::").next().unwrap_or(s).trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        let id = &s[1..s.len() - 1];
        return (!id.is_empty()).then(|| Entity {
            id: id.to_owned(),
            ..Entity::default()
        });
    }
    let (id, label) = match s.split_once('[') {
        Some((id, rest)) => (id.trim(), Some(unquote(rest.strip_suffix(']')?).to_owned())),
        None => (s, None),
    };
    (!id.is_empty() && id.chars().all(is_name_char)).then(|| Entity {
        id: id.to_owned(),
        label: label.filter(|l| !l.is_empty()),
        ..Entity::default()
    })
}

/// `type name [PK, FK] ["comment"]`.
fn attribute(s: &str) -> Option<Attribute> {
    let (s, comment) = match s.find('"') {
        Some(at) => (&s[..at], Some(unquote(&s[at..]).to_owned())),
        None => (s, None),
    };
    let mut words = s.split_whitespace();
    let ty = words.next()?.to_owned();
    let name = words.next()?.to_owned();
    let keys: Vec<String> = words
        .flat_map(|w| w.split(','))
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
        .collect();
    if keys
        .iter()
        .any(|k| !["PK", "FK", "UK"].contains(&k.to_ascii_uppercase().as_str()))
    {
        return None;
    }
    Some(Attribute {
        ty,
        name,
        keys,
        comment: comment.filter(|c| !c.is_empty()),
    })
}

type Parsed = (
    Entity,
    Entity,
    Cardinality,
    Cardinality,
    bool,
    Option<String>,
);

/// `A ||--o{ B : label` or `A 1 to zero or more B : label`.
fn relationship(t: &str) -> Option<Parsed> {
    let (body, label) = match t.split_once(" :").or_else(|| t.split_once(':')) {
        Some((b, l)) => (
            b.trim(),
            Some(unquote(l).to_owned()).filter(|l| !l.is_empty()),
        ),
        None => (t.trim(), None),
    };
    symbolic(body)
        .or_else(|| worded(body))
        .map(|(a, b, ca, cb, ident)| (a, b, ca, cb, ident, label))
}

fn left_marker(m: &str) -> Option<Cardinality> {
    Some(match m {
        "|o" => Cardinality::ZeroOrOne,
        "||" => Cardinality::ExactlyOne,
        "}o" => Cardinality::ZeroOrMore,
        "}|" => Cardinality::OneOrMore,
        _ => return None,
    })
}

fn right_marker(m: &str) -> Option<Cardinality> {
    Some(match m {
        "o|" => Cardinality::ZeroOrOne,
        "||" => Cardinality::ExactlyOne,
        "o{" => Cardinality::ZeroOrMore,
        "|{" => Cardinality::OneOrMore,
        _ => return None,
    })
}

/// `A ||--o{ B`: two marker characters, `--` or `..`, two more.
fn symbolic(body: &str) -> Option<(Entity, Entity, Cardinality, Cardinality, bool)> {
    let at = body.find("--").or_else(|| body.find(".."))?;
    let identifying = body[at..].starts_with("--");
    let (left, right) = (body[..at].trim_end(), body[at + 2..].trim_start());
    // The markers are the two characters on each side.
    let cut = left.char_indices().rev().nth(1)?.0;
    let (a, lm) = left.split_at(cut);
    let cut = right.char_indices().nth(2).map_or(right.len(), |(i, _)| i);
    let (rm, b) = right.split_at(cut);
    Some((
        entity_ref(a)?,
        entity_ref(b)?,
        left_marker(lm)?,
        right_marker(rm)?,
        identifying,
    ))
}

/// Mermaid's word forms for each side of `to`.
fn worded_card(s: &str) -> Option<Cardinality> {
    Some(match s.trim() {
        "one or zero" | "zero or one" => Cardinality::ZeroOrOne,
        "one or more" | "one or many" | "many(1)" | "1+" => Cardinality::OneOrMore,
        "zero or more" | "zero or many" | "many(0)" | "0+" => Cardinality::ZeroOrMore,
        "only one" | "1" => Cardinality::ExactlyOne,
        _ => return None,
    })
}

const WORD_CARDS: &[&str] = &[
    "one or zero",
    "zero or one",
    "one or more",
    "one or many",
    "zero or more",
    "zero or many",
    "only one",
    "many(1)",
    "many(0)",
    "1+",
    "0+",
    "1",
];

/// `A 1 to zero or more B` (`optionally to` is non-identifying).
fn worded(body: &str) -> Option<(Entity, Entity, Cardinality, Cardinality, bool)> {
    let (left, right, identifying) = match body.split_once(" optionally to ") {
        Some((l, r)) => (l, r, false),
        None => {
            let (l, r) = body.split_once(" to ")?;
            (l, r, true)
        }
    };
    let (a, ca) = WORD_CARDS.iter().find_map(|w| {
        let a = left.trim_end().strip_suffix(w)?;
        a.ends_with(' ').then(|| (a.trim(), *w))
    })?;
    let (cb, b) = WORD_CARDS.iter().find_map(|w| {
        let b = right.trim_start().strip_prefix(w)?;
        b.starts_with(' ').then(|| (*w, b.trim()))
    })?;
    Some((
        entity_ref(a)?,
        entity_ref(b)?,
        worded_card(ca)?,
        worded_card(cb)?,
        identifying,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse as parse_text};

    fn er(text: &str) -> (ErDiagram, Vec<String>) {
        let p = parse_text(text).unwrap();
        match p.diagram {
            Diagram::Er(d) => (d, p.warnings),
            _ => panic!("not an ER diagram"),
        }
    }

    #[test]
    fn mermaid_docs_example() {
        let (d, w) = er(r#"erDiagram
    direction LR
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
    CUSTOMER }|..|{ DELIVERY-ADDRESS : uses
    CUSTOMER {
        string name
        string custNumber PK "the customer's number"
        int id PK, FK
        string email UK
    }
    p[Person] {
        varchar(255) firstName
    }
    "Customer Account" { string login }
"#);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(d.direction, Direction::LeftRight);
        let ids: Vec<&str> = d.entities.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "CUSTOMER",
                "ORDER",
                "LINE-ITEM",
                "DELIVERY-ADDRESS",
                "p",
                "Customer Account"
            ]
        );
        let r = &d.relationships;
        assert_eq!(
            (r[0].a_card, r[0].b_card, r[0].identifying),
            (Cardinality::ExactlyOne, Cardinality::ZeroOrMore, true)
        );
        assert_eq!(r[0].label.as_deref(), Some("places"));
        assert_eq!(r[1].b_card, Cardinality::OneOrMore);
        assert_eq!(
            (r[2].a_card, r[2].b_card, r[2].identifying),
            (Cardinality::OneOrMore, Cardinality::OneOrMore, false)
        );
        let c = &d.entities[0].attributes;
        assert_eq!(c.len(), 4);
        assert_eq!(
            (c[1].name.as_str(), c[1].ty.as_str()),
            ("custNumber", "string")
        );
        assert!(c[1].has_key("PK"));
        assert_eq!(c[1].comment.as_deref(), Some("the customer's number"));
        assert!(c[2].has_key("PK") && c[2].has_key("FK"));
        assert!(c[3].has_key("UK"));
        assert_eq!(d.entities[4].name(), "Person");
        assert_eq!(d.entities[4].attributes[0].ty, "varchar(255)");
        assert_eq!(d.entities[5].attributes[0].name, "login");
    }

    #[test]
    fn word_forms_and_unlabelled() {
        let (d, w) = er("erDiagram
    CAR 1 to zero or more NAMED-DRIVER : allows
    PERSON many(0) optionally to 1+ CAR : drives
    A |o--o| B");
        assert!(w.is_empty(), "{w:?}");
        let r = &d.relationships;
        assert_eq!((r[0].a.as_str(), r[0].b.as_str()), ("CAR", "NAMED-DRIVER"));
        assert_eq!(
            (r[0].a_card, r[0].b_card, r[0].identifying),
            (Cardinality::ExactlyOne, Cardinality::ZeroOrMore, true)
        );
        assert_eq!(
            (r[1].a_card, r[1].b_card, r[1].identifying),
            (Cardinality::ZeroOrMore, Cardinality::OneOrMore, false)
        );
        assert_eq!(
            (r[2].a_card, r[2].b_card, r[2].label.clone()),
            (Cardinality::ZeroOrOne, Cardinality::ZeroOrOne, None)
        );
    }

    #[test]
    fn mistakes_warn_or_fail() {
        let (_, w) = er("erDiagram\nA {\n  string\n  int id XK\n}\n???");
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(parse_text("erDiagram\nA {\nstring name").is_err());
        assert!(parse_text("erDiagram\nA { string name\n}").is_err());
        for t in [
            "A ||--",
            "--o{ B",
            "A ||--o{",
            "\"\" {",
            "A[ {",
            "A ||..x| B",
            "a€--b",
            "€--€",
            "--",
        ] {
            let _ = parse_text(&format!("erDiagram\n{t}"));
        }
    }
}
