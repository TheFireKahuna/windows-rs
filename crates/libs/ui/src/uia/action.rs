//! What a client asks the application to do.
//!
//! Every one of these is documented as asynchronous — `IInvokeProvider::Invoke` "is an
//! asynchronous call and must return immediately without blocking", and the same is said
//! of `Toggle`, `Select` and `SetValue`. So queueing and returning `S_OK` is the contract
//! rather than a shortcut, and it is also what keeps a screen reader off the critical path
//! of a busy front thread.
//!
//! The queue is drained by the tick that services input, reached by the same
//! request-for-service the pointer stack posts. There is no second dispatch path: an
//! invoke runs the widget's own front-side handler, publishes its pixels and queues its
//! intent, exactly as a tap does.

use std::sync::Mutex;
use windows_scene::ControlId;

/// One request. `Copy`, because nothing here is a string or a closure.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    Invoke(ControlId),
    Toggle(ControlId),
    Select(ControlId),
    Expand(ControlId, bool),
    SetValue(ControlId, f64),
    Focus(ControlId),
    /// Bring this element into view. Named by control, resolved by the front thread
    /// against the scroll ancestry the hit array already carries.
    Reveal(ControlId),
}

/// The queue itself. A mutex and a `Vec`, because a client action arrives at human rate.
#[derive(Debug, Default)]
pub struct Queue(Mutex<Vec<Action>>);

impl Queue {
    /// Records an action. Answers whether the queue was empty, which is the only moment
    /// the front thread has to be woken.
    pub fn push(&self, action: Action) -> bool {
        let mut queue = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let first = queue.is_empty();
        // A client that holds a slider and drags it sends a `SetValue` per step; only the
        // last one is a fact about where the slider is. Superseding here keeps a burst
        // from becoming a burst of intents the application has to reconcile.
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

    /// Minted through the real authority: an id is generational, and forging one would
    /// test a shape the rest of the stack cannot produce.
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
