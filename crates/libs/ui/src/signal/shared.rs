//! The one thing about a signal that is not the app thread's: a write from a producer.
//!
//! A producer thread cannot touch the graph, so it stages the *write* rather than
//! performing it — a boxed closure that knows how to put its value into the cell's payload
//! and answers whether the value actually moved. The app thread applies whatever is staged
//! at the top of its next flush.
//!
//! Two properties fall out of the shape and both are contractual:
//!
//! - **Writes coalesce by construction.** The staging table is indexed by node, so a
//!   producer that outruns the app thread overwrites its own pending write. Memory is
//!   bounded by the number of live cells, not by the write rate.
//! - **At most one wake is in flight.** The event is signalled on the empty→pending
//!   transition only, so a producer writing at any rate wakes the app thread once per
//!   flush.
//!
//! What is *not* free here is one box per cross-thread write. That is the honest cost of
//! type-erasing a write the receiver cannot name, and it is why a display-rate producer
//! publishes through an [`Epoch`](super::Epoch) — which carries no value — rather than
//! through a cell.

use super::graph::SignalId;
use core::any::Any;
use std::sync::{LazyLock, Mutex};
use windows_window::Event;

/// A staged write: puts its value into the cell's payload, and answers whether that
/// changed anything.
pub(super) type Apply = Box<dyn FnOnce(&dyn Any) -> bool + Send>;

/// Indexed by graph, then by node index, so replacing a pending write is O(1) with no hash
/// and no scan — and so two graphs cannot alias each other's staged writes. Production
/// allocates exactly one graph; the nesting exists because a node index is unique only
/// within the graph that minted it.
#[derive(Default)]
struct Inbox(Vec<Pending>);

#[derive(Default)]
struct Pending {
    slots: Vec<Option<(SignalId, Apply)>>,
    /// Which slots are occupied, so a drain is O(pending) rather than O(cells).
    dirty: Vec<u32>,
}

struct Shared {
    inbox: Mutex<Inbox>,
    event: Event,
}

static SHARED: LazyLock<Shared> = LazyLock::new(|| Shared {
    inbox: Mutex::new(Inbox::default()),
    // A wake source with no event cannot be waited on, and an app thread that cannot be
    // woken by a producer is a polling loop. There is nothing to fall back to.
    event: Event::auto_reset().expect("an event is available"),
});

/// Signalled when a producer's write lands, so the app thread's wait has something to
/// name alongside its other wake sources.
///
/// Auto-reset, and signalled only on the empty→pending transition.
#[must_use]
pub fn written() -> &'static Event {
    &SHARED.event
}

/// Stages a write against `id`, replacing any write already pending for it.
pub(super) fn post(id: SignalId, apply: Apply) {
    let wake = {
        let mut inbox = lock();
        let pending = inbox.pending(id.graph);
        let index = id.index as usize;
        if pending.slots.len() <= index {
            pending.slots.resize_with(index + 1, || None);
        }
        let first = pending.dirty.is_empty();
        if pending.slots[index].is_none() {
            pending.dirty.push(id.index);
        }
        pending.slots[index] = Some((id, apply));
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
    for index in dirty.drain(..) {
        if let Some(entry) = pending.slots[index as usize].take() {
            out.push(entry);
        }
    }
    pending.dirty = dirty;
}

/// Discards anything staged against a node that has been disposed.
///
/// Without this a write in flight when a screen unmounts would be kept alive for a flush
/// and then applied to whatever now occupies the slot — which the generation check catches,
/// but only after the fact.
pub(super) fn release(id: SignalId) {
    let mut inbox = lock();
    let pending = inbox.pending(id.graph);
    let index = id.index as usize;
    if let Some(slot) = pending.slots.get_mut(index)
        && slot.as_ref().is_some_and(|(staged, _)| *staged == id)
    {
        *slot = None;
        if let Some(at) = pending.dirty.iter().position(|dirty| *dirty == id.index) {
            pending.dirty.swap_remove(at);
        }
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

/// The inbox, recovering from a poisoned lock.
///
/// A producer panicking mid-`post` leaves the table structurally sound — the slot it was
/// writing is either replaced or not — and refusing every subsequent write because one
/// thread died is a worse failure than continuing.
fn lock() -> std::sync::MutexGuard<'static, Inbox> {
    SHARED.inbox.lock().unwrap_or_else(|e| e.into_inner())
}
