//! Form rows: labels and controls lined up in columns, settings groups, key
//! caps, text fields and colour rows.

use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Label, Layout, Margin, Popup,
    PopupCloseBehavior, Response, RichText, Role, Sense, Slider, Stroke, TextEdit, TextStyle, Ui,
    Vec2, WidgetInfo,
    color_picker::{Alpha, color_picker_color32, show_color},
    emath::Numeric,
    vec2,
};
use egui_mev_theme::{Metrics, Palette};

// ---------------------------------------------------------------------------
// Key/value rows and form rows
// ---------------------------------------------------------------------------

fn label_column(ui: &mut Ui, label: &str, color: Color32) {
    let height = ui.spacing().interact_size.y;
    let width = Metrics::of(ui).label_width;
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        Layout::left_to_right(Align::Center),
        |ui| {
            // Fill the column even for short labels, so values line up.
            ui.set_min_width(width);
            ui.add(Label::new(RichText::new(label).color(color)).truncate());
        },
    );
}

/// One `label : value` row. Use inside any vertical layout.
pub fn kv_row(ui: &mut Ui, key: &str, value: impl Into<String>) {
    let color = Palette::of(ui).text;
    kv_row_colored(ui, key, value, color);
}

/// One `label : value` row with an explicit value colour, such as
/// `Palette::caution` for a degraded count.
pub fn kv_row_colored(ui: &mut Ui, key: &str, value: impl Into<String>, color: Color32) {
    let weak = Palette::of(ui).text_weak;
    ui.horizontal(|ui| {
        label_column(ui, key, weak);
        ui.add(Label::new(RichText::new(value.into()).color(color)).truncate());
    });
}

/// One `label : value` row whose value is monospaced (IDs, seeds, paths).
pub fn kv_row_mono(ui: &mut Ui, key: &str, value: impl Into<String>) {
    let p = Palette::of(ui);
    ui.horizontal(|ui| {
        label_column(ui, key, p.text_weak);
        ui.add(Label::new(RichText::new(value.into()).monospace().color(p.text)).truncate());
    });
}

/// A left-aligned fixed-width label column followed by a control.
pub fn field_row<R>(ui: &mut Ui, label: &str, add_control: impl FnOnce(&mut Ui) -> R) -> R {
    let color = Palette::of(ui).text_label;
    ui.horizontal(|ui| {
        label_column(ui, label, color);
        add_control(ui)
    })
    .inner
}

/// A checkbox row with the theme's label colour.
pub fn toggle_row(ui: &mut Ui, label: &str, value: &mut bool) -> Response {
    let color = Palette::of(ui).text_label;
    ui.checkbox(value, RichText::new(label).color(color))
}

/// One label + control row with the label right-aligned beside the control,
/// the macOS form convention. Controls may size themselves from
/// `available_width`, which would make `egui::Grid` columns oscillate.
pub fn ui_labeled_row<R>(ui: &mut Ui, label: &str, add_control: impl FnOnce(&mut Ui) -> R) -> R {
    let width = Metrics::of(ui).label_width;
    ui_labeled_row_width(ui, width, label, add_control)
}

/// [`ui_labeled_row`] with a custom label-column width, for forms whose labels
/// are longer than usual.
pub fn ui_labeled_row_width<R>(
    ui: &mut Ui,
    label_width: f32,
    label: &str,
    add_control: impl FnOnce(&mut Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        form_label_width(ui, label_width, label);
        add_control(ui)
    })
    .inner
}

/// Fixed-width, right-aligned, vertically centred form label. Returns its id,
/// for `Response::labelled_by`.
pub fn form_label(ui: &mut Ui, label: &str) -> Id {
    let width = Metrics::of(ui).label_width;
    form_label_width(ui, width, label)
}

/// [`form_label`] with a custom column width.
pub fn form_label_width(ui: &mut Ui, width: f32, label: &str) -> Id {
    let height = ui.spacing().interact_size.y;
    let color = Palette::of(ui).text_label;
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(width, height));
            ui.add(Label::new(RichText::new(label).color(color)).truncate())
                .id
        },
    )
    .inner
}

/// A System Settings style group: an optional semibold title above a full-width
/// rounded box holding `add_body`.
pub fn grouped_form<R>(ui: &mut Ui, title: Option<&str>, add_body: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    if let Some(title) = title {
        ui.label(
            RichText::new(title)
                .strong()
                .size(m.body)
                .color(p.text_weak),
        );
        ui.add_space(2.0);
    }
    crate::containers::card_frame(ui)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add_body(ui)
        })
        .inner
}

/// Slider visuals for the rest of `ui`: a rail that shows on a card (the default
/// rail is the card's own fill) and a dim accent trailing fill.
fn slider_style(ui: &mut Ui) {
    let p = Palette::of(ui);
    let visuals = ui.visuals_mut();
    visuals.widgets.inactive.bg_fill = p.extreme_bg;
    visuals.selection.bg_fill = p.accent_dim;
}

/// A labelled slider: the label column, a filled track and the value.
pub fn slider_row<N: Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
) -> Response {
    field_row(ui, label, |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 64.0).max(40.0);
        slider_style(ui);
        ui.add(Slider::new(value, range).trailing_fill(true))
            .accessible_name(label)
    })
}

/// A [`slider_row`] whose value carries a unit suffix (` m`, `°`).
pub fn slider_row_suffix<N: Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    suffix: &str,
) -> Response {
    field_row(ui, label, |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 72.0).max(40.0);
        slider_style(ui);
        ui.add(Slider::new(value, range).trailing_fill(true).suffix(suffix))
            .accessible_name(label)
    })
}

// ---------------------------------------------------------------------------
// Switches, settings groups and key caps
// ---------------------------------------------------------------------------

/// An on/off switch, as Windows and macOS settings use for options that apply
/// at once. `label` names it for screen readers and tests.
pub fn toggle_switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(40.0, 20.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let checked = *on;
    response.widget_info(|| WidgetInfo::selected(Role::CheckBox, ui.is_enabled(), checked, label));
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, checked);
        let p = Palette::of(ui);
        let dim = |color: Color32| {
            if ui.is_enabled() {
                color
            } else {
                color.gamma_multiply(0.45)
            }
        };
        let hovered = response.hovered() && ui.is_enabled();
        let (fill, stroke, knob) = if checked {
            let accent = if hovered { p.accent_hover } else { p.accent };
            (dim(accent), Stroke::NONE, dim(p.on_accent))
        } else {
            let fill = if hovered {
                p.subtle_hover
            } else {
                Color32::TRANSPARENT
            };
            (fill, Stroke::new(1.0, dim(p.text_weak)), dim(p.text_weak))
        };
        let painter = ui.painter();
        let radius = rect.height() / 2.0;
        painter.rect(rect, radius, fill, stroke, egui::StrokeKind::Inside);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        let knob_radius = if hovered { 6.0 } else { 5.0 };
        painter.circle_filled(egui::pos2(x, rect.center().y), knob_radius, knob);
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                radius + 2.0,
                Stroke::new(1.5, p.accent),
                egui::StrokeKind::Outside,
            );
        }
    }
    response
}

/// One group of settings: a heading and a full-width card whose rows are
/// separated by thin lines. Add rows with [`SettingsGroup::row`].
pub fn settings_group<R>(
    ui: &mut Ui,
    title: &str,
    rows: impl FnOnce(&mut SettingsGroup<'_>) -> R,
) -> R {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    ui.add_space(14.0);
    ui.label(RichText::new(title).strong().color(p.text_weak));
    ui.add_space(6.0);
    Frame::new()
        .fill(p.card_bg)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(m.card_radius))
        .inner_margin(Margin::symmetric(16, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            rows(&mut SettingsGroup { ui, rows: 0 })
        })
        .inner
}

/// The rows of a [`settings_group`].
pub struct SettingsGroup<'a> {
    /// The group's body, for anything other than a row.
    pub ui: &'a mut Ui,
    rows: usize,
}

impl SettingsGroup<'_> {
    /// A setting: its title and an optional description on the left, its control
    /// (built by `control`, laid out right to left) on the right.
    pub fn row<R>(
        &mut self,
        title: &str,
        description: Option<&str>,
        control: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        if self.rows > 0 {
            let x = self.ui.max_rect().x_range();
            let y = self.ui.cursor().top();
            self.ui.painter().hline(
                x.min - 16.0..=x.max + 16.0,
                y,
                Stroke::new(1.0, Palette::of(self.ui).card_stroke),
            );
        }
        self.rows += 1;
        setting_row(self.ui, title, description, control)
    }

    /// A row of explanatory text with no control.
    pub fn note(&mut self, text: &str) {
        self.row("", Some(text), |_| {});
    }
}

fn setting_row<R>(
    ui: &mut Ui,
    title: &str,
    description: Option<&str>,
    control: impl FnOnce(&mut Ui) -> R,
) -> R {
    let p = Palette::of(ui);
    let width = ui.available_width();
    let top_left = ui.cursor().min;
    // The height of a row with no description.
    let min_height = (Metrics::of(ui).control_height + 14.0).max(44.0);
    // The control goes first, right-aligned, so the text can wrap in the space
    // it leaves.
    let control_area = egui::Rect::from_min_size(
        egui::pos2(width.mul_add(0.4, top_left.x), top_left.y),
        Vec2::new(width * 0.6, min_height),
    );
    let mut control_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(control_area)
            .layout(Layout::right_to_left(Align::Center)),
    );
    let inner = control(&mut control_ui);
    let used = control_ui.min_rect();
    let control_width = if used.width() > 0.0 {
        used.width() + 16.0
    } else {
        0.0
    };
    let text_width = (width - control_width).max(80.0);

    let title_galley = ui.painter().layout(
        title.to_owned(),
        TextStyle::Body.resolve(ui.style()),
        p.text,
        text_width,
    );
    let description_galley = description.map(|text| {
        ui.painter().layout(
            text.to_owned(),
            TextStyle::Small.resolve(ui.style()),
            p.text_weak,
            text_width,
        )
    });
    // A note has no title, only its (description) text.
    let title_height = if title.is_empty() {
        0.0
    } else {
        title_galley.size().y + 2.0
    };
    let text_height = title_height
        + description_galley
            .as_ref()
            .map_or(0.0, |galley| galley.size().y);
    let height = min_height.max(text_height + 16.0);
    let rect = egui::Rect::from_min_size(top_left, Vec2::new(width, height));
    ui.allocate_rect(rect, Sense::hover());
    let mut y = rect.center().y - text_height / 2.0;
    if !title.is_empty() {
        ui.painter()
            .galley(egui::pos2(rect.left(), y), title_galley, p.text);
    }
    y += title_height;
    if let Some(galley) = description_galley {
        ui.painter()
            .galley(egui::pos2(rect.left(), y), galley, p.text_weak);
    }
    inner
}

/// Key combinations drawn as key caps, such as "Ctrl+F" and "F3". In a right to
/// left row, pass them last first.
pub fn key_caps(ui: &mut Ui, combinations: &[impl AsRef<str>]) {
    let p = Palette::of(ui);
    let font = FontId::monospace(Metrics::of(ui).caption);
    for keys in combinations {
        let keys = keys.as_ref();
        let galley = ui
            .painter()
            .layout_no_wrap(keys.to_owned(), font.clone(), p.text);
        let size = galley.size() + Vec2::new(12.0, 6.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, keys));
        let painter = ui.painter();
        painter.rect(
            rect,
            4,
            p.card_bg,
            Stroke::new(1.0, p.card_stroke),
            egui::StrokeKind::Inside,
        );
        // A slightly heavier bottom edge, like a key.
        painter.hline(
            rect.x_range().shrink(2.0),
            rect.bottom() - 0.5,
            Stroke::new(1.0, p.text_weak.gamma_multiply(0.5)),
        );
        painter.galley(rect.min + Vec2::new(6.0, 3.0), galley, p.text);
    }
}

// ---------------------------------------------------------------------------
// Text fields
// ---------------------------------------------------------------------------

/// A one-line field as tall as the other controls, with its text centred. Name it
/// for assistive technology with `Response::labelled_by` or
/// `Response::accessible_name` on what `ui.add` returns.
pub fn text_field<'t>(ui: &Ui, text: &'t mut dyn egui::TextBuffer) -> TextEdit<'t> {
    let m = Metrics::of(ui);
    // Fluent draws its field text closer to the edge than the other looks.
    let margin_x = if m == Metrics::FLUENT { 4 } else { 8 };
    TextEdit::singleline(text)
        .min_size(Vec2::new(0.0, m.control_height))
        .align(Align2::LEFT_CENTER)
        .margin(Margin::symmetric(margin_x, 2))
}

// ---------------------------------------------------------------------------
// Colour rows
// ---------------------------------------------------------------------------

/// How many recent colours are kept, most recent first.
const MAX_RECENT_COLORS: usize = 12;
const RECENT_SWATCH_SIZE: f32 = 18.0;
/// Room for "#RRGGBB" in the widest digits (px).
const HEX_FIELD_WIDTH: f32 = 76.0;

/// A "label + colour swatch + hex" row with the swatch directly beside its
/// label. The swatch opens a picker with the recently used colours of this
/// context. Colours are `[r, g, b]`, straight sRGB.
pub fn color_swatch_row(ui: &mut Ui, label: &str, color: &mut [u8; 3]) {
    ui_labeled_row(ui, label, |ui| {
        let id = ui.scope_id().with(("color_row", label));
        let before = *color;
        let picking = swatch_with_picker(ui, id, label, color);
        let hex_focused = hex_field(ui, id, label, color);
        remember_when_settled(ui, id, *color, before, picking || hex_focused);
    });
}

/// The swatch and its popup. Returns whether a pointer drag may still be
/// changing the colour.
fn swatch_with_picker(ui: &mut Ui, id: Id, label: &str, color: &mut [u8; 3]) -> bool {
    let [r, g, b] = *color;
    let mut srgba = Color32::from_rgb(r, g, b);
    let size = ui.spacing().interact_size;
    let swatch = show_color(ui, srgba, size)
        .interact(Sense::click())
        .on_hover_text("Click to edit color")
        .accessible_name(format!("{label} color"));
    Popup::menu(&swatch)
        .id(id.with("popup"))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().slider_width = 275.0;
            color_picker_color32(ui, &mut srgba, Alpha::Opaque);
            *color = [srgba.r(), srgba.g(), srgba.b()];
            recent_swatches(ui, color);
        });
    ui.input(|input| input.pointer.any_down())
}

/// Clickable recent colours, shown only once there are some.
fn recent_swatches(ui: &mut Ui, color: &mut [u8; 3]) {
    let recent = recent_colors(ui.ctx());
    if recent.is_empty() {
        return;
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for candidate in recent {
            let [r, g, b] = candidate;
            let swatch = show_color(
                ui,
                Color32::from_rgb(r, g, b),
                vec2(RECENT_SWATCH_SIZE, RECENT_SWATCH_SIZE),
            )
            .interact(Sense::click())
            .on_hover_text(to_hex(candidate))
            .accessible_name(to_hex(candidate));
            if swatch.clicked() {
                *color = candidate;
            }
        }
    });
}

/// The hex field. While it has focus it edits a draft, so partial input ("F8")
/// isn't replaced by the current colour every frame; a valid draft applies as it
/// is typed, an invalid one is dropped on blur. Returns whether it has focus.
fn hex_field(ui: &mut Ui, id: Id, label: &str, color: &mut [u8; 3]) -> bool {
    let draft_id = id.with("hex_draft");
    let field_id = id.with("hex_field");
    let focused = ui.memory(|memory| memory.has_focus(field_id));
    let mut hex = if focused {
        ui.data(|data| data.get_temp::<String>(draft_id))
            .unwrap_or_else(|| to_hex(*color))
    } else {
        to_hex(*color)
    };
    let response = ui
        .add(
            TextEdit::singleline(&mut hex)
                .id(field_id)
                .desired_width(HEX_FIELD_WIDTH)
                .char_limit(7),
        )
        .accessible_name(format!("{label} hex"));
    if response.changed()
        && let Some(parsed) = parse_hex(&hex)
    {
        *color = parsed;
    }
    let has_focus = response.has_focus();
    if has_focus {
        ui.data_mut(|data| data.insert_temp(draft_id, hex));
    } else {
        ui.data_mut(|data| data.remove::<String>(draft_id));
    }
    has_focus
}

/// Note the colour as used once the edit is over, so a picker drag or typing
/// adds one entry instead of one per frame.
fn remember_when_settled(ui: &Ui, id: Id, color: [u8; 3], before: [u8; 3], editing: bool) {
    let pending_id = id.with("pending_recent");
    if color != before {
        ui.data_mut(|data| data.insert_temp(pending_id, color));
    }
    if editing {
        return;
    }
    if let Some(pending) = ui.data_mut(|data| data.remove_temp::<[u8; 3]>(pending_id)) {
        ui.data_mut(|data| {
            remember(
                data.get_persisted_mut_or_default(recent_colors_id()),
                pending,
            );
        });
    }
}

fn recent_colors_id() -> Id {
    Id::unique("egui_mev_widgets::recent_colors")
}

fn recent_colors(ctx: &egui::Context) -> Vec<[u8; 3]> {
    ctx.data_mut(|data| {
        data.get_persisted::<Vec<[u8; 3]>>(recent_colors_id())
            .unwrap_or_default()
    })
}

/// Move `color` to the front; the oldest drops off past [`MAX_RECENT_COLORS`].
fn remember(recent: &mut Vec<[u8; 3]>, color: [u8; 3]) {
    recent.retain(|existing| *existing != color);
    recent.insert(0, color);
    recent.truncate(MAX_RECENT_COLORS);
}

fn to_hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// Parse `RRGGBB` or the `RGB` shorthand, with or without a leading `#`.
fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let text = text.trim().trim_start_matches('#');
    if !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    match text.len() {
        // Each digit doubled ("F80" is FF8800).
        3 => {
            let digit = |shift: u32| ((value >> shift) & 0xF) as u8 * 17;
            Some([digit(8), digit(4), digit(0)])
        }
        6 => {
            let [_, r, g, b] = value.to_be_bytes();
            Some([r, g, b])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::presets;

    use super::*;

    #[test]
    fn parses_full_and_shorthand_hex() {
        assert_eq!(parse_hex("#FF8000"), Some([0xFF, 0x80, 0x00]));
        assert_eq!(parse_hex("ff8000"), Some([0xFF, 0x80, 0x00]));
        assert_eq!(parse_hex(" #F80 "), Some([0xFF, 0x88, 0x00]));
    }

    #[test]
    fn rejects_partial_and_invalid_hex() {
        for text in [
            "", "#", "F8", "FF80", "FF800", "FF80000", "GGGGGG", "+12345",
        ] {
            assert_eq!(parse_hex(text), None, "{text:?}");
        }
    }

    #[test]
    fn hex_round_trips() {
        let color = [0x0A, 0x84, 0xFF];
        assert_eq!(to_hex(color), "#0A84FF");
        assert_eq!(parse_hex(&to_hex(color)), Some(color));
    }

    #[test]
    fn recent_colors_move_to_front_without_duplicates() {
        let mut recent = Vec::new();
        remember(&mut recent, [1, 1, 1]);
        remember(&mut recent, [2, 2, 2]);
        remember(&mut recent, [1, 1, 1]);
        assert_eq!(recent, [[1, 1, 1], [2, 2, 2]]);
    }

    #[test]
    fn recent_colors_stay_within_the_cap() {
        let mut recent = Vec::new();
        for n in 0..=MAX_RECENT_COLORS as u8 {
            remember(&mut recent, [n, 0, 0]);
        }
        assert_eq!(recent.len(), MAX_RECENT_COLORS);
        assert_eq!(recent[0], [MAX_RECENT_COLORS as u8, 0, 0]);
    }

    #[test]
    fn a_toggle_switch_flips_and_reports_a_change() {
        let mut harness = Harness::builder().with_size([200.0, 80.0]).build_ui_state(
            |ui, (on, changed): &mut (bool, u32)| {
                if toggle_switch(ui, on, "Wi-Fi").changed() {
                    *changed += 1;
                }
            },
            (false, 0),
        );
        harness.run();
        harness.get_by_label("Wi-Fi").click();
        harness.run();
        assert_eq!(*harness.state(), (true, 1));
    }

    #[test]
    fn a_settings_group_shows_titles_and_controls() {
        let mut harness = Harness::builder().with_size([500.0, 300.0]).build_ui_state(
            |ui, on: &mut bool| {
                settings_group(ui, "General", |group| {
                    group.row("Sounds", Some("Play sounds on events"), |ui| {
                        toggle_switch(ui, on, "Sounds switch");
                    });
                    group.note("Changes apply at once.");
                });
            },
            false,
        );
        harness.run();
        harness.get_by_label("Sounds switch").click();
        harness.run();
        assert!(*harness.state());
    }

    #[test]
    fn typing_a_valid_hex_applies_it_and_partial_input_does_not() {
        let mut harness = Harness::builder().with_size([400.0, 80.0]).build_ui_state(
            |ui, color: &mut [u8; 3]| color_swatch_row(ui, "Accent", color),
            [0, 0, 0],
        );
        harness.run();
        harness.get_by_label("Accent hex").focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.event(egui::Event::Text("F8".to_owned()));
        harness.run();
        assert_eq!(*harness.state(), [0, 0, 0]);
        harness.event(egui::Event::Text("0".to_owned()));
        harness.run();
        assert_eq!(*harness.state(), [0xFF, 0x88, 0x00]);
    }

    #[test]
    fn key_caps_and_rows_are_labelled() {
        let mut harness = Harness::builder().with_size([400.0, 120.0]).build_ui(|ui| {
            ui.horizontal(|ui| key_caps(ui, &["Ctrl+F", "F3"]));
            kv_row(ui, "Vertices", "538");
        });
        harness.run();
        assert!(harness.query_by_label("Ctrl+F").is_some());
        assert!(harness.query_by_label("F3").is_some());
        assert!(harness.query_by_label("538").is_some());
    }

    #[test]
    fn text_fields_are_as_tall_as_the_metrics_say() {
        for (metrics, expected) in [(Metrics::GRAPHITE, 30.0), (Metrics::FLUENT, 32.0)] {
            let ctx = egui::Context::default();
            egui_mev_theme::Theme::dark_only(presets::GRAPHITE_DARK)
                .with_metrics(metrics)
                .install(&ctx);
            egui_mev_theme::fonts::install_test(&ctx, &egui_mev_theme::FontOptions::default());
            crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |_| {});
            crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
                let mut text = String::new();
                let edit = text_field(ui, &mut text);
                let response = ui.add(edit);
                assert!(response.rect.height() >= expected, "{metrics:?}");
            });
        }
    }
}
