//! Startup stage timing: one wall-clock base at process start, `startup: <stage>
//! +N ms` log lines along the launch path, and one line when the first UI frame
//! has begun. It logs nothing unless the env var the app names is set, and a
//! GPU readback is measured only when requested.
//!
//! ```ignore
//! fn main() {
//!     startup::begin("MY_APP_STARTUP_TIMING");
//!     // ...
//!     startup::stage("config loaded");
//! }
//! ```

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

use web_time::Instant;

static TIMER: OnceLock<StartupTimer> = OnceLock::new();

/// Record the base time (the very first thing `main` does), and turn the rest
/// of this module on if `env_var` is set. Only the first call counts.
pub fn begin(env_var: &str) {
    let _ = TIMER.get_or_init(|| StartupTimer::new(std::env::var_os(env_var).is_some()));
}

/// Log when one named stage finished.
pub fn stage(name: &str) {
    if let Some(line) = TIMER.get().and_then(|timer| timer.stage_line(name)) {
        log::info!("{line}");
    }
}

/// Log entry into the first UI frame once; later calls do nothing.
pub fn first_frame() {
    if let Some(line) = TIMER.get().and_then(StartupTimer::first_frame_line) {
        log::info!("{line}");
    }
}

/// Whether [`begin`] ran and found its env var set.
pub fn enabled() -> bool {
    TIMER.get().is_some_and(|timer| timer.enabled)
}

/// Request a single GPU readback as a measurable completed-frame boundary.
pub fn request_readback(ctx: &egui::Context) {
    if TIMER.get().is_some_and(StartupTimer::claim_readback) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
            egui::ScreenshotCallback::new(|_| stage("first frame readback")),
        ));
    }
}

/// The state behind the free functions, separate so it can be tested without
/// the process-wide instance.
struct StartupTimer {
    start: Instant,
    enabled: bool,
    first_frame: AtomicBool,
    readback: AtomicBool,
}

impl StartupTimer {
    fn new(enabled: bool) -> Self {
        Self {
            start: Instant::now(),
            enabled,
            first_frame: AtomicBool::new(false),
            readback: AtomicBool::new(false),
        }
    }

    fn stage_line(&self, name: &str) -> Option<String> {
        self.enabled
            .then(|| format!("startup: {name} +{} ms", self.start.elapsed().as_millis()))
    }

    fn first_frame_line(&self) -> Option<String> {
        (self.enabled && !self.first_frame.swap(true, Ordering::Relaxed)).then(|| {
            format!(
                "startup: first UI entry +{} ms",
                self.start.elapsed().as_millis()
            )
        })
    }

    fn claim_readback(&self) -> bool {
        self.enabled && !self.readback.swap(true, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::StartupTimer;

    #[test]
    fn with_the_gate_off_nothing_is_produced() {
        let timer = StartupTimer::new(false);
        assert_eq!(timer.stage_line("config"), None);
        assert_eq!(timer.first_frame_line(), None);
        assert!(!timer.claim_readback());
    }

    #[test]
    fn with_the_gate_on_stages_are_named() {
        let timer = StartupTimer::new(true);
        let line = timer.stage_line("config").expect("should produce a line");
        assert!(line.starts_with("startup: config +"), "{line}");
        assert!(line.ends_with(" ms"), "{line}");
    }

    #[test]
    fn first_frame_and_readback_happen_once() {
        let timer = StartupTimer::new(true);
        assert!(timer.first_frame_line().is_some());
        assert_eq!(timer.first_frame_line(), None);
        assert!(timer.claim_readback());
        assert!(!timer.claim_readback());
    }

    /// The only test that touches the process-wide timer.
    #[test]
    fn begin_with_an_unset_variable_leaves_the_module_off() {
        super::begin("EGUI_MEV_APP_TEST_STARTUP_TIMING_NEVER_SET");
        assert!(!super::enabled());
        // No logger is asserted on: these only have to return without effect.
        super::stage("config");
        super::first_frame();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            super::request_readback(ui.ctx());
        });
        output.textures_delta.clear();
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Screenshot(_))),
            "{commands:?}"
        );
    }
}
