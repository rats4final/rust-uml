//! Turns a diagram into a display list ([`Scene`]) that any backend can draw:
//! the egui canvas, SVG export, and later PNG/PDF.
//!
//! Notation follows the UML specification; measurements are this crate's own.
//! Text is measured with Liberation Sans, which is metric-compatible with the
//! Arial StarUML uses, so box sizes come out close to StarUML's.

pub mod autolayout;
mod geom;
mod layout;
mod png;
mod route;
mod scene;
mod svg;
mod text;

pub use geom::{Color, Point, Rect};
pub use layout::{NodeLayout, node_layout};
pub use png::{encode_rgba_png, to_png};
pub use route::reflow;
pub use scene::{Align, Hit, HitShape, Prim, Scene, TextStyle, build_scene};
pub use svg::to_svg;
pub use text::{FONT_BOLD, FONT_BOLD_ITALIC, FONT_ITALIC, FONT_REGULAR, Fonts};
