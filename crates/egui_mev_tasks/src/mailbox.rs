//! Bounded latest-value delivery for replaceable snapshots.
//!
//! Sending never blocks and never wakes egui: this has no [`egui::Context`], so
//! pair it with a [`RepaintWaker`](crate::RepaintWaker) when the UI should
//! react straight away.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        mpsc::{SendError, TryRecvError},
    },
};

struct State<T> {
    values: BTreeMap<usize, T>,
    receiver_alive: bool,
}

/// A non-blocking snapshot publisher. Each key retains only its newest value.
pub struct Sender<T> {
    state: Arc<Mutex<State<T>>>,
    key: fn(&T) -> usize,
    sources: usize,
}

/// The single consumer of a snapshot mailbox.
pub struct Receiver<T> {
    state: Arc<Mutex<State<T>>>,
}

/// Create a mailbox for `sources` independent snapshot sources: `key` must
/// return a number below `sources`, or [`Sender::send`] refuses the value.
#[must_use]
pub fn channel<T>(sources: usize, key: fn(&T) -> usize) -> (Sender<T>, Receiver<T>) {
    let state = Arc::new(Mutex::new(State {
        values: BTreeMap::new(),
        receiver_alive: true,
    }));
    (
        Sender {
            state: Arc::clone(&state),
            key,
            sources,
        },
        Receiver { state },
    )
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            key: self.key,
            sources: self.sources,
        }
    }
}

impl<T> Sender<T> {
    /// Replace the pending snapshot for this source, without blocking on the
    /// consumer. Fails if the receiver is gone or the key is out of range.
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        let key = (self.key)(&value);
        let Ok(mut state) = self.state.lock() else {
            return Err(SendError(value));
        };
        if !state.receiver_alive || key >= self.sources {
            return Err(SendError(value));
        }
        state.values.insert(key, value);
        Ok(())
    }
}

impl<T> Receiver<T> {
    /// Take one pending source's latest snapshot, lowest key first.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = self.state.lock().map_err(|_| TryRecvError::Disconnected)?;
        state
            .values
            .pop_first()
            .map(|(_, value)| value)
            .ok_or(TryRecvError::Empty)
    }

    /// Drain the snapshots currently available.
    pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.try_recv().ok())
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.receiver_alive = false;
            state.values.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn coalesces_each_source_and_detects_dropped_receiver() {
        let (tx, rx) = super::channel(16, |v: &(usize, usize)| v.0);
        for i in 0..1000 {
            tx.send((0, i)).expect("send");
        }
        tx.send((1, 7)).expect("send");
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![(0, 999), (1, 7)]);
        drop(rx);
        assert!(tx.send((0, 0)).is_err());
    }

    #[test]
    fn refuses_keys_past_the_source_count() {
        let (tx, rx) = super::channel(2, |v: &usize| *v);
        assert!(tx.send(1).is_ok());
        assert!(tx.send(2).is_err());
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn a_drained_source_can_publish_again() {
        let (tx, rx) = super::channel(1, |_: &usize| 0);
        tx.clone().send(1).expect("send");
        assert_eq!(rx.try_recv(), Ok(1));
        assert!(rx.try_recv().is_err());
        tx.send(2).expect("send");
        assert_eq!(rx.try_recv(), Ok(2));
    }
}
