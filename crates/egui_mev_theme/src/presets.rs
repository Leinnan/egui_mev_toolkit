//! Ready-made palettes. An app can also fill in a [`Palette`] of its own, or
//! start from one of these with `Palette { accent, ..GRAPHITE_DARK }`.

use egui::Color32;

use crate::{AccentShades, Palette, contrast};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// macOS-style graphite with the system blue accent, shared by most desktop apps.
pub const GRAPHITE_DARK: Palette = Palette {
    text: rgb(0xEC, 0xEC, 0xF0),
    text_weak: rgb(0x98, 0x98, 0x9E),
    text_label: rgb(0xD2, 0xD2, 0xD7),
    hint_text: rgb(0x6E, 0x6E, 0x74),
    body_bg: rgb(0x12, 0x12, 0x14),
    panel_bg: rgb(0x1E, 0x1E, 0x20),
    sidebar_bg: rgb(0x26, 0x26, 0x28),
    toolbar_bg: rgb(0x2B, 0x2B, 0x2D),
    pane_tab_active: rgb(0x30, 0x30, 0x33),
    window_bg: rgb(0x2A, 0x2A, 0x2C),
    card_bg: rgb(0x30, 0x30, 0x33),
    card_bg_hover: rgb(0x3A, 0x3A, 0x3D),
    card_bg_selected: rgb(0x1D, 0x3A, 0x5F),
    card_stroke: rgb(0x3D, 0x3D, 0x41),
    section_bg: rgb(0x2A, 0x2A, 0x2C),
    extreme_bg: rgb(0x12, 0x12, 0x14),
    control_track: rgb(0x4A, 0x4A, 0x4E),
    border: rgb(0x3D, 0x3D, 0x41),
    border_subtle: rgb(0x35, 0x35, 0x38),
    accent: rgb(0x0A, 0x84, 0xFF),
    accent_hover: rgb(0x0A, 0x84, 0xFF),
    accent_dim: rgb(0x1D, 0x4E, 0x89),
    accent_secondary: rgb(0x0A, 0x84, 0xFF),
    on_accent: rgb(0xEC, 0xEC, 0xF0),
    selection_bg: rgb(0x1D, 0x4E, 0x89),
    subtle_hover: rgb(0x3A, 0x3A, 0x3D),
    subtle_selected: rgb(0x1D, 0x4E, 0x89),
    success: rgb(0x30, 0xD1, 0x58),
    caution: rgb(0xFF, 0xD6, 0x0A),
    critical: rgb(0xFF, 0x45, 0x3A),
    neutral: rgb(0x8E, 0x8E, 0x93),
    section_label: rgb(0x8E, 0x8E, 0x93),
    ghost: rgb(0x6E, 0x6E, 0x74),
    smoke: Color32::from_black_alpha(153),
    info_bg: rgb(0x30, 0x30, 0x33),
    success_bg: rgb(0x30, 0x30, 0x33),
    caution_bg: rgb(0x30, 0x30, 0x33),
    critical_bg: rgb(0x30, 0x30, 0x33),
};

/// The light companion of [`GRAPHITE_DARK`], derived from Catppuccin Latte.
pub const GRAPHITE_LIGHT: Palette = Palette {
    text: rgb(0x4C, 0x4F, 0x69),
    text_weak: rgb(0x7C, 0x7F, 0x93),
    text_label: rgb(0x4C, 0x4F, 0x69),
    hint_text: rgb(0x9C, 0xA0, 0xB0),
    body_bg: rgb(0xDC, 0xDF, 0xE4),
    panel_bg: rgb(0xE6, 0xE9, 0xEF),
    sidebar_bg: rgb(0xE6, 0xE9, 0xEF),
    toolbar_bg: rgb(0xEF, 0xF1, 0xF5),
    pane_tab_active: rgb(0xEF, 0xF1, 0xF5),
    window_bg: rgb(0xEF, 0xF1, 0xF5),
    card_bg: rgb(0xCC, 0xD0, 0xDA),
    card_bg_hover: rgb(0xBC, 0xC0, 0xCC),
    card_bg_selected: rgb(0xAC, 0xB0, 0xBE),
    card_stroke: rgb(0xBC, 0xC0, 0xCC),
    section_bg: rgb(0xE6, 0xE9, 0xEF),
    extreme_bg: rgb(0xDC, 0xDF, 0xE4),
    // Darker than `card_bg`, so an off switch and a slider rail show on a card.
    control_track: rgb(0xAC, 0xB0, 0xBE),
    border: rgb(0xBC, 0xC0, 0xCC),
    // One step off `card_bg`, so pills and hairlines show on a card.
    border_subtle: rgb(0xBC, 0xC0, 0xCC),
    accent: rgb(0x1E, 0x66, 0xF5),
    accent_hover: rgb(0x1E, 0x66, 0xF5),
    accent_dim: rgb(0xD6, 0xE4, 0xFF),
    accent_secondary: rgb(0x04, 0xA5, 0xE5),
    on_accent: Color32::WHITE,
    selection_bg: rgb(0xD6, 0xE4, 0xFF),
    subtle_hover: rgb(0xBC, 0xC0, 0xCC),
    subtle_selected: rgb(0xAC, 0xB0, 0xBE),
    success: rgb(0x40, 0xA0, 0x2B),
    caution: rgb(0xDF, 0x8E, 0x1D),
    critical: rgb(0xD2, 0x0F, 0x39),
    neutral: rgb(0x9C, 0xA0, 0xB0),
    section_label: rgb(0x6C, 0x6F, 0x85),
    ghost: rgb(0x9C, 0xA0, 0xB0),
    smoke: Color32::from_black_alpha(153),
    info_bg: rgb(0xCC, 0xD0, 0xDA),
    success_bg: rgb(0xCC, 0xD0, 0xDA),
    caution_bg: rgb(0xCC, 0xD0, 0xDA),
    critical_bg: rgb(0xCC, 0xD0, 0xDA),
};

/// A neutral slate with a gold accent, the default look of game developer tools.
pub const SLATE_DARK: Palette = Palette {
    text: rgb(0xF1, 0xF5, 0xF9),
    text_weak: rgb(0x94, 0xA3, 0xB8),
    text_label: rgb(0xCB, 0xD5, 0xE1),
    hint_text: rgb(0x64, 0x74, 0x8B),
    body_bg: rgb(0x08, 0x09, 0x0B),
    panel_bg: rgb(0x12, 0x13, 0x17),
    sidebar_bg: rgb(0x12, 0x13, 0x17),
    toolbar_bg: rgb(0x12, 0x13, 0x17),
    pane_tab_active: rgb(0x1A, 0x1C, 0x21),
    window_bg: rgb(0x14, 0x16, 0x1A),
    card_bg: rgb(0x1A, 0x1C, 0x21),
    card_bg_hover: rgb(0x24, 0x27, 0x2E),
    card_bg_selected: rgb(0x23, 0x2A, 0x38),
    card_stroke: rgb(0x2E, 0x32, 0x3C),
    section_bg: rgb(0x20, 0x23, 0x2A),
    extreme_bg: rgb(0x0C, 0x0D, 0x10),
    control_track: rgb(0x2E, 0x32, 0x3C),
    border: rgb(0x2E, 0x32, 0x3C),
    border_subtle: rgb(0x23, 0x26, 0x2D),
    accent: rgb(0xFF, 0xCD, 0x7C),
    accent_hover: rgb(0xFF, 0xE1, 0x90),
    accent_dim: rgb(0x6B, 0x54, 0x20),
    accent_secondary: rgb(0x3B, 0x82, 0xF6),
    on_accent: rgb(0x1A, 0x12, 0x05),
    selection_bg: rgb(0x25, 0x34, 0x60),
    subtle_hover: rgb(0x24, 0x27, 0x2E),
    subtle_selected: rgb(0x23, 0x2A, 0x38),
    success: rgb(0x34, 0xA8, 0x53),
    caution: rgb(0xD9, 0xA4, 0x41),
    critical: rgb(0xDC, 0x50, 0x50),
    neutral: rgb(0x71, 0x71, 0x7A),
    section_label: rgb(0xFF, 0xCD, 0x7C),
    ghost: rgb(0x4A, 0x50, 0x68),
    smoke: Color32::from_black_alpha(153),
    info_bg: rgb(0x16, 0x22, 0x33),
    success_bg: rgb(0x13, 0x27, 0x1A),
    caution_bg: rgb(0x2B, 0x22, 0x0E),
    critical_bg: rgb(0x2E, 0x16, 0x16),
};

/// A light macOS look: white content, a soft grey sidebar and the system blue.
pub const MACOS_LIGHT: Palette = Palette {
    text: rgb(0x1E, 0x1E, 0x22),
    text_weak: rgb(0x64, 0x64, 0x6C),
    text_label: rgb(0x3A, 0x3A, 0x40),
    hint_text: rgb(0x8E, 0x8E, 0x93),
    body_bg: rgb(0xE8, 0xE8, 0xEB),
    panel_bg: rgb(0xFF, 0xFF, 0xFF),
    sidebar_bg: rgb(0xF2, 0xF2, 0xF5),
    toolbar_bg: rgb(0xF7, 0xF7, 0xF9),
    pane_tab_active: rgb(0xFF, 0xFF, 0xFF),
    window_bg: rgb(0xF2, 0xF2, 0xF5),
    card_bg: rgb(0xFA, 0xFA, 0xFC),
    card_bg_hover: rgb(0xE8, 0xE8, 0xEC),
    card_bg_selected: rgb(0xD2, 0xE6, 0xFF),
    card_stroke: rgb(0xD8, 0xD8, 0xDE),
    section_bg: rgb(0xF2, 0xF2, 0xF5),
    extreme_bg: rgb(0xFF, 0xFF, 0xFF),
    control_track: rgb(0xC8, 0xC8, 0xCE),
    border: rgb(0xD8, 0xD8, 0xDE),
    border_subtle: rgb(0xE4, 0xE4, 0xE8),
    accent: rgb(0x00, 0x70, 0xE8),
    accent_hover: rgb(0x00, 0x66, 0xD6),
    accent_dim: rgb(0xD2, 0xE6, 0xFF),
    accent_secondary: rgb(0x00, 0x70, 0xE8),
    on_accent: Color32::WHITE,
    selection_bg: rgb(0xD2, 0xE6, 0xFF),
    subtle_hover: rgb(0xE8, 0xE8, 0xEC),
    subtle_selected: rgb(0xD2, 0xE6, 0xFF),
    success: rgb(0x24, 0x8A, 0x3D),
    caution: rgb(0x9D, 0x6A, 0x00),
    critical: rgb(0xD7, 0x00, 0x15),
    neutral: rgb(0x8E, 0x8E, 0x93),
    section_label: rgb(0x6E, 0x6E, 0x73),
    ghost: rgb(0xAE, 0xAE, 0xB2),
    smoke: Color32::from_black_alpha(77),
    info_bg: rgb(0xEA, 0xF2, 0xFF),
    success_bg: rgb(0xE6, 0xF6, 0xEA),
    caution_bg: rgb(0xFF, 0xF6, 0xDE),
    critical_bg: rgb(0xFD, 0xE8, 0xE8),
};

/// [`GRAPHITE_DARK`] around `accent`. The system blue gives the preset unchanged.
#[must_use]
pub fn macos_dark(accent: AccentShades) -> Palette {
    if accent.is_macos_blue() {
        return GRAPHITE_DARK;
    }
    let base = accent.base;
    let selection = base.lerp_to_gamma(GRAPHITE_DARK.panel_bg, 0.55);
    Palette {
        accent: base,
        accent_hover: base,
        accent_dim: selection,
        accent_secondary: base,
        on_accent: contrast::readable_on(base),
        selection_bg: selection,
        subtle_selected: selection,
        card_bg_selected: base.lerp_to_gamma(GRAPHITE_DARK.panel_bg, 0.7),
        ..GRAPHITE_DARK
    }
}

/// [`MACOS_LIGHT`] around `accent`. The system blue gives the preset unchanged.
#[must_use]
pub fn macos_light(accent: AccentShades) -> Palette {
    if accent.is_macos_blue() {
        return MACOS_LIGHT;
    }
    let base = accent.base;
    let selection = base.lerp_to_gamma(Color32::WHITE, 0.75);
    Palette {
        accent: base,
        accent_hover: base.lerp_to_gamma(Color32::BLACK, 0.1),
        accent_dim: selection,
        accent_secondary: base,
        on_accent: contrast::readable_on(base),
        selection_bg: selection,
        subtle_selected: selection,
        card_bg_selected: selection,
        ..MACOS_LIGHT
    }
}

#[cfg(test)]
mod tests {
    use egui::Color32;

    use super::{GRAPHITE_DARK, MACOS_LIGHT, macos_dark, macos_light};
    use crate::{AccentShades, contrast::contrast};

    #[test]
    fn the_system_blue_keeps_the_presets() {
        for accent in [
            AccentShades::MACOS_DEFAULT,
            AccentShades::from_base(Color32::from_rgb(0x00, 0x7A, 0xFF)),
        ] {
            assert_eq!(macos_dark(accent), GRAPHITE_DARK);
            assert_eq!(macos_light(accent), MACOS_LIGHT);
        }
    }

    #[test]
    fn another_accent_changes_only_the_accent_roles() {
        let pink = AccentShades::from_base(Color32::from_rgb(0xF7, 0x4F, 0x9E));
        let dark = macos_dark(pink);
        assert_eq!(dark.accent, pink.base);
        assert_eq!(dark.panel_bg, GRAPHITE_DARK.panel_bg);
        let light = macos_light(pink);
        assert_eq!(light.accent, pink.base);
        assert_eq!(light.panel_bg, MACOS_LIGHT.panel_bg);
    }

    #[test]
    fn text_stays_readable_on_a_bright_accent() {
        let yellow = AccentShades::from_base(Color32::from_rgb(0xFF, 0xC6, 0x00));
        for palette in [macos_dark(yellow), macos_light(yellow)] {
            assert!(contrast(palette.on_accent, palette.accent) >= 4.5);
        }
    }
}
