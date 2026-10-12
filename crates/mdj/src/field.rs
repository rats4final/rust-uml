use serde_json::Value;

use crate::Id;

/// One field of an element, classified by its shape in the file.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    /// Anything that is not an element or reference: strings, numbers,
    /// booleans, enum words, `Points`/`Font` text, empty arrays, or data we
    /// don't understand. Written back unchanged.
    Value(Value),
    /// `{"$ref": id}`
    Ref(Id),
    /// `[{"$ref": id}, …]`
    Refs(Vec<Id>),
    /// An owned element nested inline.
    Child(Id),
    /// A list of owned elements nested inline.
    Children(Vec<Id>),
}

impl Field {
    pub fn str(s: impl Into<String>) -> Self {
        Field::Value(Value::String(s.into()))
    }

    pub fn bool(b: bool) -> Self {
        Field::Value(Value::Bool(b))
    }

    pub fn int(n: i64) -> Self {
        Field::Value(Value::from(n))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Field::Value(Value::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Field::Value(Value::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Field::Value(Value::Number(n)) => n.as_f64(),
            _ => None,
        }
    }

    pub fn as_ref_id(&self) -> Option<&str> {
        match self {
            Field::Ref(id) => Some(id),
            _ => None,
        }
    }

    /// Ids of owned children, treating an empty JSON array as an empty list.
    pub fn children(&self) -> &[Id] {
        match self {
            Field::Children(ids) => ids,
            Field::Child(id) => std::slice::from_ref(id),
            _ => &[],
        }
    }

    /// Ids this field points at without owning.
    pub fn refs(&self) -> &[Id] {
        match self {
            Field::Ref(id) => std::slice::from_ref(id),
            Field::Refs(ids) => ids,
            _ => &[],
        }
    }
}
