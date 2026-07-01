//! `Win32Dispatcher`: the reactor [`Dispatcher`] + [`SendDispatcher`] for the
//! self-hosted backend. Render/effect work is queued and a `WM_APP_DISPATCH`
//! is posted to wake the blocking `GetMessageW` pump; the WndProc drains the
//! queue on the UI thread. No timer, no per-vblank tick — the thread sleeps in
//! `GetMessageW` whenever the queues are empty (true idle, zero CPU at rest).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::backend::{Dispatcher, SendDispatcher, UiMarshaller};
use crate::system_bindings::{PostMessageW, HWND, LPARAM, WPARAM};
use crate::DispatcherQueuePriority;

/// App message that wakes the pump to drain the dispatch queues.
pub(crate) const WM_APP_DISPATCH: u32 = crate::system_bindings::WM_APP + 0x42;

/// UI-thread queue: normal-priority work runs before low-priority.
#[derive(Default)]
pub(crate) struct LocalQueue {
    normal: RefCell<VecDeque<Box<dyn FnOnce()>>>,
    low: RefCell<VecDeque<Box<dyn FnOnce()>>>,
}

impl LocalQueue {
    fn push(&self, priority: DispatcherQueuePriority, f: Box<dyn FnOnce()>) {
        match priority {
            DispatcherQueuePriority::Low => self.low.borrow_mut().push_back(f),
            _ => self.normal.borrow_mut().push_back(f),
        }
    }

    fn pop(&self) -> Option<Box<dyn FnOnce()>> {
        if let Some(f) = self.normal.borrow_mut().pop_front() {
            return Some(f);
        }
        self.low.borrow_mut().pop_front()
    }
}

/// Cross-thread half: a `Send` closure queue plus the window handle (as `isize`,
/// since `HWND` is a raw pointer) so any thread can post the wake message.
pub(crate) struct SendInner {
    hwnd: isize,
    queue: Mutex<VecDeque<Box<dyn FnOnce() + Send>>>,
}

// SAFETY: `hwnd` is only used to `PostMessageW`, which is documented as callable
// from any thread; the closures are `Send` and run on the UI thread.
unsafe impl Send for SendInner {}
unsafe impl Sync for SendInner {}

impl SendDispatcher for SendInner {
    fn enqueue_send(
        &self,
        _priority: DispatcherQueuePriority,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> bool {
        self.queue.lock().unwrap().push_back(f);
        post_wake(self.hwnd);
        true
    }
}

/// The dispatcher handed to `RenderHost`. Clonable handles to its queues are kept
/// by the host so its WndProc can drain them.
pub struct Win32Dispatcher {
    hwnd: isize,
    local: Rc<LocalQueue>,
    send: Arc<SendInner>,
}

impl Win32Dispatcher {
    pub(crate) fn new(hwnd: HWND) -> Self {
        let hwnd = hwnd as isize;
        Self {
            hwnd,
            local: Rc::new(LocalQueue::default()),
            send: Arc::new(SendInner {
                hwnd,
                queue: Mutex::new(VecDeque::new()),
            }),
        }
    }

    /// Shared handles for the host's WndProc to drain on `WM_APP_DISPATCH`.
    pub(crate) fn queues(&self) -> (Rc<LocalQueue>, Arc<SendInner>) {
        (Rc::clone(&self.local), Arc::clone(&self.send))
    }

    /// A thread-safe marshaller backed by this dispatcher (for `use_async_state`).
    pub fn marshaller(&self) -> UiMarshaller {
        UiMarshaller::new(Arc::clone(&self.send) as Arc<dyn SendDispatcher>)
    }
}

impl Dispatcher for Win32Dispatcher {
    fn enqueue(&self, priority: DispatcherQueuePriority, f: Box<dyn FnOnce()>) -> bool {
        self.local.push(priority, f);
        post_wake(self.hwnd);
        true
    }
}

/// Run every queued closure (cross-thread first, then local, normal-before-low).
/// Returning to the pump afterwards lets newly-queued follow-ups wake it again.
pub(crate) fn drain(local: &LocalQueue, send: &SendInner) {
    loop {
        let next = send.queue.lock().unwrap().pop_front();
        match next {
            Some(f) => f(),
            None => break,
        }
    }
    while let Some(f) = local.pop() {
        f();
    }
}

fn post_wake(hwnd: isize) {
    unsafe {
        let _ = PostMessageW(hwnd as HWND, WM_APP_DISPATCH, 0 as WPARAM, 0 as LPARAM);
    }
}
