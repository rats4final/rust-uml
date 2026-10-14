//! Text shown for elements (UML notation).

use mdj::{Document, Element, Field};

pub fn visibility_symbol(v: &str) -> &'static str {
    match v {
        "private" => "-",
        "protected" => "#",
        "package" => "~",
        _ => "+",
    }
}

/// A `var` field: either a reference (show its name) or plain text.
pub fn var_text<'a>(doc: &'a Document, el: &'a Element, key: &str) -> &'a str {
    match el.get(key) {
        Some(Field::Ref(id)) => doc.get(id).map(Element::name).unwrap_or(""),
        Some(f) => f.as_str().unwrap_or(""),
        None => "",
    }
}

pub fn attribute_text(doc: &Document, attr: &Element) -> String {
    let mut s = format!(
        "{}{}",
        visibility_symbol(attr.str("visibility").unwrap_or("public")),
        attr.name()
    );
    let ty = var_text(doc, attr, "type");
    if !ty.is_empty() {
        s += &format!(": {ty}");
    }
    let mult = attr.str("multiplicity").unwrap_or("");
    if !mult.is_empty() {
        s += &format!("[{mult}]");
    }
    let default = attr.str("defaultValue").unwrap_or("");
    if !default.is_empty() {
        s += &format!(" = {default}");
    }
    s
}

pub fn operation_text(doc: &Document, op: &Element) -> String {
    let mut params = Vec::new();
    let mut ret = String::new();
    for p in op
        .children("parameters")
        .iter()
        .filter_map(|id| doc.get(id))
    {
        let ty = var_text(doc, p, "type");
        match p.str("direction").unwrap_or("in") {
            "return" => ret = ty.to_owned(),
            dir => {
                let prefix = if dir == "in" {
                    String::new()
                } else {
                    format!("{dir} ")
                };
                params.push(if ty.is_empty() {
                    format!("{prefix}{}", p.name())
                } else {
                    format!("{prefix}{}: {ty}", p.name())
                });
            }
        }
    }
    let mut s = format!(
        "{}{}({})",
        visibility_symbol(op.str("visibility").unwrap_or("public")),
        op.name(),
        params.join(", ")
    );
    if !ret.is_empty() {
        s += &format!(": {ret}");
    }
    s
}

/// `«stereotype»` text for an element, or `None`.
pub fn stereotype_text(doc: &Document, el: &Element) -> Option<String> {
    let s = var_text(doc, el, "stereotype");
    (!s.is_empty()).then(|| format!("«{s}»"))
}

/// `name : Type`, leaving out whichever side is empty (instances, parts,
/// collaboration uses).
pub fn typed_name(name: &str, ty: &str) -> String {
    match (name.is_empty(), ty.is_empty()) {
        (_, true) => name.to_owned(),
        (true, false) => format!(": {ty}"),
        (false, false) => format!("{name} : {ty}"),
    }
}

/// Heading of an instance (object, node instance, …): `name : Classifier`.
pub fn instance_text(doc: &Document, inst: &Element) -> String {
    typed_name(inst.name(), var_text(doc, inst, "classifier"))
}

/// A part: `name : Type [multiplicity]`.
pub fn part_text(doc: &Document, part: &Element) -> String {
    let mut s = typed_name(part.name(), var_text(doc, part, "type"));
    let mult = part.str("multiplicity").unwrap_or("");
    if !mult.is_empty() {
        s += &format!(" [{mult}]");
    }
    s
}

/// An object's slot: `name = value`, named after its defining feature if set.
pub fn slot_text(doc: &Document, slot: &Element) -> String {
    let name = match slot.ref_id("definingFeature").and_then(|f| doc.get(f)) {
        Some(feature) => feature.name(),
        None => slot.name(),
    };
    let value = slot.str("value").unwrap_or("");
    if value.is_empty() {
        name.to_owned()
    } else {
        format!("{name} = {value}")
    }
}

/// A transition's label: `triggers [guard] / effects`. Falls back to the
/// transition's name when it has no triggers.
pub fn transition_text(doc: &Document, t: &Element) -> String {
    let names = |key: &str| -> Vec<&str> {
        t.children(key)
            .iter()
            .filter_map(|id| doc.get(id))
            .map(Element::name)
            .filter(|n| !n.is_empty())
            .collect()
    };
    let triggers = names("triggers");
    let mut s = if triggers.is_empty() {
        t.name().to_owned()
    } else {
        triggers.join(", ")
    };
    let guard = t.str("guard").unwrap_or("");
    if !guard.is_empty() {
        if !s.is_empty() {
            s.push(' ');
        }
        s += &format!("[{guard}]");
    }
    let effects = names("effects");
    if !effects.is_empty() {
        s += &format!(" / {}", effects.join(", "));
    }
    s
}

/// A sequence message's label: `[guard] seq: name(arguments)`, with `:=`
/// for an assignment target, as UML writes it.
pub fn message_text(msg: &Element) -> String {
    let field = |k: &str| msg.str(k).unwrap_or("");
    let mut s = String::new();
    if !field("guard").is_empty() {
        s += &format!("[{}] ", field("guard"));
    }
    if !field("sequenceNumber").is_empty() {
        s += &format!("{}: ", field("sequenceNumber"));
    }
    if !field("assignmentTarget").is_empty() {
        s += &format!("{} := ", field("assignmentTarget"));
    }
    s += msg.name();
    let sort = field("messageSort");
    let is_call = matches!(sort, "" | "synchCall" | "asynchCall");
    if is_call && !msg.name().is_empty() || !field("arguments").is_empty() {
        s += &format!("({})", field("arguments"));
    }
    s
}

/// A communication message's label, as StarUML writes it:
/// `number [guard] : target = name(arguments)`. `number` is the message's
/// sequence number when the diagram shows one.
pub fn numbered_message_text(msg: &Element, number: Option<&str>) -> String {
    let field = |k: &str| msg.str(k).unwrap_or("");
    let mut s = String::new();
    if let Some(n) = number.filter(|n| !n.is_empty()) {
        s += &format!("{n} ");
    }
    if !field("guard").is_empty() {
        s += &format!("[{}] ", field("guard"));
    }
    if !s.is_empty() {
        s += ": ";
    }
    if !field("assignmentTarget").is_empty() {
        s += &format!("{} = ", field("assignmentTarget"));
    }
    s += msg.name();
    if !field("arguments").is_empty() {
        s += &format!("({})", field("arguments"));
    }
    s
}

/// An activity edge's label: `name [guard]`.
pub fn flow_text(flow: &Element) -> String {
    let guard = flow.str("guard").unwrap_or("");
    match (flow.name().is_empty(), guard.is_empty()) {
        (_, true) => flow.name().to_owned(),
        (true, false) => format!("[{guard}]"),
        (false, false) => format!("{} [{guard}]", flow.name()),
    }
}

/// An ERD column's keys, as StarUML lists them: `PK`, `FK`, and for
/// columns that aren't primary keys `N` (nullable) and `U` (unique).
pub fn column_keys(column: &Element) -> String {
    let flag = |k: &str| column.bool(k, false);
    let primary = flag("primaryKey");
    let keys = [
        (primary, "PK"),
        (flag("foreignKey"), "FK"),
        (!primary && flag("nullable"), "N"),
        (!primary && flag("unique"), "U"),
    ];
    keys.iter()
        .filter(|(on, _)| *on)
        .map(|(_, k)| *k)
        .collect::<Vec<_>>()
        .join(",")
}

/// An ERD column's type with its length: `VARCHAR(20)`.
pub fn column_type(column: &Element) -> String {
    let ty = column.str("type").unwrap_or("");
    // StarUML stores the length as text, but its own column editor writes 0.
    let length = match column.str("length") {
        Some(s) => s.to_owned(),
        None => {
            let n = column.num("length", 0.0);
            if n == 0.0 {
                String::new()
            } else {
                n.to_string()
            }
        }
    };
    if length.is_empty() {
        ty.to_owned()
    } else {
        format!("{ty}({length})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_message_labels() {
        let doc = Document::from_json_str(
            r#"{"_type":"Project","_id":"P","ownedElements":[
              {"_type":"UMLMessage","_id":"M","name":"pay","arguments":"total"},
              {"_type":"UMLMessage","_id":"G","name":"ship","guard":"paid","assignmentTarget":"ok"}]}"#,
        )
        .unwrap();
        let m = doc.get("M").unwrap();
        assert_eq!(numbered_message_text(m, Some("1")), "1 : pay(total)");
        assert_eq!(numbered_message_text(m, None), "pay(total)");
        let g = doc.get("G").unwrap();
        assert_eq!(
            numbered_message_text(g, Some("2.1")),
            "2.1 [paid] : ok = ship"
        );
    }

    #[test]
    fn transition_and_flow_labels() {
        let doc = Document::from_json_str(
            r#"{"_type":"Project","_id":"P","ownedElements":[
              {"_type":"UMLTransition","_id":"T","guard":"ready",
               "triggers":[{"_type":"UMLEvent","_id":"E","name":"start"}],
               "effects":[{"_type":"UMLOpaqueBehavior","_id":"B","name":"log"}]},
              {"_type":"UMLControlFlow","_id":"F","guard":"x > 0"}]}"#,
        )
        .unwrap();
        assert_eq!(
            transition_text(&doc, doc.get("T").unwrap()),
            "start [ready] / log"
        );
        assert_eq!(flow_text(doc.get("F").unwrap()), "[x > 0]");
    }

    #[test]
    fn message_labels() {
        let doc = Document::from_json_str(
            r#"{"_type":"Project","_id":"P","ownedElements":[
              {"_type":"UMLMessage","_id":"M","name":"pay","arguments":"amount","guard":"ok","sequenceNumber":"2"},
              {"_type":"UMLMessage","_id":"R","name":"receipt","messageSort":"reply"}]}"#,
        )
        .unwrap();
        assert_eq!(message_text(doc.get("M").unwrap()), "[ok] 2: pay(amount)");
        assert_eq!(message_text(doc.get("R").unwrap()), "receipt");
    }

    #[test]
    fn column_keys_and_types() {
        let doc = Document::from_json_str(
            r#"{"_type":"Project","_id":"P","ownedElements":[
              {"_type":"ERDColumn","_id":"A","primaryKey":true,"nullable":true,"type":"INTEGER"},
              {"_type":"ERDColumn","_id":"B","foreignKey":true,"nullable":true,"unique":true,
               "type":"VARCHAR","length":"20"},
              {"_type":"ERDColumn","_id":"C","length":0}]}"#,
        )
        .unwrap();
        let get = |id: &str| doc.get(id).unwrap();
        assert_eq!(column_keys(get("A")), "PK");
        assert_eq!(column_keys(get("B")), "FK,N,U");
        assert_eq!(column_keys(get("C")), "");
        assert_eq!(column_type(get("A")), "INTEGER");
        assert_eq!(column_type(get("B")), "VARCHAR(20)");
        assert_eq!(column_type(get("C")), "");
    }

    #[test]
    fn typed_names() {
        assert_eq!(typed_name("o", "Order"), "o : Order");
        assert_eq!(typed_name("", "Order"), ": Order");
        assert_eq!(typed_name("o", ""), "o");
    }
}
