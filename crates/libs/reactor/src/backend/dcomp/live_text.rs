//! Text a non-UI thread replaces without a reconcile.
//!
//! A readout fed by a render pump — a level meter's number, a transport clock —
//! changes at display rate and never passes through the reconciler: the value
//! arrives on the producer's own thread, and a component that never re-renders
//! has no pass in which to reshape its words. Until now the only answer was to
//! paint the glyphs imperatively into a composition surface, which is the last
//! thing in the app that rasterizes text.
//!
//! The visual tree is thread-affine — creating, parenting and placing sprites is
//! COM work on the compositor's thread, so no thread-affine object ever reaches a
//! producer. This does not hand the producer any part of the tree. It follows the
//! same rule the other front-serviced op queues do: **queue from any thread, apply
//! on the front thread.** A [`LiveText`] carries nothing but a window handle and a
//! control id, both plain integers.
//!
//! Two properties make it cheap enough to sit under a display-rate producer:
//!
//! - **Coalescing.** The queue is a map keyed by control, so a producer that
//!   outruns the front thread overwrites its own pending value instead of
//!   growing a backlog. Memory is bounded by the number of live readouts, not
//!   by the publish rate.
//! - **One wake in flight.** A post to the front thread happens only on the
//!   empty→pending transition. Every publish until that post is serviced is a
//!   lock and a map write, with no message traffic at all.
//!
//! On the front thread the update is the same shape as
//! [`DCompBackend::repaint_caption`](super::DCompBackend::repaint_caption): set
//! the node's words, mark it dirty, repaint. The run then reshapes through
//! [`Shaped`](super::glyph_text::Shaped) at *placement* time, the way a knob's
//! dial text does — deliberately not through the node's layout-pass
//! `text_layout`, which is gated on `text_dirty` and so would put a live run back
//! behind the reconcile it exists to avoid.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;

use crate::backend::ControlId;

/// Pending text per control. A `Mutex` rather than a thread-local: publishes
/// originate wherever the producer runs, which is never the thread that services
/// them.
static PENDING: Mutex<Option<HashMap<ControlId, String>>> = Mutex::new(None);

/// Whether a service call is already on its way to the front thread. Gates the
/// post so a producer running at display rate leaves at most one message in
/// flight no matter how fast it publishes.
static POSTED: AtomicBool = AtomicBool::new(false);

/// The front thread's window, for waking it from a producer thread.
///
/// A plain integer, published once when the window is created. The window is
/// unique per process and outlives every producer, so this needs no lifetime
/// beyond "set before anything can publish".
static FRONT_HWND: AtomicIsize = AtomicIsize::new(0);

/// Record the front window. Called once, from the thread that creates it.
pub(crate) fn set_front_hwnd(hwnd: isize) {
    FRONT_HWND.store(hwnd, Ordering::Release);
}

/// The front window, or `0` before it exists — the wake address every producer
/// seam shares. Read by [`bar_field`](super::bar_field), which queues from
/// producer threads on exactly the same terms and has no second window to post
/// to.
pub(crate) fn front_hwnd() -> isize {
    FRONT_HWND.load(Ordering::Acquire)
}

/// A handle to one control's text, writable from any thread.
///
/// Obtained from a mounted `TextBlock`. Cheap to clone and `Send`, because it
/// holds no COM: the control id names a node the front thread owns, and nothing
/// here can touch that node directly.
///
/// A handle outliving its control is harmless — the update is dropped when the
/// id no longer resolves, which is what makes it safe to keep one in a producer
/// that does not observe unmounts.
#[derive(Clone, Copy, Debug)]
pub struct LiveText {
    id: ControlId,
}

impl LiveText {
    pub(crate) fn new(id: ControlId) -> Self {
        Self { id }
    }

    /// Replace this control's text from any thread.
    ///
    /// Publishing the *same* string still costs a map write here; the reshape is
    /// what it saves, since `Shaped` compares before it builds a layout. A
    /// producer that can cheaply tell its value has not moved should skip the
    /// call entirely rather than rely on that — an unchanged publish that never
    /// happens is the only free one.
    pub fn set(&self, text: &str) {
        let first = {
            let Ok(mut q) = PENDING.lock() else {
                return;
            };
            let map = q.get_or_insert_with(HashMap::new);
            // Overwrite rather than append: a producer outrunning the front
            // thread replaces its own pending value, so the queue holds at most
            // one entry per control however fast it publishes.
            map.insert(self.id, text.to_string());
            map.len() == 1
        };

        // Wake the front thread only when this publish started a batch, and only
        // if no earlier wake is still outstanding. `first` alone is not enough:
        // a second control publishing before the service runs would make the map
        // non-empty again without being the batch that already posted.
        if first && !POSTED.swap(true, Ordering::AcqRel) {
            let hwnd = FRONT_HWND.load(Ordering::Acquire);
            if hwnd != 0 {
                super::host::post_ui(hwnd, || {
                    if let Some(s) = super::host::shared() {
                        s.backend.borrow_mut().service_live_text();
                    }
                });
            } else {
                // No window yet — drop the wake claim so a later publish can
                // post one, rather than leaving the batch stranded.
                POSTED.store(false, Ordering::Release);
            }
        }
    }
}

/// Take the pending updates for the front thread to apply, and release the wake
/// claim so the next publish posts again.
///
/// The claim is released *before* the caller applies the batch, so a publish
/// landing during the apply schedules another service rather than being folded
/// into one already in progress and missed.
pub(crate) fn drain() -> Vec<(ControlId, String)> {
    POSTED.store(false, Ordering::Release);
    match PENDING.lock() {
        Ok(mut q) => q
            .as_mut()
            .map(|m| m.drain().collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The queue is process-global — correctly so, since a process has one
    /// window and producers publish from anywhere — which means these tests
    /// contend for it. Serialize them, or one test drains another's batch and
    /// both see the wrong thing.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Take the serialization lock and clear the queue. Returns the guard, which
    /// the caller must hold for the body of the test.
    ///
    /// Ignores lock poisoning: a panicking test leaves the guard poisoned, and
    /// every later test would then fail for a reason that has nothing to do with
    /// what it asserts.
    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        *PENDING.lock().unwrap() = None;
        POSTED.store(false, Ordering::Release);
        g
    }

    fn cid(n: u32) -> ControlId {
        ControlId(std::num::NonZeroU32::new(n).expect("test ids are non-zero"))
    }

    /// The coalescing property: many publishes to one control leave one entry,
    /// holding the value published last.
    #[test]
    fn a_control_keeps_only_its_latest_value() {
        let _g = reset();
        let t = LiveText::new(cid(7));
        t.set("-14.0");
        t.set("-13.9");
        t.set("-13.8");

        let batch = drain();
        assert_eq!(batch.len(), 1, "one control, one entry");
        assert_eq!(batch[0], (cid(7), "-13.8".to_string()), "the last write wins");
        assert!(drain().is_empty(), "the queue is empty once taken");
    }

    /// Distinct controls coexist — coalescing is per control, not global.
    #[test]
    fn separate_controls_each_keep_an_entry() {
        let _g = reset();
        LiveText::new(cid(1)).set("a");
        LiveText::new(cid(2)).set("b");

        let mut batch = drain();
        batch.sort_by_key(|(id, _)| id.0.get());
        assert_eq!(batch, vec![(cid(1), "a".into()), (cid(2), "b".into())]);
    }

    /// Draining must release the wake claim, or the first batch would be the
    /// only one ever serviced.
    #[test]
    fn draining_releases_the_wake_claim() {
        let _g = reset();
        POSTED.store(true, Ordering::Release);
        let _ = drain();
        assert!(
            !POSTED.load(Ordering::Acquire),
            "a serviced batch must let the next publish post again"
        );
    }
}
