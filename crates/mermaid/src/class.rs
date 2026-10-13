//! `classDiagram`.

use crate::lines::{Line, is_styling};
use crate::{Direction, ParseError, breaks, err, generics, unquote};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassDiagram {
    pub direction: Direction,
    /// In order of first mention.
    pub classes: Vec<Class>,
    pub relations: Vec<Relation>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Class {
    /// The id used in the text.
    pub id: String,
    /// `class A["Label"]`; the name shown is the label when given.
    pub label: Option<String>,
    /// `<<interface>>`, `<<enumeration>>`, `<<abstract>>`, `<<service>>`, … as written.
    pub annotations: Vec<String>,
    /// `class Square~Shape~` → `Shape`.
    pub generic: Option<String>,
    /// `namespace N { … }`, innermost.
    pub namespace: Option<String>,
    pub members: Vec<Member>,
}

impl Class {
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }

    pub fn has_annotation(&self, a: &str) -> bool {
        self.annotations.iter().any(|x| x.eq_ignore_ascii_case(a))
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Member {
    /// `+`, `-`, `#` or `~`.
    pub visibility: Option<char>,
    pub name: String,
    /// Attribute type or method return type.
    pub ty: Option<String>,
    /// `Some` for methods (possibly empty).
    pub params: Option<Vec<Param>>,
    pub is_static: bool,
    pub is_abstract: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Param {
    pub name: String,
    pub ty: Option<String>,
}

/// What an end of a relation line looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum End {
    #[default]
    Plain,
    /// `<` or `>`
    Arrow,
    /// `<|` or `|>`
    Triangle,
    /// `*`
    Composition,
    /// `o`
    Aggregation,
    /// `()`
    Lollipop,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Relation {
    pub left: String,
    pub right: String,
    pub left_end: End,
    pub right_end: End,
    /// `..` instead of `--`.
    pub dotted: bool,
    pub label: Option<String>,
    /// `A "1" --> "*" B`
    pub left_card: Option<String>,
    pub right_card: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Note {
    /// `note for A "…"`; `None` for a free-standing note.
    pub class: Option<String>,
    pub text: String,
}

struct Parser<'w> {
    d: ClassDiagram,
    title: Option<String>,
    warnings: &'w mut Vec<String>,
    namespaces: Vec<String>,
}

pub(crate) fn parse(
    lines: &[Line],
    warnings: &mut Vec<String>,
) -> Result<(ClassDiagram, Option<String>), ParseError> {
    let mut p = Parser {
        d: ClassDiagram::default(),
        title: None,
        warnings,
        namespaces: Vec::new(),
    };
    let mut open_class: Option<(usize, String)> = None;
    for line in lines {
        let t = line.text.as_str();
        let n = line.number;
        if let Some((_, id)) = &open_class {
            let id = id.clone();
            if t == "}" {
                open_class = None;
            } else if let Some(body) = t.strip_suffix('}') {
                p.member_or_annotation(&id, body.trim(), n);
                open_class = None;
            } else {
                p.member_or_annotation(&id, t, n);
            }
            continue;
        }
        p.statement(t, n, &mut open_class)?;
    }
    if let Some((n, id)) = open_class {
        return Err(err(n, format!("class `{id}` is missing its closing `}}`")));
    }
    if !p.namespaces.is_empty() {
        return Err(err(0, "a namespace is missing its closing `}`"));
    }
    Ok((p.d, p.title))
}

impl Parser<'_> {
    fn warn(&mut self, n: usize, what: &str) {
        self.warnings.push(format!("line {n}: skipped `{what}`"));
    }

    fn class(&mut self, id: &str) -> &mut Class {
        let i = match self.d.classes.iter().position(|c| c.id == id) {
            Some(i) => i,
            None => {
                self.d.classes.push(Class {
                    id: id.to_owned(),
                    namespace: self.namespaces.last().cloned(),
                    ..Class::default()
                });
                self.d.classes.len() - 1
            }
        };
        &mut self.d.classes[i]
    }

    fn statement(
        &mut self,
        t: &str,
        n: usize,
        open_class: &mut Option<(usize, String)>,
    ) -> Result<(), ParseError> {
        if is_styling(t) {
            return Ok(());
        }
        if let Some(d) = t.strip_prefix("direction ") {
            match Direction::parse(d) {
                Some(d) => self.d.direction = d,
                None => self.warn(n, t),
            }
            return Ok(());
        }
        if let Some(title) = t.strip_prefix("title ") {
            self.title = Some(unquote(title).to_owned());
            return Ok(());
        }
        if t == "}" {
            if self.namespaces.pop().is_none() {
                return Err(err(n, "`}` without an open class or namespace"));
            }
            return Ok(());
        }
        if let Some(rest) = t.strip_prefix("namespace ") {
            let name = rest.trim_end_matches('{').trim();
            if name.is_empty() || !rest.trim_end().ends_with('{') {
                return Err(err(n, "write `namespace Name {`"));
            }
            self.namespaces.push(unquote(name).to_owned());
            return Ok(());
        }
        if let Some(rest) = t.strip_prefix("class ") {
            return self.class_declaration(rest, n, open_class);
        }
        if let Some(rest) = t.strip_prefix("note ") {
            self.note(rest, n);
            return Ok(());
        }
        if t.starts_with("<<") {
            // `<<interface>> Shape`
            if let Some(end) = t.find(">>") {
                let annotation = t[2..end].trim().to_owned();
                let id = t[end + 2..].trim();
                if !id.is_empty() {
                    let id = class_id(id);
                    self.class(&id).annotations.push(annotation);
                    return Ok(());
                }
            }
            self.warn(n, t);
            return Ok(());
        }
        if let Some(r) = relation(t) {
            for id in [r.left.clone(), r.right.clone()] {
                self.class(&id);
            }
            self.d.relations.push(r);
            return Ok(());
        }
        // `Shape : +area() double`
        if let Some((id, member)) = t.split_once(':') {
            let id = id.trim();
            if is_id(id) {
                let id = class_id(id);
                self.member_or_annotation(&id, member.trim(), n);
                return Ok(());
            }
        }
        // A bare id declares a class.
        if is_id(t) {
            self.class(&class_id(t));
            return Ok(());
        }
        self.warn(n, t);
        Ok(())
    }

    fn class_declaration(
        &mut self,
        rest: &str,
        n: usize,
        open_class: &mut Option<(usize, String)>,
    ) -> Result<(), ParseError> {
        let (head, body) = match rest.find('{') {
            Some(at) => (rest[..at].trim(), Some(rest[at + 1..].trim())),
            None => (rest.trim(), None),
        };
        // Drop `:::cssClass`.
        let head = head.split(":::").next().unwrap_or_default().trim();
        let (head, annotation) = match head.find("<<") {
            Some(at) => (
                head[..at].trim(),
                head[at + 2..]
                    .split(">>")
                    .next()
                    .map(|a| a.trim().to_owned()),
            ),
            None => (head, None),
        };
        let (head, label) = match head.find('[') {
            Some(at) => (
                head[..at].trim(),
                Some(unquote(head[at + 1..].trim_end_matches(']')).to_owned()),
            ),
            None => (head, None),
        };
        let (id, generic) = match head.split_once('~') {
            Some((id, g)) => (id.trim(), Some(generics(g.trim_end_matches('~')))),
            None => (head, None),
        };
        if id.is_empty() {
            return Err(err(n, "`class` needs a name"));
        }
        let id = class_id(id);
        {
            let c = self.class(&id);
            if label.is_some() {
                c.label = label;
            }
            if generic.is_some() {
                c.generic = generic;
            }
            c.annotations.extend(annotation);
        }
        match body {
            None => {}
            Some(b) if b.ends_with('}') => {
                // `class A { +x }` on one line
                let inner = b.trim_end_matches('}').trim();
                for part in inner.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                    self.member_or_annotation(&id, part, n);
                }
            }
            Some(b) => {
                if !b.is_empty() {
                    self.member_or_annotation(&id, b, n);
                }
                *open_class = Some((n, id));
            }
        }
        Ok(())
    }

    fn member_or_annotation(&mut self, id: &str, t: &str, n: usize) {
        if t.is_empty() {
            return;
        }
        if let Some(a) = t.strip_prefix("<<") {
            let a = a.trim_end_matches(">>").trim().to_owned();
            self.class(id).annotations.push(a);
            return;
        }
        match member(t) {
            Some(m) => self.class(id).members.push(m),
            None => self.warn(n, t),
        }
    }

    fn note(&mut self, rest: &str, n: usize) {
        let rest = rest.trim();
        if let Some(r) = rest.strip_prefix("for ") {
            let r = r.trim();
            let (id, text) = match r.find(|c: char| c.is_whitespace()) {
                Some(at) => (&r[..at], r[at..].trim()),
                None => (r, ""),
            };
            let id = class_id(id);
            self.class(&id);
            self.d.notes.push(Note {
                class: Some(id),
                text: breaks(unquote(text)),
            });
        } else if rest.starts_with('"') {
            self.d.notes.push(Note {
                class: None,
                text: breaks(unquote(rest)),
            });
        } else {
            self.warn(n, &format!("note {rest}"));
        }
    }
}

/// Class ids are names, or anything in backticks.
fn is_id(s: &str) -> bool {
    let s = s.trim();
    (s.len() > 2 && s.starts_with('`') && s.ends_with('`'))
        || (!s.is_empty()
            && s.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '~'))
}

/// `Square~Shape~` → `Square`; backticks removed.
fn class_id(s: &str) -> String {
    let s = unquote(s.trim());
    s.split('~').next().unwrap_or(s).trim().to_owned()
}

/// Parse a relation line, e.g. `A "1" *-- "many" B : has`.
pub(crate) fn relation(t: &str) -> Option<Relation> {
    let (body, label) = split_label(t);
    let tokens = tokenize(body);
    let at = tokens.iter().position(|w| is_arrow(w))?;
    let (left, arrow, right) = (&tokens[..at], &tokens[at], &tokens[at + 1..]);
    let (left_id, left_card) = match left {
        [id] => (id, None),
        [id, card] if card.starts_with('"') => (id, Some(unquote(card).to_owned())),
        _ => return None,
    };
    let (right_id, right_card) = match right {
        [id] => (id, None),
        [card, id] if card.starts_with('"') => (id, Some(unquote(card).to_owned())),
        _ => return None,
    };
    if !is_id(left_id) || !is_id(right_id) {
        return None;
    }
    let dotted = arrow.contains("..");
    let core = if dotted { ".." } else { "--" };
    let start = arrow.find(core)?;
    let end = arrow.rfind(core)? + 2;
    let left_end = end_of(&arrow[..start])?;
    let right_end = end_of(&arrow[end..])?;
    Some(Relation {
        left: class_id(left_id),
        right: class_id(right_id),
        left_end,
        right_end,
        dotted,
        label: label.map(|l| breaks(unquote(l))),
        left_card,
        right_card,
    })
}

fn end_of(s: &str) -> Option<End> {
    Some(match s {
        "" => End::Plain,
        "<" | ">" => End::Arrow,
        "<|" | "|>" => End::Triangle,
        "*" => End::Composition,
        "o" => End::Aggregation,
        "()" => End::Lollipop,
        _ => return None,
    })
}

fn is_arrow(w: &str) -> bool {
    (w.contains("--") || w.contains(".."))
        && w.chars().all(|c| "<>|*o().-".contains(c))
        && !w.starts_with('"')
}

/// Split `body : label` at the first `:` outside quotes.
fn split_label(t: &str) -> (&str, Option<&str>) {
    let mut quoted = false;
    for (i, c) in t.char_indices() {
        match c {
            '"' | '`' => quoted = !quoted,
            ':' if !quoted => {
                let label = t[i + 1..].trim();
                return (t[..i].trim(), (!label.is_empty()).then_some(label));
            }
            _ => {}
        }
    }
    (t.trim(), None)
}

/// Whitespace-separated words, keeping quoted parts (with their quotes) whole.
fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '`' => {
                if !cur.is_empty() && !cur.ends_with(['"', '`']) {
                    // `A"1"` – a card glued to a name.
                    out.push(std::mem::take(&mut cur));
                }
                quote = Some(c);
                cur.push(c);
            }
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parse a member: `+String owner`, `-id: int`, `+deposit(amount) bool`,
/// `someMethod()$`, `+area()* double`.
pub(crate) fn member(t: &str) -> Option<Member> {
    let mut t = t.trim();
    let mut m = Member::default();
    if let Some(c) = t.chars().next().filter(|c| "+-#~".contains(*c)) {
        m.visibility = Some(c);
        t = t[1..].trim_start();
    }
    if t.is_empty() {
        return None;
    }
    if let (Some(open), Some(close)) = (t.find('('), t.rfind(')'))
        && open < close
    {
        m.name = t[..open].trim().to_owned();
        let params = t[open + 1..close].trim();
        m.params = Some(
            params
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(param)
                .collect(),
        );
        let mut after = t[close + 1..].trim();
        loop {
            if let Some(a) = after.strip_prefix('$') {
                m.is_static = true;
                after = a.trim_start();
            } else if let Some(a) = after.strip_prefix('*') {
                m.is_abstract = true;
                after = a.trim_start();
            } else {
                break;
            }
        }
        let after = after.trim_start_matches(':').trim();
        let after = after.strip_suffix('$').map_or(after, |a| {
            m.is_static = true;
            a.trim_end()
        });
        let after = after.strip_suffix('*').map_or(after, |a| {
            m.is_abstract = true;
            a.trim_end()
        });
        if !after.is_empty() {
            m.ty = Some(generics(after));
        }
    } else {
        if let Some(a) = t.strip_suffix('$') {
            m.is_static = true;
            t = a.trim_end();
        }
        let p = param(t);
        m.name = p.name;
        m.ty = p.ty;
    }
    if m.name.is_empty() {
        return None;
    }
    m.name = generics(&m.name);
    Some(m)
}

/// `name`, `Type name` or `name: Type`.
fn param(p: &str) -> Param {
    let p = p.trim();
    if let Some((name, ty)) = p.split_once(':') {
        return Param {
            name: name.trim().to_owned(),
            ty: Some(generics(ty.trim())).filter(|t| !t.is_empty()),
        };
    }
    match p.rsplit_once(char::is_whitespace) {
        Some((ty, name)) => Param {
            name: name.trim().to_owned(),
            ty: Some(generics(ty.trim())),
        },
        None => Param {
            name: p.to_owned(),
            ty: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse as parse_text};

    fn class_diagram(text: &str) -> (ClassDiagram, Vec<String>) {
        let p = parse_text(text).unwrap();
        match p.diagram {
            Diagram::Class(d) => (d, p.warnings),
            _ => panic!("not a class diagram"),
        }
    }

    #[test]
    fn mermaid_docs_animal_example() {
        let (d, warnings) = class_diagram(
            r#"classDiagram
    note "From Duck till Zebra"
    Animal <|-- Duck
    note for Duck "can fly\ncan swim"
    Animal <|-- Fish
    Animal <|-- Zebra
    Animal : +int age
    Animal : +String gender
    Animal: +isMammal()
    Animal: +mate()
    class Duck{
        +String beakColor
        +swim()
        +quack()
    }
    class Fish{
        -int sizeInFeet
        -canEat()
    }
    class Zebra{
        +bool is_wild
        +run()
    }
"#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        let names: Vec<&str> = d.classes.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(names, ["Animal", "Duck", "Fish", "Zebra"]);
        assert_eq!(d.relations.len(), 3);
        assert_eq!(d.relations[0].left_end, End::Triangle);
        assert_eq!(d.relations[0].right, "Duck");
        let animal = &d.classes[0];
        assert_eq!(animal.members.len(), 4);
        assert_eq!(animal.members[0].name, "age");
        assert_eq!(animal.members[0].ty.as_deref(), Some("int"));
        assert_eq!(animal.members[0].visibility, Some('+'));
        assert!(animal.members[2].params.is_some());
        assert_eq!(d.classes[2].members[0].visibility, Some('-'));
        assert_eq!(d.notes.len(), 2);
        assert_eq!(d.notes[1].class.as_deref(), Some("Duck"));
        assert_eq!(d.notes[1].text, "can fly\ncan swim");
    }

    #[test]
    fn relations_with_cards_labels_and_every_end() {
        let r = relation(r#"Customer "1" --> "*" Ticket : buys"#).unwrap();
        assert_eq!((r.left.as_str(), r.right.as_str()), ("Customer", "Ticket"));
        assert_eq!(r.left_card.as_deref(), Some("1"));
        assert_eq!(r.right_card.as_deref(), Some("*"));
        assert_eq!(r.right_end, End::Arrow);
        assert_eq!(r.label.as_deref(), Some("buys"));
        let cases = [
            ("A <|-- B", End::Triangle, End::Plain, false),
            ("A *-- B", End::Composition, End::Plain, false),
            ("A o-- B", End::Aggregation, End::Plain, false),
            ("A --> B", End::Plain, End::Arrow, false),
            ("A -- B", End::Plain, End::Plain, false),
            ("A ..> B", End::Plain, End::Arrow, true),
            ("A ..|> B", End::Plain, End::Triangle, true),
            ("A .. B", End::Plain, End::Plain, true),
            ("A <|--|> B", End::Triangle, End::Triangle, false),
            ("A ()-- B", End::Lollipop, End::Plain, false),
        ];
        for (text, l, rt, dotted) in cases {
            let r = relation(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(
                (r.left_end, r.right_end, r.dotted),
                (l, rt, dotted),
                "{text}"
            );
        }
        assert!(relation("A : +x").is_none());
        assert!(relation("A --x B").is_none());
    }

    #[test]
    fn members() {
        let m = member("+deposit(amount) bool").unwrap();
        assert_eq!(m.name, "deposit");
        assert_eq!(m.ty.as_deref(), Some("bool"));
        assert_eq!(m.params.as_ref().unwrap()[0].name, "amount");
        let m = member("+getMessages(int max, filter: String) List~string~").unwrap();
        let p = m.params.unwrap();
        assert_eq!(
            (p[0].name.as_str(), p[0].ty.as_deref()),
            ("max", Some("int"))
        );
        assert_eq!(
            (p[1].name.as_str(), p[1].ty.as_deref()),
            ("filter", Some("String"))
        );
        assert_eq!(m.ty.as_deref(), Some("List<string>"));
        let m = member("someAbstractMethod()*").unwrap();
        assert!(m.is_abstract && m.ty.is_none());
        let m = member("+create()$ Shape").unwrap();
        assert!(m.is_static);
        assert_eq!(m.ty.as_deref(), Some("Shape"));
        let m = member("String count$").unwrap();
        assert!(m.is_static && m.params.is_none());
        let m = member("-id: int").unwrap();
        assert_eq!((m.name.as_str(), m.ty.as_deref()), ("id", Some("int")));
        let m = member("+List~int~ position").unwrap();
        assert_eq!(m.ty.as_deref(), Some("List<int>"));
        assert!(member("+").is_none());
    }

    #[test]
    fn annotations_labels_generics_namespaces() {
        let (d, warnings) = class_diagram(
            r#"classDiagram
direction LR
class Shape{
    <<interface>>
    noOfVertices
    draw()
}
<<enumeration>> Color
Color : RED
class Square~Shape~
class Btn["Big Button"]:::fancy
class Gear <<abstract>>
namespace Engines {
    class Diesel
    class Petrol { +fuel }
}
Square ..|> Shape
style Square fill:#f9f
something odd here !
"#,
        );
        assert_eq!(d.direction, Direction::LeftRight);
        let get = |id: &str| d.classes.iter().find(|c| c.id == id).unwrap();
        assert!(get("Shape").has_annotation("interface"));
        assert_eq!(get("Shape").members.len(), 2);
        assert!(get("Color").has_annotation("enumeration"));
        assert_eq!(get("Square").generic.as_deref(), Some("Shape"));
        assert_eq!(get("Btn").name(), "Big Button");
        assert!(get("Gear").has_annotation("abstract"));
        assert_eq!(get("Diesel").namespace.as_deref(), Some("Engines"));
        assert_eq!(get("Petrol").members[0].name, "fuel");
        assert_eq!(get("Shape").namespace, None);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].starts_with("line 19"));
    }

    #[test]
    fn structural_mistakes_are_errors() {
        assert!(parse_text("classDiagram\nclass A {\n+x").is_err());
        assert!(parse_text("classDiagram\nnamespace N {\nclass A").is_err());
        assert!(parse_text("classDiagram\n}").is_err());
    }
}
