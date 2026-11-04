//! Helpers for `egui_kittest` tests, usable by any crate as a dev-dependency.
//!
//! ```ignore
//! let mut harness = Harness::builder()
//!     .with_size([400.0, 300.0])
//!     .with_context_setup(|ctx| egui_mev_test::install(ctx, Theme::default()))
//!     .build_ui(|ui| my_widget(ui));
//! egui_mev_test::settle(&mut harness);
//! let _guard = egui_mev_test::snapshot_lock();
//! harness.snapshot_options("my_widget", &egui_mev_test::snapshot_options());
//! ```

mod golden;
mod walk;

use std::sync::{Mutex, MutexGuard, PoisonError};

use egui::{Context, Event, Modifiers, PointerButton, Pos2};
use egui_kittest::{Harness, SnapshotOptions};
use egui_mev_theme::{FontOptions, Theme, fonts};

pub use golden::{Golden, hash_rgba, render_hash};
pub use walk::Walk;

/// A starting `kittest.toml`, with per-platform thresholds. Copy it to a
/// project's root, next to `Cargo.lock`, which is where kittest looks for it.
pub const KITTEST_TOML_TEMPLATE: &str = include_str!("../templates/kittest.toml");

/// Snapshot options for this platform. Windows renders text differently from
/// everything else, so its images live in their own folder.
#[must_use]
pub fn snapshot_options() -> SnapshotOptions {
    let options = SnapshotOptions::new();
    if cfg!(windows) {
        options.output_path("tests/snapshots/windows")
    } else {
        options
    }
}

static SNAPSHOT_LOCK: Mutex<()> = Mutex::new(());

/// Hold this while a test takes snapshots, so tests that compare images in one
/// process don't compete for the GPU. A failed snapshot panics with the lock
/// held, which must not fail every later test, so poison is ignored.
pub fn snapshot_lock() -> MutexGuard<'static, ()> {
    SNAPSHOT_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Install `theme` and machine-independent fonts on `ctx`.
pub fn install(ctx: &Context, theme: Theme) {
    install_with(ctx, theme, &FontOptions::default());
}

/// [`install`], with icon fonts or a bundled font from `options`.
pub fn install_with(ctx: &Context, theme: Theme, options: &FontOptions) {
    theme.install(ctx);
    fonts::install_test(ctx, options);
}

/// Run a few frames, for UIs with an animation that never fully settles.
pub fn settle<S>(harness: &mut Harness<'_, S>) {
    harness.run_steps(3);
}

/// Drag with `button` from `from` to `to` in screen points, in four moves.
pub fn drag<S>(
    harness: &mut Harness<'_, S>,
    from: Pos2,
    to: Pos2,
    button: PointerButton,
    modifiers: Modifiers,
) {
    harness.event(Event::ModifiersChanged(modifiers));
    harness.event(Event::PointerMoved(from));
    settle(harness);
    harness.event(Event::PointerButton {
        pos: from,
        button,
        pressed: true,
        modifiers,
    });
    settle(harness);
    for t in [0.25, 0.5, 0.75, 1.0] {
        harness.event(Event::PointerMoved(from + (to - from) * t));
        settle(harness);
    }
    harness.event(Event::PointerButton {
        pos: to,
        button,
        pressed: false,
        modifiers,
    });
    settle(harness);
    harness.event(Event::ModifiersChanged(Modifiers::NONE));
    settle(harness);
}

/// Step frames until `done` holds, for background work that finishes on its own.
///
/// # Panics
/// When it hasn't held after 400 frames.
pub fn wait_until<S>(harness: &mut Harness<'_, S>, done: impl Fn(&S) -> bool) {
    wait_up_to(harness, 400, done);
}

/// [`wait_until`] with a chosen number of frames, for work that takes seconds
/// on a software GPU in CI.
///
/// # Panics
/// When `done` hasn't held after `frames` frames.
pub fn wait_up_to<S>(harness: &mut Harness<'_, S>, frames: u32, done: impl Fn(&S) -> bool) {
    for _ in 0..frames {
        harness.step();
        if done(harness.state()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("condition not reached after {frames} frames");
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable as _;
    use egui_mev_theme::{Palette, presets};

    use super::*;

    #[test]
    fn install_sets_the_theme_on_that_context_only() {
        let (a, b) = (Context::default(), Context::default());
        install(&a, Theme::dark_only(presets::SLATE_DARK));
        install(&b, Theme::dark_only(presets::GRAPHITE_DARK));
        assert_eq!(Palette::of_ctx(&a), presets::SLATE_DARK);
        assert_eq!(Palette::of_ctx(&b), presets::GRAPHITE_DARK);
    }

    #[test]
    fn install_works_as_a_harness_setup() {
        let mut harness = Harness::builder()
            .with_context_setup(|ctx| install(ctx, Theme::dark_only(presets::SLATE_DARK)))
            .build_ui(|ui| {
                assert_eq!(Palette::of(ui), presets::SLATE_DARK);
                ui.label("text");
            });
        harness.run();
        assert!(harness.query_by_label("text").is_some());
    }

    #[test]
    fn wait_until_stops_once_the_state_says_so() {
        let mut harness = Harness::builder().build_ui_state(|_, count: &mut u32| *count += 1, 0);
        wait_until(&mut harness, |count| *count >= 5);
        assert!(*harness.state() >= 5);
    }

    #[test]
    #[should_panic(expected = "condition not reached")]
    fn wait_up_to_gives_up() {
        let mut harness = Harness::builder().build_ui_state(|_, (): &mut ()| {}, ());
        wait_up_to(&mut harness, 2, |()| false);
    }

    #[test]
    fn drag_moves_a_dragged_value() {
        let mut harness = Harness::builder().with_size([200.0, 100.0]).build_ui_state(
            |ui, total: &mut egui::Vec2| {
                let response = ui.allocate_response(ui.available_size(), egui::Sense::drag());
                *total += response.drag_delta();
            },
            egui::Vec2::ZERO,
        );
        harness.run();
        drag(
            &mut harness,
            egui::pos2(20.0, 20.0),
            egui::pos2(60.0, 40.0),
            PointerButton::Primary,
            Modifiers::NONE,
        );
        let total = *harness.state();
        assert!(total.x > 0.0 && total.y > 0.0, "{total:?}");
    }

    #[test]
    fn the_lock_survives_a_panicking_holder() {
        let _ = std::thread::spawn(|| {
            let _guard = snapshot_lock();
            panic!("expected: a test failing while it holds the lock");
        })
        .join();
        drop(snapshot_lock());
    }

    #[test]
    fn the_template_parses_as_kittest_keys() {
        assert!(KITTEST_TOML_TEMPLATE.contains("max_failed_pixels"));
        assert!(KITTEST_TOML_TEMPLATE.contains("[mac]"));
    }
}
