//! Queues what a client asks the application to do.
//!
//! `IInvokeProvider::Invoke` and its neighbours `Toggle`, `Select` and `SetValue` are
//! asynchronous: each must return immediately without blocking, so a provider queues the
//! request here and answers `S_OK`. That also keeps a screen reader off the critical path
//! of a busy front thread.
//!
//! The queue is drained by the tick that services input, woken by the same
//! request-for-service the pointer stack posts. An invoke runs the widget's own front-side
//! handler, publishes its pixels and queues its intent, exactly as a tap does.

use crate::text_input::Selection;
use std::sync::{Mutex, PoisonError};
use windows_scene::ControlId;

/// One queued request. `Copy`: every variant is an id and a scalar.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    Invoke(ControlId),
    Toggle(ControlId),
    Select(ControlId),
    Expand(ControlId, bool),
    SetValue(ControlId, f64),
    Focus(ControlId),
    /// Brings the element into view. Named by control and resolved by the front thread
    /// against the scroll ancestry the hit array already carries.
    Reveal(ControlId),
    /// Moves one scroll container's content to an absolute offset in DIPs. Named by the
    /// container's own control, which is the element the scroll pattern hangs on.
    ScrollTo(ControlId, f32, f32),
}

/// One queued editing request. Variable-length, which is why it is not on [`Action`].
#[derive(Debug)]
pub(crate) enum TextAction {
    Replace(ControlId, u64, Vec<u16>),
    Select(ControlId, u64, Selection),
}

/// Decides whether a newly queued row replaces one already queued.
pub(crate) trait Supersedes {
    /// Returns whether `self` stands in place of `queued`, which is dropped when it does.
    ///
    /// # Contract
    ///
    /// `self` must carry everything `queued` would have conveyed. A row that names a
    /// position answers `true` for an earlier row naming the same control, because the
    /// later position is where the control is; a row that names an edit answers `false`,
    /// because two edits are two changes and neither states the other.
    fn supersedes(&self, queued: &Self) -> bool;
}

impl Supersedes for Action {
    fn supersedes(&self, queued: &Self) -> bool {
        matches!(
            (self, queued),
            (Self::SetValue(id, _), Self::SetValue(held, _)) if id == held)
            || matches!(
            (self, queued),
            (Self::ScrollTo(id, ..), Self::ScrollTo(held, ..)) if id == held
        )
    }
}

impl Supersedes for TextAction {
    fn supersedes(&self, queued: &Self) -> bool {
        matches!(
            (self, queued),
            (Self::Select(id, ..), Self::Select(held, ..)) if id == held
        )
    }
}

/// The pending requests. A mutex and a `Vec`, because a client action arrives at human rate.
#[derive(Debug)]
pub(crate) struct Queue<T>(Mutex<Vec<T>>);

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self(Mutex::new(Vec::new()))
    }
}

impl<T: Supersedes> Queue<T> {
    /// Records `action` and returns whether the queue was empty before it, which is the one
    /// moment the front thread has to be woken.
    ///
    /// An action that supersedes a queued one replaces it in place and returns `false`: a
    /// client dragging a slider sends one `SetValue` per step and only the last states
    /// where the slider is.
    pub fn push(&self, action: T) -> bool {
        let mut queue = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let first = queue.is_empty();
        if let Some(held) = queue.iter_mut().rfind(|held| action.supersedes(held)) {
            *held = action;
            return false;
        }
        queue.push(action);
        first
    }

    /// Moves everything queued into `out`, keeping the queue's allocation.
    pub fn drain(&self, out: &mut Vec<T>) {
        let mut queue = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        out.append(&mut queue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mints `count` ids through the id authority. An id is generational, so a value not
    /// minted here is not one the stack can produce.
    fn ids(count: usize) -> Vec<ControlId> {
        let mut authority = windows_scene::Ids::<{ windows_scene::CONTROL }>::default();
        (0..count).map(|_| authority.mint()).collect()
    }

    #[test]
    fn only_the_first_action_asks_for_a_wake() {
        let id = ids(3);
        let queue = Queue::default();
        assert!(queue.push(Action::Invoke(id[0])), "the queue was empty");
        assert!(!queue.push(Action::Invoke(id[1])), "and now it is not");

        let mut out = Vec::new();
        queue.drain(&mut out);
        assert_eq!(out.len(), 2);
        assert!(
            queue.push(Action::Invoke(id[2])),
            "a drained queue is empty"
        );
    }

    #[test]
    fn a_repeated_set_value_supersedes_rather_than_accumulates() {
        let id = ids(2);
        let queue = Queue::default();
        queue.push(Action::SetValue(id[0], 0.25));
        queue.push(Action::SetValue(id[1], 9.0));
        queue.push(Action::SetValue(id[0], 0.75));

        let mut out = Vec::new();
        queue.drain(&mut out);
        assert_eq!(
            out,
            [Action::SetValue(id[0], 0.75), Action::SetValue(id[1], 9.0)],
            "one entry per control, holding its latest value"
        );
    }

    /// Two edits are two changes, so the queue keeps both; two selections name one caret,
    /// so the later one stands.
    #[test]
    fn an_edit_accumulates_and_a_selection_supersedes() {
        let id = ids(1);
        let queue = Queue::default();
        queue.push(TextAction::Replace(id[0], 1, vec![b'a' as u16]));
        queue.push(TextAction::Replace(id[0], 2, vec![b'b' as u16]));
        queue.push(TextAction::Select(id[0], 2, Selection::at(0)));
        queue.push(TextAction::Select(id[0], 2, Selection::at(1)));

        let mut out = Vec::new();
        queue.drain(&mut out);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[2], TextAction::Select(_, _, s) if s.caret == 1));
    }
}
