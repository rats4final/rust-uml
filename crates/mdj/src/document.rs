use std::collections::HashMap;

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::undo::{Editor, History};
use crate::{Field, Id, IdGen};

/// One element of the model tree: a model element, a diagram, or a view.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub id: Id,
    pub type_name: String,
    pub parent: Option<Id>,
    /// Every field except `_type`, `_id` and `_parent`, in file order.
    pub fields: IndexMap<String, Field>,
}

impl Element {
    pub fn new(id: Id, type_name: impl Into<String>) -> Self {
        Self {
            id,
            type_name: type_name.into(),
            parent: None,
            fields: IndexMap::new(),
        }
    }

    pub fn with(mut self, key: &str, value: Field) -> Self {
        self.fields.insert(key.to_owned(), value);
        self
    }

    pub fn get(&self, key: &str) -> Option<&Field> {
        self.fields.get(key)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Field::as_str)
    }

    pub fn name(&self) -> &str {
        self.str("name").unwrap_or("")
    }

    pub fn bool(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(Field::as_bool).unwrap_or(default)
    }

    pub fn num(&self, key: &str, default: f64) -> f64 {
        self.get(key).and_then(Field::as_f64).unwrap_or(default)
    }

    pub fn ref_id(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Field::as_ref_id)
    }

    pub fn children(&self, key: &str) -> &[Id] {
        self.get(key).map(Field::children).unwrap_or(&[])
    }

    /// Owned children across every field, in field order.
    pub fn all_children(&self) -> impl Iterator<Item = &Id> {
        self.fields.values().flat_map(Field::children)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("expected an element object with \"_type\" and \"_id\" at {0}")]
    NotElement(String),
    #[error("element id {0} appears more than once")]
    DuplicateId(Id),
}

/// A whole project: the element tree plus undo history.
#[derive(Debug, Clone)]
pub struct Document {
    pub(crate) elements: HashMap<Id, Element>,
    root: Id,
    /// `_parent` of the root when it points outside the file, as in profile
    /// (`.mfj`) fragments exported from a larger project. Kept for round trips.
    root_parent: Option<Id>,
    /// Bumped on every change, so views can cache derived data.
    pub(crate) revision: u64,
    pub(crate) history: History,
    pub(crate) ids: IdGen,
}

impl Document {
    /// An empty project with one model and an empty class diagram, matching
    /// the shape of StarUML's default template.
    pub fn new_project() -> Self {
        let mut ids = IdGen::default();
        let (project, model, diagram) = (ids.next_id(), ids.next_id(), ids.next_id());
        let mut elements = HashMap::new();
        let mut p = Element::new(project.clone(), "Project")
            .with("name", Field::str("Untitled"))
            .with("ownedElements", Field::Children(vec![model.clone()]));
        p.parent = None;
        let mut m = Element::new(model.clone(), "UMLModel")
            .with("name", Field::str("Model"))
            .with("ownedElements", Field::Children(vec![diagram.clone()]));
        m.parent = Some(project.clone());
        let mut d = Element::new(diagram.clone(), "UMLClassDiagram")
            .with("name", Field::str("Main"))
            .with("defaultDiagram", Field::bool(true));
        d.parent = Some(model.clone());
        elements.insert(project.clone(), p);
        elements.insert(model, m);
        elements.insert(diagram, d);
        Self {
            elements,
            root: project,
            root_parent: None,
            revision: 0,
            history: History::default(),
            ids,
        }
    }

    pub fn from_json_str(text: &str) -> Result<Self, LoadError> {
        // Some editors prepend a UTF-8 byte order mark.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Self::from_value(serde_json::from_str(text)?)
    }

    pub fn from_value(value: Value) -> Result<Self, LoadError> {
        let mut elements = HashMap::new();
        let Value::Object(obj) = value else {
            return Err(LoadError::NotElement("root".into()));
        };
        let root_parent = obj.get("_parent").and_then(as_ref).map(str::to_owned);
        let root = read_element(obj, None, "root", &mut elements)?;
        Ok(Self {
            elements,
            root,
            root_parent,
            revision: 0,
            history: History::default(),
            ids: IdGen::default(),
        })
    }

    pub fn to_value(&self) -> Value {
        self.element_to_value(&self.root).unwrap_or(Value::Null)
    }

    /// Serialise with tab indentation, as StarUML does.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        use serde::Serialize;
        let mut out = Vec::new();
        let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
        self.to_value().serialize(&mut ser)?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    /// `None` only if `id` is missing, which the editing API never allows;
    /// a missing child is then left out rather than written as garbage.
    fn element_to_value(&self, id: &str) -> Option<Value> {
        let el = self.elements.get(id)?;
        let mut obj = Map::new();
        obj.insert("_type".into(), Value::String(el.type_name.clone()));
        obj.insert("_id".into(), Value::String(el.id.clone()));
        let parent = el
            .parent
            .as_ref()
            .or(self.root_parent.as_ref().filter(|_| id == self.root));
        if let Some(parent) = parent {
            obj.insert("_parent".into(), ref_value(parent));
        }
        for (key, field) in &el.fields {
            let v = match field {
                Field::Value(v) => v.clone(),
                Field::Ref(id) => ref_value(id),
                Field::Refs(ids) => Value::Array(ids.iter().map(|i| ref_value(i)).collect()),
                Field::Child(id) => match self.element_to_value(id) {
                    Some(v) => v,
                    None => continue,
                },
                Field::Children(ids) => Value::Array(
                    ids.iter()
                        .filter_map(|i| self.element_to_value(i))
                        .collect(),
                ),
            };
            obj.insert(key.clone(), v);
        }
        Some(Value::Object(obj))
    }

    /// Changes whenever the content changes (edits, undo, redo).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn get(&self, id: &str) -> Option<&Element> {
        self.elements.get(id)
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Element> {
        self.elements.values()
    }

    /// `id` and all its owned descendants, parents before children.
    pub fn subtree_ids(&self, id: &str) -> Vec<Id> {
        let mut out = Vec::new();
        let mut stack = vec![id.to_owned()];
        while let Some(cur) = stack.pop() {
            if let Some(el) = self.elements.get(&cur) {
                // Push in reverse so the stack pops children in file order.
                let kids: Vec<Id> = el.all_children().cloned().collect();
                stack.extend(kids.into_iter().rev());
                out.push(cur);
            }
        }
        out
    }

    /// Elements holding a reference (not ownership) to `target`, with the field name.
    pub fn referrers(&self, target: &str) -> Vec<(Id, String)> {
        let mut out = Vec::new();
        for el in self.elements.values() {
            for (key, field) in &el.fields {
                if field.refs().iter().any(|r| r == target) {
                    out.push((el.id.clone(), key.clone()));
                }
            }
        }
        out
    }

    /// Where `id` sits inside its parent: `(parent, field, index)`, with
    /// `index == None` for a single-child (`obj`) field.
    pub fn position_of(&self, id: &str) -> Option<(Id, String, Option<usize>)> {
        let parent = self.elements.get(id)?.parent.clone()?;
        let p = self.elements.get(&parent)?;
        p.fields.iter().find_map(|(key, field)| match field {
            Field::Child(c) if c == id => Some((parent.clone(), key.clone(), None)),
            Field::Children(cs) => cs
                .iter()
                .position(|c| c == id)
                .map(|i| (parent.clone(), key.clone(), Some(i))),
            _ => None,
        })
    }

    /// Run `f` as one undoable change.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Editor) -> R) -> R {
        let mut editor = Editor::new(self);
        let result = f(&mut editor);
        let inverse = editor.finish();
        self.history.record(inverse);
        result
    }

    /// Like [`Document::edit`], but if `f` fails every change it made is
    /// rolled back and nothing is recorded.
    pub fn try_edit<R, E>(&mut self, f: impl FnOnce(&mut Editor) -> Result<R, E>) -> Result<R, E> {
        let mut editor = Editor::new(self);
        let result = f(&mut editor);
        let inverse = editor.finish();
        match result {
            Ok(r) => {
                self.history.record(inverse);
                Ok(r)
            }
            Err(e) => {
                self.apply_all(inverse);
                Err(e)
            }
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(ops) = self.history.take_undo() else {
            return false;
        };
        let redo = self.apply_all(ops);
        self.history.push_redo(redo);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(ops) = self.history.take_redo() else {
            return false;
        };
        let undo = self.apply_all(ops);
        self.history.push_undo_keep_redo(undo);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// How many steps can be undone; pass it to [`Document::squash_undo_since`].
    pub fn undo_depth(&self) -> usize {
        self.history.undo_depth()
    }

    /// Merge every undo step recorded since the history had `depth` steps
    /// into one, so a group of edits (an import, a script) undoes at once.
    pub fn squash_undo_since(&mut self, depth: usize) {
        self.history.squash_since(depth);
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn is_modified(&self) -> bool {
        self.history.is_modified()
    }

    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Change a field without recording undo, for derived data such as
    /// recomputed geometry written just before saving.
    pub fn set_untracked(&mut self, id: &str, key: &str, value: Field) {
        self.revision += 1;
        if let Some(el) = self.elements.get_mut(id) {
            el.fields.insert(key.to_owned(), value);
        }
    }

    pub fn new_id(&mut self) -> Id {
        self.ids.next_id()
    }
}

fn ref_value(id: &str) -> Value {
    let mut m = Map::new();
    m.insert("$ref".into(), Value::String(id.to_owned()));
    Value::Object(m)
}

fn is_element(v: &Value) -> bool {
    matches!(v, Value::Object(o) if o.contains_key("_type"))
}

fn as_ref(v: &Value) -> Option<&str> {
    match v {
        Value::Object(o) if o.len() == 1 => o.get("$ref")?.as_str(),
        _ => None,
    }
}

fn read_element(
    mut obj: Map<String, Value>,
    parent: Option<Id>,
    path: &str,
    out: &mut HashMap<Id, Element>,
) -> Result<Id, LoadError> {
    let (Some(Value::String(type_name)), Some(Value::String(id))) =
        (obj.shift_remove("_type"), obj.shift_remove("_id"))
    else {
        return Err(LoadError::NotElement(path.to_owned()));
    };
    // `_parent` is implied by nesting; the nesting wins if the two disagree.
    obj.shift_remove("_parent");

    let mut el = Element::new(id.clone(), type_name);
    el.parent = parent;
    for (key, value) in obj {
        let child_path = format!("{path}/{key}");
        let field = match value {
            Value::Object(o) if o.contains_key("_type") => {
                Field::Child(read_element(o, Some(id.clone()), &child_path, out)?)
            }
            v if as_ref(&v).is_some() => match as_ref(&v) {
                Some(r) => Field::Ref(r.to_owned()),
                None => Field::Value(v),
            },
            Value::Array(items) if !items.is_empty() && items.iter().all(is_element) => {
                let mut ids = Vec::with_capacity(items.len());
                for (i, item) in items.into_iter().enumerate() {
                    if let Value::Object(o) = item {
                        ids.push(read_element(
                            o,
                            Some(id.clone()),
                            &format!("{child_path}/{i}"),
                            out,
                        )?);
                    }
                }
                Field::Children(ids)
            }
            Value::Array(items)
                if !items.is_empty() && items.iter().all(|v| as_ref(v).is_some()) =>
            {
                Field::Refs(items.iter().filter_map(as_ref).map(str::to_owned).collect())
            }
            other => Field::Value(other),
        };
        el.fields.insert(key, field);
    }
    if out.insert(id.clone(), el).is_some() {
        return Err(LoadError::DuplicateId(id));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "_type": "Project", "_id": "P", "name": "Untitled",
        "ownedElements": [{
            "_type": "UMLModel", "_id": "M", "_parent": {"$ref": "P"}, "name": "Model",
            "ownedElements": [
                {"_type": "UMLClass", "_id": "C", "_parent": {"$ref": "M"}, "name": "A",
                 "attributes": [{"_type": "UMLAttribute", "_id": "A1", "_parent": {"$ref": "C"},
                                 "name": "x", "type": "int"}]},
                {"_type": "UMLClassDiagram", "_id": "D", "_parent": {"$ref": "M"}, "name": "Main",
                 "ownedViews": [{"_type": "UMLClassView", "_id": "V", "_parent": {"$ref": "D"},
                                 "model": {"$ref": "C"}, "left": 10, "top": 20,
                                 "containedViews": [], "font": "Arial;13;0",
                                 "someFutureField": {"nested": [1, 2]}}]}
            ]
        }]
    }"#;

    #[test]
    fn squashed_steps_undo_and_redo_together() {
        let mut doc = Document::from_json_str(SAMPLE).unwrap();
        let before = doc.to_json_string().unwrap();
        doc.edit(|e| e.set("C", "name", Field::str("B")));
        let depth = doc.undo_depth();
        doc.edit(|e| e.set("C", "name", Field::str("Cc")));
        doc.edit(|e| e.set("A1", "name", Field::str("y")));
        doc.edit(|e| e.set("C", "isAbstract", Field::Value(serde_json::json!(true))));
        doc.squash_undo_since(depth);
        assert_eq!(doc.undo_depth(), depth + 1);
        let after = doc.to_json_string().unwrap();

        assert!(doc.undo());
        assert_eq!(doc.get("C").unwrap().name(), "B");
        assert_eq!(doc.get("A1").unwrap().name(), "x");
        assert!(doc.get("C").unwrap().get("isAbstract").is_none());
        assert!(doc.redo());
        assert_eq!(doc.to_json_string().unwrap(), after);
        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.to_json_string().unwrap(), before);

        // Nothing or one step to merge: unchanged.
        doc.squash_undo_since(doc.undo_depth());
        assert!(!doc.can_undo());
    }

    #[test]
    fn round_trip_is_lossless() {
        let doc = Document::from_json_str(SAMPLE).unwrap();
        let original: Value = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(doc.to_value(), original);
        assert_eq!(doc.len(), 6);
    }

    #[test]
    fn classifies_fields_by_shape() {
        let doc = Document::from_json_str(SAMPLE).unwrap();
        let v = doc.get("V").unwrap();
        assert_eq!(v.ref_id("model"), Some("C"));
        assert_eq!(v.num("left", 0.0), 10.0);
        assert_eq!(doc.get("C").unwrap().children("attributes"), ["A1"]);
        assert_eq!(doc.subtree_ids("M"), ["M", "C", "A1", "D", "V"]);
        assert_eq!(
            doc.referrers("C"),
            vec![("V".to_owned(), "model".to_owned())]
        );
    }

    #[test]
    fn failed_edit_rolls_back() {
        let mut doc = Document::from_json_str(SAMPLE).unwrap();
        let before = doc.to_value();
        let r: Result<(), &str> = doc.try_edit(|e| {
            e.set("C", "name", Field::str("Changed"));
            e.remove("V");
            Err("nope")
        });
        assert!(r.is_err());
        assert_eq!(doc.to_value(), before);
        assert!(!doc.can_undo());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let bad = r#"{"_type":"Project","_id":"P","ownedElements":[
            {"_type":"UMLModel","_id":"P"}]}"#;
        assert!(matches!(
            Document::from_json_str(bad),
            Err(LoadError::DuplicateId(_))
        ));
    }

    #[test]
    fn new_project_matches_default_template_shape() {
        let doc = Document::new_project();
        let v = doc.to_value();
        assert_eq!(v["_type"], "Project");
        assert_eq!(v["ownedElements"][0]["_type"], "UMLModel");
        let d = &v["ownedElements"][0]["ownedElements"][0];
        assert_eq!(d["_type"], "UMLClassDiagram");
        assert_eq!(d["defaultDiagram"], true);
        assert_eq!(d["_parent"]["$ref"], v["ownedElements"][0]["_id"]);
    }
}
