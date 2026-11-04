//! A ring buffer of recent log lines, a [`log::Log`] that fills it, and an egui
//! viewer.
//!
//! ```ignore
//! let log = LogCapture::new(1000).install()?;
//! // Each frame, with `log_open` owned by the app:
//! if let Some(lines) = log_modal(ctx, &log, &LogModalOptions::default(), &mut log_open) {
//!     status = format!("copied {lines} log lines");
//! }
//! ```

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use egui::{
    Align, Align2, Button, Context, CornerRadius, Frame, Id, Label, Layout, Margin, Modal, Order,
    RichText, ScrollArea, Stroke, Ui, Vec2, vec2,
};
use egui_mev_theme::Palette;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

/// The most recent log lines, oldest first. Clones share one buffer, so the
/// logger and the UI can each hold one.
#[derive(Clone, Debug)]
pub struct LogBuffer {
    inner: Arc<Mutex<Lines>>,
}

#[derive(Debug)]
struct Lines {
    lines: VecDeque<String>,
    capacity: usize,
}

impl LogBuffer {
    /// An empty buffer that keeps the newest `capacity` lines (at least one).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            inner: Arc::new(Mutex::new(Lines {
                lines: VecDeque::new(),
                capacity,
            })),
        }
    }

    /// Add a line, dropping the oldest once the buffer is full. Apps can add
    /// their own events here, next to what the logger captures.
    pub fn push(&self, line: impl Into<String>) {
        let mut inner = self.lock();
        if inner.lines.len() == inner.capacity {
            inner.lines.pop_front();
        }
        inner.lines.push_back(line.into());
    }

    /// How many lines are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().lines.len()
    }

    /// Whether no lines are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().lines.is_empty()
    }

    /// The viewer's body: the newest `limit` lines, oldest first, one per line,
    /// ready for one selectable label.
    #[must_use]
    pub fn tail(&self, limit: usize) -> String {
        let inner = self.lock();
        let start = inner.lines.len().saturating_sub(limit);
        join(inner.lines.iter().skip(start))
    }

    /// Every line, oldest first, one per line.
    #[must_use]
    pub fn all(&self) -> String {
        join(self.lock().lines.iter())
    }

    fn lock(&self) -> MutexGuard<'_, Lines> {
        // A panic elsewhere can't leave the queue half-updated in a way that
        // matters, and losing the log right after a panic would be the worst time.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn join<'a>(lines: impl Iterator<Item = &'a String>) -> String {
    let mut out = String::new();
    for (index, line) in lines.enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

/// A [`log::Log`] that writes each record to a [`LogBuffer`], and optionally on
/// to another logger so the app keeps its stderr or file output.
pub struct LogCapture {
    buffer: LogBuffer,
    level: LevelFilter,
    forward: Option<Box<dyn Log>>,
}

impl LogCapture {
    /// A capture keeping `capacity` lines, at `Info` and more severe.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: LogBuffer::new(capacity),
            level: LevelFilter::Info,
            forward: None,
        }
    }

    /// Capture at `level` and more severe. A forwarded logger sees the same
    /// records, so set the most verbose level either side needs.
    #[must_use]
    pub fn with_level(mut self, level: LevelFilter) -> Self {
        self.level = level;
        self
    }

    /// Pass each record on to `logger` as well.
    #[must_use]
    pub fn forward_to(mut self, logger: Box<dyn Log>) -> Self {
        self.forward = Some(logger);
        self
    }

    /// The buffer this capture fills.
    #[must_use]
    pub fn buffer(&self) -> LogBuffer {
        self.buffer.clone()
    }

    /// Make this the process's logger and return its buffer. It fails if a
    /// logger is already installed.
    pub fn install(self) -> Result<LogBuffer, SetLoggerError> {
        let buffer = self.buffer();
        let level = self.level;
        log::set_boxed_logger(Box::new(self))?;
        log::set_max_level(level);
        Ok(buffer)
    }
}

impl Log for LogCapture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        self.buffer.push(format!(
            "{:<5} {}: {}",
            record.level(),
            record.target(),
            record.args()
        ));
        if let Some(forward) = &self.forward {
            forward.log(record);
        }
    }

    fn flush(&self) {
        if let Some(forward) = &self.forward {
            forward.flush();
        }
    }
}

/// Layout of [`log_modal`]. The defaults suit a window with a status bar along
/// its bottom edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogModalOptions {
    /// Minimum width of the modal's content.
    pub min_width: f32,
    /// Maximum height of the scrolling log.
    pub max_height: f32,
    /// How many lines to show. The body is one text run, so a long session
    /// would otherwise lay out one giant galley every frame.
    pub view_limit: usize,
    /// Offset of the modal from the window's bottom-right corner.
    pub anchor_offset: Vec2,
}

impl Default for LogModalOptions {
    fn default() -> Self {
        Self {
            min_width: 480.0,
            max_height: 260.0,
            view_limit: 100,
            anchor_offset: vec2(-8.0, -38.0),
        }
    }
}

/// The log as one selectable text run, so drag-selection and copying span
/// lines (per-line labels can't do that). It behaves like a terminal: it opens
/// on the newest line and stays there as lines arrive, but scrolling up is never
/// fought. New lines appear at the next repaint.
pub fn log_view(ui: &mut Ui, buffer: &LogBuffer, max_height: f32, view_limit: usize) {
    let palette = Palette::of(ui);
    ScrollArea::vertical()
        .id_salt("log_scroll")
        .max_height(max_height)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if buffer.is_empty() {
                ui.label(RichText::new("nothing logged yet").color(palette.text_weak));
            } else {
                ui.add(
                    Label::new(
                        RichText::new(buffer.tail(view_limit))
                            .small()
                            .monospace()
                            .color(palette.text),
                    )
                    .selectable(true),
                );
            }
        });
}

/// The recent-events modal, anchored to the bottom-right. It draws while `open`
/// is true, and clears it when the user closes the modal. `Copy all` puts the
/// entire, untruncated log on the clipboard and returns how many lines it
/// copied, so the app can confirm it however it shows status.
pub fn log_modal(
    ctx: &Context,
    buffer: &LogBuffer,
    options: &LogModalOptions,
    open: &mut bool,
) -> Option<usize> {
    if !*open {
        return None;
    }
    let palette = Palette::of_ctx(ctx);
    let frame = Frame::window(&ctx.global_style())
        .fill(palette.window_bg)
        .stroke(Stroke::new(1.0, palette.border))
        .inner_margin(Margin::same(16))
        .corner_radius(CornerRadius::same(10));
    let id = Id::unique("log_modal");
    let area = Modal::default_area(id)
        .anchor(Align2::RIGHT_BOTTOM, options.anchor_offset)
        .order(Order::Foreground);
    let mut copied = None;
    let response = Modal::new(id).area(area).frame(frame).show(ctx, |ui| {
        ui.set_min_width(options.min_width);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Log").strong().size(16.0).color(palette.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        Button::new(RichText::new("×").color(palette.text_weak))
                            .corner_radius(CornerRadius::same(6)),
                    )
                    .on_hover_text("Close")
                    .accessible_name("Close")
                    .clicked()
                {
                    *open = false;
                }
                if ui
                    .add_enabled(
                        !buffer.is_empty(),
                        Button::new(RichText::new("Copy all").small())
                            .corner_radius(CornerRadius::same(6)),
                    )
                    .on_hover_text("Copy every log line to the clipboard")
                    .clicked()
                {
                    ui.ctx().copy_text(buffer.all());
                    copied = Some(buffer.len());
                }
            });
        });
        ui.add_space(4.0);
        log_view(ui, buffer, options.max_height, options.view_limit);
    });
    if response.should_close() {
        *open = false;
    }
    copied
}

#[cfg(test)]
mod tests {
    use super::{LogBuffer, LogCapture, LogModalOptions, log_modal};
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::Theme;
    use log::{Level, LevelFilter, Log, Record};
    use std::sync::{Arc, Mutex};

    fn record<'a>(level: Level, args: std::fmt::Arguments<'a>) -> Record<'a> {
        Record::builder()
            .level(level)
            .target("some::module")
            .args(args)
            .build()
    }

    #[test]
    fn ring_buffer_drops_its_oldest_past_capacity() {
        let buffer = LogBuffer::new(3);
        for line in ["a", "b", "c", "d", "e"] {
            buffer.push(line);
        }
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.all(), "c\nd\ne");
    }

    #[test]
    fn zero_capacity_still_keeps_the_newest_line() {
        let buffer = LogBuffer::new(0);
        buffer.push("a");
        buffer.push("b");
        assert_eq!(buffer.all(), "b");
    }

    /// The modal body keeps the newest entries, in chronological order.
    #[test]
    fn tail_windows_to_the_newest_entries() {
        let log = LogBuffer::new(1000);
        for i in 0..130 {
            log.push(format!("line {i}"));
        }
        let body = log.tail(100);
        assert_eq!(body.lines().count(), 100, "the view is capped");
        assert!(body.starts_with("line 30"), "oldest entries are dropped");
        assert!(body.ends_with("line 129"), "newest entry is last");
        // Short logs pass through unchanged.
        let short = LogBuffer::new(10);
        short.push("a");
        short.push("b");
        assert_eq!(short.tail(100), "a\nb");
        assert_eq!(LogBuffer::new(10).tail(100), "");
    }

    /// `Copy all` text covers every entry, unlike the capped view body.
    #[test]
    fn all_is_untruncated() {
        let log = LogBuffer::new(1000);
        for i in 0..130 {
            log.push(format!("line {i}"));
        }
        let text = log.all();
        assert_eq!(text.lines().count(), 130);
        assert!(text.starts_with("line 0"));
        assert!(text.ends_with("line 129"));
    }

    #[test]
    fn capture_formats_records_and_filters_by_level() {
        let capture = LogCapture::new(10).with_level(LevelFilter::Info);
        let buffer = capture.buffer();
        capture.log(&record(Level::Info, format_args!("hello {}", 1)));
        capture.log(&record(Level::Debug, format_args!("too quiet")));
        capture.log(&record(Level::Error, format_args!("boom")));
        assert_eq!(
            buffer.all(),
            "INFO  some::module: hello 1\nERROR some::module: boom"
        );
    }

    #[test]
    fn capture_forwards_what_it_accepts() {
        struct Spy(Arc<Mutex<Vec<String>>>);
        impl Log for Spy {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool {
                true
            }
            fn log(&self, record: &Record<'_>) {
                self.0
                    .lock()
                    .expect("should lock")
                    .push(record.args().to_string());
            }
            fn flush(&self) {}
        }

        let seen = Arc::new(Mutex::new(Vec::new()));
        let capture = LogCapture::new(10).forward_to(Box::new(Spy(Arc::clone(&seen))));
        capture.log(&record(Level::Warn, format_args!("kept")));
        capture.log(&record(Level::Trace, format_args!("dropped")));
        assert_eq!(*seen.lock().expect("should lock"), ["kept"]);
    }

    /// The only test that sets the process's logger.
    #[test]
    fn install_captures_through_the_log_crate_and_only_once() {
        let buffer = LogCapture::new(1000)
            .install()
            .expect("no other test installs a logger");
        log::info!(target: "egui_mev_app_install_test", "through the facade");
        assert!(
            buffer
                .all()
                .contains("egui_mev_app_install_test: through the facade")
        );
        assert!(LogCapture::new(1).install().is_err());
    }

    fn harness(buffer: LogBuffer) -> Harness<'static, (bool, Option<usize>)> {
        Harness::builder()
            .with_size(egui::vec2(640.0, 400.0))
            .build_ui_state(
                move |ui, state: &mut (bool, Option<usize>)| {
                    Theme::default().install(ui.ctx());
                    let copied =
                        log_modal(ui.ctx(), &buffer, &LogModalOptions::default(), &mut state.0);
                    state.1 = state.1.or(copied);
                },
                (true, None),
            )
    }

    fn copied_text(harness: &Harness<'_, (bool, Option<usize>)>) -> Vec<String> {
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// `Copy all` puts the full log on the clipboard (oldest first) and reports
    /// how many lines it copied.
    #[test]
    fn modal_copy_all() {
        let buffer = LogBuffer::new(10);
        buffer.push("recording started");
        buffer.push("Summarizing failed: boom");
        buffer.push("meeting ready");
        let mut harness = harness(buffer);
        harness.run();
        harness.get_by_label("Copy all").click();
        // One step consumes the queued pointer events; the last frame is the
        // button release, whose output carries the clipboard command.
        harness.run_steps(1);

        assert_eq!(
            copied_text(&harness),
            ["recording started\nSummarizing failed: boom\nmeeting ready"]
        );
        assert_eq!(harness.state().1, Some(3));
    }

    /// With nothing logged, `Copy all` is disabled and copies nothing.
    #[test]
    fn modal_copy_all_disabled_when_empty() {
        let mut harness = harness(LogBuffer::new(10));
        harness.run();
        harness.get_by_label("nothing logged yet");
        // A disabled egui button swallows the click; the proof is that no
        // clipboard command is emitted.
        harness.get_by_label("Copy all").click();
        harness.run_steps(1);
        assert!(copied_text(&harness).is_empty());
        assert_eq!(harness.state().1, None);
    }

    #[test]
    fn close_button_clears_open_and_the_modal_stops_drawing() {
        let mut harness = harness(LogBuffer::new(10));
        harness.run();
        harness.get_by_label("Close").click();
        harness.run();
        assert!(!harness.state().0);
        assert!(harness.query_by_label("Copy all").is_none());
    }

    #[test]
    fn closed_modal_draws_nothing() {
        let mut harness = harness(LogBuffer::new(10));
        harness.state_mut().0 = false;
        harness.run();
        assert!(harness.query_by_label("Log").is_none());
    }

    #[test]
    fn modal_snapshot() {
        let buffer = LogBuffer::new(10);
        buffer.push("INFO  app::config: loaded settings");
        buffer.push("WARN  app::device: reconnecting");
        buffer.push("ERROR app::export: disk full");
        let mut harness = harness(buffer);
        harness.run();
        harness.snapshot("log_modal");
    }
}
