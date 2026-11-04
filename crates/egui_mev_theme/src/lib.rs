//! A shared egui look. An app picks a [`Theme`] and installs it once per
//! context; widgets read it back with [`Palette::of`], [`Metrics::of`] and
//! [`ThemeFamily::of`].
//!
//! ```ignore
//! // The look of the OS this runs on, following its light or dark setting.
//! Theme::system().install_with_fonts(&cc.egui_ctx, &FontOptions::default());
//! ```

mod accent;
pub mod contrast;
pub mod fluent;
pub mod fonts;
pub mod icons;
mod metrics;
mod palette;
pub mod presets;
mod theme;
#[cfg(all(feature = "native", windows))]
pub mod win32;

pub use accent::AccentShades;
pub use fonts::FontOptions;
pub use icons::{Glyph, IconFont};
pub use metrics::Metrics;
pub use palette::Palette;
pub use theme::{Appearance, StyleFn, Theme, ThemeFamily, standard_style, type_scale};

/// One frame on `ctx`, dropping the texture updates no renderer is there to apply.
#[cfg(test)]
pub(crate) fn run_frame(ctx: &egui::Context, mut ui: impl FnMut(&mut egui::Ui)) {
    let mut output = ctx.run_ui(egui::RawInput::default(), |inner| ui(inner));
    output.textures_delta.clear();
}
