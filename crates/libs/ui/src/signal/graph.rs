//! The signal graph: one arena of nodes, two-level marking, and a flush that allocates
//! nothing.
//!
//! A cell, a memo, an effect, a retained resource and a disposal scope are one node with a
//! role tag. A node's payload is an `Rc<dyn Any>` — a cell's `RefCell<T>`, a memo's cached
//! `RefCell<Option<T>>`, a resource's `T` — and its work is one `FnMut() -> bool` answering
//! whether the value moved, which is a memo's cutoff and an effect's constant `false`. A
//! scope's `subs` column holds what it owns instead of what reads it.
//!
//! # Edges are generational indices
//!
//! An edge names a slot and the generation that minted the node in it, so no node holds an
//! `Rc` to another node and no cycle of nodes can leak. An edge whose target was disposed
//! fails the generation check and is pruned on the next traversal.
//!
//! # The graph is never borrowed across application code
//!
//! Every path that runs a closure clones the `Rc` out and drops the borrow first, which is
//! what lets such a closure read and even write other signals.

use core::any::Any;
use core::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use windows_window::{Tick, Wake};

use crate::seam::Ring;

/// How many passes a flush may take before it stops.
///
/// An effect that writes a cell adds a pass; one that writes a cell it also reads never
/// settles. A flush that has not settled after this many passes trips a debug assertion.
const MAX_PASSES: u32 = 8;

pub(crate) const CELL: u8 = 0;
pub(crate) const MEMO: u8 = 1;
pub(crate) const EFFECT: u8 = 2;
pub(crate) const RESOURCE: u8 = 3;
pub(crate) const SCOPE: u8 = 4;
const ROLE: u8 = 0b0000_0111;

const CLEAN: u8 = 0;
const CHECK: u8 = 0b0000_1000;
const DIRTY: u8 = 0b0001_0000;
const STATE: u8 = CHECK | DIRTY;
const QUEUED: u8 = 0b0010_0000;
pub(crate) const GEOMETRY: u8 = 0b0100_0000;

const UPDATE: usize = 0;
const GEOM: usize = 1;

/// A node's identity: which graph, a dense index into it, and the generation that minted the
/// node in that slot.
///
/// `Copy` with no reference count, so a `move ||` closure captures one at no cost. The graph
/// is part of the identity rather than implied by the thread, because a producer's staged
/// write is looked up by id from a thread other than the one that minted it, and an index is
/// unique only within the graph that minted it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SignalId {
    graph: u32,
    edge: Edge,
}

/// An edge within one graph: a slot, and the generation that minted the node in it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
struct Edge {
    index: u32,
    generation: u32,
}

/// One node's work: recompute, and answer whether the value moved.
///
/// `Rc<RefCell<..>>` rather than `Box`, so the graph borrow is released before the closure
/// runs and the closure may read and write other signals.
pub(crate) type Work = Rc<RefCell<dyn FnMut() -> bool>>;

/// Detached graph payload, dropped after the caller releases its runtime borrow.
pub(crate) type RetiredEffect = Work;

/// The signal runtime. One per thread that builds signals.
#[derive(Default)]
struct Graph {
    /// This graph's process-unique id, stamped into every [`SignalId`] it mints.
    id: u32,
    generation: Vec<u32>,
    flags: Vec<u8>,
    /// Creation order. Effects run in it, so a parent's effect lands before its child's.
    order: Vec<u64>,
    /// How many times this node's value has moved.
    version: Vec<u64>,
    /// What this node read, most recently.
    deps: Vec<Vec<Edge>>,
    /// What read this node — or, for a scope, what it owns, in creation order.
    subs: Vec<Vec<Edge>>,
    value: Vec<Option<Rc<dyn Any>>>,
    work: Vec<Option<Work>>,
    /// Disposed nodes, parked for their edge buffers. A recycled node keeps its `deps` and
    /// `subs` capacity, so the second mount of a screen allocates no edge storage.
    free: Vec<u32>,
    /// Live nodes other than scopes, which is what a leak assertion counts.
    live: u32,
    next_order: u64,
    /// The node currently collecting dependencies, if any.
    observer: Option<Edge>,
    /// The scope new nodes register with, if any.
    scope: Option<Edge>,
    /// Effects marked since the last pass, per phase.
    queue: [Vec<Edge>; 2],
    /// The pass being drained. Separate from `queue`, so a write from inside an effect
    /// appends to the *next* pass rather than to the one in progress.
    spare: Vec<Edge>,
    /// The `Check` propagation frontier. Pooled: marking allocates nothing.
    stack: Vec<Edge>,
    /// Cross-thread writes, held between drains so the buffer keeps its capacity.
    inbox: Vec<(Edge, Post)>,
    flushing: bool,
}

/// Hands each graph the process-unique id it stamps into every [`SignalId`].
static GRAPHS: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static GRAPH: RefCell<Graph> = RefCell::new(Graph {
        // `Relaxed`: the counter publishes no data, and only its uniqueness is read.
        id: GRAPHS.fetch_add(1, Ordering::Relaxed),
        ..Graph::default()
    });

    /// What to call when this graph acquires update work; installed by [`set_waker`].
    ///
    /// Held outside [`Graph`] so it can be called with no graph borrow held: a waker is host
    /// code and may re-enter the graph.
    static WAKER: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Runs `f` with the graph borrowed. `f` must not call application code, which may re-enter
/// and borrow the graph again.
fn with<R>(f: impl FnOnce(&mut Graph) -> R) -> R {
    GRAPH.with(|g| f(&mut g.borrow_mut()))
}

/// Runs `f` with the graph borrowed, answering `None` where the graph is not reachable.
///
/// The graph is unreachable while the thread's locals are being destroyed and its own node
/// storage is dropping. A node can hold an [`Owner`](super::Owner) — a `Branch`'s arm and a
/// `Keyed`'s row both do — so dropping one asks the graph to dispose a scope from inside the
/// graph's own destructor. `with` panics there, inside a `Drop`, which aborts the process;
/// every path reached from a `Drop` uses this instead.
fn try_with<R>(f: impl FnOnce(&mut Graph) -> R) -> Option<R> {
    GRAPH.try_with(|g| f(&mut g.borrow_mut())).ok()
}

impl Graph {
    fn slot(&self, e: Edge) -> Option<usize> {
        (self.generation.get(e.index as usize) == Some(&e.generation)).then_some(e.index as usize)
    }

    /// Mints a node, registering it with the scope in force.
    fn mint(&mut self, role: u8, value: Option<Rc<dyn Any>>, work: Option<Work>) -> Edge {
        let order = self.next_order;
        self.next_order += 1;
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.generation.push(1);
                self.flags.push(0);
                self.order.push(0);
                self.version.push(0);
                self.deps.push(Vec::new());
                self.subs.push(Vec::new());
                self.value.push(None);
                self.work.push(None);
                self.generation.len() as u32 - 1
            }
        };
        let i = index as usize;
        // Minted `Dirty`: a memo's first read is what runs it, and `resolve` recomputes only
        // what is marked.
        self.flags[i] = role | DIRTY;
        self.order[i] = order;
        self.version[i] = 0;
        self.value[i] = value;
        self.work[i] = work;
        if role & ROLE != SCOPE {
            self.live += 1;
        }
        let e = Edge { index, generation: self.generation[i] };
        if let Some(scope) = self.scope.and_then(|s| self.slot(s)) {
            // A replaced binding retires its node before its scope ends, so reclaim the dead
            // entries before growing: lifetime storage stays bounded by live work.
            if self.subs[scope].len() == self.subs[scope].capacity() {
                let mut owned = core::mem::take(&mut self.subs[scope]);
                owned.retain(|&child| self.slot(child).is_some());
                self.subs[scope] = owned;
            }
            self.subs[scope].push(e);
        }
        e
    }

    /// Records that the current observer read `dep`.
    fn record(&mut self, dep: Edge) {
        let Some(obs) = self.observer else { return };
        // A node reads the same dependency more than once per recompute all the time
        // (`a.get() + a.get()`); the edge is a set, and `deps` is short enough that a scan
        // beats a hash.
        let deps = &mut self.deps[obs.index as usize];
        if deps.contains(&dep) {
            return;
        }
        deps.push(dep);
        self.subs[dep.index as usize].push(obs);
    }

    /// Marks everything downstream of `e`: its direct subscribers `Dirty`, everything below
    /// them `Check`, which is what makes a diamond's shared node evaluate once.
    ///
    /// The frontier walk uses the pooled stack, so a write of any fan-out allocates nothing.
    fn mark(&mut self, e: Edge) {
        let mut stack = core::mem::take(&mut self.stack);
        let mut from = e;
        let mut state = DIRTY;
        loop {
            for i in 0..self.subs[from.index as usize].len() {
                // Re-indexed each step rather than held: `raise` never touches a subscriber
                // list, so the position stays valid across the call.
                let sub = self.subs[from.index as usize][i];
                self.raise(sub, state, &mut stack);
            }
            state = CHECK;
            match stack.pop() {
                Some(next) => from = next,
                None => break,
            }
        }
        self.stack = stack;
    }

    /// Raises `e` to `state`, queues it if it is an effect, and puts it on the frontier the
    /// first time it is raised at all.
    fn raise(&mut self, e: Edge, state: u8, stack: &mut Vec<Edge>) {
        let Some(i) = self.slot(e) else { return };
        let was = self.flags[i];
        if was & STATE >= state {
            return;
        }
        self.flags[i] = (was & !STATE) | state;
        if was & ROLE == EFFECT && was & QUEUED == 0 {
            self.flags[i] |= QUEUED;
            self.queue[usize::from(was & GEOMETRY != 0)].push(e);
        }
        // Only a first raise propagates: a node already `Check` has already pushed `Check`
        // through everything below it, and promoting it to `Dirty` changes nothing there.
        if was & STATE == CLEAN {
            stack.push(e);
        }
    }

    /// Clears `e`'s mark and answers whether it was `Dirty`.
    fn take_dirty(&mut self, e: Edge) -> bool {
        let Some(i) = self.slot(e) else { return false };
        let was = self.flags[i];
        self.flags[i] = was & !STATE;
        was & STATE == DIRTY
    }

    /// Drops every edge between `e` and what it reads, so a recompute collects a fresh set.
    ///
    /// A node that stops reading a source stops being woken by it, so a hidden branch costs
    /// nothing once its arm has stopped reading.
    fn clear_deps(&mut self, e: Edge) {
        // Taken rather than drained in place, because the loop borrows the columns again.
        // The capacity goes back at the end.
        let mut deps = core::mem::take(&mut self.deps[e.index as usize]);
        for dep in deps.drain(..) {
            if let Some(i) = self.slot(dep) {
                self.subs[i].retain(|&s| s != e);
            }
        }
        self.deps[e.index as usize] = deps;
    }

    /// Disposes `e`, and everything created under it in reverse creation order where it is a
    /// scope, pushing each payload onto `out` for the caller to drop outside the borrow.
    fn release(&mut self, e: Edge, out: &mut Vec<(Option<Rc<dyn Any>>, Option<Work>)>) {
        let Some(i) = self.slot(e) else { return };
        self.generation[i] = self.generation[i].wrapping_add(1);
        let scope = self.flags[i] & ROLE == SCOPE;
        if !scope {
            self.live -= 1;
        }
        self.clear_deps(e);
        let mut owned = core::mem::take(&mut self.subs[i]);
        if scope {
            // Reverse creation order: a child's effect that reads its parent's cell is torn
            // down before the cell is.
            while let Some(child) = owned.pop() {
                self.release(child, out);
            }
        }
        // A subscriber that outlives its source is legal: it is never woken again, and its
        // stale edge is pruned by the generation check the next time it is walked.
        owned.clear();
        self.subs[i] = owned;
        out.push((self.value[i].take(), self.work[i].take()));
        // Parked: holds nothing, and keeps its edge capacity for the next mint.
        self.flags[i] = 0;
        self.free.push(e.index);
    }
}

/// Restores a graph slot on the way out, including where application code panicked.
struct Restore(fn(&mut Graph) -> &mut Option<Edge>, Option<Edge>);

impl Restore {
    fn set(slot: fn(&mut Graph) -> &mut Option<Edge>, to: Option<Edge>) -> Self {
        Self(slot, with(|g| core::mem::replace(slot(g), to)))
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        let prev = self.1;
        try_with(|g| *self.0(g) = prev);
    }
}

/// Mints a node of `role`, registering it with the scope in force.
pub(crate) fn mint(role: u8, value: Option<Rc<dyn Any>>, work: Option<Work>) -> SignalId {
    super::assert_writable();
    with(|g| {
        assert!(role & ROLE != RESOURCE || g.scope.is_some(), "retain outside a scope");
        SignalId { graph: g.id, edge: g.mint(role, value, work) }
    })
}

/// Runs `f` with the graph borrowed, and asks the waker where `f` left update work behind
/// where there was none. The waker runs with no borrow held, so it may write signals.
fn writing<R>(f: impl FnOnce(&mut Graph) -> R) -> R {
    let (r, asks) = with(|g| {
        let idle = g.queue[UPDATE].is_empty() && !g.flushing;
        let r = f(g);
        (r, idle && !g.queue[UPDATE].is_empty())
    });
    if asks {
        wake();
    }
    r
}

/// Returns a node's payload, recording the read against the current observer.
///
/// `None` where the node was disposed: the generation check makes a stale handle read nothing
/// rather than whatever now occupies its slot.
pub(crate) fn read(id: SignalId) -> Option<Rc<dyn Any>> {
    with(|g| {
        let i = g.slot(id.edge)?;
        g.record(id.edge);
        g.value[i].clone()
    })
}

/// Returns a node's payload without recording a dependency.
pub(crate) fn peek(id: SignalId) -> Option<Rc<dyn Any>> {
    with(|g| g.slot(id.edge).and_then(|i| g.value[i].clone()))
}

/// Bumps `id`'s version and marks everything downstream of it. The push half of propagation,
/// and the only place a version moves.
pub(crate) fn bump(id: SignalId) {
    writing(|g| {
        if let Some(i) = g.slot(id.edge) {
            g.version[i] += 1;
            g.mark(id.edge);
        }
    });
}

/// Queues `id` to run at its phase's next drain, whatever it last read.
pub(crate) fn schedule(id: SignalId) {
    writing(|g| {
        let Some(i) = g.slot(id.edge) else { return };
        if g.flags[i] & QUEUED != 0 {
            return;
        }
        g.flags[i] = (g.flags[i] & !STATE) | QUEUED | DIRTY;
        g.queue[usize::from(g.flags[i] & GEOMETRY != 0)].push(id.edge);
    });
}

/// Returns how many times `id`'s value has moved. Zero for a node that is gone.
pub(crate) fn version(id: SignalId) -> u64 {
    with(|g| g.slot(id.edge).map_or(0, |i| g.version[i]))
}

/// Returns whether this thread's graph holds `id` and it is live.
pub(crate) fn alive(id: SignalId) -> bool {
    try_with(|g| g.id == id.graph && g.slot(id.edge).is_some()).unwrap_or(false)
}

/// Returns whether this thread's graph minted `id`, which separates the owning thread's
/// direct write from a producer's staged one.
pub(crate) fn owns(id: SignalId) -> bool {
    try_with(|g| g.id == id.graph).unwrap_or(false)
}

/// Returns how many signal nodes are live on this thread.
///
/// A disposal scope is a node and is not counted: this is the instrument leak assertions
/// read, and what they assert is that mounting and unmounting a screen returns the number of
/// cells, memos, effects and resources to its baseline.
pub fn live_nodes() -> usize {
    with(|g| g.live as usize)
}

/// Brings `id` up to date if anything it reads changed. The pull half of propagation.
pub(crate) fn resolve(id: SignalId) {
    let mut n = 0;
    // A `Check` node recomputes only where a dependency actually changed: resolving one may
    // promote this node to `Dirty`, and nothing else does, so the state is re-read each round.
    while let Some(dep) = with(|g| match g.slot(id.edge).map(|i| g.flags[i] & STATE) {
        Some(CHECK) => g.deps[id.edge.index as usize].get(n).copied(),
        _ => None,
    }) {
        resolve(SignalId { graph: id.graph, edge: dep });
        n += 1;
    }
    if with(|g| g.take_dirty(id.edge)) {
        recompute(id);
    }
}

/// Runs `id`'s work with no borrow of the graph held, and propagates where it moved.
pub(crate) fn recompute(id: SignalId) {
    let Some(work) = with(|g| {
        let i = g.slot(id.edge)?;
        // Cleared here as well as in `resolve`, so the one caller that runs an effect the
        // moment it is created leaves it `Clean` and a later write can raise it again.
        g.flags[i] &= !STATE;
        g.clear_deps(id.edge);
        g.work[i].clone()
    }) else {
        return;
    };
    let moved = {
        let _tracking = Restore::set(|g| &mut g.observer, Some(id.edge));
        let mut work = work.borrow_mut();
        work()
    };
    if moved {
        // Where the value did not move, subscribers stay `Check` and are not cleared.
        // Clearing them is unsound in the shape two-level marking exists for: in a diamond
        // whose two branches resolve in sequence, clearing drops the first branch's `Dirty`
        // and the shared node answers from a stale cache.
        bump(id);
    }
}

/// Runs `f` with no observer installed, so nothing `f` reads is subscribed to.
///
/// The read this exists for is one taken while building, inside an effect that reconciles
/// structure: without it a row's own bound value is recorded as a dependency of the list's
/// reconcile effect, and changing one label rebuilds the list.
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
    let _tracking = Restore::set(|g| &mut g.observer, None);
    f()
}

/// Runs `f` with `scope` installed as the scope new nodes register with.
pub(crate) fn in_scope<R>(scope: Option<SignalId>, f: impl FnOnce() -> R) -> R {
    let _scope = Restore::set(|g| &mut g.scope, scope.map(|s| s.edge));
    f()
}

/// Disposes a scope and everything created under it, in reverse creation order.
pub(crate) fn dispose(id: SignalId) {
    // The payloads outlive the borrow: one may own an `Owner`, and dropping it asks the graph
    // to dispose a scope.
    let mut out = Vec::new();
    try_with(|g| g.release(id.edge, &mut out));
}

/// Disposes one node and returns its work for the caller to drop outside its own borrow.
pub(crate) fn retire(id: SignalId) -> Option<RetiredEffect> {
    let mut out = Vec::new();
    try_with(|g| g.release(id.edge, &mut out));
    out.pop().and_then(|(_, work)| work)
}

/// Installs the callback this graph invokes when its effect queue goes from empty to
/// non-empty.
///
/// Without a waker a write schedules nothing: a write marks nodes and queues effects, and
/// nothing downstream runs until a caller invokes [`flush`]. The callback runs on the
/// empty-to-non-empty transition and at no other time, so a burst of writes asks once. A write
/// made from inside a flush does not call it: that flush picks the work up on its next pass.
/// It runs with no borrow of the graph held, so it may write signals.
pub fn set_waker(f: impl Fn() + 'static) {
    WAKER.with(|w| *w.borrow_mut() = Some(Rc::new(f)));
}

/// Calls the waker, holding no borrow of either the graph or the waker slot while it runs.
fn wake() {
    let waker = WAKER.with(|w| w.borrow().clone());
    if let Some(waker) = waker {
        waker();
    }
}

/// Applies staged cross-thread writes and runs every marked update-phase effect, in creation
/// order.
///
/// Returns whether anything moved *or* a geometry effect is owed, since a caller that solves
/// on the answer has to solve before [`flush_geometry`] can report a settled box.
///
/// Effects run after the memos they read have resolved, so no effect observes a half-updated
/// graph. A pass allocates nothing: both queues are drained rather than dropped, the staging
/// buffer is swapped back, and the sort is in place. A call made while a flush is running
/// returns immediately, leaving the work to the running flush.
pub fn flush() -> bool {
    if with(|g| core::mem::replace(&mut g.flushing, true)) {
        return false;
    }
    // Both sides run: one producer batch is admitted per flush, and the effects it marks are
    // drained by the same call.
    let moved = apply_posts() | drain(UPDATE);
    with(|g| g.flushing = false);
    moved || with(|g| !g.queue[GEOM].is_empty())
}

/// Runs the geometry effects the update phase held back, once the solve has settled.
pub(crate) fn flush_geometry() {
    if with(|g| core::mem::replace(&mut g.flushing, true)) {
        return;
    }
    drain(GEOM);
    with(|g| g.flushing = false);
}

/// Drains one phase to a fixed point, and answers whether anything ran.
fn drain(phase: usize) -> bool {
    let mut moved = false;
    for _ in 0..MAX_PASSES {
        if !run_phase(phase) {
            return moved;
        }
        moved = true;
    }
    debug_assert!(
        with(|g| g.queue[phase].is_empty()),
        "signal flush did not settle in {MAX_PASSES} passes: an effect writes a cell it also \
         reads"
    );
    moved
}

/// Runs one pass of `phase`'s queue, in creation order.
fn run_phase(phase: usize) -> bool {
    let (graph, mut queued) = with(|g| {
        let mut queued = core::mem::take(&mut g.spare);
        core::mem::swap(&mut queued, &mut g.queue[phase]);
        // Creation order is the contract: a parent's effect writes the container a child's
        // effect fills. Sorting in place allocates nothing.
        queued.sort_unstable_by_key(|e| g.order[e.index as usize]);
        for &e in &queued {
            if let Some(i) = g.slot(e) {
                g.flags[i] &= !QUEUED;
            }
        }
        (g.id, queued)
    });
    let ran = !queued.is_empty();
    for edge in queued.drain(..) {
        // Resolved rather than run: an effect marked only `Check` re-asks its dependencies and
        // does not run if none of them moved, which is the memo's cutoff one level down.
        resolve(SignalId { graph, edge });
    }
    with(|g| g.spare = queued);
    ran
}

/// A staged write: puts its value into the cell's payload and returns whether it moved.
pub(crate) type Post = Box<dyn FnOnce(&dyn Any) -> bool + Send>;

/// How a staged write reaches the thread that owns the graph.
///
/// A graph flushed from a window's pump has no wait of its own, so the write asks the pacer
/// for a frame and the frame message carries the flush. A graph flushed from a loop parked on
/// a doorbell has no clock, so the write rings that doorbell instead.
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
    /// The doorbell has been rung. It holds the signal until the sleeper consumes it, so there
    /// is nothing to keep beyond the fact that the edge has been taken.
    Rung,
}

/// One graph's staged writes.
///
/// Indexing by graph and then by node index makes replacing a pending write O(1) with no hash
/// and no scan, and keeps two graphs from aliasing each other's staged writes: a node index is
/// unique only within the graph that minted it.
#[derive(Default)]
struct Inbox {
    how: Option<PostWake>,
    held: Option<Held>,
    /// The pending write per slot, with the generation it was staged against: a producer
    /// holding a stale handle must not replace a newer pending write at the same index.
    slots: Vec<Option<(u32, Post)>>,
    /// Which slots are occupied, so a drain costs O(pending) rather than O(cells).
    pending: Vec<u32>,
}

/// Every graph's staged writes, indexed by graph id.
static INBOX: Mutex<Vec<Inbox>> = Mutex::new(Vec::new());

/// Locks the inbox, recovering from poisoning.
///
/// A producer panicking mid-post leaves the table structurally sound: the slot it was writing
/// is either replaced or not, so a poisoned lock is taken rather than propagated.
fn inbox() -> MutexGuard<'static, Vec<Inbox>> {
    INBOX.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Holds a graph's post routing. Dropping it releases the routing and any wake it held.
pub(crate) struct PostGuard(u32);

impl Drop for PostGuard {
    fn drop(&mut self) {
        let mut inbox = inbox();
        let graph = &mut inbox[self.0 as usize];
        graph.how = None;
        graph.held = None;
    }
}

/// Routes producer writes to this graph's thread until the returned guard drops.
pub(crate) fn arm_posts(how: impl Into<PostWake>) -> PostGuard {
    let id = with(|g| g.id);
    let mut inbox = inbox();
    if inbox.len() <= id as usize {
        inbox.resize_with(id as usize + 1, Inbox::default);
    }
    let how = how.into();
    let graph = &mut inbox[id as usize];
    // Writes staged before the routing existed are still owed a wake, and the new routing is
    // the first thing that can deliver one.
    graph.held = (!graph.pending.is_empty()).then(|| raise(&how));
    graph.how = Some(how);
    PostGuard(id)
}

/// Stages a write and holds one wake until this graph drains it.
pub(crate) fn stage(id: SignalId, post: Post) {
    let (index, age) = (id.edge.index as usize, id.edge.generation);
    let mut inbox = inbox();
    // Grown rather than looked up: a write staged before this graph armed its routing is still
    // owed a wake, which `arm_posts` raises.
    if inbox.len() <= id.graph as usize {
        inbox.resize_with(id.graph as usize + 1, Inbox::default);
    }
    let graph = &mut inbox[id.graph as usize];
    if graph.slots.len() <= index {
        graph.slots.resize_with(index + 1, || None);
    }
    // A producer holding a stale handle must not replace a newer pending write at this index.
    if graph.slots[index].as_ref().is_some_and(|(pending, _)| *pending > age) {
        return;
    }
    if graph.slots[index].replace((age, post)).is_none() {
        graph.pending.push(id.edge.index);
    }
    // Raised on the empty-to-pending transition only, so a producer writing at any rate leaves
    // at most one signal in flight.
    if graph.held.is_none() {
        graph.held = graph.how.as_ref().map(raise);
    }
}

/// Raises one wake on the graph's thread.
///
/// The pacer arm returns the frame request, so the pending batch keeps it live. The ring arm
/// returns nothing to keep: a ring signals only the parked-to-running edge, so a second ring
/// while the sleeper is already running is not a second wake.
fn raise(how: &PostWake) -> Held {
    match how {
        PostWake::Pacer(wake) => Held::Tick(wake.tick()),
        PostWake::Ring(ring) => {
            ring.ring();
            Held::Rung
        }
    }
}

/// Returns whether the owning graph has staged producer writes to consume.
pub(crate) fn posts_pending() -> bool {
    let id = with(|g| g.id);
    inbox().get(id as usize).is_some_and(|graph| !graph.pending.is_empty())
}

/// Applies whatever producer threads staged, coalesced to at most one write per cell, and
/// answers whether anything was staged.
fn apply_posts() -> bool {
    let (id, mut staged) = with(|g| (g.id, core::mem::take(&mut g.inbox)));
    {
        let mut inbox = inbox();
        if let Some(graph) = inbox.get_mut(id as usize) {
            for index in graph.pending.drain(..) {
                if let Some((generation, post)) = graph.slots[index as usize].take() {
                    staged.push((Edge { index, generation }, post));
                }
            }
            graph.held = None;
        }
    }
    // The lock is released before any of it is applied, so a producer never waits on the
    // graph's own work.
    let any = !staged.is_empty();
    for (edge, post) in staged.drain(..) {
        let id = SignalId { graph: id, edge };
        // A write in flight when a screen unmounts reads nothing here and is dropped: the
        // generation check is the one every read already makes.
        let Some(value) = peek(id) else { continue };
        if post(&*value) {
            bump(id);
        }
    }
    // Back with its capacity, so a steady producer stages and drains without allocating.
    with(|g| g.inbox = staged);
    any
}
