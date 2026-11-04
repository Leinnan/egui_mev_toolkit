//! Two kinds of undo history.
//!
//! [`UndoStack`] holds actions the app knows how to invert. [`SnapshotHistory`]
//! holds copies of a value, for documents that are cheap to share with `Arc`.

use std::{collections::VecDeque, sync::Arc, time::Duration};

use web_time::Instant;

/// Steps kept when no limit is given; the oldest go first.
pub const DEFAULT_MAX_STEPS: usize = 50;
/// Edits closer together than this join one step in a [`SnapshotHistory`].
pub const DEFAULT_COALESCE_WINDOW: Duration = Duration::from_millis(800);

/// Where a finished operation came from, so its inverse lands on the right stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The user did it; this also clears the redo stack.
    User,
    /// Undoing something, so the inverse can be redone.
    Undo,
    /// Redoing something, so the inverse can be undone again.
    Redo,
}

/// Undo and redo stacks of invertible actions.
#[derive(Debug)]
pub struct UndoStack<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    max_steps: usize,
}

impl<T> Default for UndoStack<T> {
    fn default() -> Self {
        Self::with_limit(DEFAULT_MAX_STEPS)
    }
}

impl<T> UndoStack<T> {
    /// An empty history that keeps at most `max_steps` per stack.
    #[must_use]
    pub const fn with_limit(max_steps: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            max_steps,
        }
    }

    /// Record the inverse of a finished operation.
    pub fn record(&mut self, origin: Origin, inverse: T) {
        let stack = match origin {
            Origin::User => {
                self.redo.clear();
                &mut self.undo
            }
            Origin::Redo => &mut self.undo,
            Origin::Undo => &mut self.redo,
        };
        stack.push(inverse);
        if stack.len() > self.max_steps {
            stack.remove(0);
        }
    }

    /// Take the action to run for an undo.
    pub fn pop_undo(&mut self) -> Option<T> {
        self.undo.pop()
    }

    /// Take the action to run for a redo.
    pub fn pop_redo(&mut self) -> Option<T> {
        self.redo.pop()
    }

    /// The action an undo would run.
    #[must_use]
    pub fn peek_undo(&self) -> Option<&T> {
        self.undo.last()
    }

    /// The action a redo would run.
    #[must_use]
    pub fn peek_redo(&self) -> Option<&T> {
        self.redo.last()
    }
}

/// Undo and redo as snapshots of a copy-on-write value.
///
/// Edits save as they are made, so history is recorded where they are saved. A
/// burst of saves (a slider drag, typing) counts as one step.
#[derive(Debug)]
pub struct SnapshotHistory<T> {
    undo: VecDeque<Arc<T>>,
    redo: Vec<Arc<T>>,
    /// The value as last recorded; what a new step would restore.
    current: Arc<T>,
    last_recorded: Option<Instant>,
    max_steps: usize,
    coalesce_window: Duration,
}

impl<T: PartialEq> SnapshotHistory<T> {
    /// Start with no history at `value`, with the default limits.
    #[must_use]
    pub fn new(value: Arc<T>) -> Self {
        Self::with_limits(value, DEFAULT_MAX_STEPS, DEFAULT_COALESCE_WINDOW)
    }

    /// Start with no history at `value`.
    #[must_use]
    pub fn with_limits(value: Arc<T>, max_steps: usize, coalesce_window: Duration) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            current: value,
            last_recorded: None,
            max_steps,
            coalesce_window,
        }
    }

    /// Forget everything: another document took over, or this one changed in a
    /// way history can't restore.
    pub fn reset(&mut self, value: Arc<T>) {
        self.undo.clear();
        self.redo.clear();
        self.current = value;
        self.last_recorded = None;
    }

    /// Note `value` as saved at `now`; a change from the last one is a new
    /// step, or joins the previous step when it follows closely.
    pub fn record(&mut self, value: &Arc<T>, now: Instant) {
        if Arc::ptr_eq(value, &self.current) || **value == *self.current {
            return;
        }
        let joins_previous = !self.undo.is_empty()
            && self
                .last_recorded
                .is_some_and(|last| now.duration_since(last) < self.coalesce_window);
        let previous = std::mem::replace(&mut self.current, Arc::clone(value));
        if !joins_previous {
            self.undo.push_back(previous);
            if self.undo.len() > self.max_steps {
                self.undo.pop_front();
            }
        }
        self.redo.clear();
        self.last_recorded = Some(now);
    }

    /// Step back from `value`, returning the value to restore.
    pub fn undo(&mut self, value: &Arc<T>, now: Instant) -> Option<Arc<T>> {
        self.record(value, now);
        let previous = self.undo.pop_back()?;
        self.redo
            .push(std::mem::replace(&mut self.current, Arc::clone(&previous)));
        self.last_recorded = None;
        Some(previous)
    }

    /// Step forward again, returning the value to restore.
    pub fn redo(&mut self, value: &Arc<T>, now: Instant) -> Option<Arc<T>> {
        self.record(value, now);
        let next = self.redo.pop()?;
        self.undo
            .push_back(std::mem::replace(&mut self.current, Arc::clone(&next)));
        self.last_recorded = None;
        Some(next)
    }

    /// Whether an undo would do anything for a value that hasn't changed.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether a redo would do anything for a value that hasn't changed.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use web_time::Instant;

    use super::{DEFAULT_COALESCE_WINDOW, DEFAULT_MAX_STEPS, Origin, SnapshotHistory, UndoStack};

    fn named(name: &str) -> Arc<String> {
        Arc::new(name.to_owned())
    }

    /// Far enough apart to be separate steps.
    fn later(start: Instant, step: u32) -> Instant {
        start + (DEFAULT_COALESCE_WINDOW + Duration::from_millis(1)) * step
    }

    #[test]
    fn a_user_action_clears_redo_but_an_undo_does_not() {
        let mut stack = UndoStack::default();
        stack.record(Origin::User, 1);
        stack.record(Origin::Undo, 2);
        assert_eq!(stack.peek_redo(), Some(&2));
        stack.record(Origin::Redo, 3);
        assert_eq!(stack.peek_redo(), Some(&2));
        stack.record(Origin::User, 4);
        assert_eq!(stack.pop_redo(), None);
        assert_eq!(stack.pop_undo(), Some(4));
        assert_eq!(stack.pop_undo(), Some(3));
        assert_eq!(stack.pop_undo(), Some(1));
    }

    #[test]
    fn the_stack_drops_its_oldest_past_the_limit() {
        let mut stack = UndoStack::with_limit(2);
        for action in 0..4 {
            stack.record(Origin::User, action);
        }
        assert_eq!(stack.pop_undo(), Some(3));
        assert_eq!(stack.pop_undo(), Some(2));
        assert_eq!(stack.pop_undo(), None);
    }

    #[test]
    fn undo_and_redo_restore_the_exact_value() {
        let start = Instant::now();
        let (a, b, c) = (named("a"), named("b"), named("c"));
        let mut history = SnapshotHistory::new(Arc::clone(&a));
        history.record(&b, later(start, 1));
        history.record(&c, later(start, 2));

        let at = later(start, 3);
        assert_eq!(history.undo(&c, at), Some(Arc::clone(&b)));
        assert_eq!(history.undo(&b, at), Some(Arc::clone(&a)));
        assert_eq!(history.undo(&a, at), None);
        assert_eq!(history.redo(&a, at), Some(Arc::clone(&b)));
        assert_eq!(history.redo(&b, at), Some(Arc::clone(&c)));
        assert_eq!(history.redo(&c, at), None);
    }

    #[test]
    fn saves_in_quick_succession_make_one_step() {
        let start = Instant::now();
        let a = named("a");
        let mut history = SnapshotHistory::new(Arc::clone(&a));
        let last = named("drag 3");
        history.record(&named("drag 1"), start);
        history.record(&named("drag 2"), start + DEFAULT_COALESCE_WINDOW / 2);
        history.record(&last, start + DEFAULT_COALESCE_WINDOW);

        assert_eq!(history.undo(&last, later(start, 2)), Some(a));
    }

    #[test]
    fn first_edit_after_a_pause_stays_undoable_on_its_own() {
        let start = Instant::now();
        let (a, b, c) = (named("a"), named("b"), named("c"));
        let mut history = SnapshotHistory::new(Arc::clone(&a));
        history.record(&b, start);
        history.record(&c, later(start, 1));

        assert_eq!(history.undo(&c, later(start, 2)), Some(b));
    }

    #[test]
    fn saving_without_a_change_records_nothing() {
        let a = named("a");
        let mut history = SnapshotHistory::new(Arc::clone(&a));
        let at = Instant::now();
        history.record(&a, at);
        history.record(&named("a"), at);

        assert_eq!(history.undo(&a, at), None);
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let start = Instant::now();
        let (a, b, c) = (named("a"), named("b"), named("c"));
        let mut history = SnapshotHistory::new(Arc::clone(&a));
        history.record(&b, start);
        history.undo(&b, later(start, 1));
        history.record(&c, later(start, 2));

        assert_eq!(history.redo(&c, later(start, 3)), None);
    }

    #[test]
    fn an_edit_right_after_undo_is_its_own_step() {
        let start = Instant::now();
        let (a, b, c) = (named("a"), named("b"), named("c"));
        let mut history = SnapshotHistory::new(Arc::clone(&a));
        history.record(&b, start);
        let at = later(start, 1);
        history.undo(&b, at);
        history.record(&c, at);

        assert_eq!(history.undo(&c, at), Some(a));
    }

    #[test]
    fn undo_includes_an_edit_not_saved_yet() {
        let a = named("a");
        let unsaved = named("unsaved");
        let mut history = SnapshotHistory::new(Arc::clone(&a));

        assert_eq!(history.undo(&unsaved, Instant::now()), Some(a));
    }

    #[test]
    fn history_stays_within_the_cap() {
        let start = Instant::now();
        let mut history = SnapshotHistory::new(named("start"));
        for step in 0..DEFAULT_MAX_STEPS + 10 {
            history.record(&named(&format!("{step}")), later(start, step as u32));
        }

        assert_eq!(history.undo.len(), DEFAULT_MAX_STEPS);
    }

    #[test]
    fn reset_forgets_everything() {
        let start = Instant::now();
        let (a, b, other) = (named("a"), named("b"), named("other"));
        let mut history = SnapshotHistory::new(a);
        history.record(&b, start);
        history.reset(Arc::clone(&other));

        assert!(!history.can_undo());
        assert_eq!(history.undo(&other, later(start, 1)), None);
        assert_eq!(history.redo(&other, later(start, 1)), None);
    }
}
