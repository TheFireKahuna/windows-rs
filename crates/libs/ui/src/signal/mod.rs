//! Signals: the handles, and the scheduler behind them.
//!
//! A signal is a **stable, named, addressable handle meaning "the current value of X"**.
//! That is the whole reason this is not a virtual DOM: binding a value to a compositor
//! property — a tracker expression, a retargetable spring, a coalesced cross-thread write
//! — needs such a handle, and a value that exists only transiently inside a `render()`
//! call is not one. With signals, a producer-written level, a hover state, a
//! tracker-driven offset and an automation-visible number are all the same thing.
//!
//! # The three primitives
//!
//! | | is | runs |
//! |---|---|---|
//! | [`Cell`] | a source | never — it is written |
//! | [`Memo`] | a pure derivation | lazily, when read and a dependency moved |
//! | [`Effect`] | a leaf that touches the world | at the end of a flush, in creation order |
//!
//! [`Owner`] is the disposal scope that owns all three, and [`Epoch`] is the payload-free
//! twin of a `Cell` for a consumer that wants a wake rather than a value.
//!
//! # Propagation is glitch-free
//!
//! A *glitch* is an observer seeing a derived value computed from a mix of old and new
//! inputs. Two properties prevent it: **two-level marking**, so a diamond's shared node is
//! evaluated once, and a **value-equality cutoff**, so a derivation whose result did not
//! change stops the propagation at itself. Both live in `graph`.
//!
//! # There is no timer
//!
//! Nothing here ticks. [`flush`] is called by whoever woke — a write, a published config,
//! a theme flip, a resize — and does exactly the work those writes implied.
//!
//! # Threads
//!
//! The graph is the app thread's. [`Cell`] is `Send` when its value is, so a producer may
//! hold one and call [`Cell::post`]; it is deliberately **not** `Sync`, and [`Memo`] and
//! [`Effect`] are neither, so a handle whose graph lives elsewhere cannot reach that graph
//! by being shared into a closure.

mod epoch;
mod graph;
mod shared;
#[cfg(test)]
mod tests;

pub use epoch::Epoch;
pub use graph::{SignalId, flush, live_nodes, set_waker, untracked};
pub use shared::written;

use core::any::Any;
use core::cell::RefCell;
use core::marker::PhantomData;
use std::rc::Rc;

/// A source cell. Reads track; writes invalidate subscribers.
///
/// `Copy`, eight bytes, and no reference count: a cell may be captured by any number of
/// closures with no bookkeeping, which is what keeps `move ||` at a binding site free.
///
/// `Send` when `T` is — a producer thread holds a copy and calls [`post`](Self::post).
/// Never `Sync`: every other method reads or writes the app thread's graph, and sharing a
/// reference across threads is the one way to reach them from the wrong one.
pub struct Cell<T: 'static> {
    id: SignalId,
    /// Exactly the auto-trait behaviour wanted: `Copy` whatever `T` is, `Send` when `T`
    /// is, and never `Sync`.
    marker: PhantomData<core::cell::Cell<T>>,
}

impl<T: 'static> Copy for Cell<T> {}
impl<T: 'static> Clone for Cell<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> core::fmt::Debug for Cell<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Cell").field(&self.id).finish()
    }
}

impl<T: 'static> Cell<T> {
    /// A new cell holding `v`, registered with the enclosing [`Owner`].
    #[must_use]
    pub fn new(v: T) -> Self {
        Self {
            id: graph::source(Rc::new(RefCell::new(v))),
            marker: PhantomData,
        }
    }

    /// This cell's identity. What a sink binds to, and what a diagnostic names.
    #[must_use]
    pub fn id(self) -> SignalId {
        self.id
    }

    /// Reads through `f`, registering a dependency if called inside a [`Memo`] or
    /// [`Effect`].
    ///
    /// # Panics
    ///
    /// If `f` reads or writes *this* cell. Nested access to a **different** cell is fine;
    /// re-entering the same one is a value defined in terms of itself.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        let value = graph::read_source(self.id).expect("the cell is live");
        let slot = downcast::<T>(&*value).borrow();
        f(&slot)
    }

    /// Replaces the value through `f` and propagates unconditionally.
    ///
    /// The equality gate cannot apply here — the mutation is opaque — so an `update` that
    /// changes nothing still wakes every subscriber. Prefer [`set`](Self::set) wherever
    /// the value can be compared.
    pub fn update(self, f: impl FnOnce(&mut T)) {
        let value = graph::peek_source(self.id).expect("the cell is live");
        f(&mut downcast::<T>(&*value).borrow_mut());
        graph::invalidate(self.id);
    }

    /// How many times this cell's value has moved.
    ///
    /// Cheap change detection for a consumer that does not want the value — and the reason
    /// it is a counter rather than a flag is that a consumer may miss any number of writes
    /// and still needs to know that it did.
    #[must_use]
    pub fn version(self) -> u64 {
        graph::version(self.id)
    }

    /// Whether this cell is still live. A disposed handle answers `false` rather than
    /// reading whatever now occupies its slot.
    #[must_use]
    pub fn alive(self) -> bool {
        graph::peek_source(self.id).is_some()
    }
}

impl<T: Clone + 'static> Cell<T> {
    /// The current value, registering a dependency.
    #[must_use]
    pub fn get(self) -> T {
        self.with(Clone::clone)
    }

    /// The current value **without** registering a dependency.
    ///
    /// Deliberate, not a shortcut: it is how a write avoids depending on what it writes,
    /// and how a diagnostic reads state it must not subscribe to.
    #[must_use]
    pub fn peek(self) -> T {
        let value = graph::peek_source(self.id).expect("the cell is live");
        let slot = downcast::<T>(&*value).borrow();
        slot.clone()
    }
}

impl<T: PartialEq + 'static> Cell<T> {
    /// Writes, and propagates **only if the value moved**.
    ///
    /// Value equality gates everything downstream: without it, a derivation over a clamped
    /// input would wake on every write to that input even where the clamp already held.
    pub fn set(self, v: T) {
        let value = graph::peek_source(self.id).expect("the cell is live");
        if write_slot(downcast::<T>(&*value), v) {
            graph::invalidate(self.id);
        }
    }
}

impl<T: PartialEq + Send + 'static> Cell<T> {
    /// Writes from **any** thread.
    ///
    /// On the thread that owns the graph this is exactly [`set`](Self::set). Anywhere else
    /// the write is staged and applied at the app thread's next [`flush`], coalesced so
    /// that a producer outrunning the app thread overwrites its own pending value and
    /// wakes it once. [`written`] is the event to wait on.
    ///
    /// A staged write costs one allocation, which is why a display-rate producer publishes
    /// through an [`Epoch`] — which carries no value — instead.
    pub fn post(self, v: T) {
        if graph::owns(self.id) {
            self.set(v);
            return;
        }
        shared::post(
            self.id,
            Box::new(move |slot: &dyn Any| {
                slot.downcast_ref::<RefCell<T>>()
                    .is_some_and(|slot| write_slot(slot, v))
            }),
        );
    }
}

/// A memoized pure derivation.
///
/// It tracks its own reads, so there is no dependency list to state and none to get wrong.
/// It recomputes lazily — a memo nothing reads never runs — and its result is compared
/// with the previous one, so a derivation that settles stops the propagation at itself.
pub struct Memo<T: 'static> {
    id: SignalId,
    /// Neither `Send` nor `Sync`: the closure lives in the app thread's graph, so a handle
    /// on another thread could only fail.
    marker: PhantomData<*const T>,
}

impl<T: 'static> Copy for Memo<T> {}
impl<T: 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> core::fmt::Debug for Memo<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Memo").field(&self.id).finish()
    }
}

struct Derivation<T, F> {
    f: F,
    cache: RefCell<Option<T>>,
}

impl<T: PartialEq + 'static, F: Fn() -> T + 'static> graph::MemoCell for Derivation<T, F> {
    fn recompute(&self) -> bool {
        let next = (self.f)();
        // Written into the existing cache rather than boxed afresh, so a memo recomputing
        // on every flush allocates nothing of its own.
        let mut cache = self.cache.borrow_mut();
        if cache.as_ref() == Some(&next) {
            return false;
        }
        *cache = Some(next);
        true
    }

    fn value(&self) -> &dyn Any {
        &self.cache
    }
}

impl<T: PartialEq + 'static> Memo<T> {
    /// A derivation over whatever `f` reads.
    ///
    /// `T: PartialEq` is the cutoff's requirement rather than a convenience: comparing the
    /// result is what stops a write propagating past a derivation it did not change.
    #[must_use]
    pub fn new(f: impl Fn() -> T + 'static) -> Self {
        Self {
            id: graph::memo(Rc::new(Derivation {
                f,
                cache: RefCell::new(None),
            })),
            marker: PhantomData,
        }
    }

    /// This memo's identity.
    #[must_use]
    pub fn id(self) -> SignalId {
        self.id
    }

    /// Reads through `f`, resolving first and registering a dependency.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        let cell = graph::read_memo(self.id).expect("the memo is live");
        let cache = downcast::<Option<T>>(cell.value()).borrow();
        f(cache.as_ref().expect("a resolved memo holds its value"))
    }
}

impl<T: PartialEq + Clone + 'static> Memo<T> {
    /// The current value, resolving first and registering a dependency.
    #[must_use]
    pub fn get(self) -> T {
        self.with(Clone::clone)
    }
}

/// A leaf that performs a side effect when its dependencies change. Where a value reaches
/// a sink.
///
/// It runs once when created — which is what collects its dependency set — and thereafter
/// at the end of any flush in which something it read moved. Effects run **after** every
/// memo has resolved, so one never observes a half-updated graph.
#[derive(Copy, Clone, Debug)]
pub struct Effect {
    id: SignalId,
    marker: PhantomData<*const ()>,
}

impl Effect {
    /// Runs `f` now, and again whenever what it read changes.
    pub fn new(f: impl FnMut() + 'static) -> Self {
        Self {
            id: graph::effect(Rc::new(RefCell::new(f))),
            marker: PhantomData,
        }
    }

    /// This effect's identity.
    #[must_use]
    pub fn id(self) -> SignalId {
        self.id
    }
}

/// A disposal scope. Owns every [`Cell`], [`Memo`], [`Effect`] and nested scope created
/// under it, and disposes them in reverse creation order when dropped.
///
/// Every structural node — a mounted widget, a realized list row, an open flyout — owns
/// one. Unmounting drops it. That is the whole disposal story, and it is why there is no
/// `unsubscribe` anywhere in this module.
pub struct Owner(graph::OwnerId);

impl Owner {
    /// Runs `f` with a fresh scope installed, and hands back the scope alongside its
    /// result.
    ///
    /// Dropping the returned `Owner` disposes everything `f` created.
    #[must_use = "dropping the scope immediately disposes everything it just created"]
    pub fn scope<R>(f: impl FnOnce() -> R) -> (Self, R) {
        let id = graph::open_scope();
        let outer = graph::enter_scope(Some(id));
        let out = f();
        graph::enter_scope(outer);
        (Self(id), out)
    }

    /// Runs `f` inside this scope, so what it creates is owned here rather than wherever
    /// the caller happens to be. How a list row rebinds without leaking into its parent.
    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let outer = graph::enter_scope(Some(self.0));
        let out = f();
        graph::enter_scope(outer);
        out
    }

    /// Runs `f` with **no** scope installed, so what it creates outlives every enclosing
    /// one. For state that must survive the structure reading it.
    pub fn detached<R>(f: impl FnOnce() -> R) -> R {
        let outer = graph::enter_scope(None);
        let out = f();
        graph::enter_scope(outer);
        out
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        graph::dispose_scope(self.0);
    }
}

impl core::fmt::Debug for Owner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Owner").field(&self.0).finish()
    }
}

// ── the reactive overload ───────────────────────────────────────────────────────

/// A constant.
#[derive(Copy, Clone, Debug)]
pub struct IsValue;
/// A [`Cell`].
#[derive(Copy, Clone, Debug)]
pub struct IsCell;
/// A [`Memo`].
#[derive(Copy, Clone, Debug)]
pub struct IsMemo;
/// A closure.
#[derive(Copy, Clone, Debug)]
pub struct IsFn;

/// The four readable things, unified, so a builder need not care which it was handed.
///
/// This is what lets every value-taking method have a reactive overload at no call-site
/// cost: `opacity(0.6)` and `opacity(move || hover.get())` are the same method,
/// `fn opacity<M>(self, v: impl Signal<f32, M>)`.
///
/// `Marker` exists because the four impls would otherwise overlap — a closure is a value
/// and a `Cell` is a value — and Rust resolves an overlap by refusing to compile it. It is
/// inferred at every call site and never written.
pub trait Signal<T, Marker = IsValue> {
    /// The current value, registering a dependency where there is one to register.
    fn read(&self) -> T;

    /// Whether reading can ever produce a different answer.
    ///
    /// A constant needs no [`Effect`], and that is the difference between a static label
    /// costing one sprite and costing a graph node. Most of a screen is static.
    fn is_constant(&self) -> bool;
}

impl<T: Clone> Signal<T, IsValue> for T {
    fn read(&self) -> T {
        self.clone()
    }

    fn is_constant(&self) -> bool {
        true
    }
}

impl<T: Clone + 'static> Signal<T, IsCell> for Cell<T> {
    fn read(&self) -> T {
        self.get()
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<T: Clone + PartialEq + 'static> Signal<T, IsMemo> for Memo<T> {
    fn read(&self) -> T {
        self.get()
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<T, F: Fn() -> T> Signal<T, IsFn> for F {
    fn read(&self) -> T {
        self()
    }

    fn is_constant(&self) -> bool {
        false
    }
}

// ── payload access ──────────────────────────────────────────────────────────────

fn downcast<T: 'static>(value: &dyn Any) -> &RefCell<T> {
    value
        .downcast_ref::<RefCell<T>>()
        .expect("a signal's payload has the type its handle names")
}

fn write_slot<T: PartialEq>(slot: &RefCell<T>, v: T) -> bool {
    let mut current = slot.borrow_mut();
    if *current == v {
        return false;
    }
    *current = v;
    true
}
