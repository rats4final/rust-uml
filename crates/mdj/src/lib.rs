//! Reading, editing and writing StarUML `.mdj` / `.mfj` model files.
//!
//! The loader is schema-free: it recognises owned children (`{"_type": …}`) and
//! references (`{"$ref": …}`) by shape, so files containing element types this
//! crate knows nothing about still load and save without losing data.
//!
//! No filesystem or clock access happens here beyond `web-time`, so the crate
//! builds unchanged for native targets and `wasm32-unknown-unknown`.

mod document;
mod field;
mod id;
mod undo;
pub mod values;

pub use document::{Document, Element, LoadError};
pub use field::Field;
pub use id::IdGen;
pub use undo::{Editor, History, Op};

/// Element identifier as stored in `_id`.
pub type Id = String;
