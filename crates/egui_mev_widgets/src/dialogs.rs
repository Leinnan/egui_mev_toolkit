//! Modal dialogs and the button row at the bottom of one. Graphite draws macOS
//! sheets anchored under the toolbar, with buttons at the right and the main action
//! last. Fluent draws `WinUI` `ContentDialog`s: centred over a smoke backdrop, with
//! a footer band of equal-width buttons and the main action first.

use egui::{
    Align, Align2, Button, Color32, Context, CornerRadius, Frame, Id, Layout, Margin, Modal,
    Response, RichText, Stroke, Ui, Vec2,
};
use egui_mev_theme::{Metrics, Palette, ThemeFamily, fonts};

use crate::buttons::{neutral_text, primary_button, secondary_button};

/// Height a macOS sheet needs besides its body: title, buttons, margins.
const SHEET_CHROME: f32 = 150.0;

/// Height a `ContentDialog` needs besides its body: title, footer band, margins.
const CONTENT_DIALOG_CHROME: f32 = 216.0;

/// Width of a `ContentDialog`, unless its body asks for more (px).
const CONTENT_DIALOG_WIDTH: f32 = 400.0;

/// Padding of a `ContentDialog`'s content and footer (px).
const CONTENT_DIALOG_MARGIN: i8 = 24;

/// Room to keep beside a `ContentDialog` in a narrow window (px).
const CONTENT_DIALOG_OUTSIDE: f32 = 48.0;

/// Narrowest a button of [`dialog_buttons`] gets (px).
const DIALOG_BUTTON_WIDTH: f32 = 88.0;

/// What the user did with a dialog this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    /// Still open.
    Open,
    /// OK / default button.
    Accepted,
    /// Cancel or Escape.
    Cancelled,
}

fn sheet_frame(ctx: &Context) -> Frame {
    let margin = match ThemeFamily::of_ctx(ctx) {
        ThemeFamily::Graphite => Margin::same(20),
        // The content and the footer band each bring their own.
        ThemeFamily::Fluent => Margin::ZERO,
    };
    Frame::window(&ctx.global_style())
        .fill(Palette::of_ctx(ctx).window_bg)
        .inner_margin(margin)
        .corner_radius(CornerRadius::same(Metrics::of_ctx(ctx).overlay_radius))
}

/// A modal in the place and over the backdrop `ctx`'s family gives its dialogs.
fn sheet_modal(ctx: &Context, id: Id) -> Modal {
    let modal = Modal::new(id).frame(sheet_frame(ctx));
    let area = Modal::default_area(id.with("area"));
    match ThemeFamily::of_ctx(ctx) {
        ThemeFamily::Graphite => modal.area(area.anchor(Align2::CENTER_TOP, [0.0, 52.0])),
        ThemeFamily::Fluent => modal
            .area(area.anchor(Align2::CENTER_CENTER, [0.0, 0.0]))
            .backdrop_color(Palette::of_ctx(ctx).smoke),
    }
}

/// The content part of a `ContentDialog`: everything above its footer band.
fn content_dialog_body(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let spacing = ui.spacing().item_spacing.y;
    // No gap between the content and the band; the content keeps its own spacing.
    ui.spacing_mut().item_spacing.y = 0.0;
    Frame::new()
        .inner_margin(Margin::same(CONTENT_DIALOG_MARGIN))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = spacing;
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

/// The footer band of a `ContentDialog`, reaching its bottom corners.
fn content_dialog_footer<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let radius = Metrics::of(ui).overlay_radius;
    Frame::new()
        .fill(Palette::of(ui).sidebar_bg)
        .corner_radius(CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius,
            se: radius,
        })
        .inner_margin(Margin::same(CONTENT_DIALOG_MARGIN))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

/// `count` columns of equal width, each filled by its button.
fn equal_columns(ui: &mut Ui, count: usize, mut add_button: impl FnMut(usize, &mut Ui)) {
    if count == 0 {
        return;
    }
    ui.columns(count, |columns| {
        for (index, column) in columns.iter_mut().enumerate() {
            column.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                add_button(index, ui);
            });
        }
    });
}

/// The widest a `ContentDialog` gets in a window of this width, for a body of
/// `body_width`.
fn content_dialog_width(ctx: &Context, body_width: f32) -> f32 {
    let margins = 2.0 * f32::from(CONTENT_DIALOG_MARGIN);
    let room = ctx.content_rect().width() - CONTENT_DIALOG_OUTSIDE;
    (body_width + margins).max(CONTENT_DIALOG_WIDTH).min(room)
}

/// Draw a sheet titled `title` whose body is `body`, with OK and Cancel buttons.
/// Return accepts and Escape cancels.
pub fn sheet(ctx: &Context, title: &str, width: f32, body: impl FnOnce(&mut Ui)) -> DialogResult {
    sheet_with(ctx, title, width, false, body)
}

/// Like [`sheet`]; `preview` docks the sheet at the top right without dimming the
/// window, so a live preview under it stays visible.
pub fn sheet_with(
    ctx: &Context,
    title: &str,
    width: f32,
    preview: bool,
    body: impl FnOnce(&mut Ui),
) -> DialogResult {
    sheet_labeled(ctx, title, width, preview, "OK", body)
}

/// Like [`sheet_with`], with `ok` as the default button's label (a verb such as
/// "Rasterize and Apply").
pub fn sheet_labeled(
    ctx: &Context,
    title: &str,
    width: f32,
    preview: bool,
    ok: &str,
    body: impl FnOnce(&mut Ui),
) -> DialogResult {
    let family = ThemeFamily::of_ctx(ctx);
    let id = Id::unique(("egui_mev_widgets::sheet", title));
    let mut modal = sheet_modal(ctx, id);
    if preview {
        modal = modal
            .area(Modal::default_area(id.with("area")).anchor(Align2::RIGHT_TOP, [-16.0, 84.0]))
            .backdrop_color(Color32::TRANSPARENT);
    }
    let chrome = match family {
        ThemeFamily::Graphite if preview => 84.0 + SHEET_CHROME,
        ThemeFamily::Graphite => 52.0 + SHEET_CHROME,
        ThemeFamily::Fluent => CONTENT_DIALOG_CHROME,
    };
    // A tall body scrolls, so OK and Cancel stay on screen in a short window.
    let room = (ctx.content_rect().height() - chrome).max(120.0);
    let scroll = egui::ScrollArea::vertical()
        .id_salt(("sheet_body", title))
        .max_height(room)
        .auto_shrink([false, true]);
    let modal = modal.show(ctx, |ui| match family {
        ThemeFamily::Graphite => macos_sheet(ui, title, width, scroll, ok, body),
        ThemeFamily::Fluent => content_dialog(ui, title, width, scroll, ok, body),
    });
    let mut result = modal.inner;
    if preview {
        ctx.data_mut(|d| d.insert_temp(preview_sheet_id(), Some(modal.response.rect)));
    }
    // Escape cancels (unless it closes a popup first). A click outside the sheet
    // does not: with a live preview that is the content, which stays usable.
    let popup = modal.any_popup_open || egui::Popup::is_any_open(ctx);
    let escape = || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if result == DialogResult::Open
        && (modal.response.should_close() || (modal.is_top_modal && !popup && escape()))
    {
        result = DialogResult::Cancelled;
    }
    // Return accepts, unless it is choosing an item in an open popup.
    if result == DialogResult::Open && !popup && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        result = DialogResult::Accepted;
    }
    result
}

/// A macOS sheet: a title, the body with muted labels, and Cancel to the left of
/// the default button at the right.
fn macos_sheet(
    ui: &mut Ui,
    title: &str,
    width: f32,
    scroll: egui::ScrollArea,
    ok: &str,
    body: impl FnOnce(&mut Ui),
) -> DialogResult {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    ui.set_width(width);
    ui.label(RichText::new(title).strong().size(m.subtitle).color(p.text));
    ui.add_space(6.0);
    scroll.show(ui, |ui| {
        // Labels only: buttons and text fields in the body keep their own colour.
        ui.visuals_mut().widgets.noninteractive.fg_stroke.color = p.text_weak;
        body(ui);
    });
    ui.add_space(16.0);
    let mut result = DialogResult::Open;
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if primary_button(ui, ok).clicked() {
            result = DialogResult::Accepted;
        }
        if secondary_button(ui, "Cancel").clicked() {
            result = DialogResult::Cancelled;
        }
    });
    result
}

/// A `WinUI` `ContentDialog`: a semibold title, the body, and a footer band with
/// the default button and Cancel side by side.
fn content_dialog(
    ui: &mut Ui,
    title: &str,
    width: f32,
    scroll: egui::ScrollArea,
    ok: &str,
    body: impl FnOnce(&mut Ui),
) -> DialogResult {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    ui.set_width(content_dialog_width(ui.ctx(), width));
    content_dialog_body(ui, |ui| {
        let font = fonts::semibold(ui.ctx(), m.subtitle);
        ui.label(RichText::new(title).font(font).color(p.text));
        ui.add_space(12.0);
        scroll.show(ui, body);
    });
    let mut result = DialogResult::Open;
    content_dialog_footer(ui, |ui| {
        equal_columns(ui, 2, |index, ui| {
            if index == 0 {
                if primary_button(ui, ok).clicked() {
                    result = DialogResult::Accepted;
                }
            } else if secondary_button(ui, "Cancel").clicked() {
                result = DialogResult::Cancelled;
            }
        });
    });
    result
}

/// Give `response`'s widget keyboard focus on the first frame its sheet shows, so
/// typing goes straight into it.
pub fn focus_on_open(ui: &Ui, response: &Response) {
    let id = response.id.with("shown_at_pass");
    let pass = ui.ctx().cumulative_pass_nr();
    let last: Option<u64> = ui.data(|d| d.get_temp(id));
    if last.is_none_or(|last| last + 1 < pass) {
        response.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(id, pass));
}

fn preview_sheet_id() -> Id {
    Id::unique("egui_mev_widgets::preview_sheet_rect")
}

/// Screen rectangle of the live-preview sheet drawn last frame, if one is open, so
/// the content under it can navigate around it. Each call takes the value, so it
/// is `None` once the sheet stops being drawn.
#[must_use]
pub fn take_preview_sheet_rect(ctx: &Context) -> Option<egui::Rect> {
    ctx.data_mut(|d| d.remove_temp::<Option<egui::Rect>>(preview_sheet_id()))
        .flatten()
}

/// A modal sheet with a spinner, `label` ("Applying Blur…"), the share done when
/// known, and Cancel (or Escape); returns whether it was cancelled. With `visible`
/// false it draws nothing but still blocks input, for the first moments of a short
/// task.
pub fn progress(ctx: &Context, label: &str, fraction: Option<f32>, visible: bool) -> bool {
    let id = Id::unique("egui_mev_widgets::progress_sheet");
    let modal = sheet_modal(ctx, id);
    if !visible {
        modal
            .frame(Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |_| ());
        return false;
    }
    let cancel = modal
        .show(ctx, |ui| match ThemeFamily::of(ui) {
            ThemeFamily::Graphite => {
                ui.set_width(280.0);
                progress_body(ui, label, fraction);
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    secondary_button(ui, "Cancel").clicked()
                })
                .inner
            }
            ThemeFamily::Fluent => {
                ui.set_width(content_dialog_width(ui.ctx(), 0.0));
                content_dialog_body(ui, |ui| progress_body(ui, label, fraction));
                content_dialog_footer(ui, |ui| {
                    let mut cancel = false;
                    // A lone button takes the second column, as in a `ContentDialog`.
                    equal_columns(ui, 2, |index, ui| {
                        if index == 1 {
                            cancel = secondary_button(ui, "Cancel").clicked();
                        }
                    });
                    cancel
                })
            }
        })
        .inner;
    cancel || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

/// The spinner, the label and, when the share done is known, a bar.
fn progress_body(ui: &mut Ui, label: &str, fraction: Option<f32>) {
    let p = Palette::of(ui);
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(RichText::new(label).strong().color(p.text));
    });
    if let Some(fraction) = fraction {
        ui.add_space(10.0);
        // The bar's own percentage is drawn in the selection colour, which
        // would be blue on blue here.
        let percent = RichText::new(format!("{:.0}%", fraction * 100.0)).color(p.text);
        // On a light dialog the recessed well is as white as the dialog.
        if !ui.visuals().dark_mode {
            ui.visuals_mut().extreme_bg_color = p.control_track;
        }
        ui.add(
            egui::ProgressBar::new(fraction)
                .text(percent)
                .desired_height(14.0),
        );
        // Work finishes without waking the UI; keep the bar moving.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// How a dialog button is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The dialog's main action, filled with the accent colour.
    Primary,
    /// A main action that destroys data, filled red.
    Danger,
    /// Any other button.
    Plain,
}

/// One button of [`dialog_buttons`].
#[derive(Clone, Copy, Debug)]
pub struct DialogButton<'a> {
    /// The button's text.
    pub label: &'a str,
    /// How it is drawn.
    pub kind: ButtonKind,
    /// Whether it can be clicked.
    pub enabled: bool,
}

impl<'a> DialogButton<'a> {
    /// The dialog's main action.
    #[must_use]
    pub const fn primary(label: &'a str) -> Self {
        Self {
            label,
            kind: ButtonKind::Primary,
            enabled: true,
        }
    }

    /// A main action that destroys data.
    #[must_use]
    pub const fn danger(label: &'a str) -> Self {
        Self {
            label,
            kind: ButtonKind::Danger,
            enabled: true,
        }
    }

    /// Any other button, such as Cancel.
    #[must_use]
    pub const fn plain(label: &'a str) -> Self {
        Self {
            label,
            kind: ButtonKind::Plain,
            enabled: true,
        }
    }

    /// The same button, enabled or not.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// The button row at the bottom of a dialog. List `buttons` with the main action
/// first and Cancel last; the family decides where each is drawn. Graphite aligns
/// them right with the main action rightmost, as macOS does. Fluent gives them equal
/// widths across the row with the main action leftmost, as Windows does. With
/// `focus_first`, the first enabled button takes keyboard focus so Enter runs it;
/// dialogs with a text field leave it off. Returns the index in `buttons` of the
/// clicked button.
pub fn dialog_buttons(
    ui: &mut Ui,
    focus_first: bool,
    buttons: &[DialogButton<'_>],
) -> Option<usize> {
    let m = Metrics::of(ui);
    // In a row the gap would push the buttons sideways instead.
    if !ui.layout().is_horizontal() {
        ui.add_space(12.0);
    }
    let first = buttons.iter().position(|button| button.enabled);
    let focus = focus_first.then_some(first).flatten();
    let mut clicked = None;
    match ThemeFamily::of(ui) {
        ThemeFamily::Graphite => {
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), m.control_height),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    // Right to left, so the first button ends up rightmost.
                    for (index, button) in buttons.iter().enumerate() {
                        let size = Vec2::new(DIALOG_BUTTON_WIDTH, m.control_height);
                        if dialog_button(ui, button, size, focus == Some(index)) {
                            clicked = Some(index);
                        }
                    }
                },
            );
        }
        ThemeFamily::Fluent => {
            equal_columns(ui, buttons.len(), |index, ui| {
                let size = Vec2::new(0.0, m.control_height);
                if dialog_button(ui, &buttons[index], size, focus == Some(index)) {
                    clicked = Some(index);
                }
            });
        }
    }
    clicked
}

/// One button of [`dialog_buttons`]. Returns whether it was activated.
fn dialog_button(ui: &mut Ui, button: &DialogButton<'_>, min_size: Vec2, focus: bool) -> bool {
    let p = Palette::of(ui);
    let fill = match button.kind {
        _ if !button.enabled => None,
        ButtonKind::Primary => Some(p.accent),
        ButtonKind::Danger => Some(p.critical),
        ButtonKind::Plain => None,
    };
    let widget = fill.map_or_else(
        || Button::new(RichText::new(button.label).color(neutral_text(ui))),
        |fill| {
            Button::new(RichText::new(button.label).color(p.on_accent))
                .fill(fill)
                .stroke(Stroke::NONE)
        },
    );
    let response = ui.add_enabled(button.enabled, widget.min_size(min_size));
    if focus && ui.memory(|mem| mem.focused().is_none()) {
        response.request_focus();
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            Metrics::of(ui).control_radius,
            Stroke::new(1.0, ui.visuals().strong_text_color()),
            egui::StrokeKind::Outside,
        );
    }
    response.clicked() || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
}

#[cfg(test)]
mod tests {
    use egui::Key;
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::presets;

    use super::*;
    use crate::test_support::{family_theme, frame_output, painted_colors, themed_family};

    const FAMILIES: [ThemeFamily; 2] = [ThemeFamily::Graphite, ThemeFamily::Fluent];

    fn sheet_harness<'a>(family: ThemeFamily) -> Harness<'a, (DialogResult, u32)> {
        Harness::builder()
            .with_size([600.0, 400.0])
            .with_context_setup(move |ctx| egui_mev_test::install(ctx, family_theme(family)))
            .build_ui_state(
                |ui, (result, _): &mut (DialogResult, u32)| {
                    let outcome = sheet(ui.ctx(), "Resize", 240.0, |ui| {
                        ui.label("Body");
                    });
                    if outcome != DialogResult::Open {
                        *result = outcome;
                    }
                },
                (DialogResult::Open, 0),
            )
    }

    #[test]
    fn ok_accepts_and_cancel_cancels() {
        for family in FAMILIES {
            let mut harness = sheet_harness(family);
            harness.run();
            harness.get_by_label("OK").click();
            harness.run();
            assert_eq!(harness.state().0, DialogResult::Accepted, "{family:?}");

            let mut harness = sheet_harness(family);
            harness.run();
            harness.get_by_label("Cancel").click();
            harness.run();
            assert_eq!(harness.state().0, DialogResult::Cancelled, "{family:?}");
        }
    }

    #[test]
    fn return_accepts_and_escape_cancels() {
        for family in FAMILIES {
            let mut harness = sheet_harness(family);
            harness.run();
            harness.key_press(Key::Enter);
            harness.run();
            assert_eq!(harness.state().0, DialogResult::Accepted, "{family:?}");

            let mut harness = sheet_harness(family);
            harness.run();
            harness.key_press(Key::Escape);
            harness.run();
            assert_eq!(harness.state().0, DialogResult::Cancelled, "{family:?}");
        }
    }

    #[test]
    fn a_sheet_puts_its_buttons_where_its_family_does() {
        let mut harness = sheet_harness(ThemeFamily::Graphite);
        harness.run();
        let (ok, cancel) = (
            harness.get_by_label("OK").rect(),
            harness.get_by_label("Cancel").rect(),
        );
        assert!(cancel.right() <= ok.left(), "Cancel is left of OK on macOS");

        let mut harness = sheet_harness(ThemeFamily::Fluent);
        harness.run();
        let (ok, cancel) = (
            harness.get_by_label("OK").rect(),
            harness.get_by_label("Cancel").rect(),
        );
        assert!(ok.right() <= cancel.left(), "OK comes first on Windows");
        assert!((ok.width() - cancel.width()).abs() < 1.0);
    }

    #[test]
    fn a_content_dialog_is_centred_and_a_macos_sheet_hangs_from_the_top() {
        let mut harness = sheet_harness(ThemeFamily::Graphite);
        harness.run();
        harness.run();
        let graphite_title = harness.get_by_label("Resize").rect();

        let mut harness = sheet_harness(ThemeFamily::Fluent);
        harness.run();
        harness.run();
        let fluent_title = harness.get_by_label("Resize").rect();

        assert!(graphite_title.top() < 80.0, "under the toolbar");
        assert!(fluent_title.top() > 80.0, "in the middle of the window");
    }

    #[test]
    fn a_content_dialog_is_400_wide_unless_its_body_needs_more_or_the_window_less() {
        let ctx = themed_family(ThemeFamily::Fluent, egui::Theme::Dark);
        // The test window is 800 wide.
        assert_eq!(content_dialog_width(&ctx, 240.0), 400.0);
        assert_eq!(content_dialog_width(&ctx, 500.0), 548.0);
        assert_eq!(content_dialog_width(&ctx, 2000.0), 752.0);
    }

    #[test]
    fn only_fluent_dialogs_use_the_smoke_backdrop_and_footer_band() {
        for (family, band) in [(ThemeFamily::Graphite, false), (ThemeFamily::Fluent, true)] {
            let ctx = themed_family(family, egui::Theme::Dark);
            let p = egui_mev_theme::Theme::of(&ctx).dark;
            let draw = |ui: &mut Ui| {
                sheet(ui.ctx(), "Resize", 240.0, |ui| {
                    ui.label("Body");
                });
            };
            // An area paints nothing on the frame it measures itself, then fades in.
            for frame in 0..30 {
                frame_output(&ctx, f64::from(frame) / 60.0, Vec::new(), draw);
            }
            let output = frame_output(&ctx, 0.5, Vec::new(), draw);
            let colors = painted_colors(&output);
            assert_eq!(colors.contains(&p.smoke), band, "{family:?}");
            assert_eq!(colors.contains(&p.sidebar_bg), band, "{family:?}");
        }
    }

    #[test]
    fn dialog_buttons_put_the_main_action_where_the_family_does() {
        for family in FAMILIES {
            let mut harness = Harness::builder()
                .with_size([400.0, 120.0])
                .with_context_setup(move |ctx| egui_mev_test::install(ctx, family_theme(family)))
                .build_ui_state(
                    |ui, clicked: &mut Option<usize>| {
                        let buttons =
                            [DialogButton::primary("Save"), DialogButton::plain("Cancel")];
                        if let Some(index) = dialog_buttons(ui, false, &buttons) {
                            *clicked = Some(index);
                        }
                    },
                    None,
                );
            harness.run();
            let save = harness.get_by_label("Save").rect();
            let cancel = harness.get_by_label("Cancel").rect();
            match family {
                ThemeFamily::Graphite => {
                    assert!(cancel.right() <= save.left());
                    assert!(save.right() > 350.0, "right-aligned");
                }
                ThemeFamily::Fluent => {
                    assert!(save.right() <= cancel.left());
                    assert!((save.width() - cancel.width()).abs() < 1.0);
                    assert!(save.width() > 150.0, "the columns share the row");
                }
            }
            // The index is the caller's order, wherever the button is drawn.
            harness.get_by_label("Cancel").click();
            harness.run();
            assert_eq!(*harness.state(), Some(1), "{family:?}");
            harness.get_by_label("Save").click();
            harness.run();
            assert_eq!(*harness.state(), Some(0), "{family:?}");
        }
    }

    #[test]
    fn a_macos_sheet_mutes_labels_but_not_buttons_in_its_body() {
        let ctx = themed_family(ThemeFamily::Graphite, egui::Theme::Dark);
        let output = frame_output(&ctx, 0.0, Vec::new(), |ui| {
            sheet(ui.ctx(), "Resize", 240.0, |ui| {
                let visuals = &ui.visuals().widgets;
                assert_eq!(
                    visuals.noninteractive.fg_stroke.color,
                    presets::GRAPHITE_DARK.text_weak
                );
                assert_eq!(
                    visuals.inactive.fg_stroke.color,
                    presets::GRAPHITE_DARK.text
                );
            });
        });
        assert!(!output.shapes.is_empty());
    }

    #[test]
    fn a_preview_sheet_reports_its_rect_once() {
        let mut harness = Harness::builder().with_size([600.0, 400.0]).build_ui(|ui| {
            sheet_with(ui.ctx(), "Blur", 200.0, true, |ui| {
                ui.label("Body");
            });
        });
        harness.run();
        let ctx = harness.ctx.clone();
        assert!(take_preview_sheet_rect(&ctx).is_some());
        assert!(take_preview_sheet_rect(&ctx).is_none());
    }

    #[test]
    fn progress_reports_cancel_and_stays_hidden_when_not_visible() {
        let mut harness = Harness::builder().with_size([600.0, 400.0]).build_ui_state(
            |ui, (visible, cancelled): &mut (bool, bool)| {
                *cancelled |= progress(ui.ctx(), "Applying Blur…", None, *visible);
            },
            (false, false),
        );
        harness.run_steps(2);
        assert!(harness.query_by_label("Applying Blur…").is_none());
        harness.state_mut().0 = true;
        harness.run_steps(2);
        harness.get_by_label("Cancel").click();
        harness.run_steps(2);
        assert!(harness.state().1);
    }

    #[test]
    fn dialog_buttons_return_the_clicked_index_in_the_callers_order() {
        let mut harness = Harness::builder().with_size([400.0, 120.0]).build_ui_state(
            |ui, clicked: &mut Option<usize>| {
                let buttons = [
                    DialogButton::primary("Save"),
                    DialogButton::plain("Discard").enabled(false),
                    DialogButton::plain("Cancel"),
                ];
                if let Some(index) = dialog_buttons(ui, false, &buttons) {
                    *clicked = Some(index);
                }
            },
            None,
        );
        harness.run();
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(*harness.state(), Some(2));
        harness.get_by_label("Save").click();
        harness.run();
        assert_eq!(*harness.state(), Some(0));
    }

    #[test]
    fn focus_first_makes_enter_run_the_first_enabled_button() {
        for family in FAMILIES {
            let mut harness = Harness::builder()
                .with_size([400.0, 120.0])
                .with_context_setup(move |ctx| egui_mev_test::install(ctx, family_theme(family)))
                .build_ui_state(
                    |ui, clicked: &mut Option<usize>| {
                        let buttons = [
                            DialogButton::primary("Save").enabled(false),
                            DialogButton::danger("Delete"),
                            DialogButton::plain("Cancel"),
                        ];
                        if let Some(index) = dialog_buttons(ui, true, &buttons) {
                            *clicked = Some(index);
                        }
                    },
                    None,
                );
            harness.run();
            harness.key_press(Key::Enter);
            harness.run();
            assert_eq!(*harness.state(), Some(1), "{family:?}");
        }
    }

    #[test]
    fn sheet_frames_read_the_palette_of_the_current_mode() {
        for (mode, palette) in [
            (egui::Theme::Dark, presets::GRAPHITE_DARK),
            (egui::Theme::Light, presets::GRAPHITE_LIGHT),
        ] {
            let ctx = crate::test_support::themed(mode);
            assert_eq!(sheet_frame(&ctx).fill, palette.window_bg, "{mode:?}");
        }
    }
}
