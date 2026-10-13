//! The egui user interface, shared by the desktop and web apps.
//!
//! The UI is thin: every change goes through an `engine` command. File
//! dialogs and disk access are injected through [`FileService`], so this
//! crate stays free of platform code and builds for WebAssembly.

mod app;
mod canvas;
mod explorer;
mod fonts;
mod platform;
mod properties;

pub use app::{Capture, Remote, RemoteJob, UmlApp};
pub use platform::{FileEvent, FileKind, FileService};

/// Shown in the window title and About box. Deliberately not "StarUML":
/// this is an independent, unaffiliated reimplementation.
pub const APP_NAME: &str = "rust-uml";
