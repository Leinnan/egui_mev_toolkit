//! Modal sheets anchored under the toolbar, a progress sheet, and the button row
//! at the bottom of a dialog.

use egui::{
    Align, Align2, Button, Color32, Context, Frame, Id, Layout, Margin, Modal, Response, RichText,
    Stroke, Ui, Vec2,
};
use egui_mev_theme::{Metrics, Palette};

use crate::buttons::{primary_button, secondary_button};

/// Height a sheet needs besides its body: title, buttons, margins.
const SHEET_CHROME: f32 = 150.0;

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
    Frame::window(&ctx.global_style())
        .fill(Palette::of_ctx(ctx).window_bg)
        .inner_margin(Margin::same(20))
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
    let mut result = DialogResult::Open;
    let (anchor, offset) = if preview {
        (Align2::RIGHT_TOP, [-16.0, 84.0])
    } else {
        (Align2::CENTER_TOP, [0.0, 52.0])
    };
    let mut modal = Modal::new(Id::unique(("egui_mev_widgets::sheet", title)))
        .frame(sheet_frame(ctx))
        .area(
            Modal::default_area(Id::unique(("egui_mev_widgets::sheet_area", title)))
                .anchor(anchor, offset),
        );
    if preview {
        modal = modal.backdrop_color(Color32::TRANSPARENT);
    }
    let modal = modal.show(ctx, |ui| {
        ui.set_width(width);
        ui.label(RichText::new(title).heading().strong());
        ui.add_space(12.0);
        // A tall body scrolls, so OK and Cancel stay on screen in a short window.
        let room = ctx.content_rect().height() - offset[1] - SHEET_CHROME;
        egui::ScrollArea::vertical()
            .id_salt(("sheet_body", title))
            .max_height(room.max(120.0))
            .auto_shrink([false, true])
            .show(ui, body);
        ui.add_space(16.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if primary_button(ui, ok).clicked() {
                result = DialogResult::Accepted;
            }
            if secondary_button(ui, "Cancel").clicked() {
                result = DialogResult::Cancelled;
            }
        });
    });
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
    let mut modal = Modal::new(id)
        .area(Modal::default_area(id.with("area")).anchor(Align2::CENTER_TOP, [0.0, 52.0]));
    if !visible {
        modal
            .frame(Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |_| ());
        return false;
    }
    modal = modal.frame(sheet_frame(ctx));
    let p = Palette::of_ctx(ctx);
    let cancel = modal
        .show(ctx, |ui| {
            ui.set_width(280.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(label).strong());
            });
            if let Some(fraction) = fraction {
                ui.add_space(10.0);
                // The bar's own percentage is drawn in the selection colour, which
                // would be blue on blue here.
                let percent = RichText::new(format!("{:.0}%", fraction * 100.0)).color(p.text);
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .text(percent)
                        .desired_height(14.0),
                );
                // Work finishes without waking the UI; keep the bar moving.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
            }
            ui.add_space(14.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                secondary_button(ui, "Cancel").clicked()
            })
            .inner
        })
        .inner;
    cancel || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
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

/// The button row at the bottom of a dialog: right-aligned, as on Windows, with
/// `buttons` in reading order (main action first, Cancel last) and of equal minimum
/// width. With `focus_first`, the first enabled button takes keyboard focus so Enter
/// runs it; dialogs with a text field leave it off. Returns the index of the clicked
/// button.
pub fn dialog_buttons(
    ui: &mut Ui,
    focus_first: bool,
    buttons: &[DialogButton<'_>],
) -> Option<usize> {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    ui.add_space(12.0);
    let height = m.control_height;
    let mut clicked = None;
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let first = buttons.iter().position(|button| button.enabled);
            // Right to left, so the last button is added first.
            for (index, button) in buttons.iter().enumerate().rev() {
                let fill = match button.kind {
                    _ if !button.enabled => None,
                    ButtonKind::Primary => Some(p.accent),
                    ButtonKind::Danger => Some(p.critical),
                    ButtonKind::Plain => None,
                };
                let widget = fill.map_or_else(
                    || Button::new(button.label),
                    |fill| {
                        Button::new(RichText::new(button.label).color(p.on_accent))
                            .fill(fill)
                            .stroke(Stroke::NONE)
                    },
                );
                let response = ui.add_enabled(
                    button.enabled,
                    widget.min_size(Vec2::new(DIALOG_BUTTON_WIDTH, height)),
                );
                if focus_first && Some(index) == first && ui.memory(|mem| mem.focused().is_none()) {
                    response.request_focus();
                }
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        response.rect.expand(2.0),
                        m.control_radius,
                        Stroke::new(1.0, ui.visuals().strong_text_color()),
                        egui::StrokeKind::Outside,
                    );
                }
                if response.clicked()
                    || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    clicked = Some(index);
                }
            }
        },
    );
    clicked
}

#[cfg(test)]
mod tests {
    use egui::Key;
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::presets;

    use super::*;

    fn sheet_harness<'a>() -> Harness<'a, (DialogResult, u32)> {
        Harness::builder().with_size([600.0, 400.0]).build_ui_state(
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
        let mut harness = sheet_harness();
        harness.run();
        harness.get_by_label("OK").click();
        harness.run();
        assert_eq!(harness.state().0, DialogResult::Accepted);

        let mut harness = sheet_harness();
        harness.run();
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(harness.state().0, DialogResult::Cancelled);
    }

    #[test]
    fn return_accepts_and_escape_cancels() {
        let mut harness = sheet_harness();
        harness.run();
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().0, DialogResult::Accepted);

        let mut harness = sheet_harness();
        harness.run();
        harness.key_press(Key::Escape);
        harness.run();
        assert_eq!(harness.state().0, DialogResult::Cancelled);
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
    fn dialog_buttons_return_the_clicked_index_in_reading_order() {
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
        let mut harness = Harness::builder().with_size([400.0, 120.0]).build_ui_state(
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
        assert_eq!(*harness.state(), Some(1));
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
