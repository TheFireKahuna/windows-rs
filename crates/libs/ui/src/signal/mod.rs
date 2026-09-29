//! Reactive signals: the value handles, and the flush that propagates writes through them.
//!
//! A signal is a stable, `Copy` handle naming "the current value of X". A sink binds to the
//! handle rather than to a value produced inside one call, so a producer-written level, a
//! hover state, a tracker-driven offset and an automation-visible number are read the same way
//! and outlive the code that created them.
//!
//! | | is | runs |
//! |---|---|---|
//! | [`Cell`] | a source | never — it is written |
//! | [`Memo`] | a pure derivation | lazily, when read and a dependency moved |
//! | [`Effect`] | a leaf that touches the world | after the memos, in creation order |
//!
//! [`Owner`] is the disposal scope that owns all three. [`Epoch`] is the payload-free
//! counterpart of a `Cell`, for a consumer that wants a wake rather than a value.
//!
//! Effects run on one of two phases. [`flush`] runs the update phase; a geometry effect is
//! held back and run by the host once the solve has settled, so it reads a box from the solve
//! it belongs to rather than from the one before it. [`flush`] answers `true` when a geometry
//! effect is owed as well as when something moved.
//!
//! # Propagation is glitch-free
//!
//! A *glitch* is an observer seeing a derived value computed from a mix of old and new inputs.
//! Two properties prevent it: two-level marking, so a diamond's shared node is evaluated once,
//! and a value-equality cutoff, so a derivation whose result did not change stops the
//! propagation at itself. Both are in the `graph` module.
//!
//! # There is no timer
//!
//! Nothing here ticks. [`flush`] runs when a caller invokes it — after a write, a published
//! config, a theme flip, a resize — and performs exactly the work those writes marked.
//!
//! # Threads
//!
//! The graph belongs to the thread that flushes it. [`Cell`] is `Send` when its value is, so a
//! producer may hold one and call [`Cell::post`] from anywhere: the write is staged and that
//! thread is rung, and the write lands at the top of its next flush. `Cell` is not `Sync`, and
//! [`Memo`] and [`Effect`] are neither, so a handle cannot reach a graph on another thread by
//! being shared into a closure.
//!
//! A staged write costs one box whether or not the value moved, because the cutoff runs on the
//! consuming thread. [`Posted`] holds the producer's own copy beside the cell and compares
//! before staging, for a producer on a stream that mostly repeats itself.

mod epoch;
mod graph;
#[cfg(test)]
mod tests;

pub use epoch::Epoch;
pub use graph::{SignalId, flush, live_nodes, set_waker, untracked};
pub(crate) use graph::{PostGuard, PostWake, RetiredEffect, arm_posts, flush_geometry, posts_pending};

use core::any::Any;
use core::cell::RefCell;
use core::marker::PhantomData;
use std::rc::Rc;

thread_local! {
    static READ_ONLY: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Panics where a drawing callback is writing a signal.
pub(crate) fn assert_writable() {
    assert!(!READ_ONLY.get(), "geometry callbacks may only read signals and fill their output");
}

/// Runs `f` with writes refused, which is what a drawing callback is run under.
pub(crate) fn read_only<R>(f: impl FnOnce() -> R) -> R {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            READ_ONLY.set(self.0);
        }
    }
    let _reset = Reset(READ_ONLY.replace(true));
    f()
}

/// Returns a node's payload as the type its handle names.
fn payload<T: 'static>(id: SignalId, track: bool) -> Rc<RefCell<T>> {
    let any = if track { graph::read(id) } else { graph::peek(id) };
    let any = any.expect("signal disposed");
    any.downcast().unwrap_or_else(|_| unreachable!("a handle names its payload's type"))
}

/// A source cell. Reads track; writes invalidate subscribers.
///
/// `Copy` and reference-count free, so any number of closures may capture a cell with no
/// bookkeeping at the binding site.
///
/// `Send` when `T` is, so a producer thread may hold a copy and call [`post`](Self::post). Not
/// `Sync`: every other method reads or writes the app thread's graph, and only a shared
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

/// Prints the identity, which is all a cell has that does not need the graph: the payload is
/// behind an `Rc` the formatter cannot borrow while a caller holds it.
impl<T: 'static> core::fmt::Debug for Cell<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Cell").field(&self.id).finish()
    }
}

impl<T: 'static> Cell<T> {
    /// Creates a cell holding `v`, registered with the enclosing [`Owner`].
    pub fn new(v: T) -> Self {
        let value = Rc::new(RefCell::new(v));
        Self { id: graph::mint(graph::CELL, Some(value), None), marker: PhantomData }
    }

    /// Returns this cell's identity, which is what a sink binds to and what a diagnostic names.
    pub fn id(self) -> SignalId {
        self.id
    }

    /// Returns `true` while this cell is live. A disposed handle answers `false` rather than
    /// reading whatever now occupies its slot.
    pub fn alive(self) -> bool {
        graph::alive(self.id)
    }

    /// Returns how many times this cell's value has changed.
    ///
    /// Change detection for a consumer that does not want the value: a counter rather than a
    /// flag, so a consumer that misses any number of writes still sees that it missed them. A
    /// disposed cell reports zero.
    pub fn version(self) -> u64 {
        graph::version(self.id)
    }

    /// Reads through `f`, registering a dependency if called inside a [`Memo`] or [`Effect`].
    ///
    /// # Panics
    ///
    /// Panics if the cell has been disposed, or if `f` writes this same cell: the read borrow
    /// taken here is still held for the duration of `f`. Reading any cell from inside `f`,
    /// including this one, is allowed.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        let value = payload::<T>(self.id, true);
        let slot = RefCell::borrow(&value);
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
        assert_writable();
        f(&mut RefCell::borrow_mut(&payload::<T>(self.id, false)));
        graph::bump(self.id);
    }
}

impl<T: Clone + 'static> Cell<T> {
    /// Returns a clone of the current value, registering a dependency.
    pub fn get(self) -> T {
        self.with(T::clone)
    }

    /// Returns a clone of the current value without registering a dependency.
    ///
    /// The read a write takes of its own target, and the read a diagnostic takes of state it
    /// must not subscribe to.
    pub fn peek(self) -> T {
        let value = payload::<T>(self.id, false);
        let slot = RefCell::borrow(&value);
        slot.clone()
    }
}

impl<T: PartialEq + 'static> Cell<T> {
    /// Writes `v`, and propagates only where it differs from the current value.
    ///
    /// The comparison gates everything downstream, so a derivation over a clamped input is not
    /// woken by a write the clamp absorbs.
    pub fn set(self, v: T) {
        assert_writable();
        let value = payload::<T>(self.id, false);
        let mut slot = RefCell::borrow_mut(&value);
        if *slot == v {
            return;
        }
        *slot = v;
        drop(slot);
        graph::bump(self.id);
    }
}

impl<T: PartialEq + Send + 'static> Cell<T> {
    /// Writes `v` from any thread.
    ///
    /// On the thread that owns the graph this is [`set`](Self::set). Anywhere else the write is
    /// staged and applied at the app thread's next [`flush`], coalesced so that a producer
    /// outrunning the app thread overwrites its own pending value and holds one wake until the
    /// graph drains the write.
    ///
    /// A staged write allocates one box. A display-rate producer publishes through an
    /// [`Epoch`], which carries no value and allocates nothing.
    ///
    /// # Panics
    ///
    /// Panics on the thread that owns the graph if the cell has been disposed. A write staged
    /// from any other thread is dropped at the drain instead.
    pub fn post(self, v: T) {
        if graph::owns(self.id) {
            self.set(v);
            return;
        }
        graph::stage(
            self.id,
            Box::new(move |any: &dyn Any| {
                let value = any.downcast_ref::<RefCell<T>>().expect("payload type");
                let mut slot = value.borrow_mut();
                if *slot == v {
                    return false;
                }
                *slot = v;
                true
            }),
        );
    }
}

/// A [`Cell`] a producer thread writes, beside that thread's own copy of what it last wrote
/// there.
///
/// [`Cell::post`] stages a boxed write whatever value it carries, and the equality cutoff runs
/// on the consuming thread — after the box. The comparison here runs before it, on the
/// producer's side: a value that did not move allocates nothing and raises no wake, and one
/// that did costs what `post` costs.
///
/// `Send` and not `Sync`, like the cell it holds, so the copy belongs to the one thread that
/// writes it.
pub struct Posted<T: 'static> {
    cell: Cell<T>,
    /// What was last written through this handle. Borrowed only by the producer, which owns it
    /// outright: `Posted` is not `Sync`, so no second thread holds a reference to one.
    last: RefCell<T>,
}

impl<T: Clone + PartialEq + Send + 'static> Posted<T> {
    /// Takes the producer's handle on `cell`, reading the value it holds now.
    ///
    /// Called on the thread that owns the graph, which is the one that can read a cell.
    ///
    /// # Panics
    ///
    /// Panics if `cell` has been disposed.
    pub fn new(cell: Cell<T>) -> Self {
        Self { last: RefCell::new(cell.peek()), cell }
    }

    /// Writes `v` where it differs from the last value written here, and answers whether it did.
    pub fn set(&self, v: T) -> bool {
        if *self.last.borrow() == v {
            return false;
        }
        self.cell.post(v.clone());
        *self.last.borrow_mut() = v;
        true
    }

    /// Writes what `next` answers where `probe` differs from the last value written here, and
    /// answers whether it did.
    ///
    /// The comparison runs against `probe`, so a value whose owned form costs an allocation — a
    /// slice shared into an `Arc`, a scratch buffer copied out — is built only on the pass that
    /// moved.
    ///
    /// `probe` must be what `next` answers: the two are compared and stored as one value.
    pub fn set_by<U>(&self, probe: &U, next: impl FnOnce() -> T) -> bool
    where
        T: core::borrow::Borrow<U>,
        U: PartialEq + ?Sized,
    {
        if <T as core::borrow::Borrow<U>>::borrow(&self.last.borrow()) == probe {
            return false;
        }
        let v = next();
        debug_assert!(
            <T as core::borrow::Borrow<U>>::borrow(&v) == probe,
            "probe must be what next answers"
        );
        self.cell.post(v.clone());
        *self.last.borrow_mut() = v;
        true
    }
}

/// A memoized pure derivation.
///
/// Tracks its own reads, so its dependency set is collected rather than declared. It recomputes
/// lazily — a memo nothing reads never runs — and its result is compared with the previous one,
/// so a derivation whose value settles stops the propagation at itself.
pub struct Memo<T: 'static> {
    id: SignalId,
    /// Neither `Send` nor `Sync`: the closure lives in the app thread's graph, and every method
    /// on the handle reaches that graph.
    marker: PhantomData<Rc<T>>,
}

impl<T: 'static> Copy for Memo<T> {}

impl<T: 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: PartialEq + 'static> Memo<T> {
    /// Creates a derivation over whatever `f` reads.
    ///
    /// `f` does not run here; the first read runs it. `T: PartialEq` is what the cutoff needs:
    /// comparing the result stops a write propagating past a derivation it did not change.
    pub fn new(f: impl Fn() -> T + 'static) -> Self {
        let cache: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
        let into = Rc::clone(&cache);
        // Writes into the cache it already holds rather than replacing it, so a memo that
        // recomputes on every flush allocates nothing.
        let work = Rc::new(RefCell::new(move || {
            let next = f();
            let mut slot = into.borrow_mut();
            match &*slot {
                Some(prev) if *prev == next => false,
                _ => {
                    *slot = Some(next);
                    true
                }
            }
        }));
        Self { id: graph::mint(graph::MEMO, Some(cache), Some(work)), marker: PhantomData }
    }

    /// Returns this memo's identity.
    pub fn id(self) -> SignalId {
        self.id
    }

    /// Reads through `f`, resolving the memo first and registering a dependency.
    ///
    /// # Panics
    ///
    /// Panics if the memo has been disposed.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        graph::resolve(self.id);
        let cache = payload::<Option<T>>(self.id, true);
        let value = RefCell::borrow(&cache);
        f(value.as_ref().expect("a resolved memo holds its value"))
    }
}

impl<T: Clone + PartialEq + 'static> Memo<T> {
    /// Returns a clone of the current value, resolving the memo first and registering a
    /// dependency.
    pub fn get(self) -> T {
        self.with(T::clone)
    }
}

/// A leaf that performs a side effect when what it read changes. The point at which a value
/// reaches a sink.
///
/// Runs once when created, which collects its dependency set, and thereafter at the end of any
/// flush in which something it read moved. Effects run after every memo has resolved, so none
/// observes a half-updated graph.
#[derive(Copy, Clone)]
pub struct Effect {
    id: SignalId,
    /// Neither `Send` nor `Sync`: the closure lives in the app thread's graph.
    marker: PhantomData<*const ()>,
}

impl Effect {
    /// Runs `f` now, and again whenever what it read changes.
    pub fn new(f: impl FnMut() + 'static) -> Self {
        let effect = Self::install(f, 0);
        graph::resolve(effect.id);
        effect
    }

    /// A UI binding, which establishes its dependencies after the creation borrow has ended.
    pub(crate) fn deferred(f: impl FnMut() + 'static) -> Self {
        let effect = Self::install(f, 0);
        graph::schedule(effect.id);
        effect
    }

    /// A drawing effect, run by the host after the solve has settled rather than in the flush.
    pub(crate) fn geometry(f: impl FnMut() + 'static) -> Self {
        let effect = Self::install(f, graph::GEOMETRY);
        graph::schedule(effect.id);
        effect
    }

    fn install(mut f: impl FnMut() + 'static, phase: u8) -> Self {
        let work = Rc::new(RefCell::new(move || {
            f();
            false
        }));
        Self { id: graph::mint(graph::EFFECT | phase, None, Some(work)), marker: PhantomData }
    }

    /// Queues this effect for its phase's next drain, whatever it last read.
    pub(crate) fn schedule(self) {
        graph::schedule(self.id);
    }

    /// Disposes this effect and returns its closure, for the caller to drop after releasing its
    /// own borrow of the host.
    pub(crate) fn retire(self) -> Option<RetiredEffect> {
        graph::retire(self.id)
    }

    /// Returns this effect's identity.
    pub fn id(self) -> SignalId {
        self.id
    }
}

/// A disposal scope. Owns every [`Cell`], [`Memo`], [`Effect`] and nested scope created under
/// it, and disposes them in reverse creation order when dropped.
///
/// Every structural node — a mounted widget, a realized list row, an open flyout — owns one,
/// and unmounting drops it. Disposal is by scope alone; no signal carries an unsubscribe.
pub struct Owner(SignalId);

impl Owner {
    /// Runs `f` with a fresh scope installed, and returns the scope alongside `f`'s result.
    ///
    /// Dropping the returned `Owner` disposes everything `f` created.
    #[must_use = "dropping the scope immediately disposes everything it just created"]
    pub fn scope<R>(f: impl FnOnce() -> R) -> (Self, R) {
        let id = graph::mint(graph::SCOPE, None, None);
        let result = graph::in_scope(Some(id), f);
        (Self(id), result)
    }

    /// Runs `f` inside this scope, so what `f` creates is owned here rather than by the scope
    /// current at the call site.
    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        graph::in_scope(Some(self.0), f)
    }

    /// Runs `f` with no scope installed, so what `f` creates belongs to no enclosing scope and
    /// outlives every one of them. The caller takes responsibility for disposing it, which is
    /// what a scope `f` itself opens and returns is for.
    pub fn detached<R>(f: impl FnOnce() -> R) -> R {
        graph::in_scope(None, f)
    }

    /// Retains `value` until the current owner is disposed, without creating a signal.
    ///
    /// # Panics
    ///
    /// Panics outside an owner scope.
    pub fn retain<T: 'static>(value: T) -> Resource<T> {
        let id = graph::mint(graph::RESOURCE, Some(Rc::new(value)), None);
        Resource { id, marker: PhantomData }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        graph::dispose(self.0);
    }
}

/// An immutable, owner-scoped resource. Reads are untracked and never wake the graph. The
/// handle is app-thread only; clone an owned value to hand it to another thread.
pub struct Resource<T: 'static> {
    id: SignalId,
    marker: PhantomData<Rc<T>>,
}

impl<T: 'static> Copy for Resource<T> {}

impl<T: 'static> Clone for Resource<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Resource<T> {
    /// Borrows the live resource outside the graph borrow.
    ///
    /// # Panics
    ///
    /// Panics if its owner has been disposed.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        let value = graph::peek(self.id).expect("resource owner disposed");
        f(value.downcast_ref().expect("a handle names its payload's type"))
    }
}

impl<T: Clone + 'static> Resource<T> {
    /// Takes an owned clone, without creating a signal dependency.
    pub fn get(self) -> T {
        self.with(T::clone)
    }
}

/// [`Signal`] marker: the source is a constant.
pub struct IsValue;
/// [`Signal`] marker: the source is a [`Cell`].
pub struct IsCell;
/// [`Signal`] marker: the source is a [`Memo`].
pub struct IsMemo;
/// [`Signal`] marker: the source is a closure.
pub struct IsFn;

/// Anything readable as a `T`: a constant, a [`Cell`], a [`Memo`], or a closure.
///
/// One method accepts all four, so `opacity(0.6)` and `opacity(move || hover.get())` reach the
/// same `fn opacity<M>(self, v: impl Signal<f32, M>)`.
///
/// `Marker` separates the four impls, which would otherwise overlap: a closure is a value and
/// so is a `Cell`. It is inferred at every call site and never written.
pub trait Signal<T, Marker = IsValue> {
    /// Returns the current value, registering a dependency where the source has one.
    fn read(&self) -> T;

    /// Returns whether reading can ever produce a different answer. A caller binding a constant
    /// creates no [`Effect`] and no graph node for it.
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
