//! Events from outside egui's frame (platform callbacks, other threads) to the
//! app. Senders queue an event and wake egui; the app drains its inbox each
//! frame.

use std::sync::{Arc, OnceLock, mpsc};

/// The sending half, cloned into callbacks and threads.
#[derive(Debug)]
pub struct EventSender<E> {
    tx: mpsc::Sender<E>,
    ctx: Arc<OnceLock<egui::Context>>,
}

impl<E> Clone for EventSender<E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            ctx: Arc::clone(&self.ctx),
        }
    }
}

impl<E> EventSender<E> {
    /// Queue `event` and wake the UI.
    pub fn send(&self, event: E) {
        // The inbox lives as long as the app; a failed send means it is
        // shutting down, when the event no longer matters.
        if self.tx.send(event).is_ok()
            && let Some(ctx) = self.ctx.get()
        {
            ctx.request_repaint();
        }
    }
}

/// The receiving half, owned by the app.
#[derive(Debug)]
pub struct EventInbox<E> {
    rx: mpsc::Receiver<E>,
    ctx: Arc<OnceLock<egui::Context>>,
}

impl<E> EventInbox<E> {
    /// Let senders wake `ctx` from now on. Only the first call has an effect.
    /// Events sent earlier stay queued for the next [`drain`](Self::drain).
    pub fn attach(&self, ctx: &egui::Context) {
        let _ = self.ctx.set(ctx.clone());
    }

    /// Every event sent since the last call, oldest first.
    pub fn drain(&self) -> Vec<E> {
        self.rx.try_iter().collect()
    }
}

/// A connected sender and inbox.
#[must_use]
pub fn channel<E>() -> (EventSender<E>, EventInbox<E>) {
    let (tx, rx) = mpsc::channel();
    let ctx = Arc::new(OnceLock::new());
    (
        EventSender {
            tx,
            ctx: Arc::clone(&ctx),
        },
        EventInbox { rx, ctx },
    )
}

#[cfg(test)]
mod tests {
    use super::channel;
    use crate::test_support::{TIMEOUT, repaint_signal};

    #[test]
    fn events_arrive_in_order() {
        let (sender, inbox) = channel();
        sender.send("a.png");
        sender.send("b.png");
        assert_eq!(inbox.drain(), vec!["a.png", "b.png"]);
        assert_eq!(inbox.drain(), Vec::<&str>::new(), "drained");
    }

    #[test]
    fn sending_wakes_the_attached_context() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let (sender, inbox) = channel();

        sender.send(1);
        assert!(repainted.try_recv().is_err(), "nothing attached yet");

        inbox.attach(&ctx);
        std::thread::spawn(move || sender.send(2))
            .join()
            .expect("send");
        repainted.recv_timeout(TIMEOUT).expect("woken");
        assert_eq!(inbox.drain(), vec![1, 2], "earlier events are kept");
    }

    #[test]
    fn sender_clones_without_a_clonable_event() {
        struct NotClone;
        let (sender, inbox) = channel();
        sender.clone().send(NotClone);
        assert_eq!(inbox.drain().len(), 1);
    }

    #[test]
    fn sending_after_the_inbox_is_gone_does_not_wake() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let (sender, inbox) = channel();
        inbox.attach(&ctx);
        drop(inbox);
        sender.send(1);
        assert!(repainted.try_recv().is_err());
    }
}
