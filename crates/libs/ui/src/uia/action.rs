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

use std::sync::Mutex;
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
}

/// The pending actions. A mutex and a `Vec`, because a client action arrives at human rate.
#[derive(Debug, Default)]
pub struct Queue(Mutex<Vec<Action>>);

impl Queue {
    /// Records `action` and returns whether the queue was empty before it, which is the one
    /// moment the front thread has to be woken.
    ///
    /// A `SetValue` replaces a queued `SetValue` for the same control instead of being
    /// appended, and returns `false`.
    pub fn push(&self, action: Action) -> bool {
        let mut queue = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let first = queue.is_empty();
        // A client dragging a slider sends one `SetValue` per step and only the last states
        // where the slider is, so a repeat replaces the queued one in place.
        if let Action::SetValue(id, _) = action
            && let Some(last) = queue
                .iter_mut()
                .rfind(|queued| matches!(queued, Action::SetValue(queued, _) if *queued == id))
        {
            *last = action;
            return false;
        }
        queue.push(action);
        first
    }

    /// Moves everything queued into `out`, keeping the queue's allocation.
    pub fn drain(&self, out: &mut Vec<Action>) {
        let mut queue = self.0.lock().unwrap_or_else(|e| e.into_inner());
        out.append(&mut queue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mints `count` ids through the id authority. An id is generational, so a value not
    /// minted here is not one the stack can produce.
    fn ids(count: usize) -> Vec<ControlId> {
        let mut authority = windows_scene::Ids::<windows_scene::Control>::new();
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
}
