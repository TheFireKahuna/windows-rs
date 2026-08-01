//! The graph: one arena, two-level marking, and a flush that allocates nothing.
//!
//! Everything here is the app thread's. A node holds its dependency and subscriber lists
//! as `Vec`s that are **cleared, never dropped** — and a disposed node returns to a free
//! list with that capacity intact, so mounting and unmounting a screen repeatedly
//! allocates on the first cycle and never again.
//!
//! # Why an arena, and not `Rc<RefCell<Node>>`
//!
//! Edges are generational indices. A node whose target was disposed is detected by a
//! generation mismatch and pruned on the next traversal, so there is **no `Rc` cycle to
//! leak** — the usual failure of a signal library in a language without a garbage
//! collector.
//!
//! # The one rule every function here obeys
//!
//! **The graph is never borrowed across application code.** Every path that must run a
//! closure — recomputing a memo, running an effect, reading a cell through `with` — clones
//! an `Rc` out and drops the borrow first. That is why a memo's cell and an effect's
//! closure are `Rc` rather than `Box`, and it is what lets a closure read and even write
//! other signals.

use super::shared;
use core::any::Any;
use core::cell::RefCell;
use core::num::NonZeroU32;
use std::rc::Rc;

/// How many times a flush may re-enter before it is treated as a cycle.
///
/// An `Effect` that writes a `Cell` legitimately re-enters; one that writes a `Cell` it
/// also reads does not terminate. Eight passes is far above any real fixpoint and far
/// below a hang.
pub(super) const MAX_PASSES: u32 = 8;

/// A node's identity: which graph, a dense index into it, and the generation that minted
/// the node in that slot.
///
/// `Copy` and twelve bytes, which is what makes `move ||` captures free — there is no `Rc`
/// to clone at a binding site.
///
/// The graph is part of the identity rather than implied by the thread, because a
/// producer's staged write is looked up by id on a thread that is not the one that minted
/// it. Without it, an index is unique only *within* a graph, and two graphs would alias
/// each other's staged writes — reading as a write that silently vanished.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SignalId {
    pub(super) graph: u32,
    pub(super) index: u32,
    pub(super) generation: NonZeroU32,
}

/// Where a node sits between "a source it reads changed" and "it has caught up".
///
/// Two levels rather than one is what makes a diamond evaluate its shared node **once**:
/// `Check` says only *a transitive dependency may have changed*, and resolving it walks
/// the dependencies rather than recomputing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum State {
    Clean,
    Check,
    Dirty,
}

/// What a memo owns: its closure, its cached value, and the comparison that stops
/// propagation.
///
/// The trait exists so the graph can recompute a memo without naming its closure type and
/// a reader can reach the cache without naming it either. Recomputing writes **into** the
/// existing cache rather than producing a new box, so a memo that recomputes on every
/// flush still allocates nothing.
pub(super) trait MemoCell {
    /// Recomputes, and answers whether the value changed.
    fn recompute(&self) -> bool;
    /// The cache, as `RefCell<Option<T>>`.
    fn value(&self) -> &dyn Any;
}

/// The three node kinds.
enum Kind {
    /// A cell. The payload is `RefCell<T>`, behind an `Rc` so a reader can take it out of
    /// the graph before running a closure over it.
    Source(Rc<dyn Any>),
    Memo(Rc<dyn MemoCell>),
    Effect(Rc<RefCell<dyn FnMut()>>),
    /// A freed slot, waiting on the free list.
    Free,
}

struct Node {
    generation: NonZeroU32,
    state: State,
    kind: Kind,
    /// Creation order. Effects run in it, so a parent's effect lands before its child's.
    order: u64,
    /// What this node read, most recently.
    deps: Vec<SignalId>,
    /// What read this node.
    subs: Vec<SignalId>,
    /// Already in the effect queue. Cheaper than scanning it, and it is what makes a burst
    /// of writes to one cell enqueue its effects once.
    queued: bool,
    /// How many times this node's value has moved. Read by a consumer that wants change
    /// detection without the value.
    version: u64,
}

/// A disposal scope's identity.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) struct OwnerId {
    index: u32,
    generation: NonZeroU32,
}

/// What a scope disposes, in reverse creation order.
#[derive(Copy, Clone)]
enum Child {
    Signal(SignalId),
    Owner(OwnerId),
}

struct OwnerNode {
    generation: NonZeroU32,
    children: Vec<Child>,
}

/// The runtime. One per thread that builds signals, and in practice one per process.
struct Graph {
    /// This graph's process-unique id, stamped into every [`SignalId`] it mints.
    id: u32,
    nodes: Vec<Node>,
    free: Vec<u32>,
    owners: Vec<OwnerNode>,
    owners_free: Vec<u32>,
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

/// Hands each graph its id. Production allocates exactly one.
static NEXT_GRAPH: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

thread_local! {
    static GRAPH: RefCell<Graph> = RefCell::new(Graph {
        id: NEXT_GRAPH.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
        nodes: Vec::new(),
        free: Vec::new(),
        owners: Vec::new(),
        owners_free: Vec::new(),
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

/// Runs `f` with the graph borrowed. See the module's one rule.
fn with<R>(f: impl FnOnce(&mut Graph) -> R) -> R {
    GRAPH.with(|g| f(&mut g.borrow_mut()))
}

impl Graph {
    fn node(&self, id: SignalId) -> Option<&Node> {
        if id.graph != self.id {
            return None;
        }
        self.nodes
            .get(id.index as usize)
            .filter(|node| node.generation == id.generation)
    }

    fn node_mut(&mut self, id: SignalId) -> Option<&mut Node> {
        if id.graph != self.id {
            return None;
        }
        self.nodes
            .get_mut(id.index as usize)
            .filter(|node| node.generation == id.generation)
    }

    fn mint(&mut self, kind: Kind) -> SignalId {
        self.order += 1;
        let order = self.order;
        let id = if let Some(index) = self.free.pop() {
            // The recycled node keeps its `deps` and `subs` capacity, which is what makes
            // the second mount of a screen allocation-free.
            let node = &mut self.nodes[index as usize];
            node.state = State::Dirty;
            node.kind = kind;
            node.order = order;
            node.queued = false;
            node.version = 0;
            SignalId {
                graph: self.id,
                index,
                generation: node.generation,
            }
        } else {
            let index = u32::try_from(self.nodes.len()).expect("fewer than 4 billion signals");
            self.nodes.push(Node {
                generation: NonZeroU32::MIN,
                state: State::Dirty,
                kind,
                order,
                deps: Vec::new(),
                subs: Vec::new(),
                queued: false,
                version: 0,
            });
            SignalId {
                graph: self.id,
                index,
                generation: NonZeroU32::MIN,
            }
        };
        if let Some(scope) = self.scope {
            self.attach(scope, Child::Signal(id));
        }
        id
    }

    fn attach(&mut self, scope: OwnerId, child: Child) {
        if let Some(owner) = self
            .owners
            .get_mut(scope.index as usize)
            .filter(|owner| owner.generation == scope.generation)
        {
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
            // `set_state` never touches a subscriber list, so indexing is stable here.
            let sub = self.nodes[id.index as usize].subs[i];
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

/// A cell's payload, with the read recorded against the current observer.
///
/// Returns `None` if the cell was disposed, which is the generation check doing its work:
/// a stale `Cell` handle reads nothing rather than reading whatever now occupies its slot.
pub(super) fn read_source(id: SignalId) -> Option<Rc<dyn Any>> {
    with(|g| {
        g.track(id);
        match g.node(id).map(|node| &node.kind) {
            Some(Kind::Source(value)) => Some(Rc::clone(value)),
            _ => None,
        }
    })
}

/// A cell's payload, **without** recording a dependency.
pub(super) fn peek_source(id: SignalId) -> Option<Rc<dyn Any>> {
    with(|g| match g.node(id).map(|node| &node.kind) {
        Some(Kind::Source(value)) => Some(Rc::clone(value)),
        _ => None,
    })
}

/// A memo's cache, resolved first, with the read recorded.
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

/// Brings `id` up to date if anything it reads changed. The pull half of the scheme.
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
            // subscribers if its value actually moves.
            //
            // The no-change arm leaves subscribers **`Check`** rather than marking them
            // **`Clean`**. Clearing them is unsound in exactly the shape two-level marking
            // exists for: where a diamond's two branches resolve in sequence, the second
            // clears the first's `Dirty` and the shared node answers from a stale cache. A
            // `Check` node re-asks its dependencies before believing its cache.
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

/// Drops every edge into `id`'s dependencies, so a recompute can collect a fresh set.
///
/// A node that stops reading a source must stop being woken by it, or a `when(false)`
/// branch keeps costing what it did when it was true.
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

/// Bumps `id`'s version and marks everything downstream of it. The push half of the
/// scheme, and the only place a version moves.
pub(super) fn invalidate(id: SignalId) {
    with(|g| {
        if let Some(node) = g.node_mut(id) {
            node.version = node.version.wrapping_add(1);
        }
        g.invalidate(id);
        g.stack.clear();
    });
}

/// How many times `id`'s value has moved. Zero for a node that is gone.
pub(super) fn version(id: SignalId) -> u64 {
    with(|g| g.node(id).map_or(0, |node| node.version))
}

/// Whether this thread's graph holds `id` — the test that separates the owning thread's
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
            g.running.sort_unstable_by_key(|id| {
                nodes
                    .get(id.index as usize)
                    .filter(|node| node.generation == id.generation)
                    .map_or(u64::MAX, |node| node.order)
            });
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

            // An effect marked only `Check` re-asks its dependencies, and if none of them
            // actually moved it does not run. That is the value-equality cutoff doing its
            // work one level below a memo.
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
        let id = if let Some(index) = g.owners_free.pop() {
            let owner = &mut g.owners[index as usize];
            OwnerId {
                index,
                generation: owner.generation,
            }
        } else {
            let index = u32::try_from(g.owners.len()).expect("fewer than 4 billion scopes");
            g.owners.push(OwnerNode {
                generation: NonZeroU32::MIN,
                children: Vec::new(),
            });
            OwnerId {
                index,
                generation: NonZeroU32::MIN,
            }
        };
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
    let Some(mut children) = with(|g| {
        let owner = g
            .owners
            .get_mut(id.index as usize)
            .filter(|owner| owner.generation == id.generation)?;
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

    with(|g| {
        if let Some(owner) = g
            .owners
            .get_mut(id.index as usize)
            .filter(|owner| owner.generation == id.generation)
        {
            // The `Vec` goes back with its capacity, which is what makes remounting free.
            owner.children = children;
            owner.generation = owner.generation.checked_add(1).unwrap_or(NonZeroU32::MIN);
            g.owners_free.push(id.index);
        }
    });
}

/// Disposes one node: drops its outgoing edges, releases its payload, and frees its slot.
pub(super) fn dispose(id: SignalId) {
    with(|g| {
        if g.node(id).is_none() {
            return;
        }
        clear_deps(g, id);
        shared::release(id);
        let Some(node) = g.node_mut(id) else { return };
        // A subscriber that outlives its source is legal — it is simply never woken again
        // — and its stale edge is pruned by the generation check the next time it is
        // walked. What must not survive is this node's entry in anyone else's list, and
        // `clear_deps` above is what removes those.
        node.subs.clear();
        node.kind = Kind::Free;
        node.state = State::Clean;
        node.queued = false;
        node.generation = node.generation.checked_add(1).unwrap_or(NonZeroU32::MIN);
        g.free.push(id.index);
    });
}

/// How many signal nodes are live on this thread.
///
/// The leak assertion's instrument and nothing else: a screen mounted and unmounted a
/// thousand times must return this to its baseline.
#[must_use]
pub fn live_nodes() -> usize {
    with(|g| g.nodes.len() - g.free.len())
}
