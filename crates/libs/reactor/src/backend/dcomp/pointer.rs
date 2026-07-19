//! Capture-capable pointer subscriptions for viz surfaces on the
//! DirectComposition backend.
//!
//! On the WinUI backend a custom-drawn control (knob / slider / EQ canvas)
//! opens a [`PointerSurface`](crate::PointerSurface) over its hosting XAML
//! `UIElement` and receives `PointerPressed/Moved/Released/WheelChanged` with
//! capture. The self-hosted backend has no XAML element — the mounted native
//! object is the node's system-compositor `ContainerVisual`. This module is the
//! bridge, mirroring `size.rs`: `PointerSurface` registers a sink set keyed by
//! [`ControlId`], and the backend's input router (`input.rs`) delivers
//! element-relative pointer events to the deepest registered surface under the
//! pointer, with implicit capture for the press-to-release span of a drag. The
//! hit-test walk already carries the id, so matching costs no COM call.
//!
//! **The closures live app-side; the router reads only plain bits.** The sink
//! set is split across the same seam the event handlers ride (`record.rs`):
//!
//! * The [`PointerSinks`] closures stay in the app-side [`SINKS`] map. Nothing
//!   on the input path touches them — they are consulted **only** by the
//!   recorder's intent drain, which clones one into a deferred
//!   [`IntentJob`](super::record::IntentJob) that runs after the input borrow
//!   is released. That is why a cell holds an `Rc<dyn Fn>`, not a `Box`.
//! * The router routes on [`SurfaceInterest`] — the plain presence bits — read
//!   from the front-side [`INTEREST`] map. Because the cells fill *after*
//!   registration (the `on_down`/`on_move`/… builders set them), each fill
//!   redeclares the bits through the [`OPS`] queue, mirroring `crate::surface`:
//!   a `Send` declaration path so the two halves can later live on different
//!   threads. The router services that queue once per frame, so a freshly
//!   filled bit is visible on the next input message — one frame stale by
//!   design (the declared-ahead-of-time model).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Mutex;

use rustc_hash::FxHashMap;
use windows_core::Result;

use crate::backend::ControlId;
use crate::style::PointerEventInfo;
use crate::widgets::Subscription;

/// The four pointer transitions plus hover-exit a `PointerSurface` can
/// subscribe. Cells are filled by the surface's `on_down`/`on_move`/`on_up`/
/// `on_wheel`/`on_exit` builders after registration.
///
/// Each cell holds an `Rc<dyn Fn>`, not a `Box`: the recorder's intent drain
/// clones the closure out to run it a hop later, after the input borrow is
/// released, exactly as it clones an event handler's [`Callback`](crate::interaction::Callback).
#[derive(Default)]
pub struct PointerSinks {
    pub down: RefCell<Option<Rc<dyn Fn(PointerEventInfo)>>>,
    pub moved: RefCell<Option<Rc<dyn Fn(PointerEventInfo)>>>,
    pub up: RefCell<Option<Rc<dyn Fn(PointerEventInfo)>>>,
    pub wheel: RefCell<Option<Rc<dyn Fn(PointerEventInfo)>>>,
    /// Fired when the hover leaves this surface's bounds (another surface, none,
    /// or the window edge). Hover-only: a captured drag suppresses hover routing
    /// until release, so no exit fires mid-drag.
    pub exited: RefCell<Option<Rc<dyn Fn()>>>,
}

impl PointerSinks {
    /// The front-side presence declaration for the currently-filled cells.
    fn interest(&self) -> SurfaceInterest {
        SurfaceInterest {
            down: self.down.borrow().is_some(),
            moved: self.moved.borrow().is_some(),
            up: self.up.borrow().is_some(),
            wheel: self.wheel.borrow().is_some(),
            exited: self.exited.borrow().is_some(),
        }
    }
}

/// Which of a surface's sinks are filled — the plain-data declaration the input
/// router routes on. `Send` (unlike the closures), so it crosses the [`OPS`]
/// queue into the front-side interest map.
///
/// Spec §6.1: a surface with no `down` sink stays click-transparent, and wheel
/// routing gates on `wheel`; the router reads these bits and never the closure.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub(crate) struct SurfaceInterest {
    pub down: bool,
    pub moved: bool,
    pub up: bool,
    pub wheel: bool,
    pub exited: bool,
}

struct Entry {
    token: i64,
    sinks: Rc<PointerSinks>,
}

thread_local! {
    /// App-side: the sink closures, by node. Consulted only by the recorder's
    /// intent drain ([`sinks_for`]); the router never reaches it.
    static SINKS: RefCell<FxHashMap<ControlId, Entry>> =
        RefCell::new(FxHashMap::default());
    /// Front-side: the presence bits the router routes on, refreshed from
    /// [`OPS`] once per frame ([`service_ops`]). One frame stale by design.
    static INTEREST: RefCell<FxHashMap<ControlId, SurfaceInterest>> =
        RefCell::new(FxHashMap::default());
    static NEXT_TOKEN: Cell<i64> = const { Cell::new(1) };
}

/// A presence declaration crossing app→front. A plain `Mutex` rather than a
/// thread-local, mirroring `crate::surface`: the cell is filled wherever the
/// surface's builder runs, which is not necessarily the thread that routes
/// input.
enum Op {
    Declare { id: ControlId, interest: SurfaceInterest },
    Forget { id: ControlId },
}

static OPS: Mutex<Vec<Op>> = Mutex::new(Vec::new());

fn push_op(op: Op) {
    if let Ok(mut ops) = OPS.lock() {
        ops.push(op);
    }
}

/// Register a pointer-sink set for node `id`. Returns the (initially empty)
/// sinks to fill and a [`Subscription`] that unregisters on drop.
///
/// No interest is declared here — the cells are all empty, so the surface is
/// inert until a builder fills one and [`declare`]s it. One sink set per node: a
/// second registration for the same id replaces the first, matching the previous
/// behaviour where the newer entry shadowed the older in the lookup scan.
pub(crate) fn register_element_pointer(
    id: ControlId,
) -> Result<(Rc<PointerSinks>, Subscription)> {
    let sinks = Rc::new(PointerSinks::default());
    let token = NEXT_TOKEN.with(|t| {
        let v = t.get();
        t.set(v + 1);
        v
    });
    SINKS.with(|l| {
        l.borrow_mut().insert(
            id,
            Entry {
                token,
                sinks: Rc::clone(&sinks),
            },
        )
    });
    Ok((sinks, Subscription::token(token, remove)))
}

/// Redeclare `id`'s sink presence to the router. Called by the surface's
/// `on_*` builders after a cell is filled: the cells fill *after* registration,
/// so the router learns the bits at fill time, not register time.
pub(crate) fn declare(id: ControlId, sinks: &PointerSinks) {
    push_op(Op::Declare {
        id,
        interest: sinks.interest(),
    });
}

/// Apply the queued declarations into the front-side interest map. Runs once
/// per frame, after the reconcile buffer is replayed, so a bit filled during a
/// render is visible to the next input message. Cheap when the queue is empty
/// (the common case).
pub(crate) fn service_ops() {
    let ops = match OPS.lock() {
        Ok(mut q) if !q.is_empty() => std::mem::take(&mut *q),
        _ => return,
    };
    INTEREST.with(|m| {
        let mut m = m.borrow_mut();
        for op in ops {
            match op {
                Op::Declare { id, interest } => {
                    m.insert(id, interest);
                }
                Op::Forget { id } => {
                    m.remove(&id);
                }
            }
        }
    });
}

/// Whether any surface presence is declared — lets the input router skip the
/// surface walk entirely in the common case.
pub(crate) fn has_listeners() -> bool {
    INTEREST.with(|m| !m.borrow().is_empty())
}

/// The declared presence bits for node `id` (front-side, plain data).
pub(crate) fn interest_for(id: ControlId) -> Option<SurfaceInterest> {
    INTEREST.with(|m| m.borrow().get(&id).copied())
}

/// The sink closures registered for node `id` (app-side). Only the recorder's
/// intent drain calls this; the router routes on [`interest_for`] instead.
pub(crate) fn sinks_for(id: ControlId) -> Option<Rc<PointerSinks>> {
    SINKS.with(|l| l.borrow().get(&id).map(|e| Rc::clone(&e.sinks)))
}

/// Drop the registration for `id`. Called when the node is destroyed so a
/// leaked [`Subscription`] cannot keep sinks alive for a dead id. Drops the
/// app-side closures now and queues a [`Op::Forget`] so the front-side bits are
/// cleared in order behind any declaration still pending for this id.
pub(crate) fn forget(id: ControlId) {
    SINKS.with(|l| {
        l.borrow_mut().remove(&id);
    });
    push_op(Op::Forget { id });
}

/// [`Subscription`] drop: unregister the app-side entry for `token` and forget
/// its front-side bits.
fn remove(token: i64) {
    let removed = SINKS.with(|l| {
        let mut map = l.borrow_mut();
        let id = map.iter().find(|(_, e)| e.token == token).map(|(id, _)| *id);
        if let Some(id) = id {
            map.remove(&id);
        }
        id
    });
    if let Some(id) = removed {
        push_op(Op::Forget { id });
    }
}
