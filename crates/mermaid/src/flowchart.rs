//! `flowchart` and `graph`.
//!
//! Statements are node groups joined by links (`A & B --> C[Text] -->|yes| D`).
//! Shapes come from the brackets (`[ ]`, `( )`, `{ }`, …) or from the
//! newer `A@{ shape: doc, label: "…" }` form. Subgraphs are read but not kept:
//! their nodes join the diagram and one warning says so.

use crate::lines::{Line, is_styling};
use crate::{Direction, ParseError, breaks, err};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Flowchart {
    pub direction: Direction,
    /// Every node, in order of first mention.
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}

/// Mermaid's node shapes, by the names its documentation uses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Shape {
    /// `[text]`, `rect`, and the process variants (lined, divided, tagged,
    /// stacked).
    #[default]
    Rect,
    /// `(text)`, `rounded`
    Rounded,
    /// `([text])`, `stadium`
    Stadium,
    /// `[[text]]`, `subroutine`
    Subroutine,
    /// `[(text)]`, `cyl`
    Cylinder,
    /// `h-cyl`: direct access storage
    HorizontalCylinder,
    /// `((text))`, `circle`
    Circle,
    /// `(((text)))`, `dbl-circ`
    DoubleCircle,
    /// `sm-circ` (start), `fr-circ` (stop), `f-circ` (junction)
    SmallCircle,
    /// `>text]`
    Odd,
    /// `{text}`, `diam`
    Diamond,
    /// `{{text}}`, `hex`
    Hexagon,
    /// `[/text/]`, `lean-r`
    LeanRight,
    /// `[\text\]`, `lean-l`
    LeanLeft,
    /// `[/text\]`, `trap-b`: wider at the bottom
    Trapezoid,
    /// `[\text/]`, `trap-t`: wider at the top (manual operation)
    InvTrapezoid,
    /// `doc`, `lin-doc`, `tag-doc`
    Document,
    /// `docs`
    Documents,
    /// `delay`
    Delay,
    /// `curv-trap`
    Display,
    /// `sl-rect`
    ManualInput,
    /// `flag`
    PaperTape,
    /// `bow-rect`
    StoredData,
    /// `win-pane`
    InternalStorage,
    /// `tri`
    Extract,
    /// `flip-tri`: manual file
    FlippedTriangle,
    /// `hourglass`
    Collate,
    /// `cross-circ`: summary
    CrossedCircle,
    /// `notch-rect`
    Card,
    /// A shape with nothing close among flowchart symbols (`text`, `brace`,
    /// `bolt`, `fork`, `notch-pent`, or a name Mermaid doesn't have).
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub id: String,
    /// The text in the brackets, if any.
    pub label: Option<String>,
    pub shape: Shape,
}

impl Node {
    /// What Mermaid shows: the text, or else the id.
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stroke {
    #[default]
    Solid,
    Dotted,
    Thick,
    /// `~~~`: only keeps nodes near each other; nothing is drawn.
    Invisible,
}

/// What a link end looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum End {
    #[default]
    None,
    Arrow,
    Circle,
    Cross,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub stroke: Stroke,
    /// The end at `from` (`<-->` has an arrow there).
    pub tail: End,
    /// The end at `to`.
    pub head: End,
}

/// The direction on the `flowchart`/`graph` line, and any statements after
/// a `;` on it (`graph LR; A-->B`).
pub(crate) fn header(text: &str) -> (Direction, Option<&str>) {
    let (head, rest) = match text.split_once(';') {
        Some((h, r)) => (h, Some(r.trim()).filter(|r| !r.is_empty())),
        None => (text, None),
    };
    let dir = head
        .split_whitespace()
        .nth(1)
        .and_then(Direction::parse)
        .unwrap_or_default();
    (dir, rest)
}

pub(crate) fn parse(
    header_line: &Line,
    lines: &[Line],
    warnings: &mut Vec<String>,
) -> Result<Flowchart, ParseError> {
    let (direction, first) = header(&header_line.text);
    let mut p = Parser {
        d: Flowchart {
            direction,
            ..Flowchart::default()
        },
        subgraphs: Vec::new(),
        edge_ids: Vec::new(),
        warned_subgraphs: false,
    };
    if let Some(first) = first {
        p.line(header_line.number, first, warnings)?;
    }
    for line in lines {
        p.line(line.number, &line.text, warnings)?;
    }
    if let Some(n) = p.subgraphs.last() {
        return Err(err(*n, "this subgraph is missing its `end`"));
    }
    Ok(p.d)
}

struct Parser {
    d: Flowchart,
    /// Lines of the open subgraphs.
    subgraphs: Vec<usize>,
    /// Link ids (`A e1@--> B`), so `e1@{ … }` isn't read as a node.
    edge_ids: Vec<String>,
    warned_subgraphs: bool,
}

impl Parser {
    fn line(&mut self, n: usize, text: &str, w: &mut Vec<String>) -> Result<(), ParseError> {
        for t in statements(text) {
            self.statement(n, t, w)?;
        }
        Ok(())
    }

    fn statement(&mut self, n: usize, t: &str, w: &mut Vec<String>) -> Result<(), ParseError> {
        if is_styling(t) || t.starts_with("class ") || t.starts_with("direction ") {
            return Ok(());
        }
        if t == "subgraph" || t.starts_with("subgraph ") {
            if !self.warned_subgraphs {
                w.push(format!(
                    "line {n}: subgraphs were not kept (flowcharts have no grouping symbol); their nodes were imported"
                ));
                self.warned_subgraphs = true;
            }
            self.subgraphs.push(n);
            return Ok(());
        }
        if t == "end" {
            if self.subgraphs.pop().is_none() {
                return Err(err(n, "`end` without an open subgraph"));
            }
            return Ok(());
        }
        // `e1@{ animate: true }` styles a link.
        if let Some((id, _)) = t.split_once("@{")
            && self.edge_ids.iter().any(|e| e == id.trim())
        {
            return Ok(());
        }
        match chain(t) {
            Some(chain) => {
                self.add(chain);
                Ok(())
            }
            None => {
                w.push(format!("line {n}: skipped `{t}`"));
                Ok(())
            }
        }
    }

    fn add(&mut self, chain: Chain) {
        for group in &chain.groups {
            for node in group {
                self.node(node);
            }
        }
        for (i, link) in chain.links.iter().enumerate() {
            if let Some(id) = &link.id {
                self.edge_ids.push(id.clone());
            }
            for from in &chain.groups[i] {
                for to in &chain.groups[i + 1] {
                    self.d.links.push(Link {
                        from: from.id.clone(),
                        to: to.id.clone(),
                        label: link.label.clone(),
                        stroke: link.stroke,
                        tail: link.tail,
                        head: link.head,
                    });
                }
            }
        }
    }

    /// Add a node or update it: a later mention with brackets sets its
    /// text and shape, as in Mermaid.
    fn node(&mut self, seen: &NodeRef) {
        match self.d.nodes.iter_mut().find(|x| x.id == seen.id) {
            Some(x) => {
                if seen.label.is_some() {
                    x.label.clone_from(&seen.label);
                }
                if let Some(s) = &seen.shape {
                    x.shape = s.clone();
                }
            }
            None => self.d.nodes.push(Node {
                id: seen.id.clone(),
                label: seen.label.clone(),
                shape: seen.shape.clone().unwrap_or_default(),
            }),
        }
    }
}

/// Split a line at `;` outside quotes and brackets.
fn statements(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quoted, mut start) = (0i32, false, 0);
    for (i, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '[' | '(' | '{' if !quoted => depth += 1,
            ']' | ')' | '}' if !quoted => depth -= 1,
            ';' if !quoted && depth <= 0 => {
                out.push(text[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(text[start..].trim());
    out.retain(|s| !s.is_empty());
    out
}

/// A node as written in one place.
#[derive(Debug, Default)]
struct NodeRef {
    id: String,
    label: Option<String>,
    shape: Option<Shape>,
}

#[derive(Debug, Default)]
struct LinkRef {
    id: Option<String>,
    label: Option<String>,
    stroke: Stroke,
    tail: End,
    head: End,
}

/// `group link group link group …`, each group `A & B & C`.
#[derive(Debug, Default)]
struct Chain {
    groups: Vec<Vec<NodeRef>>,
    links: Vec<LinkRef>,
}

struct Scanner {
    c: Vec<char>,
    at: usize,
}

impl Scanner {
    fn peek(&self) -> Option<char> {
        self.c.get(self.at).copied()
    }

    fn peek_at(&self, k: usize) -> Option<char> {
        self.c.get(self.at + k).copied()
    }

    fn done(&self) -> bool {
        self.at >= self.c.len()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += 1;
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.starts_with_at(0, s)
    }

    fn starts_with_at(&self, k: usize, s: &str) -> bool {
        s.chars()
            .enumerate()
            .all(|(i, ch)| self.peek_at(k + i) == Some(ch))
    }

    fn eat(&mut self, s: &str) -> bool {
        let ok = self.starts_with(s);
        if ok {
            self.at += s.chars().count();
        }
        ok
    }

    fn id(&mut self) -> Option<String> {
        let start = self.at;
        while self.peek().is_some_and(is_id_char) {
            self.at += 1;
        }
        (self.at > start).then(|| self.c[start..self.at].iter().collect())
    }

    /// Text up to the first of `closers`, or a quoted string followed by
    /// one. Returns the text and which closer ended it.
    fn text_until(&mut self, closers: &[&str]) -> Option<(String, usize)> {
        self.skip_ws();
        let quoted = self.peek() == Some('"');
        let mut text = String::new();
        if quoted {
            self.at += 1;
            loop {
                let ch = self.peek()?;
                self.at += 1;
                if ch == '"' {
                    break;
                }
                text.push(ch);
            }
            self.skip_ws();
        }
        loop {
            if let Some(k) = closers.iter().position(|cl| self.starts_with(cl)) {
                self.at += closers[k].chars().count();
                let text = if quoted { text } else { text.trim().to_owned() };
                return Some((text, k));
            }
            let ch = self.peek()?;
            if quoted {
                // Only spaces may follow the closing quote.
                return None;
            }
            text.push(ch);
            self.at += 1;
        }
    }
}

fn is_id_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn chain(t: &str) -> Option<Chain> {
    let mut s = Scanner {
        c: t.chars().collect(),
        at: 0,
    };
    let mut out = Chain::default();
    loop {
        out.groups.push(group(&mut s)?);
        s.skip_ws();
        if s.done() {
            return Some(out);
        }
        out.links.push(link(&mut s)?);
    }
}

fn group(s: &mut Scanner) -> Option<Vec<NodeRef>> {
    let mut nodes = vec![node(s)?];
    loop {
        let back = s.at;
        s.skip_ws();
        if s.eat("&") {
            s.skip_ws();
            nodes.push(node(s)?);
        } else {
            s.at = back;
            return Some(nodes);
        }
    }
}

/// Bracket pairs, longest openers first.
const BRACKETS: &[(&str, &[&str], Shape)] = &[
    ("(((", &[")))"], Shape::DoubleCircle),
    ("((", &["))"], Shape::Circle),
    ("([", &["])"], Shape::Stadium),
    ("[[", &["]]"], Shape::Subroutine),
    ("[(", &[")]"], Shape::Cylinder),
    ("{{", &["}}"], Shape::Hexagon),
    ("[/", &["/]", "\\]"], Shape::LeanRight),
    ("[\\", &["\\]", "/]"], Shape::LeanLeft),
    ("[", &["]"], Shape::Rect),
    ("(", &[")"], Shape::Rounded),
    ("{", &["}"], Shape::Diamond),
    (">", &["]"], Shape::Odd),
];

fn node(s: &mut Scanner) -> Option<NodeRef> {
    s.skip_ws();
    let mut r = NodeRef {
        id: s.id()?,
        ..NodeRef::default()
    };
    if s.eat("@{") {
        let start = s.at;
        let mut quoted = false;
        loop {
            match s.peek()? {
                '"' => quoted = !quoted,
                '}' if !quoted => break,
                _ => {}
            }
            s.at += 1;
        }
        let body: String = s.c[start..s.at].iter().collect();
        s.at += 1;
        for (key, value) in properties(&body) {
            match key.as_str() {
                "shape" => r.shape = Some(shape_named(&value)),
                "label" => r.label = Some(label(&value)),
                _ => {}
            }
        }
    } else {
        for (open, closers, shape) in BRACKETS {
            if !s.eat(open) {
                continue;
            }
            let (text, which) = s.text_until(closers)?;
            r.label = Some(label(&text));
            r.shape = Some(match (shape, which) {
                (Shape::LeanRight, 1) => Shape::Trapezoid,
                (Shape::LeanLeft, 1) => Shape::InvTrapezoid,
                (shape, _) => shape.clone(),
            });
            break;
        }
    }
    // `A:::cls` only styles the node.
    if s.eat(":::") {
        s.id();
    }
    Some(r)
}

/// `shape: doc, label: "A, b"` → pairs.
fn properties(body: &str) -> Vec<(String, String)> {
    let mut parts = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in body.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
        .into_iter()
        .filter_map(|p| {
            let (k, v) = p.split_once(':')?;
            Some((k.trim().to_owned(), crate::unquote(v).to_owned()))
        })
        .collect()
}

fn shape_named(name: &str) -> Shape {
    match name.trim() {
        "rect" | "rectangle" | "proc" | "process" | "lin-rect" | "lined-rectangle" | "lin-proc"
        | "lined-process" | "shaded-process" | "div-rect" | "div-proc" | "divided-rectangle"
        | "divided-process" | "st-rect" | "stacked-rectangle" | "procs" | "processes"
        | "tag-rect" | "tag-proc" | "tagged-rectangle" | "tagged-process" => Shape::Rect,
        "rounded" | "event" => Shape::Rounded,
        "stadium" | "pill" | "terminal" => Shape::Stadium,
        "subproc" | "subprocess" | "subroutine" | "fr-rect" | "framed-rectangle" => {
            Shape::Subroutine
        }
        "cyl" | "cylinder" | "db" | "database" | "lin-cyl" | "lined-cylinder" | "disk" => {
            Shape::Cylinder
        }
        "h-cyl" | "horizontal-cylinder" | "das" => Shape::HorizontalCylinder,
        "circle" | "circ" => Shape::Circle,
        "dbl-circ" | "double-circle" => Shape::DoubleCircle,
        "sm-circ" | "small-circle" | "start" | "fr-circ" | "framed-circle" | "stop" | "f-circ"
        | "filled-circle" | "junction" => Shape::SmallCircle,
        "odd" => Shape::Odd,
        "diam" | "diamond" | "decision" | "question" => Shape::Diamond,
        "hex" | "hexagon" | "prepare" => Shape::Hexagon,
        "lean-r" | "lean-right" | "in-out" => Shape::LeanRight,
        "lean-l" | "lean-left" | "out-in" => Shape::LeanLeft,
        "trap-b" | "trapezoid-bottom" | "trapezoid" | "priority" => Shape::Trapezoid,
        "trap-t" | "trapezoid-top" | "inv-trapezoid" | "manual" => Shape::InvTrapezoid,
        "doc" | "document" | "lin-doc" | "lined-document" | "tag-doc" | "tagged-document" => {
            Shape::Document
        }
        "docs" | "documents" | "st-doc" | "stacked-document" => Shape::Documents,
        "delay" | "half-rounded-rectangle" => Shape::Delay,
        "curv-trap" | "curved-trapezoid" | "display" => Shape::Display,
        "sl-rect" | "sloped-rectangle" | "manual-input" => Shape::ManualInput,
        "flag" | "paper-tape" => Shape::PaperTape,
        "bow-rect" | "bow-tie-rectangle" | "stored-data" => Shape::StoredData,
        "win-pane" | "window-pane" | "internal-storage" => Shape::InternalStorage,
        "tri" | "triangle" | "extract" => Shape::Extract,
        "flip-tri" | "flipped-triangle" | "manual-file" => Shape::FlippedTriangle,
        "hourglass" | "collate" => Shape::Collate,
        "cross-circ" | "crossed-circle" | "summary" => Shape::CrossedCircle,
        "notch-rect" | "notched-rectangle" | "card" => Shape::Card,
        other => Shape::Other(other.to_owned()),
    }
}

/// Node and link text: quotes and markdown backticks removed, line breaks
/// and `#quot;`-style entity codes turned into characters.
fn label(text: &str) -> String {
    let t = crate::unquote(crate::unquote(text));
    entities(&breaks(t))
}

fn entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('#') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let decoded = after.split_once(';').and_then(|(code, tail)| {
            let ch = match code {
                "quot" => Some('"'),
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some('\u{a0}'),
                _ => code.parse::<u32>().ok().and_then(char::from_u32),
            }?;
            Some((ch, tail))
        });
        match decoded {
            Some((ch, tail)) => {
                out.push(ch);
                rest = tail;
            }
            None => {
                out.push('#');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_link_char(c: char) -> bool {
    matches!(c, '-' | '=' | '.' | '~')
}

/// A link: `-->`, `---`, `-.->`, `==>`, `~~~`, `<-->`, `--o`, `--x`, with
/// text as `-->|text|` or `-- text -->`, optionally named `e1@-->`.
fn link(s: &mut Scanner) -> Option<LinkRef> {
    let mut r = LinkRef::default();
    // `e1@-->`
    let back = s.at;
    if let Some(id) = s.id() {
        if s.eat("@") && s.peek().is_some_and(|c| is_link_char(c) || c == '<') {
            r.id = Some(id);
        } else {
            s.at = back;
        }
    }
    r.tail = match s.peek() {
        Some('<') => End::Arrow,
        Some('o') if s.peek_at(1).is_some_and(is_link_char) => End::Circle,
        Some('x') if s.peek_at(1).is_some_and(is_link_char) => End::Cross,
        _ => End::None,
    };
    if r.tail != End::None {
        s.at += 1;
    }
    let body = run(s);
    if body.chars().count() < 2 {
        return None;
    }
    // `-- text -->`: a short opener, then text, then the rest of the link.
    if matches!(body.as_str(), "--" | "==" | "-.") && s.peek().is_some_and(char::is_whitespace) {
        let close = if body == "-." { ".-" } else { &body[..2] };
        let start = s.at;
        // The text ends at a space followed by the closing half.
        while !(s.peek()?.is_whitespace() && s.starts_with_at(1, close)) {
            s.at += 1;
        }
        let text: String = s.c[start..s.at].iter().collect();
        r.label = Some(label(text.trim())).filter(|l| !l.is_empty());
        s.skip_ws();
        let closing = run(s);
        r.stroke = stroke(&format!("{body}{closing}"));
    } else {
        r.stroke = stroke(&body);
    }
    r.head = match s.peek() {
        Some('>') => End::Arrow,
        Some('o') if !s.peek_at(1).is_some_and(is_id_char) => End::Circle,
        Some('x') if !s.peek_at(1).is_some_and(is_id_char) => End::Cross,
        _ => End::None,
    };
    if r.head != End::None {
        s.at += 1;
    }
    s.skip_ws();
    if s.eat("|") {
        let (text, _) = s.text_until(&["|"])?;
        r.label = Some(label(&text)).filter(|l| !l.is_empty());
    }
    Some(r)
}

fn run(s: &mut Scanner) -> String {
    let start = s.at;
    while s.peek().is_some_and(is_link_char) {
        s.at += 1;
    }
    s.c[start..s.at].iter().collect()
}

fn stroke(body: &str) -> Stroke {
    if body.contains('~') {
        Stroke::Invisible
    } else if body.contains('.') {
        Stroke::Dotted
    } else if body.contains('=') {
        Stroke::Thick
    } else {
        Stroke::Solid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse as parse_text};

    fn flow(text: &str) -> (Flowchart, Vec<String>) {
        let p = parse_text(text).unwrap();
        match p.diagram {
            Diagram::Flowchart(d) => (d, p.warnings),
            _ => panic!("not a flowchart"),
        }
    }

    fn pairs(d: &Flowchart) -> Vec<(String, String, Option<String>)> {
        d.links
            .iter()
            .map(|l| (l.from.clone(), l.to.clone(), l.label.clone()))
            .collect()
    }

    #[test]
    fn shapes_from_brackets() {
        let (d, w) = flow(
            r#"flowchart LR
    a[Box] --> b(Round) --> c([Pill]) --> d[[Sub]] --> e[(Db)]
    f((Circle)) --> g(((Two))) --> h>Flag] --> i{Ask?} --> j{{Prep}}
    k[/In/] --> l[\Out\] --> m[/Wide\] --> n[\Manual/]
    o["Say #quot;hi#quot; (now)"] --> p["`**md**`"] --> q[a<br>b]
    r
"#,
        );
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(d.direction, Direction::LeftRight);
        let shapes: Vec<Shape> = d.nodes.iter().map(|n| n.shape.clone()).collect();
        use Shape::*;
        assert_eq!(
            shapes,
            [
                Rect,
                Rounded,
                Stadium,
                Subroutine,
                Cylinder,
                Circle,
                DoubleCircle,
                Odd,
                Diamond,
                Hexagon,
                LeanRight,
                LeanLeft,
                Trapezoid,
                InvTrapezoid,
                Rect,
                Rect,
                Rect,
                Rect
            ]
        );
        let name = |id: &str| {
            d.nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap()
                .name()
                .to_owned()
        };
        assert_eq!(name("i"), "Ask?");
        assert_eq!(name("o"), "Say \"hi\" (now)");
        assert_eq!(name("p"), "**md**");
        assert_eq!(name("q"), "a\nb");
        assert_eq!(name("r"), "r");
        assert_eq!(d.links.len(), 13);
    }

    #[test]
    fn link_forms_labels_and_groups() {
        let (d, w) = flow(
            r#"graph TD;
    A-->B;B---C
    C -->|yes| D
    D -- no way --> E
    E -. maybe .-> F
    F == sure ==> G
    G ~~~ H
    H <--> I
    I --o J
    J --x K
    K & L --> M & N
    N e1@--> O
    e1@{ animate: true }
    O ----> P
"#,
        );
        assert!(w.is_empty(), "{w:?}");
        let p = pairs(&d);
        let s = |a: &str, b: &str| (a.to_owned(), b.to_owned());
        assert_eq!((p[0].0.clone(), p[0].1.clone()), s("A", "B"));
        assert_eq!(p[2].2.as_deref(), Some("yes"));
        assert_eq!(p[3].2.as_deref(), Some("no way"));
        assert_eq!(p[4].2.as_deref(), Some("maybe"));
        assert_eq!(p[5].2.as_deref(), Some("sure"));
        let l = &d.links;
        assert_eq!((l[0].stroke, l[0].head), (Stroke::Solid, End::Arrow));
        assert_eq!((l[1].stroke, l[1].head), (Stroke::Solid, End::None));
        assert_eq!(l[4].stroke, Stroke::Dotted);
        assert_eq!(l[5].stroke, Stroke::Thick);
        assert_eq!(l[6].stroke, Stroke::Invisible);
        assert_eq!((l[7].tail, l[7].head), (End::Arrow, End::Arrow));
        assert_eq!(l[8].head, End::Circle);
        assert_eq!(l[9].head, End::Cross);
        // K & L --> M & N is four links.
        let fan: Vec<_> = p[10..14]
            .iter()
            .map(|x| (x.0.as_str(), x.1.as_str()))
            .collect();
        assert_eq!(fan, [("K", "M"), ("K", "N"), ("L", "M"), ("L", "N")]);
        assert_eq!((p[14].0.as_str(), p[14].1.as_str()), ("N", "O"));
        assert_eq!((p[15].0.as_str(), p[15].1.as_str()), ("O", "P"));
        assert!(!d.nodes.iter().any(|n| n.id == "e1"));
    }

    #[test]
    fn new_shape_syntax_and_redefinition() {
        let (d, _) = flow(
            r#"flowchart TB
    A@{ shape: doc, label: "Report, final" } --> B@{ shape: sl-rect }
    C@{ shape: bolt }
    B[Typed in]
"#,
        );
        assert_eq!(d.nodes[0].shape, Shape::Document);
        assert_eq!(d.nodes[0].name(), "Report, final");
        assert_eq!(d.nodes[1].shape, Shape::Rect, "redefined by B[…]");
        assert_eq!(d.nodes[1].name(), "Typed in");
        assert_eq!(d.nodes[2].shape, Shape::Other("bolt".into()));
    }

    #[test]
    fn subgraphs_warn_styling_is_skipped_and_mistakes_fail() {
        let (d, w) = flow(
            r#"flowchart LR
    subgraph one [First]
      direction TB
      a1 --> a2
    end
    subgraph two
      b1:::hot --> b2
    end
    classDef hot fill:#f00
    class a1,b1 hot
    style a2 fill:#bbf
    linkStyle 0 stroke:#ff3
    click a1 callback
    ???
"#,
        );
        assert_eq!(d.nodes.len(), 4);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[0].contains("subgraphs"));
        assert!(w[1].contains("???"));
        assert!(parse_text("flowchart\nsubgraph x\nA-->B").is_err());
        assert!(parse_text("flowchart\nend").is_err());
        assert_eq!(header("graph").0, Direction::TopDown);
        assert_eq!(header("flowchart RL").0, Direction::RightLeft);
    }

    #[test]
    fn odd_input_never_panics() {
        for t in [
            "A[",
            "A[\"x",
            "A -- x",
            "A -->|x",
            "A@{",
            "&",
            "A &",
            "--> B",
            "A[/x",
            "A-->",
            "é[ü] --> ñ",
            "A#;",
            "A[#99999999;]",
            "x[#1",
        ] {
            let _ = parse_text(&format!("flowchart\n{t}"));
        }
    }
}
