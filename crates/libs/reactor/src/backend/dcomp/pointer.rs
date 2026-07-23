//! The front-side gesture registry for viz surfaces on the DirectComposition
//! backend.
//!
//! A custom-drawn control (knob / slider / EQ canvas) has no XAML element to
//! subscribe — the mounted native object is the node's system-compositor
//! `ContainerVisual`. This module is the bridge: a gesture is registered against
//! a [`ControlId`], and the backend's input router (`input.rs`) delivers
//! element-relative transitions to the deepest registered surface under the
//! pointer, with implicit capture for the press-to-release span of a drag. The
//! hit-test walk already carries the id, so matching costs no COM call.
//!
//! ## The gesture runs *here*, on the input thread
//!
//! This is the whole point, and the reason the module looks the way it does. The
//! handler is `Send` and lives front-side, so a pointer move produces its visual
//! inside the input router — no thread hop, no reconcile in the path. What
//! crosses to the app afterwards is a bare notification that the gesture has
//! news; the app drains the newest action from a slot the handler captured.
//!
//! The sink design this replaces put the closures app-side and routed on
//! separate presence bits, which meant a move produced *no* pixels until the
//! intent had crossed the seam and the app had run. See [`crate::gesture`] for
//! why that was a rule violation rather than a tuning problem.
//!
//! ## Registration still crosses a seam
//!
//! A gesture is declared where its element mounts — an effect on the app thread
//! — and consumed by the router on the front thread. So declarations ride a
//! `Send` [`OPS`] queue that the front services once per frame. A gesture declared
//! during a render is therefore routed from the next input message: one frame of
//! registration latency, by design, and unchanged from the sink model.
//!
//! What *did* change is that a declaration is now atomic. The old sinks filled
//! one cell per builder call and redeclared after each, so a surface could be
//! routed to while still half-wired; a gesture arrives whole or not at all.

use std::cell::{Cell, RefCell};
use std::sync::Mutex;

use rustc_hash::FxHashMap;

use crate::backend::ControlId;
use crate::gesture::{GestureEvent, GestureInterest, GestureOutcome};
use crate::interaction::Callback;
use crate::widgets::Subscription;

/// The boxed front-side handler. `FnMut` because a gesture owns its own live
/// state (drag anchor, hovered index) and mutates it in place; `Send` because it
/// is declared on one thread and run on another — and, more importantly,
/// because that bound is what stops app-thread state being captured at all.
pub(crate) type GestureFn = Box<dyn FnMut(GestureEvent) -> GestureOutcome + Send>;

struct Entry {
    interest: GestureInterest,
    gesture: GestureFn,
}

thread_local! {
    /// Front-side: the gestures, by node. Both the routing bits and the handler
    /// live here — there is no app-side half any more.
    static GESTURES: RefCell<FxHashMap<ControlId, Entry>> =
        RefCell::new(FxHashMap::default());
}

thread_local! {
    /// App-side: the action-drain callbacks, by node.
    ///
    /// **Not in the input path.** Nothing here is reachable from the router; a
    /// callback is consulted only when an [`Intent::Gesture`](super::record::Intent::Gesture)
    /// is drained, which is already a hop past the visual. It holds a `Callback`
    /// (an `Rc`) precisely because it never has to cross a thread — that is what
    /// distinguishes it from the gesture itself, and the distinction is the
    /// design.
    static ACTIONS: RefCell<FxHashMap<ControlId, ActionEntry>> =
        RefCell::new(FxHashMap::default());
    static NEXT_TOKEN: Cell<i64> = const { Cell::new(1) };
}

struct ActionEntry {
    token: i64,
    cb: Callback<()>,
}

/// Register the app-side drain for `id` — the callback that reads the newest
/// action out of the gesture's slot. Returns a [`Subscription`] that
/// unregisters on drop.
pub(crate) fn register_action(id: ControlId, cb: Callback<()>) -> Subscription {
    let token = NEXT_TOKEN.with(|t| {
        let v = t.get();
        t.set(v + 1);
        v
    });
    ACTIONS.with(|m| m.borrow_mut().insert(id, ActionEntry { token, cb }));
    Subscription::token(token, remove_action)
}

/// The app-side drain for node `id`, if one is registered.
pub(crate) fn action_for(id: ControlId) -> Option<Callback<()>> {
    ACTIONS.with(|m| m.borrow().get(&id).map(|e| e.cb.clone()))
}

/// [`Subscription`] drop: unregister the drain holding `token`.
fn remove_action(token: i64) {
    ACTIONS.with(|m| {
        let mut map = m.borrow_mut();
        if let Some(id) = map.iter().find(|(_, e)| e.token == token).map(|(id, _)| *id) {
            map.remove(&id);
        }
    });
}

/// A declaration crossing app→front. A plain `Mutex` rather than a thread-local:
/// the declaring thread (wherever the element mounts) is not the routing thread.
enum Op {
    Declare { id: ControlId, interest: GestureInterest, gesture: GestureFn },
    Forget { id: ControlId },
}

static OPS: Mutex<Vec<Op>> = Mutex::new(Vec::new());

fn push_op(op: Op) {
    if let Ok(mut ops) = OPS.lock() {
        ops.push(op);
    }
}

/// Declare `id`'s gesture to the router. One gesture per node: a second
/// declaration replaces the first.
///
/// A gesture interested in nothing is still recorded, so that forgetting it
/// later is symmetric; the router simply never matches it ([`GestureInterest::any`]).
pub(crate) fn declare(id: ControlId, interest: GestureInterest, gesture: GestureFn) {
    push_op(Op::Declare { id, interest, gesture });
}

/// Drop the declaration for `id`. Called when the node is destroyed, and by a
/// dropped subscription, so a dead id cannot keep a gesture alive.
pub(crate) fn forget(id: ControlId) {
    push_op(Op::Forget { id });
}

/// Apply the queued declarations. Runs once per frame, after the reconcile
/// buffer is replayed, so a gesture declared during a render is visible to the
/// next input message. Cheap when the queue is empty — the common case.
pub(crate) fn service_ops() {
    let ops = match OPS.lock() {
        Ok(mut q) if !q.is_empty() => std::mem::take(&mut *q),
        _ => return,
    };
    GESTURES.with(|m| {
        let mut m = m.borrow_mut();
        for op in ops {
            match op {
                Op::Declare { id, interest, gesture } => {
                    m.insert(id, Entry { interest, gesture });
                }
                Op::Forget { id } => {
                    m.remove(&id);
                }
            }
        }
    });
}

/// Whether any gesture is declared — lets the input router skip the surface
/// walk entirely in the common case.
pub(crate) fn has_listeners() -> bool {
    GESTURES.with(|m| !m.borrow().is_empty())
}

/// The declared routing bits for node `id`.
pub(crate) fn interest_for(id: ControlId) -> Option<GestureInterest> {
    GESTURES.with(|m| m.borrow().get(&id).map(|e| e.interest))
}

/// Run node `id`'s gesture with `event`, on this (the input) thread.
///
/// Returns the gesture's outcome, or `None` when no gesture is registered —
/// which the router treats as "nothing to notify", not as an error: an element
/// can be unregistered between a press and the release that follows it.
///
/// ## Re-entrancy
///
/// The map is borrowed mutably for the duration of the call, because the handler
/// is `FnMut`. A gesture that re-entered this function would panic — but it
/// cannot: it is handed only the transition, with no route back to the backend,
/// the arena, or this module. The `Send` bound and the bare signature together
/// make the re-entrant call unwritable rather than merely unwise.
pub(crate) fn dispatch(id: ControlId, event: GestureEvent) -> Option<GestureOutcome> {
    GESTURES.with(|m| {
        let mut m = m.borrow_mut();
        m.get_mut(&id).map(|e| (e.gesture)(event))
    })
}
