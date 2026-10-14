//! Session state and the command registry.
//!
//! Every user-visible action is a command with an id and JSON parameters.
//! The desktop and web UI, the CLI and any automation dispatch through
//! [`Registry::run`], so they behave identically. Command ids follow
//! StarUML's style where it has one (`edit.undo`, `uml.class` → here
//! `node.create` with `kind: "Class"`).
//!
//! Parameters are untrusted: every command returns an error for missing,
//! mistyped or out-of-range input and never panics.

mod commands;
mod import;
mod params;
mod quick_edit;
mod session;
mod validate;

pub use commands::{CommandError, CommandSpec, Registry};
pub use session::Session;
