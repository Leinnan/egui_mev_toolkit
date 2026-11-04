//! Controls for drawing tools: a response-curve editor, a colour editor with
//! a hue ring and triangle, and rulers for a canvas. Everything reads its
//! colours from the installed [`egui_mev_theme::Palette`].
//!
//! Colours cross the public boundary as straight (unmultiplied) RGBA `[u8; 4]`,
//! because `egui::Color32` is premultiplied and loses the colour of low-alpha
//! values.

pub mod color_editor;
pub mod curve;
pub mod rulers;
pub mod units;

pub use color_editor::{Hsva, color_editor, hue_triangle};
pub use curve::{curve_editor, curve_thumbnail};
pub use units::Unit;
