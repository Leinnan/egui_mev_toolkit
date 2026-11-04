use egui::{Context, Ui};

use crate::theme::Theme;

/// Sizing tokens: corner radii, control heights, spacing and the type ramp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Corner radius of buttons, inputs, rows and segments (px).
    pub control_radius: u8,
    /// Corner radius of cards and banners (px).
    pub card_radius: u8,
    /// Corner radius of dialogs, flyouts and windows (px).
    pub overlay_radius: u8,
    /// Corner radius of a docked pane (px).
    pub pane_radius: u8,
    /// Height of prominent custom buttons and text fields (px).
    pub control_height: f32,
    /// Height of egui's built-in interactive widgets (px).
    pub interact_height: f32,
    /// Height of a list or tree row (px).
    pub row_height: f32,
    /// Height of a single-line navigation row (px).
    pub nav_row_height: f32,
    /// Height of a two-line navigation row (px).
    pub nav_row_height_two_line: f32,
    /// Vertical gap between stacked cards and sections (px).
    pub card_gap: f32,
    /// Inner padding of a card (px).
    pub card_padding: i8,
    /// Fixed width of the label column in form rows (px).
    pub label_width: f32,
    /// Gap between docked panes, where the body shows through (px).
    pub pane_gap: f32,
    /// Inner padding of a docked pane's body (px).
    pub pane_padding: i8,
    /// Caption text size (px).
    pub caption: f32,
    /// Body text size (px).
    pub body: f32,
    /// Subtitle text size: dialog and item titles (px).
    pub subtitle: f32,
    /// Page title text size (px).
    pub title: f32,
}

impl Metrics {
    /// Compact macOS-style sizing.
    pub const GRAPHITE: Self = Self {
        control_radius: 6,
        card_radius: 8,
        overlay_radius: 12,
        pane_radius: 8,
        control_height: 30.0,
        interact_height: 22.0,
        row_height: 26.0,
        nav_row_height: 26.0,
        nav_row_height_two_line: 40.0,
        card_gap: 8.0,
        card_padding: 10,
        label_width: 116.0,
        pane_gap: 4.0,
        pane_padding: 8,
        caption: 12.0,
        body: 13.0,
        subtitle: 15.0,
        title: 17.0,
    };

    /// Dense sizing for developer tools.
    pub const DENSE: Self = Self {
        control_radius: 4,
        card_radius: 8,
        overlay_radius: 8,
        pane_radius: 8,
        control_height: 22.0,
        interact_height: 22.0,
        row_height: 24.0,
        nav_row_height: 24.0,
        nav_row_height_two_line: 36.0,
        card_gap: 6.0,
        card_padding: 8,
        label_width: 116.0,
        pane_gap: 4.0,
        pane_padding: 8,
        caption: 11.0,
        body: 13.0,
        subtitle: 15.0,
        title: 17.0,
    };

    /// WinUI 3 design tokens.
    pub const FLUENT: Self = Self {
        control_radius: 4,
        card_radius: 4,
        overlay_radius: 8,
        pane_radius: 4,
        control_height: 32.0,
        interact_height: 32.0,
        row_height: 36.0,
        nav_row_height: 36.0,
        nav_row_height_two_line: 48.0,
        card_gap: 8.0,
        card_padding: 12,
        label_width: 140.0,
        pane_gap: 4.0,
        pane_padding: 12,
        caption: 12.0,
        body: 14.0,
        subtitle: 20.0,
        title: 28.0,
    };

    /// The installed metrics of the context `ui` belongs to.
    #[must_use]
    pub fn of(ui: &Ui) -> Self {
        Theme::of(ui.ctx()).metrics
    }

    /// The installed metrics of `ctx`.
    #[must_use]
    pub fn of_ctx(ctx: &Context) -> Self {
        Theme::of(ctx).metrics
    }
}
