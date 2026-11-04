//! Call-to-action buttons and borderless icon buttons.

use egui::{
    Align2, Button, Color32, CornerRadius, FontId, Painter, Rect, Response, RichText, Role, Sense,
    Stroke, Ui, Vec2, WidgetInfo,
};
use egui_mev_theme::{Glyph, Metrics, Palette};

/// Narrowest a call-to-action button gets, so "OK" stays easy to hit (px).
const CTA_MIN_WIDTH: f32 = 72.0;

fn cta(ui: &Ui, text: RichText, fill: Color32, stroke: Color32) -> Button<'static> {
    let metrics = Metrics::of(ui);
    Button::new(text)
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .min_size(Vec2::new(CTA_MIN_WIDTH, metrics.interact_height))
        .corner_radius(CornerRadius::same(metrics.control_radius))
}

/// Filled accent button for the default action of a screen. Returned as a
/// builder so callers can pass it to `ui.add_enabled`.
pub fn primary(ui: &Ui, text: &str) -> Button<'static> {
    let p = Palette::of(ui);
    let label = RichText::new(text).color(p.on_accent).strong();
    cta(ui, label, p.accent, p.accent)
}

/// Outlined low-emphasis button.
pub fn secondary(ui: &Ui, text: &str) -> Button<'static> {
    let p = Palette::of(ui);
    cta(ui, RichText::new(text).color(p.text), p.card_bg, p.border)
}

/// Button with a red outline, for actions in a danger zone.
pub fn danger(ui: &Ui, text: &str) -> Button<'static> {
    let p = Palette::of(ui);
    let label = RichText::new(text).color(p.critical);
    cta(ui, label, p.card_bg, p.critical)
}

/// Filled red button, for a sheet's default action when it destroys something.
pub fn destructive(ui: &Ui, text: &str) -> Button<'static> {
    let p = Palette::of(ui);
    let label = RichText::new(text).color(p.on_accent).strong();
    cta(ui, label, p.critical, p.critical)
}

/// Add a [`primary`] button.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let button = primary(ui, text);
    ui.add(button)
}

/// Add a [`secondary`] button.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    let button = secondary(ui, text);
    ui.add(button)
}

/// Add a [`danger`] button.
pub fn danger_button(ui: &mut Ui, text: &str) -> Response {
    let button = danger(ui, text);
    ui.add(button)
}

/// Add a [`destructive`] button.
pub fn destructive_button(ui: &mut Ui, text: &str) -> Response {
    let button = destructive(ui, text);
    ui.add(button)
}

/// What an icon button shows on hover: its name, plus a keyboard shortcut or
/// other hint as plain text. The name alone is the accessible label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip<'a> {
    name: &'a str,
    hint: Option<String>,
}

impl<'a> From<&'a str> for Tip<'a> {
    fn from(name: &'a str) -> Self {
        Self { name, hint: None }
    }
}

impl<'a> From<&'a String> for Tip<'a> {
    fn from(name: &'a String) -> Self {
        name.as_str().into()
    }
}

impl<'a> Tip<'a> {
    /// `name` with a hint after it: a shortcut as the platform writes it
    /// ("⌘N", "Ctrl+N"), tool letters or modifier help.
    #[must_use]
    pub fn hint(name: &'a str, hint: impl Into<String>) -> Self {
        Self {
            name,
            hint: Some(hint.into()),
        }
    }

    /// The name, which is also the accessible label.
    #[must_use]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// The text shown on hover.
    #[must_use]
    pub fn text(&self) -> String {
        self.hint.as_ref().map_or_else(
            || self.name.to_owned(),
            |hint| format!("{}  {hint}", self.name),
        )
    }
}

/// How a painted button shows its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonLook {
    /// A plain action.
    Plain,
    /// An on/off option: "on" keeps a fill.
    Toggle(bool),
    /// A tool-palette entry: the selected tool gets the selection fill.
    Tool(bool),
}

/// The fill and content colour for a button in `look` and `response`'s
/// interaction state.
fn button_colors(
    p: &Palette,
    look: ButtonLook,
    response: &Response,
    enabled: bool,
) -> (Option<Color32>, Color32) {
    if !enabled {
        return (None, p.hint_text);
    }
    let (hovered, pressed) = (response.hovered(), response.is_pointer_button_down_on());
    match look {
        ButtonLook::Tool(true) => (Some(p.selection_bg), p.text),
        ButtonLook::Toggle(true) => (
            Some(if hovered || pressed {
                p.control_track
            } else {
                p.card_bg_hover
            }),
            p.text,
        ),
        ButtonLook::Plain | ButtonLook::Toggle(false) | ButtonLook::Tool(false) => {
            let fill = if pressed {
                Some(p.control_track)
            } else if hovered {
                Some(p.card_bg_hover)
            } else {
                None
            };
            let color = if look == ButtonLook::Plain || hovered {
                p.text
            } else {
                p.text_weak
            };
            (fill, color)
        }
    }
}

/// A button of `size` that paints `content` centred, with a state fill behind
/// it. Unlike egui's `Button`, whose frame (and so its content) shifts when
/// hover adds a stroke, nothing here depends on the state but the colours.
/// `content` gets the painter, the button's rect and the colour to draw in.
pub fn painted_button(
    ui: &mut Ui,
    size: Vec2,
    look: ButtonLook,
    enabled: bool,
    content: impl FnOnce(&Painter, Rect, Color32),
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let palette = Palette::of(ui);
        let (fill, color) = button_colors(&palette, look, &response, enabled);
        let radius = CornerRadius::same(Metrics::of(ui).control_radius);
        let painter = ui.painter();
        if let Some(fill) = fill {
            painter.rect_filled(rect, radius, fill);
        }
        content(painter, rect, color);
    }
    response
}

/// An icon button: `icon` at `glyph_size` centred in `size`.
fn icon_button(
    ui: &mut Ui,
    icon: &Glyph,
    size: Vec2,
    glyph_size: f32,
    look: ButtonLook,
    enabled: bool,
) -> Response {
    painted_button(ui, size, look, enabled, |painter, rect, color| {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon.ch,
            icon.font_id(glyph_size),
            color,
        );
    })
}

/// Hover text and accessible name for an icon-only button.
fn labelled(response: Response, tip: &Tip<'_>, enabled: bool) -> Response {
    let response = response.on_hover_ui(|ui| {
        ui.label(tip.text());
    });
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, tip.name));
    response
}

/// Size of a toolbar icon button: a little wider than a built-in control is tall.
fn toolbar_size(ui: &Ui) -> Vec2 {
    let height = Metrics::of(ui).interact_height.max(24.0);
    Vec2::new(height + 4.0, height)
}

const TOOLBAR_GLYPH: f32 = 16.0;

/// Borderless toolbar icon button, filled while `active` (an option that is on).
pub fn toolbar_icon<'a>(
    ui: &mut Ui,
    icon: impl Into<Glyph>,
    active: bool,
    tip: impl Into<Tip<'a>>,
) -> Response {
    let size = toolbar_size(ui);
    let look = ButtonLook::Toggle(active);
    let response = icon_button(ui, &icon.into(), size, TOOLBAR_GLYPH, look, true);
    labelled(response, &tip.into(), true)
}

/// Toolbar icon button for an action that may be unavailable (undo with an
/// empty history, paste with an empty clipboard).
pub fn toolbar_icon_enabled<'a>(
    ui: &mut Ui,
    icon: impl Into<Glyph>,
    enabled: bool,
    tip: impl Into<Tip<'a>>,
) -> Response {
    let size = toolbar_size(ui);
    let look = ButtonLook::Plain;
    let response = icon_button(ui, &icon.into(), size, TOOLBAR_GLYPH, look, enabled);
    labelled(response, &tip.into(), enabled)
}

/// Square tool-palette button: selected tools get the selection fill.
pub fn tool_button<'a>(
    ui: &mut Ui,
    icon: impl Into<Glyph>,
    selected: bool,
    tip: impl Into<Tip<'a>>,
) -> Response {
    let size = Vec2::splat(Metrics::of(ui).control_height.max(24.0));
    let look = ButtonLook::Tool(selected);
    let response = icon_button(ui, &icon.into(), size, TOOLBAR_GLYPH, look, true);
    labelled(response, &tip.into(), true)
}

/// Small borderless icon button for inline actions (swap colours, reset a
/// value), sized to its glyph.
pub fn small_icon_button<'a>(
    ui: &mut Ui,
    icon: impl Into<Glyph>,
    glyph_size: f32,
    enabled: bool,
    tip: impl Into<Tip<'a>>,
) -> Response {
    let size = Vec2::splat((glyph_size + 8.0).ceil());
    let look = ButtonLook::Plain;
    let response = icon_button(ui, &icon.into(), size, glyph_size, look, enabled);
    labelled(response, &tip.into(), enabled)
}

/// Small borderless text button for inline actions ("B/W").
pub fn small_text_button<'a>(ui: &mut Ui, text: &str, tip: impl Into<Tip<'a>>) -> Response {
    let font = FontId::proportional(Metrics::of(ui).caption);
    let color = Palette::of(ui).text;
    let width = ui
        .fonts_mut(|fonts| fonts.layout_no_wrap(text.to_owned(), font.clone(), color))
        .size()
        .x;
    let size = Vec2::new((width + 10.0).ceil(), 20.0);
    let response = painted_button(ui, size, ButtonLook::Plain, true, |painter, rect, color| {
        painter.text(rect.center(), Align2::CENTER_CENTER, text, font, color);
    });
    let tip = tip.into();
    let response = response.on_hover_ui(|ui| {
        ui.label(tip.text());
    });
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, text));
    response
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::{Theme, presets};

    use super::*;

    const ICON: char = 'x';

    #[test]
    fn a_tip_with_a_hint_keeps_its_name_as_the_label() {
        let tip = Tip::hint("Save", "⌘S");
        assert_eq!(tip.name(), "Save");
        assert_eq!(tip.text(), "Save  ⌘S");
        assert_eq!(Tip::from("Save").text(), "Save");
    }

    #[test]
    fn icon_buttons_are_found_and_clicked_by_their_tip() {
        let mut harness = Harness::builder().with_size([200.0, 80.0]).build_ui_state(
            |ui, clicks: &mut u32| {
                if toolbar_icon(ui, ICON, false, Tip::hint("Grid", "G")).clicked() {
                    *clicks += 1;
                }
            },
            0,
        );
        harness.run();
        harness.get_by_label("Grid").click();
        harness.run();
        assert_eq!(*harness.state(), 1);
    }

    #[test]
    fn a_disabled_icon_button_takes_no_clicks() {
        let mut harness = Harness::builder().with_size([200.0, 80.0]).build_ui_state(
            |ui, clicks: &mut u32| {
                if toolbar_icon_enabled(ui, ICON, false, "Undo").clicked()
                    || small_icon_button(ui, ICON, 12.0, false, "Reset").clicked()
                {
                    *clicks += 1;
                }
            },
            0,
        );
        harness.run();
        // A real click at the widget's place: an accesskit click would bypass `Sense`.
        let pos = harness.get_by_label("Undo").rect().center();
        harness.hover_at(pos);
        harness.drag_at(pos);
        harness.drop_at(pos);
        harness.run();
        assert_eq!(*harness.state(), 0);
    }

    #[test]
    fn buttons_use_the_installed_palette_in_both_modes() {
        for (mode, palette) in [
            (egui::Theme::Dark, presets::GRAPHITE_DARK),
            (egui::Theme::Light, presets::GRAPHITE_LIGHT),
        ] {
            let ctx = crate::test_support::themed(mode);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 200.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                primary_button(ui, "Save");
                danger_button(ui, "Delete");
                destructive_button(ui, "Erase");
                secondary_button(ui, "Cancel");
            });
            output.textures_delta.clear();
            let colors = crate::test_support::painted_colors(&output);
            assert!(colors.contains(&palette.accent), "{mode:?}");
            assert!(colors.contains(&palette.critical), "{mode:?}");
            assert!(colors.contains(&palette.card_bg), "{mode:?}");
        }
    }

    #[test]
    fn the_button_states_read_from_the_palette() {
        let p = presets::GRAPHITE_DARK;
        let ctx = egui::Context::default();
        Theme::default().install(&ctx);
        crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
            let response = ui.allocate_response(Vec2::splat(10.0), Sense::hover());
            assert_eq!(
                button_colors(&p, ButtonLook::Plain, &response, false),
                (None, p.hint_text)
            );
            assert_eq!(
                button_colors(&p, ButtonLook::Tool(true), &response, true),
                (Some(p.selection_bg), p.text)
            );
            assert_eq!(
                button_colors(&p, ButtonLook::Toggle(true), &response, true),
                (Some(p.card_bg_hover), p.text)
            );
            assert_eq!(
                button_colors(&p, ButtonLook::Toggle(false), &response, true),
                (None, p.text_weak)
            );
        });
    }
}
