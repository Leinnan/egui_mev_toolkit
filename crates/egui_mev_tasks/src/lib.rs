//! Getting work off the UI thread and its results back, waking egui when
//! there's something to show.
//!
//! - [`RepaintWaker`]: wake egui from anywhere, once a context is known.
//! - [`channel`]: an event queue whose sender also wakes egui.
//! - [`spawn`]: run a job in the background where the platform can.
//! - [`mailbox`]: keep only the newest value per source.
//! - [`KeyedWorker`]: a thread pool producing one value per key.
//! - [`Poller`]: re-run a closure on an interval and report changes.
//!
//! The web build has no threads, so everything here still works there by
//! running the work inline on the UI thread.
//!
//! ```ignore
//! let mut thumbnails = KeyedWorker::new(2, 34, |path: &PathBuf| load_thumbnail(path));
//! // Each frame:
//! thumbnails.submit(path.clone(), ctx);
//! for (path, image) in thumbnails.drain() { /* upload the texture */ }
//! ```

mod events;
mod keyed_worker;
pub mod mailbox;
mod poller;
mod spawn;
mod waker;

pub use events::{EventInbox, EventSender, channel};
pub use keyed_worker::KeyedWorker;
pub use poller::Poller;
pub use spawn::spawn;
pub use waker::RepaintWaker;

#[cfg(test)]
mod test_support {
    use std::{
        sync::mpsc::{self, Receiver},
        time::Duration,
    };

    /// How long a test waits for something that should happen. Only a failing
    /// test ever waits this long.
    pub const TIMEOUT: Duration = Duration::from_secs(30);

    /// A channel that receives once when `ctx` is asked to repaint sooner than
    /// it already would. egui calls this only for the first request between
    /// frames, so it fires once per test unless a frame runs.
    pub fn repaint_signal(ctx: &egui::Context) -> Receiver<()> {
        let (tx, rx) = mpsc::channel();
        ctx.set_request_repaint_callback(move |_| {
            let _ = tx.send(());
        });
        rx
    }
}
