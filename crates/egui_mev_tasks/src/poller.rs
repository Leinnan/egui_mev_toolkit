//! Background polling for state with no change notification. A short poll
//! notices changes without per-OS notification plumbing.

use std::{
    sync::{
        Mutex, PoisonError,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
};

use egui::Context;
use web_time::Instant;

use crate::mailbox;

/// Re-runs a closure on an interval and reports when its result changes, waking
/// egui each time. Dropping this stops the polling.
///
/// Native builds poll on their own thread, which ends as soon as the `Poller`
/// is dropped, or when the closure returns if it's mid-poll. If the OS refuses
/// the thread nothing is ever reported.
///
/// The web build has no threads, so [`latest`](Self::latest) runs the closure
/// inline once the interval has passed, and egui is asked to repaint in time to
/// call it. Keep the closure cheap there, as it holds up the frame.
pub struct Poller<T> {
    source: Source<T>,
}

enum Source<T> {
    Thread {
        values: mailbox::Receiver<T>,
        wake: Sender<()>,
    },
    Inline(Mutex<Inline<T>>),
}

impl<T> Poller<T>
where
    T: PartialEq + Clone + Send + 'static,
{
    /// Start polling. `initial` is the value the app already shows, so only
    /// real changes are reported. The first poll is one `interval` from now.
    pub fn start(
        ctx: &Context,
        interval: Duration,
        initial: T,
        poll: impl FnMut() -> T + Send + 'static,
    ) -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::start_inline(ctx, interval, initial, poll)
        } else {
            Self::start_thread(ctx, interval, initial, poll)
        }
    }

    fn start_thread(
        ctx: &Context,
        interval: Duration,
        initial: T,
        poll: impl FnMut() -> T + Send + 'static,
    ) -> Self {
        let (value_tx, values) = mailbox::channel(1, |_| 0);
        let (wake, wake_rx) = mpsc::channel();
        let ctx = ctx.clone();
        // Dropping the result: without a thread `latest` just stays `None`.
        let _ = thread::Builder::new()
            .name("poller".into())
            .spawn(move || run(initial, poll, interval, &wake_rx, &value_tx, &ctx));
        Self {
            source: Source::Thread { values, wake },
        }
    }

    fn start_inline(
        ctx: &Context,
        interval: Duration,
        initial: T,
        poll: impl FnMut() -> T + Send + 'static,
    ) -> Self {
        ctx.request_repaint_after(interval);
        Self {
            source: Source::Inline(Mutex::new(Inline {
                poll: Box::new(poll),
                last: initial,
                interval,
                last_poll: Instant::now(),
                refresh: false,
                ctx: ctx.clone(),
            })),
        }
    }

    /// Check now instead of at the next tick, after something that probably
    /// changed the value.
    pub fn refresh_now(&self) {
        match &self.source {
            Source::Thread { wake, .. } => {
                _ = wake.send(());
            }
            Source::Inline(inline) => {
                let mut inline = inline.lock().unwrap_or_else(PoisonError::into_inner);
                inline.refresh = true;
                inline.ctx.request_repaint();
            }
        }
    }

    /// The newest value since the last call, if it changed.
    pub fn latest(&self) -> Option<T> {
        match &self.source {
            Source::Thread { values, .. } => values.try_recv().ok(),
            Source::Inline(inline) => inline
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .latest(),
        }
    }
}

fn run<T: PartialEq + Clone>(
    mut last: T,
    mut poll: impl FnMut() -> T,
    interval: Duration,
    wake: &Receiver<()>,
    values: &mailbox::Sender<T>,
    ctx: &Context,
) {
    loop {
        match wake.recv_timeout(interval) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let value = poll();
        if value == last {
            continue;
        }
        last.clone_from(&value);
        if values.send(value).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

struct Inline<T> {
    poll: Box<dyn FnMut() -> T + Send>,
    last: T,
    interval: Duration,
    last_poll: Instant,
    refresh: bool,
    ctx: Context,
}

impl<T: PartialEq + Clone> Inline<T> {
    fn latest(&mut self) -> Option<T> {
        // Every call asks for the next repaint, because a frame caused by
        // anything else replaces the one asked for earlier.
        let elapsed = self.last_poll.elapsed();
        if !self.refresh && elapsed < self.interval {
            self.ctx.request_repaint_after(self.interval - elapsed);
            return None;
        }
        self.refresh = false;
        let value = (self.poll)();
        self.last_poll = Instant::now();
        self.ctx.request_repaint_after(self.interval);
        if value == self.last {
            return None;
        }
        self.last.clone_from(&value);
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicI32, Ordering},
            mpsc::{self, RecvTimeoutError},
        },
        time::Duration,
    };

    use super::Poller;
    use crate::test_support::{TIMEOUT, repaint_signal};

    const LONG: Duration = Duration::from_secs(3600);

    #[test]
    fn stops_when_dropped_without_waiting_for_the_interval() {
        let ctx = egui::Context::default();
        let (alive, thread_gone) = mpsc::channel::<()>();
        let poller = Poller::start(&ctx, LONG, 0, move || {
            let _alive = &alive;
            0
        });
        drop(poller);
        assert_eq!(
            thread_gone.recv_timeout(TIMEOUT),
            Err(RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn stops_when_dropped_mid_poll() {
        let ctx = egui::Context::default();
        let (entered_tx, entered) = mpsc::channel();
        let (go, go_rx) = mpsc::channel::<()>();
        let poller = Poller::start(&ctx, LONG, 0, move || {
            let _ = entered_tx.send(());
            let _ = go_rx.recv_timeout(TIMEOUT);
            1
        });
        poller.refresh_now();
        entered.recv_timeout(TIMEOUT).expect("polling");
        drop(poller);
        drop(go);
        assert_eq!(
            entered.recv_timeout(TIMEOUT),
            Err(RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn polls_on_the_interval_and_wakes() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let poller = Poller::start(&ctx, Duration::from_millis(1), 0, || 1);
        repainted.recv_timeout(TIMEOUT).expect("woken");
        assert_eq!(poller.latest(), Some(1), "ready before the wake");
        assert_eq!(poller.latest(), None);
    }

    #[test]
    fn an_unchanged_value_neither_reports_nor_wakes() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let (entered_tx, entered) = mpsc::channel();
        let (go, go_rx) = mpsc::channel::<()>();
        let calls = AtomicI32::new(0);
        let poller = Poller::start(&ctx, LONG, 0, move || {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            entered_tx.send(call).expect("test is waiting");
            go_rx.recv_timeout(TIMEOUT).expect("test releases");
            // The first call returns the initial value, the second a new one.
            call
        });

        poller.refresh_now();
        assert_eq!(entered.recv_timeout(TIMEOUT), Ok(0));
        go.send(()).expect("polling");

        // The second call starting means the first one is fully dealt with.
        poller.refresh_now();
        assert_eq!(entered.recv_timeout(TIMEOUT), Ok(1));
        assert!(repainted.try_recv().is_err(), "nothing changed");
        assert_eq!(poller.latest(), None);

        go.send(()).expect("polling");
        repainted.recv_timeout(TIMEOUT).expect("woken");
        assert_eq!(poller.latest(), Some(1));
    }

    #[test]
    fn keeps_only_the_newest_value() {
        let ctx = egui::Context::default();
        let (entered_tx, entered) = mpsc::channel();
        let (go, go_rx) = mpsc::channel::<()>();
        let calls = AtomicI32::new(0);
        let poller = Poller::start(&ctx, LONG, 0, move || {
            let call = calls.fetch_add(1, Ordering::SeqCst) + 1;
            entered_tx.send(call).expect("test is waiting");
            let _ = go_rx.recv_timeout(TIMEOUT);
            call
        });
        go.send(()).expect("polling");
        go.send(()).expect("polling");
        for call in 1..=3 {
            poller.refresh_now();
            assert_eq!(entered.recv_timeout(TIMEOUT), Ok(call));
        }
        // The third call is waiting on `go`, so the first two are published.
        assert_eq!(poller.latest(), Some(2));
        assert_eq!(poller.latest(), None);
    }

    /// The path the web build takes.
    #[test]
    fn inline_polls_from_latest_once_due() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let value = Arc::new(AtomicI32::new(0));
        let polled = Arc::clone(&value);
        let poller = Poller::start_inline(&ctx, Duration::ZERO, 0, move || {
            polled.load(Ordering::SeqCst)
        });
        repainted.try_recv().expect("asks for a frame to poll in");

        assert_eq!(poller.latest(), None, "unchanged");
        value.store(1, Ordering::SeqCst);
        assert_eq!(poller.latest(), Some(1));
        assert_eq!(poller.latest(), None);
    }

    #[test]
    fn inline_waits_for_the_interval_unless_refreshed() {
        let ctx = egui::Context::default();
        let poller = Poller::start_inline(&ctx, LONG, 0, || 1);
        assert_eq!(poller.latest(), None, "not due");
        poller.refresh_now();
        assert_eq!(poller.latest(), Some(1));
    }
}
