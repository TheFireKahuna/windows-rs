//! Document locks are a protocol, not a mutex. Never hold a Rust borrow over a TIP call.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Request {
    Grant(u32),
    Async,
    Synchronous,
}

#[derive(Default)]
pub(crate) struct Locks {
    pub held: u32,
    pending: u32,
    pub notifying: bool,
}

impl Locks {
    pub fn request(&mut self, flags: u32) -> Request {
        let access = flags & 6;
        if self.held != 0 || self.notifying {
            if flags & 1 != 0 {
                return Request::Synchronous;
            }
            self.pending |= access;
            return Request::Async;
        }
        // A later grant discharges the outstanding request only at its escalated access.
        self.held = access | core::mem::take(&mut self.pending);
        Request::Grant(self.held)
    }
    pub fn release(&mut self) {
        self.held = 0;
    }
    pub fn pending(&self) -> bool {
        self.pending != 0
    }
    pub fn take_pending(&mut self) -> Option<u32> {
        if self.held != 0 || self.notifying || self.pending == 0 {
            return None;
        }
        self.held = core::mem::take(&mut self.pending);
        Some(self.held)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asynchronous_requests_coalesce_and_escalate() {
        let mut l = Locks::default();
        assert_eq!(l.request(2), Request::Grant(2));
        for _ in 0..20 {
            assert_eq!(l.request(2), Request::Async);
        }
        assert_eq!(l.request(6), Request::Async);
        assert_eq!(l.request(3), Request::Synchronous);
        l.release();
        assert_eq!(l.take_pending(), Some(6));
        l.release();
        assert_eq!(l.take_pending(), None);
    }
    #[test]
    fn notifications_defer_locks_without_reentrant_mutation() {
        let mut l = Locks {
            notifying: true,
            ..Locks::default()
        };
        assert_eq!(l.request(7), Request::Synchronous);
        assert_eq!(l.request(6), Request::Async);
        assert_eq!(l.take_pending(), None);
        l.notifying = false;
        assert_eq!(l.take_pending(), Some(6));
    }
}
