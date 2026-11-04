//! Themed egui controls. Every colour comes from [`egui_mev_theme::Palette`] and
//! every shared size from [`egui_mev_theme::Metrics`], so a widget renders
//! whichever look the app installed. Where macOS and Windows 11 build a control
//! differently, it branches on [`egui_mev_theme::ThemeFamily`].
//!
//! Icons are [`egui_mev_theme::Glyph`]s, which a `char` or (with the theme's
//! `lucide` feature) a `lucide_icons::Icon` converts into. The chevrons, marks
//! and status icons the widgets draw for themselves are painted, so plain
//! widgets need no icon font.
//!
//! Toasts keep their state in the [`egui::Context`]; everything else keeps it
//! in egui memory or in a struct the caller owns.

pub mod breadcrumbs;
pub mod buttons;
pub mod containers;
pub mod dialogs;
pub mod forms;
mod marks;
pub mod navigation;
pub mod text_fit;
pub mod toasts;

#[cfg(feature = "autocomplete")]
pub mod autocomplete;
#[cfg(feature = "picker")]
pub mod picker;

use egui_mev_core::selection::SelectMods;

pub use breadcrumbs::{Fit, fit};
pub use buttons::{
    ButtonLook, Tip, danger, danger_button, destructive, destructive_button, painted_button,
    primary, primary_button, secondary, secondary_button, small_icon_button, small_text_button,
    tool_button, toolbar_icon, toolbar_icon_enabled,
};
pub use containers::{
    BannerKind, banner, card, card_frame, collapsible, empty_state, group_title, inspector_section,
    inspector_section_opening, page_heading, pill, section_header,
};
pub use dialogs::{
    ButtonKind, DialogButton, DialogResult, dialog_buttons, focus_on_open, progress, sheet,
    sheet_labeled, sheet_with, take_preview_sheet_rect,
};
pub use forms::{
    SettingsGroup, color_swatch_row, field_row, form_label, form_label_width, grouped_form,
    key_caps, kv_row, kv_row_colored, kv_row_mono, settings_group, slider_row, slider_row_suffix,
    text_field, toggle_row, toggle_switch, ui_labeled_row, ui_labeled_row_width,
};
pub use navigation::{
    TreeRow, TreeRowResponse, TreeRowTrailing, nav_row, search_field, segmented, segmented_control,
    segmented_width, split_button, tab, tab_strip, tree_row,
};
pub use toasts::{ToastKind, Toasts};

/// The selection gesture `modifiers` stand for.
#[must_use]
pub const fn select_mods(modifiers: egui::Modifiers) -> SelectMods {
    SelectMods {
        command: modifiers.command,
        shift: modifiers.shift,
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use egui_mev_theme::{AccentShades, Theme, ThemeFamily, fonts, presets};

    /// One frame on `ctx` at `time`, with `events`, dropping the texture
    /// updates no renderer is there to apply.
    pub fn frame_at(
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
        ui: impl FnMut(&mut egui::Ui),
    ) {
        frame_output(ctx, time, events, ui);
    }

    /// [`frame_at`] that keeps the output, to look at what was painted.
    pub fn frame_output(
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
        mut ui: impl FnMut(&mut egui::Ui),
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            time: Some(time),
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |inner| ui(inner));
        output.textures_delta.clear();
        output
    }

    /// A context with the Graphite theme and test fonts, in the given mode.
    pub fn themed(mode: egui::Theme) -> egui::Context {
        themed_with(
            Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT),
            mode,
        )
    }

    /// The default theme of `family`.
    pub fn family_theme(family: ThemeFamily) -> Theme {
        let accent = match family {
            ThemeFamily::Graphite => AccentShades::MACOS_DEFAULT,
            ThemeFamily::Fluent => AccentShades::WINDOWS_DEFAULT,
        };
        Theme::for_family(family, accent)
    }

    /// A context with the default theme of `family` and test fonts, in the given mode.
    pub fn themed_family(family: ThemeFamily, mode: egui::Theme) -> egui::Context {
        themed_with(family_theme(family), mode)
    }

    fn themed_with(theme: Theme, mode: egui::Theme) -> egui::Context {
        let ctx = egui::Context::default();
        theme.install(&ctx);
        fonts::install_test(&ctx, &fonts::FontOptions::default());
        ctx.set_theme(mode);
        // Fonts bind, and the mode applies, on the next frame.
        frame_at(&ctx, 0.0, Vec::new(), |_| {});
        ctx
    }

    /// Every filled rect painted this frame, with its fill.
    pub fn painted_rects(output: &egui::FullOutput) -> Vec<(egui::Rect, egui::Color32)> {
        fn collect(shape: &egui::Shape, rects: &mut Vec<(egui::Rect, egui::Color32)>) {
            match shape {
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, rects);
                    }
                }
                egui::Shape::Rect(rect) => rects.push((rect.rect, rect.fill)),
                _ => {}
            }
        }
        let mut rects = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut rects);
        }
        rects
    }

    /// The radius of every filled circle painted this frame.
    pub fn painted_circles(output: &egui::FullOutput) -> Vec<f32> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Circle(circle) => Some(circle.radius),
                _ => None,
            })
            .collect()
    }

    /// Every colour painted on `ctx` this frame, gathered from its shapes.
    pub fn painted_colors(output: &egui::FullOutput) -> Vec<egui::Color32> {
        let mut colors = Vec::new();
        for clipped in &output.shapes {
            collect_colors(&clipped.shape, &mut colors);
        }
        colors
    }

    fn collect_colors(shape: &egui::Shape, colors: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_colors(shape, colors);
                }
            }
            egui::Shape::Rect(rect) => {
                colors.push(rect.fill);
                colors.push(rect.stroke.color);
            }
            egui::Shape::Text(text) => {
                colors.push(text.fallback_color);
                if let Some(section) = text.galley.job.sections.first() {
                    colors.push(section.format.color);
                }
            }
            egui::Shape::LineSegment { stroke, .. } => colors.push(stroke.color),
            egui::Shape::Circle(circle) => {
                colors.push(circle.fill);
                colors.push(circle.stroke.color);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::Modifiers;

    use super::*;

    #[test]
    fn select_mods_reads_command_and_shift() {
        assert_eq!(select_mods(Modifiers::NONE), SelectMods::default());
        assert_eq!(
            select_mods(Modifiers::COMMAND),
            SelectMods {
                command: true,
                shift: false
            }
        );
        assert_eq!(
            select_mods(Modifiers::SHIFT),
            SelectMods {
                command: false,
                shift: true
            }
        );
        assert_eq!(
            select_mods(Modifiers::COMMAND | Modifiers::SHIFT),
            SelectMods {
                command: true,
                shift: true
            }
        );
    }

    #[test]
    fn select_mods_ignores_other_modifiers() {
        assert_eq!(select_mods(Modifiers::ALT), SelectMods::default());
        // Ctrl alone is not the platform's command key, so not a gesture of its own.
        assert_eq!(
            select_mods(Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            }),
            SelectMods::default()
        );
    }
}
