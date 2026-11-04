//! Window chrome behaviour: the title bar, dragging by it, collapsing the sidebar.

use egui::{Frame, InnerResponse, Panel, Rect, RichText, Sense, Ui, ViewportCommand};
use egui_mev_theme::Palette;

/// Height of a title bar or a unified toolbar.
pub const TOOLBAR_HEIGHT: f32 = 38.0;

/// Leading space a title bar leaves on this platform: room for the traffic
/// lights on macOS, a small margin elsewhere.
pub const TRAFFIC_LIGHT_INSET: f32 = WindowCaption::CURRENT.leading_inset();

/// Who draws a window's title bar. It follows the platform and never the theme
/// family: Fluent simulated on a Mac still sits under the traffic lights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowCaption {
    /// macOS: content runs under a transparent title bar that shows only the
    /// traffic lights, so the app draws the title and handles dragging.
    Unified,
    /// The OS draws a caption above the content, with the title in it, and
    /// handles dragging itself.
    Native,
}

impl WindowCaption {
    const CURRENT: Self = if cfg!(target_os = "macos") {
        Self::Unified
    } else {
        Self::Native
    };

    const fn leading_inset(self) -> f32 {
        match self {
            Self::Unified => 76.0,
            Self::Native => 8.0,
        }
    }
}

/// What [`title_bar`] shows.
#[derive(Clone, Debug, Default)]
pub struct TitleBarOptions {
    /// The window's title, drawn where the OS caption doesn't already show it.
    /// Empty draws nothing.
    pub title: String,
}

impl TitleBarOptions {
    /// A title bar with this title.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

/// A bar across the top of `ui`, [`TOOLBAR_HEIGHT`] tall, that also works as the
/// window's title bar: it leaves room for the platform's traffic lights, drags
/// the window where nothing else takes the pointer, and draws the title unless
/// the OS caption already shows it.
///
/// `contents` gets a centred left-to-right `ui` after the title and adds its own
/// trailing space. Use [`egui::Ui::with_layout`] for controls on the far side.
/// The window should come from `native::unified_titlebar`.
pub fn title_bar<R>(
    ui: &mut Ui,
    options: &TitleBarOptions,
    contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    title_bar_on(ui, options, WindowCaption::CURRENT, contents)
}

fn title_bar_on<R>(
    ui: &mut Ui,
    options: &TitleBarOptions,
    caption: WindowCaption,
    contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let palette = Palette::of(ui);
    Panel::top("egui_mev_title_bar")
        .frame(Frame::new().fill(palette.toolbar_bg))
        .exact_size(TOOLBAR_HEIGHT)
        .show(ui, |ui| {
            // First, so the controls added after it keep input priority.
            window_drag_on(ui, ui.max_rect(), caption);
            ui.horizontal_centered(|ui| {
                ui.add_space(caption.leading_inset());
                if caption == WindowCaption::Unified && !options.title.is_empty() {
                    ui.label(RichText::new(&options.title).strong().color(palette.text));
                }
                contents(ui)
            })
            .inner
        })
}

/// With a transparent macOS title bar the toolbar doubles as the drag region:
/// dragging empty toolbar space moves the window and a double-click zooms it.
/// Call it before adding the toolbar's controls, so they keep priority. It does
/// nothing on other systems, whose title bars do this themselves.
pub fn handle_window_drag(ui: &Ui, rect: Rect) {
    window_drag_on(ui, rect, WindowCaption::CURRENT);
}

fn window_drag_on(ui: &Ui, rect: Rect, caption: WindowCaption) {
    if caption == WindowCaption::Native {
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

    mod title_bar {
        use super::super::{TOOLBAR_HEIGHT, TitleBarOptions, WindowCaption, title_bar_on};
        use egui::{
            Context, Event, Modifiers, PointerButton, RawInput, Rect, ViewportCommand, ViewportId,
            pos2, vec2,
        };
        use egui_mev_theme::{AccentShades, Theme, fluent};

        const AT_EMPTY_SPACE: egui::Pos2 = pos2(300.0, 20.0);

        struct Frame {
            commands: Vec<ViewportCommand>,
            texts: Vec<String>,
            contents_left: f32,
            clicked: bool,
            bar_height: f32,
        }

        /// One frame of a bar holding a button, with `events` at `time`.
        fn frame(ctx: &Context, caption: WindowCaption, time: f64, events: Vec<Event>) -> Frame {
            let input = RawInput {
                time: Some(time),
                events,
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0))),
                ..RawInput::default()
            };
            let (mut contents_left, mut clicked, mut bar_height) = (0.0, false, 0.0);
            let mut output = ctx.run_ui(input, |ui| {
                let options = TitleBarOptions::new("Window title");
                let bar = title_bar_on(ui, &options, caption, |ui| {
                    contents_left = ui.next_widget_position().x;
                    clicked = ui.button("Go").clicked();
                });
                bar_height = bar.response.rect.height();
            });
            output.textures_delta.clear();
            let texts = output
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .collect();
            Frame {
                commands: output
                    .viewport_output
                    .remove(&ViewportId::ROOT)
                    .map(|viewport| viewport.commands)
                    .unwrap_or_default(),
                texts,
                contents_left,
                clicked,
                bar_height,
            }
        }

        fn button(pressed: bool, at: egui::Pos2) -> Event {
            Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }
        }

        /// Press at `at`, then drag away; returns the commands the drag sent.
        fn drag_from(
            ctx: &Context,
            caption: WindowCaption,
            at: egui::Pos2,
        ) -> Vec<ViewportCommand> {
            frame(ctx, caption, 0.0, vec![Event::PointerMoved(at)]);
            frame(ctx, caption, 0.1, vec![button(true, at)]);
            let moved = at + vec2(40.0, 0.0);
            frame(ctx, caption, 0.8, vec![Event::PointerMoved(moved)]).commands
        }

        #[test]
        fn the_bar_has_the_toolbar_height() {
            let ctx = Context::default();
            let bar = frame(&ctx, WindowCaption::Unified, 0.0, vec![]);
            assert_eq!(bar.bar_height, TOOLBAR_HEIGHT);
        }

        #[test]
        fn a_unified_window_makes_room_for_the_traffic_lights_and_draws_the_title() {
            let ctx = Context::default();
            let bar = frame(&ctx, WindowCaption::Unified, 0.0, vec![]);
            assert_eq!(WindowCaption::Unified.leading_inset(), 76.0);
            assert!(bar.contents_left > 76.0, "{}", bar.contents_left);
            assert!(bar.texts.iter().any(|text| text == "Window title"));
        }

        #[test]
        fn an_empty_title_leaves_contents_at_the_inset() {
            let ctx = Context::default();
            let mut contents_left = 0.0;
            let mut output = ctx.run_ui(RawInput::default(), |ui| {
                title_bar_on(
                    ui,
                    &TitleBarOptions::default(),
                    WindowCaption::Unified,
                    |ui| {
                        contents_left = ui.next_widget_position().x;
                    },
                );
            });
            output.textures_delta.clear();
            assert_eq!(contents_left, 76.0);
        }

        #[test]
        fn a_native_caption_keeps_the_title_and_a_small_inset() {
            let ctx = Context::default();
            let bar = frame(&ctx, WindowCaption::Native, 0.0, vec![]);
            assert_eq!(bar.contents_left, 8.0);
            assert!(!bar.texts.iter().any(|text| text == "Window title"));
        }

        /// The look must not decide the inset: Fluent on a Mac is still a Mac window.
        #[test]
        fn the_family_does_not_change_the_platform_layout() {
            let graphite = Context::default();
            Theme::graphite(AccentShades::MACOS_DEFAULT).install(&graphite);
            let fluent = Context::default();
            fluent::theme(AccentShades::WINDOWS_DEFAULT).install(&fluent);
            for ctx in [&graphite, &fluent] {
                let bar = frame(ctx, WindowCaption::Unified, 0.0, vec![]);
                assert!(bar.contents_left > 76.0);
                assert!(bar.texts.iter().any(|text| text == "Window title"));
                let bar = frame(ctx, WindowCaption::Native, 0.0, vec![]);
                assert_eq!(bar.contents_left, 8.0);
                assert!(!bar.texts.iter().any(|text| text == "Window title"));
            }
        }

        #[test]
        fn empty_space_drags_a_unified_window() {
            let ctx = Context::default();
            let commands = drag_from(&ctx, WindowCaption::Unified, AT_EMPTY_SPACE);
            assert!(
                commands.contains(&ViewportCommand::StartDrag),
                "{commands:?}"
            );
        }

        #[test]
        fn a_native_caption_does_its_own_dragging() {
            let ctx = Context::default();
            let commands = drag_from(&ctx, WindowCaption::Native, AT_EMPTY_SPACE);
            assert!(
                !commands.contains(&ViewportCommand::StartDrag),
                "{commands:?}"
            );
        }

        #[test]
        fn below_the_bar_does_not_drag() {
            let ctx = Context::default();
            let commands = drag_from(&ctx, WindowCaption::Unified, pos2(300.0, 100.0));
            assert!(
                !commands.contains(&ViewportCommand::StartDrag),
                "{commands:?}"
            );
        }

        #[test]
        fn controls_keep_their_clicks_over_the_drag_region() {
            let ctx = Context::default();
            let caption = WindowCaption::Unified;
            // Lay out once to find the button: it starts where the contents do.
            let left = frame(&ctx, caption, 0.0, vec![]).contents_left;
            let at = pos2(left + 4.0, TOOLBAR_HEIGHT / 2.0);
            frame(&ctx, caption, 0.1, vec![Event::PointerMoved(at)]);
            frame(&ctx, caption, 0.2, vec![button(true, at)]);
            let released = frame(&ctx, caption, 0.3, vec![button(false, at)]);
            assert!(released.clicked);
            assert!(
                !released.commands.contains(&ViewportCommand::StartDrag),
                "{:?}",
                released.commands
            );
        }
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
