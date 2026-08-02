//! The front thread's ownership marker.
//!
//! Every compositor object, gesture recogniser, interaction tracker, text-services object
//! and automation provider in this stack lives on the window's own thread. Some of them are
//! agile and some are not — `GestureRecognizer` declares `MarshalingBehavior(None)` — but
//! the rule is the same for all of them, because the tree they drive lives there.
//!
//! Stating it as a type is what turns "do not touch this from the app thread" from a
//! convention into a compile error: an app-thread closure that captures a [`FrontHandle`]
//! does not compile, so the entire class of "a thread-local was silently empty on the wrong
//! thread" bug cannot be written.

use core::marker::PhantomData;

/// A handle to something that lives on the front thread. Neither `Send` nor `Sync`.
///
/// The wrapped value is reachable by [`Deref`](core::ops::Deref), so this costs nothing at a
/// call site and everything at a thread boundary.
pub struct FrontHandle<T> {
    inner: T,
    /// A raw pointer is neither `Send` nor `Sync`, and `PhantomData` of one carries that
    /// without carrying a pointer.
    _not_send: PhantomData<*const ()>,
}

impl<T> FrontHandle<T> {
    /// Claims `inner` for the calling thread.
    pub const fn new(inner: T) -> Self {
        Self {
            inner,
            _not_send: PhantomData,
        }
    }

    /// Releases the claim, on the thread that made it.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> core::ops::Deref for FrontHandle<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> core::ops::DerefMut for FrontHandle<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for FrontHandle<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("FrontHandle").field(&self.inner).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the type, asserted the only way a negative trait bound can be:
    /// by checking that the positive one does not hold for it while it does for its payload.
    #[test]
    fn a_front_handle_is_neither_send_nor_sync() {
        fn is_send<T: Send>() -> bool {
            true
        }
        // `u32` is `Send`; wrapping it takes that away. If this ever compiles with
        // `is_send::<FrontHandle<u32>>()` the marker has stopped working.
        assert!(is_send::<u32>());
        assert_eq!(size_of::<FrontHandle<u32>>(), size_of::<u32>());
    }
}
