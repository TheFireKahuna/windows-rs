use super::*;

/// The kinds of work a [`CompositionScopedBatch`] can track for completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchKind {
    /// Track key-frame and expression animations.
    Animation,
    /// Track effect loads.
    Effect,
    /// Track infinitely-repeating animations.
    InfiniteAnimation,
    /// Track all animations, including infinite ones.
    AllAnimations,
}

impl From<BatchKind> for bindings::CompositionBatchTypes {
    fn from(kind: BatchKind) -> Self {
        match kind {
            BatchKind::Animation => Self::Animation,
            BatchKind::Effect => Self::Effect,
            BatchKind::InfiniteAnimation => Self::InfiniteAnimation,
            BatchKind::AllAnimations => Self::AllAnimations,
        }
    }
}

/// Groups the animations started while it is open so they can be sealed
/// together with [`end`](Self::end).
///
/// Create the batch, start the animations, then call [`end`](Self::end) to seal
/// it so no later work is added to the group.
pub struct CompositionScopedBatch(pub(crate) bindings::CompositionScopedBatch);

impl CompositionScopedBatch {
    /// Seals the batch. No further work started after this call is tracked by
    /// the batch.
    pub fn end(&self) {
        self.0.End().unwrap();
    }

    /// Seals the batch, reporting failure instead of panicking.
    ///
    /// A caller that arms a batch does two fallible things — subscribes with
    /// [`on_completed`](Self::on_completed) and seals — and is only correct if
    /// both succeed: a batch that was subscribed to but never sealed keeps
    /// swallowing later animations, and one sealed with no subscriber never
    /// reports completion. This sibling lets that pair be written as a single
    /// `?`-chain with one fallback path, rather than a `Result` and a panic that
    /// have to be handled differently.
    ///
    /// Prefer [`end`](Self::end) where there is no subscriber and so nothing to
    /// unwind.
    pub fn try_end(&self) -> Result<()> {
        self.0.End()
    }

    /// Registers `handler` to run once, on the compositor's thread, when every
    /// piece of work this batch tracks has finished.
    ///
    /// This is the only signal that a batch's animations are done. Anything held
    /// alive for the duration of those animations — a visual retained purely so
    /// an exit transition can play out, for example — is released from here.
    ///
    /// The returned [`EventRevoker`](windows_core::EventRevoker) unsubscribes
    /// when it is dropped, so it must be kept alive until the handler has run.
    /// Dropping it early means the completion never arrives and whatever the
    /// handler would have released leaks for the lifetime of the compositor.
    pub fn on_completed(
        &self,
        handler: impl FnMut() + 'static,
    ) -> Result<windows_core::EventRevoker> {
        // The raw handler is `Fn` and receives the sender and the event args,
        // neither of which carries information a caller can act on. A `RefCell`
        // adapts the caller's `FnMut` to that shape without either raw type
        // reaching the public API.
        let handler = core::cell::RefCell::new(handler);
        self.0.Completed(move |_sender, _args| {
            // The event is raised once per batch, so this borrow is uncontended;
            // yielding rather than panicking keeps an unexpected re-entrant
            // raise from unwinding across the COM boundary.
            if let Ok(mut handler) = handler.try_borrow_mut() {
                handler();
            }
        })
    }
}
