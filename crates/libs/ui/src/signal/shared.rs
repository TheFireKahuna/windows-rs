//! Cross-thread writes: how a producer thread stages a write that the graph's thread applies.
//!
//! A producer thread cannot touch the graph, so it stages the write rather than performing
//! it — a boxed closure that puts its value into the cell's payload and answers whether the
//! value moved. The graph's thread applies whatever is staged at the top of its next flush.
//!
//! Two properties hold by construction and both are contractual:
//!
//! - **Writes coalesce.** The staging table is indexed by node, so a producer that outruns
//!   the consuming thread overwrites its own pending write. Memory is bounded by the number
//!   of live cells, not by the write rate.
//! - **At most one wake is held per graph.** It is raised on the empty-to-pending transition
//!   and released when that graph drains the writes.
//!
//! Each cross-thread write costs one box, because the receiving side cannot name the
//! value's type. A display-rate producer publishes through an [`Epoch`](super::Epoch),
//! which carries no value and allocates nothing.

use super::graph::{Signal, SignalId};
use crate::seam::Ring;
use core::any::Any;
use std::sync::{Arc, LazyLock, Mutex};
use windows_window::{Tick, Wake};

/// A staged write: puts its value into the cell's payload and returns whether the value
/// moved.
pub(super) type Apply = Box<dyn FnOnce(&dyn Any) -> bool + Send>;

/// How a staged write reaches the thread that owns the graph.
///
/// The two arms are the two shapes that thread can take. A graph flushed from a window's
/// pump has no wait of its own, so the write asks the pacer for a frame and the frame
/// message carries the flush. A graph flushed from a loop parked on a doorbell has no
/// clock, so the write rings that doorbell instead.
pub(crate) enum PostWake {
    /// A window thread: the write holds a frame request until the frame that drains it.
    Pacer(Wake),
    /// A thread parked on a doorbell: the write rings it.
    Ring(Arc<Ring>),
}

impl From<Wake> for PostWake {
    fn from(wake: Wake) -> Self {
        Self::Pacer(wake)
    }
}

impl From<Arc<Ring>> for PostWake {
    fn from(ring: Arc<Ring>) -> Self {
        Self::Ring(ring)
    }
}

/// The outstanding wake for a graph's staged writes.
enum Held {
    /// A live frame request. Kept for its `Drop`, which releases the request and parks the
    /// pacer once no other holder wants a frame.
    Tick(#[expect(dead_code, reason = "held for its Drop, which releases the frame request")] Tick),
    /// The doorbell has been rung. It holds the signal until the sleeper consumes it, so
    /// there is nothing to keep here beyond the fact that the edge has been taken.
    Rung,
}

/// Every graph's staged writes, indexed by graph id.
///
/// Indexing by graph and then by node index makes replacing a pending write O(1) with no
/// hash and no scan, and keeps two graphs from aliasing each other's staged writes: a node
/// index is unique only within the graph that minted it.
#[derive(Default)]
struct Inbox(Vec<Pending>);

#[derive(Default)]
struct Pending {
    /// Keyed by ids the owning graph minted, so a producer can only stage a write against a
    /// cell it was handed.
    slots: windows_scene::Slots<Signal, Apply>,
    /// Which slots are occupied, so a drain costs O(pending) rather than O(cells).
    dirty: Vec<SignalId>,
    how: Option<PostWake>,
    held: Option<Held>,
}

static SHARED: LazyLock<Mutex<Inbox>> = LazyLock::new(|| Mutex::new(Inbox::default()));

/// Holds a graph's post routing. Dropping it releases the routing and any wake it held.
pub(crate) struct PostGuard(u32);

impl Drop for PostGuard {
    fn drop(&mut self) {
        let mut inbox = lock();
        let pending = inbox.pending(self.0);
        pending.held = None;
        pending.how = None;
    }
}

/// Raises one wake on the graph's thread.
///
/// The pacer arm returns the frame request, so the pending batch keeps it live. The ring
/// arm returns nothing to keep: [`Ring::ring`] signals only the parked→running edge, so a
/// second ring while the sleeper is already running is not a second wake.
fn raise(how: &PostWake) -> Held {
    match how {
        PostWake::Pacer(wake) => Held::Tick(wake.tick()),
        PostWake::Ring(ring) => {
            ring.ring();
            Held::Rung
        }
    }
}

/// Routes this graph's staged writes through `how`, replacing whatever was routing them.
pub(super) fn arm(graph: u32, how: PostWake) -> PostGuard {
    let mut inbox = lock();
    let pending = inbox.pending(graph);
    // Writes staged before the routing existed are still owed a wake, and the new routing is
    // the first thing that can deliver one.
    pending.held = (!pending.dirty.is_empty()).then(|| raise(&how));
    pending.how = Some(how);
    PostGuard(graph)
}

/// Stages a write and holds one wake until this graph drains it.
pub(super) fn post(id: SignalId, apply: Apply) {
    let mut inbox = lock();
    let pending = inbox.pending(id.graph);
    if pending.slots.get(id.id).is_none() {
        pending.dirty.push(id);
    }
    pending.slots.place(id.id, apply);
    if pending.held.is_none() {
        pending.held = pending.how.as_ref().map(raise);
    }
}

/// Moves everything staged against `graph` into `out`, oldest first.
///
/// The lock is released before any of it is applied, so a producer never waits on the
/// graph's own work.
pub(super) fn take(graph: u32, out: &mut Vec<(SignalId, Apply)>) {
    let mut inbox = lock();
    let pending = inbox.pending(graph);
    // Taken and handed back rather than drained in place, because the loop borrows the
    // table again. Both it and `out` keep their capacity, so a steady producer stages and
    // the graph's thread drains without either of them allocating.
    let mut dirty = core::mem::take(&mut pending.dirty);
    for id in dirty.drain(..) {
        if let Some(apply) = pending.slots.take(id.id) {
            out.push((id, apply));
        }
    }
    pending.dirty = dirty;
    pending.held = None;
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
    if pending.dirty.is_empty() {
        pending.held = None;
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
    SHARED.lock().unwrap_or_else(|e| e.into_inner())
}
