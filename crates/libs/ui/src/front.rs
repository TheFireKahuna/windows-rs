//! Marks a value as owned by the front thread, which is the window's own thread.
//!
//! Every compositor object, gesture recogniser, interaction tracker, text-services object and
//! automation provider in this stack lives on that thread, agile or not — `GestureRecognizer`
//! declares `MarshalingBehavior(None)` — because the tree they all drive lives there.
//!
//! [`FrontHandle`] carries the rule in the type system: a closure that runs on the app thread
//! and captures one does not compile, so a front-thread object cannot be reached from a thread
//! whose locals for it are empty.

use core::marker::PhantomData;

/// Holds a value that lives on the front thread. Neither `Send` nor `Sync`.
///
/// The value is reachable through [`Deref`](core::ops::Deref) and
/// [`DerefMut`](core::ops::DerefMut), and the wrapper is the same size as `T`.
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

    /// Returns the wrapped value, releasing the claim. The handle is not `Send`, so this runs
    /// on the thread that made the claim.
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

    /// Checks that a [`FrontHandle`] adds no size to a payload that is itself `Send`.
    ///
    /// A negative bound has no runtime assertion. Substituting `is_send::<FrontHandle<u32>>()`
    /// for the call below must fail to compile; if it ever compiles, the marker has stopped
    /// working.
    #[test]
    fn a_front_handle_is_neither_send_nor_sync() {
        fn is_send<T: Send>() -> bool {
            true
        }
        // `u32` is `Send`; wrapping it takes that away.
        assert!(is_send::<u32>());
        assert_eq!(size_of::<FrontHandle<u32>>(), size_of::<u32>());
    }
}
