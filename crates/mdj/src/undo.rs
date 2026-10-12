use serde_json::Value;

use crate::{Document, Element, Field, Id};

/// One reversible change. Applying an `Op` yields the `Op` that undoes it.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Set (`Some`) or delete (`None`) a field.
    Set {
        id: Id,
        key: String,
        value: Option<Field>,
    },
    /// Insert an owned subtree. `elements[0]` is its root; the rest are its
    /// descendants with their parents already set. `index: None` targets a
    /// single-child field.
    Insert {
        parent: Id,
        key: String,
        index: Option<usize>,
        elements: Vec<Element>,
    },
    /// Remove the owned subtree at a position. `drop_empty` also deletes
    /// the list field if this leaves it empty (undoing an insert that
    /// created the field).
    Remove {
        parent: Id,
        key: String,
        index: Option<usize>,
        drop_empty: bool,
    },
}

#[derive(Debug, Clone, Default)]
pub struct History {
    undo: Vec<Vec<Op>>,
    redo: Vec<Vec<Op>>,
    modified: bool,
}

impl History {
    pub(crate) fn record(&mut self, inverse: Vec<Op>) {
        if !inverse.is_empty() {
            self.undo.push(inverse);
            self.redo.clear();
            self.modified = true;
        }
    }
    pub(crate) fn take_undo(&mut self) -> Option<Vec<Op>> {
        self.undo.pop()
    }
    pub(crate) fn take_redo(&mut self) -> Option<Vec<Op>> {
        self.redo.pop()
    }
    pub(crate) fn push_redo(&mut self, ops: Vec<Op>) {
        self.redo.push(ops);
        self.modified = true;
    }
    pub(crate) fn push_undo_keep_redo(&mut self, ops: Vec<Op>) {
        self.undo.push(ops);
        self.modified = true;
    }
    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub(crate) fn is_modified(&self) -> bool {
        self.modified
    }
    pub(crate) fn mark_saved(&mut self) {
        self.modified = false;
    }
}

/// Handle passed to [`Document::edit`]; every change made through it is
/// grouped into one undo step.
pub struct Editor<'a> {
    doc: &'a mut Document,
    inverse: Vec<Op>,
}

impl<'a> Editor<'a> {
    pub(crate) fn new(doc: &'a mut Document) -> Self {
        Self {
            doc,
            inverse: Vec::new(),
        }
    }

    /// Inverse ops in the order they must be applied to undo the edit.
    pub(crate) fn finish(mut self) -> Vec<Op> {
        self.inverse.reverse();
        self.inverse
    }

    pub fn doc(&self) -> &Document {
        self.doc
    }

    pub fn new_id(&mut self) -> Id {
        self.doc.new_id()
    }

    pub fn apply(&mut self, op: Op) {
        if let Some(inv) = self.doc.apply_op(op) {
            self.inverse.push(inv);
        }
    }

    pub fn set(&mut self, id: &str, key: &str, value: Field) {
        let current = self.doc.get(id).and_then(|e| e.get(key));
        if current != Some(&value) {
            self.apply(Op::Set {
                id: id.to_owned(),
                key: key.to_owned(),
                value: Some(value),
            });
        }
    }

    pub fn unset(&mut self, id: &str, key: &str) {
        self.apply(Op::Set {
            id: id.to_owned(),
            key: key.to_owned(),
            value: None,
        });
    }

    /// Insert a subtree (root first) into `parent.key` at `index`.
    pub fn insert(
        &mut self,
        parent: &str,
        key: &str,
        index: Option<usize>,
        elements: Vec<Element>,
    ) -> Id {
        let root = elements.first().map(|e| e.id.clone()).unwrap_or_default();
        self.apply(Op::Insert {
            parent: parent.to_owned(),
            key: key.to_owned(),
            index,
            elements,
        });
        root
    }

    /// Append a subtree (root first) to the list field `parent.key`.
    pub fn append(&mut self, parent: &str, key: &str, elements: Vec<Element>) -> Id {
        self.insert(parent, key, Some(usize::MAX), elements)
    }

    /// Move `id` to position `index` within its parent's list. Returns
    /// `false` (and changes nothing) if it isn't in a list or the index is
    /// past the end.
    pub fn reorder(&mut self, id: &str, index: usize) -> bool {
        let Some((parent, key, Some(from))) = self.doc.position_of(id) else {
            return false;
        };
        let len = self
            .doc
            .get(&parent)
            .map(|p| p.children(&key).len())
            .unwrap_or(0);
        if index >= len || index == from {
            return false;
        }
        let Some(Op::Insert { elements, .. }) = self.doc.apply_op(Op::Remove {
            parent: parent.clone(),
            key: key.clone(),
            index: Some(from),
            drop_empty: false,
        }) else {
            return false;
        };
        self.inverse.push(Op::Insert {
            parent: parent.clone(),
            key: key.clone(),
            index: Some(from),
            elements: elements.clone(),
        });
        self.apply(Op::Insert {
            parent,
            key,
            index: Some(index),
            elements,
        });
        true
    }

    /// Remove `id` and its owned descendants. References to it elsewhere are
    /// left alone; see [`crate::uml::delete_elements`] for a cascading delete.
    pub fn remove(&mut self, id: &str) -> bool {
        let Some((parent, key, index)) = self.doc.position_of(id) else {
            return false;
        };
        self.apply(Op::Remove {
            parent,
            key,
            index,
            drop_empty: false,
        });
        true
    }
}

impl Document {
    /// Apply ops in order; return the ops that reverse them, in apply order.
    pub(crate) fn apply_all(&mut self, ops: Vec<Op>) -> Vec<Op> {
        let mut inverse: Vec<Op> = ops.into_iter().filter_map(|op| self.apply_op(op)).collect();
        inverse.reverse();
        inverse
    }

    pub(crate) fn apply_op(&mut self, op: Op) -> Option<Op> {
        self.revision = self.revision.wrapping_add(1);
        match op {
            Op::Set { id, key, value } => {
                let el = self.elements.get_mut(&id)?;
                let old = match value {
                    Some(v) => el.fields.insert(key.clone(), v),
                    None => el.fields.shift_remove(&key),
                };
                Some(Op::Set {
                    id,
                    key,
                    value: old,
                })
            }
            Op::Insert {
                parent,
                key,
                index,
                mut elements,
            } => {
                let first = elements.first_mut()?;
                first.parent = Some(parent.clone());
                let root = first.id.clone();
                let p = self.elements.get_mut(&parent)?;
                let created = !p.fields.contains_key(&key);
                let actual = match index {
                    None => {
                        p.fields.insert(key.clone(), Field::Child(root));
                        None
                    }
                    Some(i) => {
                        let slot = p
                            .fields
                            .entry(key.clone())
                            .or_insert_with(|| Field::Children(Vec::new()));
                        if !matches!(slot, Field::Children(_)) {
                            // An empty list loads as a plain value; promote it.
                            debug_assert!(
                                matches!(slot, Field::Value(Value::Array(a)) if a.is_empty())
                            );
                            *slot = Field::Children(Vec::new());
                        }
                        let Field::Children(list) = slot else {
                            return None;
                        };
                        let i = i.min(list.len());
                        list.insert(i, root);
                        Some(i)
                    }
                };
                for el in elements {
                    self.elements.insert(el.id.clone(), el);
                }
                Some(Op::Remove {
                    parent,
                    key,
                    index: actual,
                    drop_empty: created,
                })
            }
            Op::Remove {
                parent,
                key,
                index,
                drop_empty,
            } => {
                let p = self.elements.get_mut(&parent)?;
                let root = match index {
                    None => match p.fields.shift_remove(&key)? {
                        Field::Child(id) => id,
                        other => {
                            p.fields.insert(key, other);
                            return None;
                        }
                    },
                    Some(i) => match p.fields.get_mut(&key)? {
                        Field::Children(list) if i < list.len() => {
                            let id = list.remove(i);
                            if drop_empty && list.is_empty() {
                                p.fields.shift_remove(&key);
                            }
                            id
                        }
                        _ => return None,
                    },
                };
                let elements = self
                    .subtree_ids(&root)
                    .into_iter()
                    .filter_map(|id| self.elements.remove(&id))
                    .collect();
                Some(Op::Insert {
                    parent,
                    key,
                    index,
                    elements,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(id: &str) -> Element {
        Element::new(id.into(), "UMLClass").with("name", Field::str(id))
    }

    #[test]
    fn undo_and_redo_restore_exact_state() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        let before = doc.to_value();

        doc.edit(|e| {
            let id = e.append(&model, "ownedElements", vec![class("A")]);
            e.set(&id, "name", Field::str("Renamed"));
            e.append(
                &id,
                "attributes",
                vec![Element::new("X".into(), "UMLAttribute")],
            );
        });
        let after = doc.to_value();
        assert_ne!(before, after);
        assert_eq!(doc.get("A").unwrap().name(), "Renamed");
        assert!(doc.is_modified());

        assert!(doc.undo());
        assert_eq!(doc.to_value(), before);
        assert!(doc.get("X").is_none());

        assert!(doc.redo());
        assert_eq!(doc.to_value(), after);

        doc.edit(|e| assert!(e.remove("A")));
        assert!(doc.get("X").is_none());
        doc.undo();
        assert_eq!(doc.to_value(), after);
    }

    #[test]
    fn reorder_moves_and_undoes() {
        let mut doc = Document::new_project();
        let model = doc.get(doc.root()).unwrap().children("ownedElements")[0].clone();
        doc.edit(|e| {
            e.append(&model, "ownedElements", vec![class("A")]);
            e.append(&model, "ownedElements", vec![class("B")]);
        });
        let before = doc.to_value();
        assert!(doc.edit(|e| e.reorder("B", 1)));
        assert_eq!(doc.get(&model).unwrap().children("ownedElements")[1], "B");
        assert!(!doc.edit(|e| e.reorder("B", 99)));
        doc.undo();
        assert_eq!(doc.to_value(), before);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut doc = Document::new_project();
        let root = doc.root().to_owned();
        doc.edit(|e| e.set(&root, "name", Field::str("One")));
        doc.undo();
        assert!(doc.can_redo());
        doc.edit(|e| e.set(&root, "name", Field::str("Two")));
        assert!(!doc.can_redo());
    }

    #[test]
    fn unchanged_set_records_nothing() {
        let mut doc = Document::new_project();
        let root = doc.root().to_owned();
        doc.edit(|e| e.set(&root, "name", Field::str("Untitled")));
        assert!(!doc.can_undo());
    }
}
