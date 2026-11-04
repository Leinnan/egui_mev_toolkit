//! WinUI 3 (Fluent) `Visuals` and `Style` for egui's built-in widgets.
//!
//! Buttons use the neutral control fill with a 1px control stroke, inputs a
//! recessed well with an accent focus stroke, and selection a tinted accent.
//! Corner radii, control height, scroll bars and shadows follow the WinUI
//! design tokens in [`Metrics::FLUENT`].

use egui::{Color32, CornerRadius, Margin, Shadow, Stroke, Style, Theme, Visuals, style};

use crate::{AccentShades, Metrics, Palette, Theme as MevTheme, ThemeFamily};

/// Control fills (rest, hover, pressed) — WinUI `ControlFillColor*`.
const fn control_fills(theme: Theme) -> (Color32, Color32, Color32) {
    match theme {
        Theme::Dark => (
            Color32::from_rgb(0x2D, 0x2D, 0x2D),
            Color32::from_rgb(0x32, 0x32, 0x32),
            Color32::from_rgb(0x27, 0x27, 0x27),
        ),
        Theme::Light => (
            Color32::from_rgb(0xFB, 0xFB, 0xFB),
            Color32::from_rgb(0xF6, 0xF6, 0xF6),
            Color32::from_rgb(0xF5, 0xF5, 0xF5),
        ),
    }
}

/// Builds the Fluent [`Style`] for `theme`, starting from `base` so text
/// styles registered by the font setup are preserved.
#[must_use]
pub fn style(base: &Style, theme: Theme, palette: &Palette, m: &Metrics) -> Style {
    let mut style = base.clone();
    crate::type_scale(&mut style, m);
    style.visuals = visuals(theme, palette, m);

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(11.0, 5.0);
    style.spacing.interact_size.y = m.control_height;
    style.spacing.indent = 16.0;
    style.spacing.window_margin = Margin::same(16);
    style.spacing.menu_margin = Margin::same(4);
    style.spacing.scroll = style::ScrollStyle::floating();
    style
}

fn visuals(theme: Theme, p: &Palette, m: &Metrics) -> Visuals {
    let (rest, hover, pressed) = control_fills(theme);
    let control = CornerRadius::same(m.control_radius);

    let mut v = match theme {
        Theme::Dark => Visuals::dark(),
        Theme::Light => Visuals::light(),
    };
    v.override_text_color = None;
    v.weak_text_color = Some(p.hint_text);
    v.hyperlink_color = p.accent;
    v.panel_fill = p.panel_bg;
    v.window_fill = p.window_bg;
    v.faint_bg_color = p.card_bg;
    v.extreme_bg_color = p.extreme_bg;
    v.text_edit_bg_color = Some(p.extreme_bg);
    v.code_bg_color = p.card_bg;
    v.warn_fg_color = p.caution;
    v.error_fg_color = p.critical;
    v.selection.bg_fill = p.selection_bg;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.slider_trailing_fill = true;
    v.handle_shape = style::HandleShape::Circle;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(m.overlay_radius);
    v.menu_corner_radius = CornerRadius::same(m.overlay_radius);
    v.striped = false;

    let shadow_alpha = if theme == Theme::Dark { 0x60 } else { 0x24 };
    v.window_shadow = Shadow {
        offset: [0, 16],
        blur: 48,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: [0, 8],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.card_bg;
    w.noninteractive.weak_bg_fill = p.card_bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border_subtle);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = control;

    w.inactive.bg_fill = p.control_track;
    w.inactive.weak_bg_fill = rest;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = control;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = p.control_track;
    w.hovered.weak_bg_fill = hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.border);
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.corner_radius = control;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.accent;
    w.active.weak_bg_fill = pressed;
    w.active.bg_stroke = Stroke::new(1.0, p.border);
    // Full `text`, not `text_weak`: egui's strong text reads this colour too.
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.corner_radius = control;
    w.active.expansion = 0.0;

    w.open.bg_fill = pressed;
    w.open.weak_bg_fill = pressed;
    w.open.bg_stroke = Stroke::new(1.0, p.border);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = control;
    w.open.expansion = 0.0;
    v
}

/// Builds the Fluent (WinUI 3) dark palette around `accent`.
///
/// Translucent WinUI brushes are pre-blended over the Mica base `#202020`.
#[must_use]
pub fn fluent_dark(accent: AccentShades) -> Palette {
    let fill = accent.light2;
    Palette {
        text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        text_weak: Color32::from_rgb(0xCF, 0xCF, 0xCF),
        text_label: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        hint_text: Color32::from_rgb(0x9D, 0x9D, 0x9D),
        panel_bg: Color32::from_rgb(0x27, 0x27, 0x27),
        sidebar_bg: Color32::from_rgb(0x20, 0x20, 0x20),
        toolbar_bg: Color32::from_rgb(0x20, 0x20, 0x20),
        window_bg: Color32::from_rgb(0x2C, 0x2C, 0x2C),
        card_bg: Color32::from_rgb(0x2E, 0x2E, 0x2E),
        card_bg_hover: Color32::from_rgb(0x34, 0x34, 0x34),
        card_stroke: Color32::from_rgb(0x1C, 0x1C, 0x1C),
        extreme_bg: Color32::from_rgb(0x1F, 0x1F, 0x1F),
        control_track: Color32::from_rgb(0x5C, 0x5C, 0x5C),
        border: Color32::from_rgb(0x3C, 0x3C, 0x3C),
        border_subtle: Color32::from_rgb(0x35, 0x35, 0x35),
        accent: fill,
        accent_hover: fill.lerp_to_gamma(Color32::from_rgb(0x20, 0x20, 0x20), 0.1),
        on_accent: Color32::BLACK,
        selection_bg: accent
            .base
            .lerp_to_gamma(Color32::from_rgb(0x27, 0x27, 0x27), 0.45),
        subtle_hover: Color32::from_rgb(0x2D, 0x2D, 0x2D),
        subtle_selected: Color32::from_rgb(0x32, 0x32, 0x32),
        body_bg: Color32::from_rgb(0x20, 0x20, 0x20),
        pane_tab_active: Color32::from_rgb(0x2E, 0x2E, 0x2E),
        card_bg_selected: Color32::from_rgb(0x32, 0x32, 0x32),
        section_bg: Color32::from_rgb(0x2B, 0x2B, 0x2B),
        accent_dim: accent.dark1,
        accent_secondary: fill,
        ghost: Color32::from_rgb(0x5C, 0x5C, 0x5C),
        success: Color32::from_rgb(0x6C, 0xCB, 0x5F),
        caution: Color32::from_rgb(0xFC, 0xE1, 0x00),
        critical: Color32::from_rgb(0xFF, 0x99, 0xA4),
        neutral: Color32::from_rgb(0x9D, 0x9D, 0x9D),
        section_label: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        smoke: Color32::from_black_alpha(0x4D),
        info_bg: Color32::from_rgb(0x2B, 0x2B, 0x2B),
        success_bg: Color32::from_rgb(0x39, 0x3D, 0x1B),
        caution_bg: Color32::from_rgb(0x43, 0x35, 0x19),
        critical_bg: Color32::from_rgb(0x44, 0x27, 0x26),
    }
}

/// Builds the Fluent (WinUI 3) light palette around `accent`.
///
/// Translucent WinUI brushes are pre-blended over the Mica base `#F3F3F3`.
#[must_use]
pub fn fluent_light(accent: AccentShades) -> Palette {
    let fill = accent.dark1;
    Palette {
        text: Color32::from_rgb(0x1B, 0x1B, 0x1B),
        text_weak: Color32::from_rgb(0x61, 0x61, 0x61),
        text_label: Color32::from_rgb(0x1B, 0x1B, 0x1B),
        hint_text: Color32::from_rgb(0x8A, 0x8A, 0x8A),
        panel_bg: Color32::from_rgb(0xF9, 0xF9, 0xF9),
        sidebar_bg: Color32::from_rgb(0xF3, 0xF3, 0xF3),
        toolbar_bg: Color32::from_rgb(0xF3, 0xF3, 0xF3),
        window_bg: Color32::from_rgb(0xF9, 0xF9, 0xF9),
        card_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        card_bg_hover: Color32::from_rgb(0xF6, 0xF6, 0xF6),
        card_stroke: Color32::from_rgb(0xE5, 0xE5, 0xE5),
        extreme_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        control_track: Color32::from_rgb(0xC4, 0xC4, 0xC4),
        border: Color32::from_rgb(0xE0, 0xE0, 0xE0),
        border_subtle: Color32::from_rgb(0xE5, 0xE5, 0xE5),
        accent: fill,
        accent_hover: fill.lerp_to_gamma(Color32::from_rgb(0xF3, 0xF3, 0xF3), 0.1),
        on_accent: Color32::WHITE,
        selection_bg: accent.base.lerp_to_gamma(Color32::WHITE, 0.75),
        subtle_hover: Color32::from_rgb(0xEA, 0xEA, 0xEA),
        subtle_selected: Color32::from_rgb(0xE5, 0xE5, 0xE5),
        body_bg: Color32::from_rgb(0xF3, 0xF3, 0xF3),
        pane_tab_active: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        card_bg_selected: Color32::from_rgb(0xE5, 0xE5, 0xE5),
        section_bg: Color32::from_rgb(0xF6, 0xF6, 0xF6),
        accent_dim: accent.light3,
        accent_secondary: fill,
        ghost: Color32::from_rgb(0xC4, 0xC4, 0xC4),
        success: Color32::from_rgb(0x0F, 0x7B, 0x0F),
        caution: Color32::from_rgb(0x9D, 0x5D, 0x00),
        critical: Color32::from_rgb(0xC4, 0x2B, 0x1C),
        neutral: Color32::from_rgb(0x8A, 0x8A, 0x8A),
        section_label: Color32::from_rgb(0x1B, 0x1B, 0x1B),
        smoke: Color32::from_black_alpha(0x4D),
        info_bg: Color32::from_rgb(0xF6, 0xF6, 0xF6),
        success_bg: Color32::from_rgb(0xDF, 0xF6, 0xDD),
        caution_bg: Color32::from_rgb(0xFF, 0xF4, 0xCE),
        critical_bg: Color32::from_rgb(0xFD, 0xE7, 0xE9),
    }
}

/// Derives the palette used over the Mica backdrop: the window chrome
/// (sidebar, toolbar) becomes fully transparent so Mica shows through, and
/// content surfaces use the translucent WinUI layer and card brushes.
#[must_use]
pub fn mica_variant(opaque: &Palette, theme: egui::Theme) -> Palette {
    let (layer, card, card_hover) = match theme {
        // LayerFillColorDefault #3A3A3A @30%, CardBackgroundFillColorDefault
        // #FFFFFF @5%, CardBackgroundFillColorSecondary-ish hover.
        egui::Theme::Dark => (
            Color32::from_rgba_unmultiplied(0x3A, 0x3A, 0x3A, 0x4D),
            Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x0D),
            Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x15),
        ),
        // LayerFillColorDefault #FFFFFF @50%, CardBackgroundFillColorDefault
        // #FFFFFF @70%.
        egui::Theme::Light => (
            Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x80),
            Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0xB3),
            Color32::from_rgba_unmultiplied(0xF9, 0xF9, 0xF9, 0x80),
        ),
    };
    Palette {
        panel_bg: layer,
        sidebar_bg: Color32::TRANSPARENT,
        toolbar_bg: Color32::TRANSPARENT,
        card_bg: card,
        card_bg_hover: card_hover,
        ..*opaque
    }
}

/// A Fluent theme around `accent`, following the app's appearance.
#[must_use]
pub fn theme(accent: AccentShades) -> MevTheme {
    MevTheme {
        style,
        family: ThemeFamily::Fluent,
        ..MevTheme::new(fluent_dark(accent), fluent_light(accent)).with_metrics(Metrics::FLUENT)
    }
}

/// A Fluent theme for a window showing Mica through its transparent chrome.
#[must_use]
pub fn mica_theme(accent: AccentShades) -> MevTheme {
    MevTheme {
        style,
        family: ThemeFamily::Fluent,
        ..MevTheme::new(
            mica_variant(&fluent_dark(accent), Theme::Dark),
            mica_variant(&fluent_light(accent), Theme::Light),
        )
        .with_metrics(Metrics::FLUENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluent_palettes_differ_by_mode() {
        let dark = fluent_dark(AccentShades::WINDOWS_DEFAULT);
        let light = fluent_light(AccentShades::WINDOWS_DEFAULT);
        assert_ne!(dark.panel_bg, light.panel_bg);
        assert_eq!(dark.on_accent, Color32::BLACK);
        assert_eq!(light.on_accent, Color32::WHITE);
    }

    #[test]
    fn fluent_uses_light2_in_dark_and_dark1_in_light() {
        let accent = AccentShades::WINDOWS_DEFAULT;
        assert_eq!(fluent_dark(accent).accent, accent.light2);
        assert_eq!(fluent_light(accent).accent, accent.dark1);
    }

    #[test]
    fn strong_text_is_not_dimmed() {
        let accent = AccentShades::WINDOWS_DEFAULT;
        let m = Metrics::FLUENT;
        for (mode, palette) in [
            (Theme::Dark, fluent_dark(accent)),
            (Theme::Light, fluent_light(accent)),
        ] {
            let visuals = style(&Style::default(), mode, &palette, &m).visuals;
            assert_eq!(visuals.strong_text_color(), palette.text);
        }
    }

    #[test]
    fn mica_variant_clears_chrome_only() {
        let opaque = fluent_dark(AccentShades::WINDOWS_DEFAULT);
        let mica = mica_variant(&opaque, Theme::Dark);
        assert_eq!(mica.sidebar_bg, Color32::TRANSPARENT);
        assert!(mica.panel_bg.a() < 255);
        assert_eq!(mica.text, opaque.text);
        assert_eq!(mica.window_bg, opaque.window_bg);
    }

    #[test]
    fn the_fluent_theme_styles_both_modes_with_its_own_sizes() {
        let ctx = egui::Context::default();
        theme(AccentShades::WINDOWS_DEFAULT).install(&ctx);
        assert_eq!(Metrics::of_ctx(&ctx), Metrics::FLUENT);
        assert_eq!(ThemeFamily::of_ctx(&ctx), ThemeFamily::Fluent);
        let dark = ctx.style_of(Theme::Dark);
        let light = ctx.style_of(Theme::Light);
        assert_eq!(dark.spacing.interact_size.y, Metrics::FLUENT.control_height);
        assert_ne!(dark.visuals.panel_fill, light.visuals.panel_fill);
    }
}
