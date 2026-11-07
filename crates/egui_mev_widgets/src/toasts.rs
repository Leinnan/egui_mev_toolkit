//! Toasts: short messages stacked in a corner. Confirmations fade out, warnings
//! stay longer, errors stay until clicked away. Hovering a toast keeps it; long
//! text wraps. The state lives in the [`Context`], so anything that can reach the
//! context can push one, including a worker thread holding a clone of it.
//!
//! ```ignore
//! Toasts::push(ctx, "Saved", ToastKind::Ok);
//! // Once per frame, after the panels:
//! Toasts::show(ctx);
//! ```
//!
//! A toast can carry one button, such as Undo, for the moment after an action
//! when undoing it is most likely:
//!
//! ```ignore
//! Toasts::push_action(ctx, "Applied Kind of Blue", ToastKind::Ok, "Undo", undo_id);
//! // Each frame:
//! if Toasts::acted(ctx, undo_id) { undo() }
//! ```

use egui::{
    Align2, Context, CornerRadius, FontId, Id, Rect, Role, Sense, Stroke, StrokeKind, Vec2,
    WidgetInfo, vec2,
};
use egui_mev_theme::{Metrics, Palette};

use crate::containers::BannerKind;
use crate::marks;

/// Seconds a confirmation stays fully visible, then fades over [`FADE`].
const HOLD: f64 = 1.8;
/// Seconds a warning stays fully visible.
const WARN_HOLD: f64 = 6.0;
/// Seconds at least a toast with a button stays, to give time to press it.
const ACTION_HOLD: f64 = 8.0;
const FADE: f64 = 0.5;
/// Most toasts kept at once (the oldest go first).
const MAX_VISIBLE: usize = 4;
/// Widest a toast grows before its text wraps (points).
const MAX_WIDTH: f32 = 360.0;
/// Space between the window edge or the next toast and a toast (points).
const GAP: f32 = 8.0;
const EDGE: f32 = 14.0;
/// Size of a toast's severity icon, and its gap to the text (points).
const ICON: f32 = 14.0;
const ICON_GAP: f32 = 8.0;
/// Space either side of a toast button's label (points).
const ACTION_PAD: f32 = 6.0;

/// What a toast says, which sets how it looks and how long it stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// A neutral note; fades by itself.
    Info,
    /// Something worked; fades by itself.
    Ok,
    /// Something is off but nothing failed; stays longer, then fades.
    Warn,
    /// Something failed; stays until clicked.
    Error,
}

impl ToastKind {
    /// Seconds this kind stays fully visible; `None` until dismissed.
    const fn hold(self) -> Option<f64> {
        match self {
            Self::Info | Self::Ok => Some(HOLD),
            Self::Warn => Some(WARN_HOLD),
            Self::Error => None,
        }
    }

    const fn banner(self) -> BannerKind {
        match self {
            Self::Info => BannerKind::Info,
            Self::Ok => BannerKind::Success,
            Self::Warn => BannerKind::Warning,
            Self::Error => BannerKind::Error,
        }
    }
}

#[derive(Debug, Clone)]
struct Toast {
    text: String,
    kind: ToastKind,
    /// `None` until first drawn, so a hidden window doesn't use up its time.
    shown_at: Option<f64>,
    /// A button's label, and what [`Toasts::acted`] knows it by.
    action: Option<(String, Id)>,
}

impl Toast {
    /// Seconds it stays fully visible; `None` until dismissed.
    fn hold(&self) -> Option<f64> {
        let hold = self.kind.hold()?;
        Some(if self.action.is_some() {
            hold.max(ACTION_HOLD)
        } else {
            hold
        })
    }
}

/// The live toasts of a [`Context`].
#[derive(Debug, Clone, Default)]
pub struct Toasts {
    toasts: Vec<Toast>,
    /// Buttons pressed and not yet asked about.
    acted: Vec<Id>,
}

fn state_id() -> Id {
    Id::unique("egui_mev_widgets::toasts")
}

impl Toasts {
    /// Queue a toast. The same message again refreshes it instead of stacking.
    pub fn push(ctx: &Context, text: impl Into<String>, kind: ToastKind) {
        let text = text.into();
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<Self>(state_id())
                .add(text, kind, None);
        });
        ctx.request_repaint();
    }

    /// Queue a toast with a button labelled `action`, such as "Undo", that
    /// stays at least eight seconds. [`Self::acted`] with `key` says when the
    /// button was pressed; pressing it also dismisses the toast.
    pub fn push_action(
        ctx: &Context,
        text: impl Into<String>,
        kind: ToastKind,
        action: impl Into<String>,
        key: Id,
    ) {
        let text = text.into();
        let action = Some((action.into(), key));
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<Self>(state_id())
                .add(text, kind, action);
        });
        ctx.request_repaint();
    }

    /// Whether the button of the toast pushed with `key` was pressed since the
    /// last ask. Ask once per frame.
    #[must_use]
    pub fn acted(ctx: &Context, key: Id) -> bool {
        ctx.data_mut(|data| {
            let state = data.get_temp_mut_or_default::<Self>(state_id());
            let before = state.acted.len();
            state.acted.retain(|k| *k != key);
            state.acted.len() != before
        })
    }

    /// Queue a [`ToastKind::Info`] toast.
    pub fn info(ctx: &Context, text: impl Into<String>) {
        Self::push(ctx, text, ToastKind::Info);
    }

    /// Queue a [`ToastKind::Ok`] toast.
    pub fn ok(ctx: &Context, text: impl Into<String>) {
        Self::push(ctx, text, ToastKind::Ok);
    }

    /// Queue a [`ToastKind::Warn`] toast.
    pub fn warn(ctx: &Context, text: impl Into<String>) {
        Self::push(ctx, text, ToastKind::Warn);
    }

    /// Queue a [`ToastKind::Error`] toast.
    pub fn error(ctx: &Context, text: impl Into<String>) {
        Self::push(ctx, text, ToastKind::Error);
    }

    /// The live toasts, oldest first.
    #[must_use]
    pub fn live(ctx: &Context) -> Vec<(ToastKind, String)> {
        Self::load(ctx)
            .toasts
            .into_iter()
            .map(|toast| (toast.kind, toast.text))
            .collect()
    }

    /// Drop every toast.
    pub fn clear(ctx: &Context) {
        ctx.data_mut(|data| data.remove::<Self>(state_id()));
    }

    /// Draw the live toasts in the bottom-right corner of the window.
    pub fn show(ctx: &Context) {
        Self::show_in(ctx, ctx.content_rect());
    }

    /// Draw the live toasts in the bottom-right corner of `area`, such as the
    /// space above a status bar.
    pub fn show_in(ctx: &Context, area: Rect) {
        let mut state = Self::load(ctx);
        state.draw(ctx, area);
        ctx.data_mut(|data| data.insert_temp(state_id(), state));
    }

    fn load(ctx: &Context) -> Self {
        ctx.data(|data| data.get_temp::<Self>(state_id()))
            .unwrap_or_default()
    }

    fn add(&mut self, text: String, kind: ToastKind, action: Option<(String, Id)>) {
        self.toasts.retain(|toast| toast.text != text);
        self.toasts.push(Toast {
            text,
            kind,
            shown_at: None,
            action,
        });
        if self.toasts.len() > MAX_VISIBLE {
            self.toasts.drain(..self.toasts.len() - MAX_VISIBLE);
        }
    }

    /// Start the timers of toasts drawn for the first time, and drop those that
    /// have finished fading.
    fn expire(&mut self, now: f64) {
        for toast in &mut self.toasts {
            toast.shown_at.get_or_insert(now);
        }
        self.toasts.retain(|toast| {
            toast.hold().is_none_or(|hold| {
                toast
                    .shown_at
                    .is_none_or(|shown_at| now - shown_at < hold + FADE)
            })
        });
    }

    fn draw(&mut self, ctx: &Context, area: Rect) {
        let p = Palette::of_ctx(ctx);
        let m = Metrics::of_ctx(ctx);
        let id = state_id();
        let now = ctx.input(|i| i.time);
        self.expire(now);

        let mut bottom = area.bottom() - EDGE;
        let mut dismissed = Vec::new();
        for (ix, toast) in self.toasts.iter_mut().enumerate().rev() {
            let sticky = toast.hold().is_none();
            let age = now - toast.shown_at.unwrap_or(now);
            let alpha = toast.hold().map_or(1.0, |hold| {
                if age < hold {
                    1.0
                } else {
                    (1.0 - (age - hold) / FADE).clamp(0.0, 1.0) as f32
                }
            });
            let text_color = p.text.gamma_multiply(alpha);
            let galley = ctx.fonts_mut(|fonts| {
                fonts.layout(
                    toast.text.clone(),
                    FontId::proportional(m.body),
                    text_color,
                    MAX_WIDTH,
                )
            });
            let close_space = if sticky { 22.0 } else { 0.0 };
            let action_galley = toast.action.as_ref().map(|(label, _)| {
                ctx.fonts_mut(|fonts| {
                    fonts.layout_no_wrap(
                        label.clone(),
                        FontId::proportional(m.body),
                        p.accent.gamma_multiply(alpha),
                    )
                })
            });
            let action_space = action_galley
                .as_ref()
                .map_or(0.0, |g| g.size().x + 2.0 * ACTION_PAD + 8.0);
            let size =
                galley.size() + vec2(28.0 + ICON + ICON_GAP + close_space + action_space, 16.0);
            let rect = Rect::from_min_size(
                egui::pos2(area.right() - EDGE - size.x, bottom - size.y),
                size,
            );
            let (_, foreground) = toast.kind.banner().colors(&p);
            let border = if toast.kind == ToastKind::Info {
                p.border
            } else {
                foreground
            };
            let response = egui::Area::new(id.with(("toast", &toast.text)))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.min)
                // Constraining uses the size guessed before the first frame, which
                // would put the area up and left of the toast. Content allocated
                // down at `rect` then stretches it over everything between, where
                // it would swallow clicks meant for the UI underneath.
                .constrain(false)
                .show(ctx, |ui| {
                    let response = ui.allocate_rect(rect, Sense::click());
                    let painter = ui.painter();
                    painter.rect(
                        rect,
                        CornerRadius::same(m.card_radius),
                        p.card_bg.gamma_multiply(alpha),
                        Stroke::new(1.0, border.gamma_multiply(alpha)),
                        StrokeKind::Inside,
                    );
                    let icon_rect = Rect::from_center_size(
                        rect.left_center() + vec2(14.0 + ICON / 2.0, 0.0),
                        Vec2::splat(ICON),
                    );
                    marks::severity(
                        painter,
                        icon_rect,
                        toast.kind.banner(),
                        foreground.gamma_multiply(alpha),
                    );
                    let text_min = Align2::LEFT_CENTER
                        .anchor_size(
                            rect.left_center() + vec2(14.0 + ICON + ICON_GAP, 0.0),
                            galley.size(),
                        )
                        .min;
                    painter.galley(text_min, galley, text_color);
                    let mut acted = false;
                    if let (Some(action_galley), Some((label, _))) = (action_galley, &toast.action)
                    {
                        let button = Rect::from_center_size(
                            rect.right_center()
                                - vec2(
                                    close_space + 14.0 + action_galley.size().x / 2.0 + ACTION_PAD,
                                    0.0,
                                ),
                            action_galley.size() + vec2(2.0 * ACTION_PAD, 6.0),
                        );
                        let pressed =
                            ui.interact(button, id.with(("action", &toast.text)), Sense::click());
                        if pressed.hovered() {
                            painter.rect_filled(
                                button,
                                CornerRadius::same(m.control_radius),
                                p.accent.gamma_multiply(0.15 * alpha),
                            );
                        }
                        painter.galley(
                            button.center() - action_galley.size() / 2.0,
                            action_galley,
                            p.accent,
                        );
                        pressed.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
                        acted = pressed.clicked();
                    }
                    if sticky {
                        marks::cross(
                            painter,
                            rect.right_center() - vec2(14.0, 0.0),
                            8.0,
                            if response.hovered() {
                                p.text
                            } else {
                                p.text_weak
                            },
                        );
                    }
                    (response, acted)
                })
                .inner;
            let (response, acted) = response;
            // Hovering holds a fading toast; clicking any toast dismisses it.
            if (response.hovered() || response.contains_pointer()) && !sticky {
                toast.shown_at = Some(now);
            }
            if acted {
                if let Some((_, key)) = &toast.action {
                    self.acted.push(*key);
                }
                dismissed.push(ix);
            } else if response.clicked() {
                dismissed.push(ix);
            }
            let response = if sticky {
                response.on_hover_text("Click to dismiss")
            } else {
                response
            };
            response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &toast.text));
            bottom = rect.top() - GAP;
        }
        // Newest-first iteration, so these indices are already descending.
        for ix in dismissed {
            self.toasts.remove(ix);
        }

        // Idle until the next toast starts fading, then animate.
        let fade_in = self
            .toasts
            .iter()
            .filter_map(|toast| {
                let hold = toast.hold()?;
                Some(hold - (now - toast.shown_at.unwrap_or(now)))
            })
            .fold(f64::INFINITY, f64::min);
        if fade_in.is_finite() {
            if fade_in > 0.0 {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(fade_in));
            } else {
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::{Event, Pos2, pos2};
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::presets;

    use super::*;
    use crate::test_support::{frame_at, painted_colors, themed};

    fn texts(toasts: &Toasts) -> Vec<&str> {
        toasts.toasts.iter().map(|t| t.text.as_str()).collect()
    }

    /// `(kind, text)` of the toasts on `ctx`, for short asserts.
    fn live(ctx: &Context) -> Vec<String> {
        Toasts::live(ctx)
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    /// Draw one frame of toasts at `time`, with the pointer at `pointer`.
    fn draw_at(ctx: &Context, time: f64, pointer: Option<Pos2>) {
        let events = pointer.map(Event::PointerMoved).into_iter().collect();
        frame_at(ctx, time, events, |ui| Toasts::show(ui.ctx()));
    }

    #[test]
    fn fading_kinds_expire_after_their_time_but_errors_stay() {
        let mut toasts = Toasts::default();
        toasts.add("note".to_owned(), ToastKind::Info, None);
        toasts.add("saved".to_owned(), ToastKind::Ok, None);
        toasts.add("careful".to_owned(), ToastKind::Warn, None);
        toasts.add("failed".to_owned(), ToastKind::Error, None);

        toasts.expire(10.0);
        assert_eq!(texts(&toasts), ["note", "saved", "careful", "failed"]);

        toasts.expire(10.0 + HOLD + FADE - 0.01);
        assert_eq!(texts(&toasts), ["note", "saved", "careful", "failed"]);

        toasts.expire(10.0 + HOLD + FADE);
        assert_eq!(texts(&toasts), ["careful", "failed"]);

        toasts.expire(10.0 + WARN_HOLD + FADE - 0.01);
        assert_eq!(texts(&toasts), ["careful", "failed"]);

        toasts.expire(10.0 + WARN_HOLD + FADE);
        assert_eq!(texts(&toasts), ["failed"]);

        toasts.expire(1.0e9);
        assert_eq!(texts(&toasts), ["failed"]);
    }

    #[test]
    fn repeated_messages_refresh_instead_of_stacking() {
        let mut toasts = Toasts::default();
        toasts.add("first".to_owned(), ToastKind::Info, None);
        toasts.add("second".to_owned(), ToastKind::Info, None);
        toasts.add("first".to_owned(), ToastKind::Ok, None);

        assert_eq!(texts(&toasts), ["second", "first"]);
        assert_eq!(toasts.toasts[1].kind, ToastKind::Ok);
    }

    #[test]
    fn a_repeat_restarts_the_timer() {
        let ctx = themed(egui::Theme::Dark);
        Toasts::ok(&ctx, "saved");
        draw_at(&ctx, 10.0, None);
        draw_at(&ctx, 10.0 + HOLD, None);
        Toasts::ok(&ctx, "saved");
        // Past the first timer's end, but the repeat started a new one when drawn.
        draw_at(&ctx, 10.0 + HOLD + FADE, None);
        assert_eq!(live(&ctx), ["saved"]);
        draw_at(&ctx, 10.0 + HOLD + FADE + HOLD + FADE, None);
        assert!(live(&ctx).is_empty());
    }

    #[test]
    fn only_the_newest_toasts_are_kept() {
        let mut toasts = Toasts::default();
        for n in 0..MAX_VISIBLE + 2 {
            toasts.add(format!("toast {n}"), ToastKind::Error, None);
        }

        assert_eq!(toasts.toasts.len(), MAX_VISIBLE);
        assert_eq!(toasts.toasts[0].text, "toast 2");
    }

    #[test]
    fn the_timer_starts_on_the_first_draw_not_on_push() {
        let ctx = themed(egui::Theme::Dark);
        Toasts::ok(&ctx, "saved");
        // A hidden window: frames pass, nothing draws the toasts.
        frame_at(&ctx, 1000.0, Vec::new(), |_| {});
        assert_eq!(live(&ctx), ["saved"]);

        // Long after the push, the first draw starts the whole time span.
        draw_at(&ctx, 2000.0, None);
        assert_eq!(live(&ctx), ["saved"]);
        draw_at(&ctx, 2000.0 + HOLD + FADE - 0.01, None);
        assert_eq!(live(&ctx), ["saved"]);
        draw_at(&ctx, 2000.0 + HOLD + FADE + 0.01, None);
        assert!(live(&ctx).is_empty());
    }

    #[test]
    fn hovering_keeps_a_toast_and_leaving_lets_it_go() {
        let ctx = themed(egui::Theme::Dark);
        Toasts::ok(&ctx, "saved");
        // Bottom-right corner, where the toast is.
        let over = pos2(800.0 - EDGE - 10.0, 600.0 - EDGE - 10.0);
        let away = pos2(10.0, 10.0);
        draw_at(&ctx, 0.0, Some(over));
        draw_at(&ctx, 0.1, Some(over));
        // Far past its time, but each hovered frame refreshes it.
        for step in 1..=6 {
            draw_at(&ctx, f64::from(step), Some(over));
            assert_eq!(live(&ctx), ["saved"], "at {step}");
        }
        draw_at(&ctx, 6.1, Some(away));
        assert_eq!(live(&ctx), ["saved"]);
        draw_at(&ctx, 6.0 + HOLD + FADE + 0.2, Some(away));
        assert!(live(&ctx).is_empty());
    }

    #[test]
    fn a_toast_only_takes_the_pointer_over_itself() {
        let ctx = themed(egui::Theme::Dark);
        Toasts::error(&ctx, "failed");
        for frame in 0..3 {
            draw_at(&ctx, f64::from(frame), None);
        }
        let over = pos2(800.0 - EDGE - 10.0, 600.0 - EDGE - 10.0);
        let above = pos2(800.0 - EDGE - 10.0, 600.0 - EDGE - 100.0);
        let order_at = |pos| ctx.layer_id_at(pos).map(|layer| layer.order);
        assert_eq!(order_at(over), Some(egui::Order::Foreground));
        assert_ne!(order_at(above), Some(egui::Order::Foreground));
    }

    #[test]
    fn pushing_from_another_thread_reaches_the_context() {
        let ctx = themed(egui::Theme::Dark);
        let worker = ctx.clone();
        std::thread::spawn(move || Toasts::warn(&worker, "from a worker"))
            .join()
            .unwrap();
        assert_eq!(
            Toasts::live(&ctx),
            [(ToastKind::Warn, "from a worker".to_owned())]
        );
        assert!(ctx.has_requested_repaint());
    }

    #[test]
    fn each_context_has_its_own_toasts() {
        let (a, b) = (Context::default(), Context::default());
        Toasts::info(&a, "only here");
        assert_eq!(live(&a), ["only here"]);
        assert!(live(&b).is_empty());
        Toasts::clear(&a);
        assert!(live(&a).is_empty());
    }

    #[test]
    fn clicking_an_error_toast_dismisses_it() {
        let mut harness = Harness::builder().with_size([600.0, 300.0]).build_ui(|ui| {
            Toasts::show(ui.ctx());
        });
        Toasts::error(&harness.ctx, "Failed to save profile");
        harness.run();

        harness.get_by_label("Failed to save profile").click();
        harness.run();

        assert!(harness.query_by_label("Failed to save profile").is_none());
        assert!(Toasts::live(&harness.ctx).is_empty());
    }

    #[test]
    fn toasts_draw_in_the_palette_of_the_current_mode() {
        for (mode, palette) in [
            (egui::Theme::Dark, presets::GRAPHITE_DARK),
            (egui::Theme::Light, presets::GRAPHITE_LIGHT),
        ] {
            let ctx = themed(mode);
            Toasts::error(&ctx, "failed");
            Toasts::ok(&ctx, "saved");
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 300.0))),
                ..Default::default()
            };
            // An area shows from its second frame, and then fades in.
            let mut output = ctx.run_ui(input.clone(), |ui| Toasts::show(ui.ctx()));
            for _ in 0..30 {
                output.textures_delta.clear();
                output = ctx.run_ui(input.clone(), |ui| Toasts::show(ui.ctx()));
            }
            output.textures_delta.clear();
            let colors = painted_colors(&output);
            assert!(colors.contains(&palette.card_bg), "{mode:?}");
            assert!(colors.contains(&palette.critical), "{mode:?}");
            assert!(colors.contains(&palette.success), "{mode:?}");
        }
    }

    #[test]
    fn a_toast_with_a_button_stays_long_enough_to_press_it() {
        let mut toasts = Toasts::default();
        let key = Id::unique("undo");
        toasts.add("saved".to_owned(), ToastKind::Ok, None);
        toasts.add(
            "applied".to_owned(),
            ToastKind::Ok,
            Some(("Undo".to_owned(), key)),
        );
        toasts.expire(10.0);
        toasts.expire(10.0 + HOLD + FADE);
        assert_eq!(texts(&toasts), ["applied"]);
        toasts.expire(10.0 + ACTION_HOLD + FADE);
        assert!(toasts.toasts.is_empty());
    }

    #[test]
    fn pressing_a_toast_button_is_reported_once_and_dismisses_it() {
        let key = Id::unique("undo");
        let mut harness = Harness::builder().with_size([600.0, 300.0]).build_ui_state(
            |ui, acted: &mut u32| {
                if Toasts::acted(ui.ctx(), key) {
                    *acted += 1;
                }
                Toasts::show(ui.ctx());
            },
            0,
        );
        Toasts::push_action(
            &harness.ctx,
            "Applied Kind of Blue",
            ToastKind::Ok,
            "Undo",
            key,
        );
        harness.run();
        harness.get_by_label("Undo").click();
        harness.run();
        assert_eq!(*harness.state(), 1);
        assert!(Toasts::live(&harness.ctx).is_empty(), "dismissed");
        harness.run();
        assert_eq!(*harness.state(), 1, "reported once");
    }
}
