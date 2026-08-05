//! The signal graph: one arena of nodes, two-level marking, and a flush that allocates
//! nothing.
//!
//! Every node belongs to the app thread. A node holds its dependency and subscriber lists
//! as `Vec`s that are cleared, never dropped, and a disposed node returns to a free list
//! with that capacity intact, so mounting and unmounting a screen repeatedly allocates on
//! the first cycle only.
//!
//! # Edges are generational indices
//!
//! An edge names a slot and the generation that minted the node in it, so no node holds an
//! `Rc` to another node and no cycle of nodes can leak. An edge whose target was disposed
//! fails the generation check and is pruned on the next traversal.
//!
//! # The graph is never borrowed across application code
//!
//! Every path that runs a closure — recomputing a memo, running an effect, reading a cell
//! through `with` — clones an `Rc` out and drops the borrow first. A memo's cell and an
//! effect's closure are `Rc` rather than `Box` for that reason, and it is what lets such a
//! closure read and even write other signals.

use super::shared;
use core::any::Any;
use core::cell::RefCell;
use std::rc::Rc;
use windows_scene::{Id, Ids, Slots};

/// How many passes a flush may take before it stops.
///
/// An [`Effect`](super::Effect) that writes a [`Cell`](super::Cell) adds a pass; one that
/// writes a cell it also reads never settles. A flush that has not settled after this many
/// passes trips a debug assertion and returns.
pub(super) const MAX_PASSES: u32 = 8;

/// A node's identity: which graph, a dense index into it, and the generation that minted
/// the node in that slot.
///
/// `Copy`, with no reference count, so a `move ||` closure captures one at no cost.
///
/// The graph is part of the identity rather than implied by the thread, because a
/// producer's staged write is looked up by id from a thread other than the one that minted
/// it, and an index is unique only within the graph that minted it. Without the graph
/// field, two graphs would alias each other's staged writes.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SignalId {
    pub(super) graph: u32,
    pub(super) id: Id<Signal>,
}

/// The family a signal node belongs to.
#[derive(Debug)]
pub struct Signal;

/// The family a disposal scope belongs to.
#[derive(Debug)]
pub struct Owner;

/// How far a node is from being up to date.
///
/// Two levels rather than one make a diamond evaluate its shared node once: `Check` says
/// only that a transitive dependency may have changed, and resolving it walks the
/// dependencies rather than recomputing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum State {
    Clean,
    Check,
    Dirty,
}

/// A memo's closure, cached value and equality comparison, behind a type the graph can
/// name.
///
/// The graph recomputes a memo, and a reader reaches its cache, without naming the closure
/// type. An implementation must write into the existing cache rather than allocating a new
/// one, so that a memo recomputing on every flush allocates nothing.
pub(super) trait MemoCell {
    /// Recomputes the value and returns whether it changed.
    fn recompute(&self) -> bool;
    /// Returns the cache, a `RefCell<Option<T>>` erased to `dyn Any`.
    fn value(&self) -> &dyn Any;
}

/// The three node kinds.
enum Kind {
    /// A cell. The payload is `RefCell<T>`, behind an `Rc` so a reader can take it out of
    /// the graph before running a closure over it.
    Source(Rc<dyn Any>),
    Memo(Rc<dyn MemoCell>),
    Effect(Rc<RefCell<dyn FnMut()>>),
}

struct Node {
    state: State,
    kind: Kind,
    /// Creation order. Effects run in it, so a parent's effect lands before its child's.
    order: u64,
    /// What this node read, most recently.
    deps: Vec<SignalId>,
    /// What read this node.
    subs: Vec<SignalId>,
    /// Already in the effect queue, so a burst of writes to one cell enqueues its effects
    /// once and no scan of the queue is needed.
    queued: bool,
    /// How many times this node's value has moved. Read by a consumer that wants change
    /// detection without the value.
    version: u64,
}

/// A disposal scope's identity.
pub(super) type OwnerId = Id<Owner>;

/// What a scope disposes, in reverse creation order.
#[derive(Copy, Clone)]
enum Child {
    Signal(SignalId),
    Owner(OwnerId),
}

#[derive(Default)]
struct OwnerNode {
    children: Vec<Child>,
}

/// The signal runtime. One per thread that builds signals.
struct Graph {
    /// This graph's process-unique id, stamped into every [`SignalId`] it mints.
    id: u32,
    node_ids: Ids<Signal>,
    nodes: Slots<Signal, Node>,
    /// Disposed nodes, parked for their edge buffers.
    ///
    /// A recycled node keeps its `deps` and `subs` capacity, so the second mount of a
    /// screen allocates no edge storage.
    spare: Vec<Node>,
    owner_ids: Ids<Owner>,
    owners: Slots<Owner, OwnerNode>,
    owner_spare: Vec<OwnerNode>,
    /// The node currently collecting dependencies, if any.
    observer: Option<SignalId>,
    /// The scope new nodes register with, if any.
    scope: Option<OwnerId>,
    order: u64,
    /// Effects marked since the last pass.
    queue: Vec<SignalId>,
    /// The pass being drained. Separate from `queue`, so a write from inside an effect
    /// appends to the *next* pass rather than to the one in progress.
    running: Vec<SignalId>,
    /// The `Check` propagation frontier. Pooled: marking allocates nothing.
    stack: Vec<SignalId>,
    /// Cross-thread writes, held between drains so the buffer keeps its capacity.
    staged: Vec<(SignalId, shared::Apply)>,
    flushing: bool,
}

/// Hands each graph the process-unique id it stamps into every [`SignalId`].
static NEXT_GRAPH: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

thread_local! {
    static GRAPH: RefCell<Graph> = RefCell::new(Graph {
        // Relaxed: nothing is published through this counter, and the atomic
        // read-modify-write alone is what makes every returned id distinct.
        id: NEXT_GRAPH.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
        node_ids: Ids::new(),
        nodes: Slots::new(),
        spare: Vec::new(),
        owner_ids: Ids::new(),
        owners: Slots::new(),
        owner_spare: Vec::new(),
        observer: None,
        scope: None,
        order: 0,
        queue: Vec::new(),
        running: Vec::new(),
        stack: Vec::new(),
        staged: Vec::new(),
        flushing: false,
    });
}

/// Runs `f` with the graph borrowed. `f` must not call application code, which may re-enter
/// and borrow the graph again.
fn with<R>(f: impl FnOnce(&mut Graph) -> R) -> R {
    GRAPH.with(|g| f(&mut g.borrow_mut()))
}

thread_local! {
    /// What to call when this graph acquires work; installed by [`set_waker`].
    ///
    /// Held outside [`Graph`] so it can be called with no graph borrow held: a waker is
    /// host code and may re-enter the graph.
    static WAKER: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Installs the callback this graph invokes when its effect queue goes from empty to
/// non-empty.
///
/// Without a waker a write schedules nothing: [`Cell::set`](super::Cell::set) marks nodes
/// and queues effects, and nothing downstream runs until a caller invokes [`flush`]. A host
/// that blocks its loop between frames learns through this callback that a frame is owed.
/// [`written`](super::written) is the matching signal for a producer thread's write, and
/// the two are separate because that one wakes a thread and this one asks for a frame.
///
/// The callback runs on the empty-to-non-empty transition and at no other time, so a burst
/// of writes asks once. A write made from inside a flush does not call it: that flush picks
/// the work up on its next pass. It runs with no borrow of the graph held, so it may write
/// signals.
pub fn set_waker(f: impl Fn() + 'static) {
    WAKER.with(|w| *w.borrow_mut() = Some(Box::new(f)));
}

/// Calls the waker, holding no borrow of either the graph or the waker slot while it runs.
fn wake() {
    let waker = WAKER.with(|w| w.borrow_mut().take());
    if let Some(waker) = waker {
        waker();
        WAKER.with(|w| {
            let mut slot = w.borrow_mut();
            // A waker that installed a new one during the call keeps it: this is putting the
            // borrowed one back, not overwriting whatever is there now.
            if slot.is_none() {
                *slot = Some(waker);
            }
        });
    }
}

/// Runs `f` with the graph borrowed, answering `None` where the graph is not reachable.
///
/// The graph is unreachable while the thread's locals are being destroyed and its own node
/// storage is dropping. A node can hold an [`Owner`](super::Owner) — a `Branch`'s arm and a
/// `Keyed`'s row both do — so dropping one asks the graph to dispose a scope from inside
/// the graph's own destructor. `with` panics there, inside a `Drop`, which aborts the
/// process; every path reached from a `Drop` uses this instead.
fn try_with<R>(f: impl FnOnce(&mut Graph) -> R) -> Option<R> {
    GRAPH.try_with(|g| f(&mut g.borrow_mut())).ok()
}

impl Graph {
    fn node(&self, id: SignalId) -> Option<&Node> {
        if id.graph != self.id {
            return None;
        }
        self.nodes.get(id.id)
    }

    fn node_mut(&mut self, id: SignalId) -> Option<&mut Node> {
        if id.graph != self.id {
            return None;
        }
        self.nodes.get_mut(id.id)
    }

    fn mint(&mut self, kind: Kind) -> SignalId {
        self.order += 1;
        let order = self.order;
        // A parked node keeps its `deps` and `subs` capacity, which is what makes the second
        // mount of a screen allocation-free.
        let node = match self.spare.pop() {
            Some(mut node) => {
                node.state = State::Dirty;
                node.kind = kind;
                node.order = order;
                node.queued = false;
                node.version = 0;
                node
            }
            None => Node {
                state: State::Dirty,
                kind,
                order,
                deps: Vec::new(),
                subs: Vec::new(),
                queued: false,
                version: 0,
            },
        };
        let id = SignalId {
            graph: self.id,
            id: self.nodes.insert(&mut self.node_ids, node),
        };
        if let Some(scope) = self.scope {
            self.attach(scope, Child::Signal(id));
        }
        id
    }

    fn attach(&mut self, scope: OwnerId, child: Child) {
        if let Some(owner) = self.owners.get_mut(scope) {
            owner.children.push(child);
        }
    }

    /// Records that the current observer read `dep`, if a read is being tracked.
    fn track(&mut self, dep: SignalId) {
        let Some(observer) = self.observer else {
            return;
        };
        // A node reads the same dependency more than once per recompute all the time
        // (`a.get() + a.get()`); the edge is a set, and `deps` is short enough that a scan
        // beats a hash.
        let fresh = self.node_mut(observer).is_some_and(|node| {
            let fresh = !node.deps.contains(&dep);
            if fresh {
                node.deps.push(dep);
            }
            fresh
        });
        if fresh && let Some(source) = self.node_mut(dep) {
            source.subs.push(observer);
        }
    }

    /// Marks everything downstream of `id`.
    ///
    /// Direct subscribers become `Dirty`; everything below them becomes `Check`. The
    /// frontier walk uses the pooled `stack`, so a write of any fan-out allocates nothing.
    fn invalidate(&mut self, id: SignalId) {
        self.push_subs(id, State::Dirty);
        while let Some(next) = self.stack.pop() {
            self.push_subs(next, State::Check);
        }
    }

    fn push_subs(&mut self, id: SignalId, state: State) {
        let len = self.node(id).map_or(0, |node| node.subs.len());
        for i in 0..len {
            // Re-resolved each step rather than indexed once: `set_state` never touches a
            // subscriber list, so the position stays valid, and reaching the node through
            // its id keeps every read on the generation-checked path.
            let Some(sub) = self.node(id).and_then(|node| node.subs.get(i).copied()) else {
                break;
            };
            self.set_state(sub, state);
        }
    }

    /// Raises `id` to `state`, queues it if it is an effect, and puts it on the frontier
    /// the first time it is raised at all.
    fn set_state(&mut self, id: SignalId, state: State) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let was = node.state;
        if was == State::Dirty || (was == State::Check && state == State::Check) {
            return;
        }
        node.state = state;
        let queue = !node.queued && matches!(node.kind, Kind::Effect(_));
        if queue {
            node.queued = true;
            self.queue.push(id);
        }
        // Only a first raise propagates: a node already `Check` has already pushed `Check`
        // through everything below it, and promoting it to `Dirty` changes nothing there.
        if was == State::Clean {
            self.stack.push(id);
        }
    }
}

// ── minting ─────────────────────────────────────────────────────────────────────

pub(super) fn source(value: Rc<dyn Any>) -> SignalId {
    with(|g| g.mint(Kind::Source(value)))
}

pub(super) fn memo(cell: Rc<dyn MemoCell>) -> SignalId {
    // Nothing runs here: a memo is lazy, and one that is never read must never compute.
    with(|g| g.mint(Kind::Memo(cell)))
}

pub(super) fn effect(f: Rc<RefCell<dyn FnMut()>>) -> SignalId {
    let id = with(|g| g.mint(Kind::Effect(Rc::clone(&f))));
    // The first run is what collects the dependency set; an effect that never ran would
    // never be woken.
    run_effect(id, &f);
    id
}

// ── reading ─────────────────────────────────────────────────────────────────────

/// Returns a cell's payload, recording the read against the current observer.
///
/// `None` where the cell was disposed: the generation check makes a stale handle read
/// nothing rather than whatever now occupies its slot.
pub(super) fn read_source(id: SignalId) -> Option<Rc<dyn Any>> {
    with(|g| {
        g.track(id);
        match g.node(id).map(|node| &node.kind) {
            Some(Kind::Source(value)) => Some(Rc::clone(value)),
            _ => None,
        }
    })
}

/// Returns a cell's payload without recording a dependency.
pub(super) fn peek_source(id: SignalId) -> Option<Rc<dyn Any>> {
    with(|g| match g.node(id).map(|node| &node.kind) {
        Some(Kind::Source(value)) => Some(Rc::clone(value)),
        _ => None,
    })
}

/// Returns a memo's cache, resolving the memo first and recording the read.
pub(super) fn read_memo(id: SignalId) -> Option<Rc<dyn MemoCell>> {
    let cell = with(|g| {
        g.track(id);
        match g.node(id).map(|node| &node.kind) {
            Some(Kind::Memo(cell)) => Some(Rc::clone(cell)),
            _ => None,
        }
    })?;
    resolve(id);
    Some(cell)
}

/// Brings `id` up to date if anything it reads changed. The pull half of propagation.
fn resolve(id: SignalId) {
    match state_of(id) {
        None | Some(State::Clean) => {}
        Some(State::Dirty) => recompute(id),
        Some(State::Check) => {
            // Walk by index and re-read each step: resolving a dependency can promote this
            // node to `Dirty`, at which point the rest are irrelevant.
            let mut i = 0;
            while let Some(dep) = with(|g| g.node(id).and_then(|node| node.deps.get(i).copied())) {
                resolve(dep);
                if state_of(id) == Some(State::Dirty) {
                    break;
                }
                i += 1;
            }
            if state_of(id) == Some(State::Dirty) {
                recompute(id);
            } else {
                mark_clean(id);
            }
        }
    }
}

/// Runs `f` with no observer installed, so nothing `f` reads is subscribed to.
///
/// The read this exists for is one taken while building, inside an effect that reconciles
/// structure: without it a row's own bound value is recorded as a dependency of the list's
/// reconcile effect, and changing one label rebuilds the list.
///
/// The previous observer is restored even if `f` panics.
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
    // A guard rather than a line after the call: `f` is application code, and a panic that
    // skipped the restore would leave the graph with no observer for the life of the
    // thread, so every effect created afterwards would subscribe to nothing.
    struct Restore(Option<SignalId>);
    impl Drop for Restore {
        fn drop(&mut self) {
            try_with(|g| g.observer = self.0.take());
        }
    }
    let _restore = Restore(with(|g| g.observer.take()));
    f()
}

fn recompute(id: SignalId) {
    let Some(cell) = with(|g| match g.node(id).map(|node| &node.kind) {
        Some(Kind::Memo(cell)) => Some(Rc::clone(cell)),
        _ => None,
    }) else {
        mark_clean(id);
        return;
    };

    let outer = with(|g| {
        clear_deps(g, id);
        g.observer.replace(id)
    });
    // Application code, with the graph free.
    let changed = cell.recompute();

    with(|g| {
        g.observer = outer;
        if let Some(node) = g.node_mut(id) {
            node.state = State::Clean;
        }
        if changed {
            // Only the direct subscribers are promoted. Everything below them is already
            // `Check` from the write that started this, and each promotes its own
            // subscribers if its value moves.
            //
            // Where the value did not change, subscribers stay `Check` and are not marked
            // `Clean`. Clearing them is unsound in the shape two-level marking exists for:
            // in a diamond whose two branches resolve in sequence, clearing drops the first
            // branch's `Dirty` and the shared node answers from a stale cache. A `Check`
            // node re-asks its dependencies before believing its cache.
            g.push_subs(id, State::Dirty);
            g.stack.clear();
        }
    });
}

fn state_of(id: SignalId) -> Option<State> {
    with(|g| g.node(id).map(|node| node.state))
}

fn mark_clean(id: SignalId) {
    with(|g| {
        if let Some(node) = g.node_mut(id) {
            node.state = State::Clean;
        }
    });
}

/// Drops every edge between `id` and its dependencies, so a recompute collects a fresh set.
///
/// A node that stops reading a source stops being woken by it, so a hidden branch costs
/// nothing once its arm has stopped reading.
fn clear_deps(g: &mut Graph, id: SignalId) {
    let Some(node) = g.node_mut(id) else {
        return;
    };
    // Taken rather than drained, because the loop borrows the graph again. The capacity
    // goes back at the end.
    let mut deps = core::mem::take(&mut node.deps);
    for dep in deps.drain(..) {
        if let Some(source) = g.node_mut(dep)
            && let Some(at) = source.subs.iter().position(|sub| *sub == id)
        {
            source.subs.swap_remove(at);
        }
    }
    if let Some(node) = g.node_mut(id) {
        node.deps = deps;
    }
}

// ── writing ─────────────────────────────────────────────────────────────────────

/// Bumps `id`'s version and marks everything downstream of it. The push half of
/// propagation, and the only place a version moves.
pub(super) fn invalidate(id: SignalId) {
    let acquired = with(|g| {
        if let Some(node) = g.node_mut(id) {
            node.version = node.version.wrapping_add(1);
        }
        // Read before the marking, so the comparison afterwards reflects this write alone.
        let idle = !g.flushing && g.queue.is_empty();
        g.invalidate(id);
        g.stack.clear();
        idle && !g.queue.is_empty()
    });
    // Outside the borrow: a waker is host code and may do anything, including write a signal.
    if acquired {
        wake();
    }
}

/// Returns how many times `id`'s value has moved. Zero for a node that is gone.
pub(super) fn version(id: SignalId) -> u64 {
    with(|g| g.node(id).map_or(0, |node| node.version))
}

/// Returns whether this thread's graph holds `id`, which separates the owning thread's
/// direct write from a producer's staged one.
pub(super) fn owns(id: SignalId) -> bool {
    GRAPH.with(|g| g.try_borrow().is_ok_and(|g| g.node(id).is_some()))
}

// ── the flush ───────────────────────────────────────────────────────────────────

fn run_effect(id: SignalId, f: &Rc<RefCell<dyn FnMut()>>) {
    let outer = with(|g| {
        clear_deps(g, id);
        g.observer.replace(id)
    });
    // Application code, with the graph free: an effect may read and even write signals.
    (f.borrow_mut())();
    with(|g| {
        g.observer = outer;
        if let Some(node) = g.node_mut(id) {
            node.state = State::Clean;
            node.queued = false;
        }
    });
}

/// Applies staged cross-thread writes, resolves every marked memo and runs every marked
/// effect, in creation order.
///
/// Effects run after memos, so no effect observes a half-updated graph. A pass allocates
/// nothing: both queues are drained rather than dropped, the staging buffer is swapped
/// back, and the sort is in place.
///
/// A call made while a flush is running returns immediately, leaving the work to the
/// running flush.
pub fn flush() {
    if with(|g| core::mem::replace(&mut g.flushing, true)) {
        // A write from inside an effect appends to the queue the running flush picks up on
        // its next pass. Starting a second flush here would run effects out of creation
        // order.
        return;
    }

    for pass in 0..MAX_PASSES {
        apply_staged();

        let empty = with(|g| {
            debug_assert!(g.running.is_empty());
            core::mem::swap(&mut g.running, &mut g.queue);
            // Creation order is the contract: a parent's effect writes the container a
            // child's effect fills. Sorting in place allocates nothing.
            let nodes = &g.nodes;
            g.running
                .sort_unstable_by_key(|id| nodes.get(id.id).map_or(u64::MAX, |node| node.order));
            g.running.is_empty()
        });
        if empty {
            break;
        }

        let mut i = 0;
        while let Some(id) = with(|g| g.running.get(i).copied()) {
            i += 1;
            let Some(f) = with(|g| match g.node(id).map(|node| &node.kind) {
                Some(Kind::Effect(f)) => Some(Rc::clone(f)),
                // Disposed between being marked and being run, which is legal: an effect
                // in one scope may dispose another.
                _ => None,
            }) else {
                continue;
            };

            // An effect marked only `Check` re-asks its dependencies, and does not run if
            // none of them moved. The value-equality cutoff, one level below a memo.
            if state_of(id) == Some(State::Check) {
                let mut d = 0;
                while let Some(dep) = with(|g| g.node(id).and_then(|n| n.deps.get(d).copied())) {
                    resolve(dep);
                    if state_of(id) == Some(State::Dirty) {
                        break;
                    }
                    d += 1;
                }
            }
            if state_of(id) == Some(State::Dirty) {
                run_effect(id, &f);
            } else {
                with(|g| {
                    if let Some(node) = g.node_mut(id) {
                        node.state = State::Clean;
                        node.queued = false;
                    }
                });
            }
        }
        with(|g| g.running.clear());

        debug_assert!(
            pass + 1 < MAX_PASSES || with(|g| g.queue.is_empty()),
            "signal flush did not settle in {MAX_PASSES} passes: an effect writes a cell it \
             also reads"
        );
    }

    with(|g| g.flushing = false);
}

/// Applies whatever producer threads staged, coalesced to at most one write per cell.
fn apply_staged() {
    let (graph, mut staged) = with(|g| (g.id, core::mem::take(&mut g.staged)));
    shared::take(graph, &mut staged);
    for (id, apply) in staged.drain(..) {
        let Some(value) = peek_source(id) else {
            continue;
        };
        // The write itself gates on equality, so a producer republishing an unchanged
        // value costs a lock and nothing else.
        if apply(&*value) {
            invalidate(id);
        }
    }
    // Back with its capacity, so a steady producer stages and drains without allocating.
    with(|g| g.staged = staged);
}

// ── scopes ──────────────────────────────────────────────────────────────────────

pub(super) fn open_scope() -> OwnerId {
    with(|g| {
        // A parked scope keeps its child list's capacity, which is what makes remounting a
        // screen free.
        let owner = g.owner_spare.pop().unwrap_or_default();
        let id = g.owners.insert(&mut g.owner_ids, owner);
        // A scope opened inside another is disposed by it, so a subtree's scopes need no
        // separate bookkeeping and no parent walk.
        if let Some(parent) = g.scope {
            g.attach(parent, Child::Owner(id));
        }
        id
    })
}

/// Installs `id` as the scope new nodes register with, and returns the previous one.
pub(super) fn enter_scope(id: Option<OwnerId>) -> Option<OwnerId> {
    with(|g| core::mem::replace(&mut g.scope, id))
}

/// Disposes a scope and everything created under it, in reverse creation order.
pub(super) fn dispose_scope(id: OwnerId) {
    // Fallible because this runs from a `Drop`, as `try_with` describes. A graph already
    // being destroyed disposes this scope by dropping it, so the walk below has nothing
    // left to do.
    let Some(Some(mut children)) = try_with(|g| {
        let owner = g.owners.get_mut(id)?;
        Some(core::mem::take(&mut owner.children))
    }) else {
        return;
    };

    // Reverse creation order: a child's effect that reads its parent's cell is torn down
    // before the cell is.
    while let Some(child) = children.pop() {
        match child {
            Child::Signal(signal) => dispose(signal),
            Child::Owner(owner) => dispose_scope(owner),
        }
    }

    try_with(|g| {
        if let Some(mut owner) = g.owners.remove(&mut g.owner_ids, id) {
            // The `Vec` goes back with its capacity, which is what makes remounting free.
            owner.children = children;
            g.owner_spare.push(owner);
        }
    });
}

/// Disposes one node: drops its outgoing edges, releases its payload, and frees its slot.
pub(super) fn dispose(id: SignalId) {
    // Fallible for the same reason `dispose_scope` is: this is reached from a `Drop`.
    try_with(|g| {
        if g.node(id).is_none() {
            return;
        }
        clear_deps(g, id);
        shared::release(id);
        let Some(mut node) = g.nodes.remove(&mut g.node_ids, id.id) else {
            return;
        };
        // A subscriber that outlives its source is legal: it is never woken again, and its
        // stale edge is pruned by the generation check the next time it is walked. What
        // must not survive is this node's entry in anyone else's subscriber list, which
        // `clear_deps` above removes.
        node.subs.clear();
        node.state = State::Clean;
        node.queued = false;
        // Parked with its buffers. Vacancy is the store's fact, so a node needs no `Free`
        // variant of its own.
        g.spare.push(node);
    });
}

/// Returns how many signal nodes are live on this thread.
///
/// The instrument leak assertions read: mounting and unmounting a screen returns this count
/// to its baseline.
#[must_use]
pub fn live_nodes() -> usize {
    with(|g| g.nodes.len())
}
