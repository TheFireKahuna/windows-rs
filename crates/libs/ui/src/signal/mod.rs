//! Reactive signals: the value handles, and the flush that propagates writes through them.
//!
//! A signal is a stable, `Copy` handle naming "the current value of X". A sink binds to the
//! handle rather than to a value produced inside one call, so a producer-written level, a
//! hover state, a tracker-driven offset and an automation-visible number are read the same
//! way and outlive the code that created them.
//!
//! # The three primitives
//!
//! | | is | runs |
//! |---|---|---|
//! | [`Cell`] | a source | never — it is written |
//! | [`Memo`] | a pure derivation | lazily, when read and a dependency moved |
//! | [`Effect`] | a leaf that touches the world | at the end of a flush, in creation order |
//!
//! [`Owner`] is the disposal scope that owns all three. [`Epoch`] is the payload-free
//! counterpart of a `Cell`, for a consumer that wants a wake rather than a value.
//!
//! # Propagation is glitch-free
//!
//! A *glitch* is an observer seeing a derived value computed from a mix of old and new
//! inputs. Two properties prevent it: two-level marking, so a diamond's shared node is
//! evaluated once, and a value-equality cutoff, so a derivation whose result did not change
//! stops the propagation at itself. Both are implemented in the `graph` module.
//!
//! # There is no timer
//!
//! Nothing here ticks. [`flush`] runs when a caller invokes it — after a write, a published
//! config, a theme flip, a resize — and performs exactly the work those writes marked.
//!
//! # Threads
//!
//! The graph belongs to the app thread. [`Cell`] is `Send` when its value is, so a producer
//! may hold one and call [`Cell::post`]. `Cell` is not `Sync`, and [`Memo`] and [`Effect`]
//! are neither, so a handle cannot reach a graph on another thread by being shared into a
//! closure.

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
/// `Copy` and reference-count free, so any number of closures may capture a cell with no
/// bookkeeping at the binding site.
///
/// `Send` when `T` is, so a producer thread may hold a copy and call [`post`](Self::post).
/// Not `Sync`: every other method reads or writes the app thread's graph, and only a shared
/// reference could reach that graph from another thread.
pub struct Cell<T: 'static> {
    id: SignalId,
    /// Carries the auto traits: `Copy` for any `T`, `Send` when `T` is, never `Sync`.
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
    /// Creates a cell holding `v`, registered with the enclosing [`Owner`].
    #[must_use]
    pub fn new(v: T) -> Self {
        Self {
            id: graph::source(Rc::new(RefCell::new(v))),
            marker: PhantomData,
        }
    }

    /// Returns this cell's identity, which is what a sink binds to and what a diagnostic
    /// names.
    #[must_use]
    pub fn id(self) -> SignalId {
        self.id
    }

    /// Reads through `f`, registering a dependency if called inside a [`Memo`] or
    /// [`Effect`].
    ///
    /// # Panics
    ///
    /// Panics if the cell has been disposed, or if `f` writes this same cell: the read
    /// borrow taken here is still held for the duration of `f`. Reading any cell from
    /// inside `f`, including this one, is allowed.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        let value = graph::read_source(self.id).expect("the cell is live");
        let slot = downcast::<T>(&*value).borrow();
        f(&slot)
    }

    /// Mutates the value through `f` and propagates unconditionally.
    ///
    /// Subscribers wake even where `f` changed nothing, because the mutation is opaque and
    /// cannot be compared. [`set`](Self::set) gates on equality where `T: PartialEq`.
    ///
    /// # Panics
    ///
    /// Panics if the cell has been disposed, or if `f` reads or writes this same cell: the
    /// write borrow taken here is held for the duration of `f`.
    pub fn update(self, f: impl FnOnce(&mut T)) {
        let value = graph::peek_source(self.id).expect("the cell is live");
        f(&mut downcast::<T>(&*value).borrow_mut());
        graph::invalidate(self.id);
    }

    /// Returns how many times this cell's value has changed.
    ///
    /// Change detection for a consumer that does not want the value: a counter rather than
    /// a flag, so a consumer that misses any number of writes still sees that it missed
    /// them. A disposed cell reports zero.
    #[must_use]
    pub fn version(self) -> u64 {
        graph::version(self.id)
    }

    /// Returns `true` while this cell is live. A disposed handle answers `false` rather
    /// than reading whatever now occupies its slot.
    #[must_use]
    pub fn alive(self) -> bool {
        graph::peek_source(self.id).is_some()
    }
}

impl<T: Clone + 'static> Cell<T> {
    /// Returns a clone of the current value, registering a dependency.
    #[must_use]
    pub fn get(self) -> T {
        self.with(Clone::clone)
    }

    /// Returns a clone of the current value without registering a dependency.
    ///
    /// The read a write takes of its own target, and the read a diagnostic takes of state
    /// it must not subscribe to.
    #[must_use]
    pub fn peek(self) -> T {
        let value = graph::peek_source(self.id).expect("the cell is live");
        let slot = downcast::<T>(&*value).borrow();
        slot.clone()
    }
}

impl<T: PartialEq + 'static> Cell<T> {
    /// Writes `v`, and propagates only where it differs from the current value.
    ///
    /// The comparison gates everything downstream, so a derivation over a clamped input is
    /// not woken by a write the clamp absorbs.
    pub fn set(self, v: T) {
        let value = graph::peek_source(self.id).expect("the cell is live");
        if write_slot(downcast::<T>(&*value), v) {
            graph::invalidate(self.id);
        }
    }
}

impl<T: PartialEq + Send + 'static> Cell<T> {
    /// Writes `v` from any thread.
    ///
    /// On the thread that owns the graph this is [`set`](Self::set). Anywhere else the
    /// write is staged and applied at the app thread's next [`flush`], coalesced so that a
    /// producer outrunning the app thread overwrites its own pending value and wakes that
    /// thread once. [`written`] is the event the app thread waits on.
    ///
    /// A staged write allocates one box. A display-rate producer publishes through an
    /// [`Epoch`], which carries no value and allocates nothing.
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
/// Tracks its own reads, so its dependency set is collected rather than declared. It
/// recomputes lazily — a memo nothing reads never runs — and its result is compared with
/// the previous one, so a derivation whose value settles stops the propagation at itself.
pub struct Memo<T: 'static> {
    id: SignalId,
    /// Neither `Send` nor `Sync`: the closure lives in the app thread's graph, and every
    /// method on the handle reaches that graph.
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
    /// Creates a derivation over whatever `f` reads.
    ///
    /// `f` does not run here; the first read runs it. `T: PartialEq` is what the cutoff
    /// needs: comparing the result stops a write propagating past a derivation it did not
    /// change.
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

    /// Returns this memo's identity.
    #[must_use]
    pub fn id(self) -> SignalId {
        self.id
    }

    /// Reads through `f`, resolving the memo first and registering a dependency.
    ///
    /// # Panics
    ///
    /// Panics if the memo has been disposed.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        let cell = graph::read_memo(self.id).expect("the memo is live");
        let cache = downcast::<Option<T>>(cell.value()).borrow();
        f(cache.as_ref().expect("a resolved memo holds its value"))
    }
}

impl<T: PartialEq + Clone + 'static> Memo<T> {
    /// Returns a clone of the current value, resolving the memo first and registering a
    /// dependency.
    #[must_use]
    pub fn get(self) -> T {
        self.with(Clone::clone)
    }
}

/// A leaf that performs a side effect when what it read changes. The point at which a
/// value reaches a sink.
///
/// Runs once when created, which collects its dependency set, and thereafter at the end of
/// any flush in which something it read moved. Effects run after every memo has resolved,
/// so none observes a half-updated graph.
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

    /// Returns this effect's identity.
    #[must_use]
    pub fn id(self) -> SignalId {
        self.id
    }
}

/// A disposal scope. Owns every [`Cell`], [`Memo`], [`Effect`] and nested scope created
/// under it, and disposes them in reverse creation order when dropped.
///
/// Every structural node — a mounted widget, a realized list row, an open flyout — owns
/// one, and unmounting drops it. Disposal is by scope alone; no signal carries an
/// unsubscribe.
pub struct Owner(graph::OwnerId);

impl Owner {
    /// Runs `f` with a fresh scope installed, and returns the scope alongside `f`'s result.
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

    /// Runs `f` inside this scope, so what `f` creates is owned here rather than by the
    /// scope current at the call site.
    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let outer = graph::enter_scope(Some(self.0));
        let out = f();
        graph::enter_scope(outer);
        out
    }

    /// Runs `f` with no scope installed, so what `f` creates belongs to no enclosing scope
    /// and outlives every one of them. The caller takes responsibility for disposing it,
    /// which is what a scope `f` itself opens and returns is for.
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

/// [`Signal`] marker: the source is a constant.
#[derive(Copy, Clone, Debug)]
pub struct IsValue;
/// [`Signal`] marker: the source is a [`Cell`].
#[derive(Copy, Clone, Debug)]
pub struct IsCell;
/// [`Signal`] marker: the source is a [`Memo`].
#[derive(Copy, Clone, Debug)]
pub struct IsMemo;
/// [`Signal`] marker: the source is a closure.
#[derive(Copy, Clone, Debug)]
pub struct IsFn;

/// Anything readable as a `T`: a constant, a [`Cell`], a [`Memo`], or a closure.
///
/// One method accepts all four, so `opacity(0.6)` and `opacity(move || hover.get())` reach
/// the same `fn opacity<M>(self, v: impl Signal<f32, M>)`.
///
/// `Marker` separates the four impls, which would otherwise overlap: a closure is a value
/// and so is a `Cell`. It is inferred at every call site and never written.
pub trait Signal<T, Marker = IsValue> {
    /// Returns the current value, registering a dependency where the source has one.
    fn read(&self) -> T;

    /// Returns whether reading can ever produce a different answer.
    ///
    /// A caller binding a constant creates no [`Effect`] and no graph node for it.
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
