//! A shared egui look. An app picks a [`Theme`] and installs it once per
//! context; widgets read it back with [`Palette::of`] and [`Metrics::of`].
//!
//! ```ignore
//! Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT).install(&cc.egui_ctx);
//! fonts::install_system(&cc.egui_ctx, &FontOptions::default());
//! ```

pub mod contrast;
#[cfg(feature = "fluent")]
pub mod fluent;
pub mod fonts;
pub mod icons;
mod metrics;
mod palette;
pub mod presets;
mod theme;
#[cfg(all(feature = "fluent", windows))]
pub mod win32;

pub use fonts::FontOptions;
pub use icons::{Glyph, IconFont};
pub use metrics::Metrics;
pub use palette::Palette;
pub use theme::{Appearance, Theme, standard_style, type_scale};

/// One frame on `ctx`, dropping the texture updates no renderer is there to apply.
#[cfg(test)]
pub(crate) fn run_frame(ctx: &egui::Context, mut ui: impl FnMut(&mut egui::Ui)) {
    let mut output = ctx.run_ui(egui::RawInput::default(), |inner| ui(inner));
    output.textures_delta.clear();
}
