//! Window chrome behaviour: dragging by the toolbar, collapsing the sidebar.

use egui::{Rect, Sense, Ui, ViewportCommand};

/// With a transparent macOS title bar the toolbar doubles as the drag region:
/// dragging empty toolbar space moves the window and a double-click zooms it.
/// Call it before adding the toolbar's controls, so they keep priority. It does
/// nothing on other systems, whose title bars do this themselves.
pub fn handle_window_drag(ui: &Ui, rect: Rect) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let response = ui.interact(
        rect,
        ui.scope_id().with("toolbar_drag"),
        Sense::click_and_drag(),
    );
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
}

/// Decide the sidebar's `(visible, auto_hidden)` state when the window width
/// changes from `previous_width` to `width` (`previous_width` is `0.0` on the
/// first frame). Like macOS split views, the sidebar collapses when the window
/// narrows below `threshold` and reopens when it widens again, but only if it
/// was collapsed automatically. Only width crossings act, so manual toggles
/// stick at any size.
#[must_use]
pub fn sidebar_auto_collapse(
    previous_width: f32,
    width: f32,
    threshold: f32,
    visible: bool,
    auto_hidden: bool,
) -> (bool, bool) {
    let is_wide = width >= threshold;
    let was_wide = previous_width <= 0.0 || previous_width >= threshold;
    if was_wide && !is_wide && visible {
        (false, true)
    } else if !was_wide && is_wide && auto_hidden {
        (true, false)
    } else {
        (visible, auto_hidden)
    }
}

#[cfg(test)]
mod tests {
    use super::sidebar_auto_collapse;

    const THRESHOLD: f32 = 1100.0;

    #[test]
    fn sidebar_collapses_when_window_narrows() {
        assert_eq!(
            sidebar_auto_collapse(1280.0, 1000.0, THRESHOLD, true, false),
            (false, true)
        );
    }

    #[test]
    fn sidebar_collapses_on_first_narrow_frame() {
        assert_eq!(
            sidebar_auto_collapse(0.0, 900.0, THRESHOLD, true, false),
            (false, true)
        );
    }

    #[test]
    fn sidebar_reopens_when_auto_hidden_and_window_widens() {
        assert_eq!(
            sidebar_auto_collapse(1000.0, 1280.0, THRESHOLD, false, true),
            (true, false)
        );
    }

    #[test]
    fn sidebar_stays_closed_when_user_hid_it() {
        assert_eq!(
            sidebar_auto_collapse(1000.0, 1280.0, THRESHOLD, false, false),
            (false, false)
        );
    }

    #[test]
    fn sidebar_manual_open_sticks_while_narrow() {
        assert_eq!(
            sidebar_auto_collapse(1000.0, 980.0, THRESHOLD, true, false),
            (true, false)
        );
    }

    #[test]
    fn sidebar_stays_auto_hidden_while_narrow_and_keeps_the_mark() {
        assert_eq!(
            sidebar_auto_collapse(1000.0, 980.0, THRESHOLD, false, true),
            (false, true)
        );
    }

    #[test]
    fn exactly_the_threshold_counts_as_wide() {
        assert_eq!(
            sidebar_auto_collapse(1280.0, THRESHOLD, THRESHOLD, true, false),
            (true, false)
        );
        assert_eq!(
            sidebar_auto_collapse(1000.0, THRESHOLD, THRESHOLD, false, true),
            (true, false)
        );
    }

    #[cfg(target_os = "macos")]
    mod drag {
        use super::super::handle_window_drag;
        use egui::{
            Context, Event, Modifiers, PointerButton, RawInput, Rect, ViewportCommand, ViewportId,
            pos2, vec2,
        };

        /// One frame with `events` at `time`; returns the viewport commands it sent.
        fn frame(ctx: &Context, time: f64, events: Vec<Event>) -> Vec<ViewportCommand> {
            let input = RawInput {
                time: Some(time),
                events,
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0))),
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                handle_window_drag(ui, Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 40.0)));
            });
            output.textures_delta.clear();
            output
                .viewport_output
                .remove(&ViewportId::ROOT)
                .map(|viewport| viewport.commands)
                .unwrap_or_default()
        }

        fn button(pressed: bool, at: egui::Pos2) -> Event {
            Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }
        }

        #[test]
        fn dragging_the_region_starts_a_window_drag() {
            let ctx = Context::default();
            let at = pos2(100.0, 20.0);
            frame(&ctx, 0.0, vec![Event::PointerMoved(at)]);
            frame(&ctx, 0.1, vec![button(true, at)]);
            let commands = frame(&ctx, 0.8, vec![Event::PointerMoved(pos2(140.0, 20.0))]);
            assert!(
                commands.contains(&ViewportCommand::StartDrag),
                "{commands:?}"
            );
        }

        #[test]
        fn double_clicking_the_region_zooms_the_window() {
            let ctx = Context::default();
            let at = pos2(100.0, 20.0);
            frame(&ctx, 0.0, vec![Event::PointerMoved(at)]);
            frame(&ctx, 0.01, vec![button(true, at)]);
            frame(&ctx, 0.02, vec![button(false, at)]);
            frame(&ctx, 0.03, vec![button(true, at)]);
            let commands = frame(&ctx, 0.04, vec![button(false, at)]);
            assert!(
                commands.contains(&ViewportCommand::Maximized(true)),
                "{commands:?}"
            );
        }

        #[test]
        fn pressing_outside_the_region_does_nothing() {
            let ctx = Context::default();
            let at = pos2(100.0, 100.0);
            frame(&ctx, 0.0, vec![Event::PointerMoved(at)]);
            frame(&ctx, 0.1, vec![button(true, at)]);
            let commands = frame(&ctx, 0.8, vec![Event::PointerMoved(pos2(140.0, 100.0))]);
            assert!(
                !commands.contains(&ViewportCommand::StartDrag),
                "{commands:?}"
            );
        }
    }
}
