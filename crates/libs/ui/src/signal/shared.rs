//! Cross-thread writes: how a producer thread stages a write that the app thread applies.
//!
//! A producer thread cannot touch the graph, so it stages the write rather than performing
//! it — a boxed closure that puts its value into the cell's payload and answers whether the
//! value moved. The app thread applies whatever is staged at the top of its next flush.
//!
//! Two properties hold by construction and both are contractual:
//!
//! - **Writes coalesce.** The staging table is indexed by node, so a producer that outruns
//!   the app thread overwrites its own pending write. Memory is bounded by the number of
//!   live cells, not by the write rate.
//! - **At most one wake is in flight.** The event is signalled on the empty-to-pending
//!   transition only, so a producer writing at any rate wakes the app thread once per
//!   flush.
//!
//! Each cross-thread write costs one box, because the receiving side cannot name the
//! value's type. A display-rate producer publishes through an [`Epoch`](super::Epoch),
//! which carries no value and allocates nothing.

use super::graph::{Signal, SignalId};
use core::any::Any;
use std::sync::{LazyLock, Mutex};
use windows_window::Event;

/// A staged write: puts its value into the cell's payload and returns whether the value
/// moved.
pub(super) type Apply = Box<dyn FnOnce(&dyn Any) -> bool + Send>;

/// Every graph's staged writes, indexed by graph id.
///
/// Indexing by graph and then by node index makes replacing a pending write O(1) with no
/// hash and no scan, and keeps two graphs from aliasing each other's staged writes: a node
/// index is unique only within the graph that minted it.
#[derive(Default)]
struct Inbox(Vec<Pending>);

#[derive(Default)]
struct Pending {
    /// Keyed by ids the app thread's graph minted, so a producer can only stage a write
    /// against a cell it was handed.
    slots: windows_scene::Slots<Signal, Apply>,
    /// Which slots are occupied, so a drain costs O(pending) rather than O(cells).
    dirty: Vec<SignalId>,
}

struct Shared {
    inbox: Mutex<Inbox>,
    event: Event,
}

static SHARED: LazyLock<Shared> = LazyLock::new(|| Shared {
    inbox: Mutex::new(Inbox::default()),
    // The app thread waits on this event; without it, a producer's write could be observed
    // only by polling, so there is no degraded mode to fall back to.
    event: Event::auto_reset().expect("an event is available"),
});

/// Returns the event signalled when a producer's write lands, for the app thread to name
/// alongside its other wake sources.
///
/// Auto-reset, and signalled only on the empty-to-pending transition, so a burst of writes
/// releases the waiter once.
#[must_use]
pub fn written() -> &'static Event {
    &SHARED.event
}

/// Stages a write against `id`, replacing any write already pending for it, and signals
/// [`written`] where nothing was pending for `id`'s graph.
pub(super) fn post(id: SignalId, apply: Apply) {
    let wake = {
        let mut inbox = lock();
        let pending = inbox.pending(id.graph);
        let first = pending.dirty.is_empty();
        if pending.slots.get(id.id).is_none() {
            pending.dirty.push(id);
        }
        pending.slots.place(id.id, apply);
        first
    };
    if wake {
        SHARED.event.signal();
    }
}

/// Moves everything staged against `graph` into `out`, oldest first.
///
/// The lock is released before any of it is applied, so a producer never waits on the app
/// thread's graph work.
pub(super) fn take(graph: u32, out: &mut Vec<(SignalId, Apply)>) {
    let mut inbox = lock();
    let pending = inbox.pending(graph);
    // Taken and handed back rather than drained in place, because the loop borrows the
    // table again. Both it and `out` keep their capacity, so a steady producer stages and
    // the app thread drains without either of them allocating.
    let mut dirty = core::mem::take(&mut pending.dirty);
    for id in dirty.drain(..) {
        if let Some(apply) = pending.slots.take(id.id) {
            out.push((id, apply));
        }
    }
    pending.dirty = dirty;
}

/// Discards anything staged against `id`, which has been disposed.
///
/// A write in flight when a screen unmounts is dropped here rather than held until the next
/// flush, where the generation check would reject it.
pub(super) fn release(id: SignalId) {
    let mut inbox = lock();
    let pending = inbox.pending(id.graph);
    if pending.slots.take(id.id).is_some()
        && let Some(at) = pending.dirty.iter().position(|dirty| *dirty == id)
    {
        pending.dirty.swap_remove(at);
    }
}

impl Inbox {
    fn pending(&mut self, graph: u32) -> &mut Pending {
        let graph = graph as usize;
        if self.0.len() <= graph {
            self.0.resize_with(graph + 1, Pending::default);
        }
        &mut self.0[graph]
    }
}

/// Locks the inbox, recovering from poisoning.
///
/// A producer panicking mid-`post` leaves the table structurally sound: the slot it was
/// writing is either replaced or not, so a poisoned lock is taken rather than propagated.
fn lock() -> std::sync::MutexGuard<'static, Inbox> {
    SHARED.inbox.lock().unwrap_or_else(|e| e.into_inner())
}
