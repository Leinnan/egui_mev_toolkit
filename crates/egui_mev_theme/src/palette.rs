use egui::{Color32, Context, Ui};

use crate::theme::Theme;

/// Colours named by role (text, card, accent, …) rather than by value, so the
/// same widget code renders every look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Primary text.
    pub text: Color32,
    /// Secondary text (captions, subtitles, card titles).
    pub text_weak: Color32,
    /// Form labels next to controls.
    pub text_label: Color32,
    /// Placeholder / hint text and decorative icons.
    pub hint_text: Color32,
    /// The canvas behind docked panes: the gaps between them.
    pub body_bg: Color32,
    /// Central content background.
    pub panel_bg: Color32,
    /// Sidebar / navigation pane background.
    pub sidebar_bg: Color32,
    /// Toolbar background.
    pub toolbar_bg: Color32,
    /// The active tab of a pane's tab strip.
    pub pane_tab_active: Color32,
    /// Floating windows, menus and dialogs.
    pub window_bg: Color32,
    /// Cards and button faces.
    pub card_bg: Color32,
    /// Neutral hover fill for cards, segments and buttons.
    pub card_bg_hover: Color32,
    /// Selected card or row fill.
    pub card_bg_selected: Color32,
    /// Card border.
    pub card_stroke: Color32,
    /// Grouped areas and code wells.
    pub section_bg: Color32,
    /// Text-field wells and other recessed surfaces.
    pub extreme_bg: Color32,
    /// Slider rails, switch tracks and checkbox wells.
    pub control_track: Color32,
    /// Standard 1px control border.
    pub border: Color32,
    /// Hairline dividers and pill fills.
    pub border_subtle: Color32,
    /// Accent colour for fills, links and focus.
    pub accent: Color32,
    /// Accent fill while hovered.
    pub accent_hover: Color32,
    /// Dim accent, behind a selected segmented option.
    pub accent_dim: Color32,
    /// Second accent, for links and non-brand selection.
    pub accent_secondary: Color32,
    /// Text and glyphs drawn on top of an accent fill.
    pub on_accent: Color32,
    /// Selected rows, segments and text selection.
    pub selection_bg: Color32,
    /// Hover fill for borderless (subtle) items such as navigation rows.
    pub subtle_hover: Color32,
    /// Selected fill for borderless (subtle) items such as navigation rows.
    pub subtle_selected: Color32,
    /// Positive status (connected, saved).
    pub success: Color32,
    /// Warning status.
    pub caution: Color32,
    /// Errors and destructive actions.
    pub critical: Color32,
    /// Neutral status dot.
    pub neutral: Color32,
    /// Small section headers.
    pub section_label: Color32,
    /// Large ghost glyphs in empty-state placeholders.
    pub ghost: Color32,
    /// Backdrop behind modal dialogs.
    pub smoke: Color32,
    /// Informational banner background.
    pub info_bg: Color32,
    /// Success banner background.
    pub success_bg: Color32,
    /// Warning banner background.
    pub caution_bg: Color32,
    /// Error banner background.
    pub critical_bg: Color32,
}

impl Palette {
    /// The installed palette for the mode `ui` draws in.
    #[must_use]
    pub fn of(ui: &Ui) -> Self {
        Theme::of(ui.ctx()).palette(ui.visuals().dark_mode)
    }

    /// The installed palette for the context's current mode, where no `Ui` is at hand.
    #[must_use]
    pub fn of_ctx(ctx: &Context) -> Self {
        Theme::of(ctx).palette(ctx.theme() == egui::Theme::Dark)
    }
}
