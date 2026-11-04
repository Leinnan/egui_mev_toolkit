//! WCAG contrast, for tests and for picking readable text at run time.

use egui::Color32;

/// WCAG relative luminance of an sRGB colour.
#[must_use]
pub fn luminance(color: Color32) -> f32 {
    let channel = |value: u8| {
        let value = f32::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// WCAG contrast ratio of two colours: 1 (none) to 21 (black on white).
#[must_use]
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Black or white, whichever reads better on `background`.
#[must_use]
pub fn readable_on(background: Color32) -> Color32 {
    if contrast(Color32::BLACK, background) >= contrast(Color32::WHITE, background) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

#[cfg(test)]
mod tests {
    use egui::Color32;

    use super::{contrast, readable_on};
    use crate::{Palette, presets};

    fn presets() -> Vec<(&'static str, Palette)> {
        use crate::{
            AccentShades,
            fluent::{fluent_dark, fluent_light},
        };
        vec![
            ("graphite dark", presets::GRAPHITE_DARK),
            ("graphite light", presets::GRAPHITE_LIGHT),
            ("macos light", presets::MACOS_LIGHT),
            ("slate dark", presets::SLATE_DARK),
            ("fluent dark", fluent_dark(AccentShades::WINDOWS_DEFAULT)),
            ("fluent light", fluent_light(AccentShades::WINDOWS_DEFAULT)),
        ]
    }

    #[test]
    fn the_range_runs_from_one_to_twenty_one() {
        assert!((contrast(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 1e-3);
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-3);
    }

    #[test]
    fn readable_on_picks_the_stronger_of_black_and_white() {
        assert_eq!(readable_on(Color32::WHITE), Color32::BLACK);
        assert_eq!(readable_on(Color32::BLACK), Color32::WHITE);
    }

    #[test]
    fn text_meets_wcag_aa_on_the_fills_it_sits_on() {
        for (name, p) in presets() {
            for (role, text, fill) in [
                ("text on panel", p.text, p.panel_bg),
                ("text on card", p.text, p.card_bg),
                ("text on window", p.text, p.window_bg),
                ("text on input", p.text, p.extreme_bg),
                ("label on panel", p.text_label, p.panel_bg),
            ] {
                let ratio = contrast(text, fill);
                assert!(ratio >= 4.5, "{name}: {role} is only {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn secondary_and_accent_text_stay_legible() {
        for (name, p) in presets() {
            for (role, text, fill) in [
                ("weak on panel", p.text_weak, p.panel_bg),
                ("on accent", p.on_accent, p.accent),
            ] {
                let ratio = contrast(text, fill);
                assert!(ratio >= 3.0, "{name}: {role} is only {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn a_focus_ring_stands_out_from_what_it_surrounds() {
        for (name, p) in presets() {
            for (role, fill) in [("panel", p.panel_bg), ("card", p.card_bg)] {
                let ratio = contrast(p.accent, fill);
                assert!(
                    ratio >= 3.0,
                    "{name}: accent on {role} is only {ratio:.2}:1"
                );
            }
        }
    }
}
