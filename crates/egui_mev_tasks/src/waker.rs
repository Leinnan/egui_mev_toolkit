use std::sync::{Arc, Mutex, PoisonError};

use egui::Context;

/// Lightweight handle that lets background threads request an egui repaint
/// before a [`Context`] exists.
///
/// The context is injected once from the UI thread via [`RepaintWaker::bind`].
/// After binding, every call to [`RepaintWaker::wake`] posts a repaint request,
/// ensuring the event loop continues to run even when the main window is
/// hidden.
#[derive(Clone, Debug, Default)]
pub struct RepaintWaker {
    ctx: Arc<Mutex<Option<Context>>>,
}

impl RepaintWaker {
    /// Create an unbound waker. [`wake`](Self::wake) is a no-op until
    /// [`bind`](Self::bind) is called.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store the egui context so subsequent [`wake`](Self::wake) calls work.
    pub fn bind(&self, ctx: Context) {
        *self.ctx.lock().unwrap_or_else(PoisonError::into_inner) = Some(ctx);
    }

    /// Request an egui repaint if a context has been bound.
    pub fn wake(&self) {
        let maybe_ctx = self
            .ctx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(ctx) = maybe_ctx {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RepaintWaker;
    use crate::test_support::{TIMEOUT, repaint_signal};

    #[test]
    fn wake_before_bind_is_a_no_op() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let waker = RepaintWaker::new();
        waker.wake();
        assert!(repainted.try_recv().is_err(), "nothing bound yet");

        waker.bind(ctx);
        waker.wake();
        repainted.recv_timeout(TIMEOUT).expect("bound waker wakes");
    }

    #[test]
    fn clones_share_the_binding() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let waker = RepaintWaker::new();
        let clone = waker.clone();
        waker.bind(ctx);
        std::thread::spawn(move || clone.wake())
            .join()
            .expect("wake");
        repainted.recv_timeout(TIMEOUT).expect("clone wakes");
    }
}
