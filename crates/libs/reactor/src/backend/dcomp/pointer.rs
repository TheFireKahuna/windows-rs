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

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustc_hash::FxHashMap;
use windows_core::Result;

use crate::backend::ControlId;
use crate::style::PointerEventInfo;
use crate::widgets::Subscription;

/// The four pointer transitions a `PointerSurface` can subscribe. Cells are
/// filled by the surface's `on_down`/`on_move`/`on_up`/`on_wheel` builders
/// after registration.
#[derive(Default)]
pub struct PointerSinks {
    pub down: RefCell<Option<Box<dyn Fn(PointerEventInfo)>>>,
    pub moved: RefCell<Option<Box<dyn Fn(PointerEventInfo)>>>,
    pub up: RefCell<Option<Box<dyn Fn(PointerEventInfo)>>>,
    pub wheel: RefCell<Option<Box<dyn Fn(PointerEventInfo)>>>,
    /// Fired when the hover leaves this surface's bounds (another surface, none,
    /// or the window edge). Hover-only: a captured drag suppresses hover routing
    /// until release, so no exit fires mid-drag.
    pub exited: RefCell<Option<Box<dyn Fn()>>>,
}

struct Entry {
    token: i64,
    sinks: Rc<PointerSinks>,
}

thread_local! {
    static LISTENERS: RefCell<FxHashMap<ControlId, Entry>> =
        RefCell::new(FxHashMap::default());
    static NEXT_TOKEN: Cell<i64> = const { Cell::new(1) };
}

/// Register a pointer-sink set for node `id`. Returns the (initially empty)
/// sinks to fill and a [`Subscription`] that unregisters on drop.
///
/// One sink set per node: a second registration for the same id replaces the
/// first, matching the previous behaviour where the newer entry shadowed the
/// older in the lookup scan.
pub(crate) fn register_element_pointer(
    id: ControlId,
) -> Result<(Rc<PointerSinks>, Subscription)> {
    let sinks = Rc::new(PointerSinks::default());
    let token = NEXT_TOKEN.with(|t| {
        let v = t.get();
        t.set(v + 1);
        v
    });
    LISTENERS.with(|l| {
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

/// Whether any pointer surface is registered — lets the input router skip the
/// surface walk entirely in the common case.
pub(crate) fn has_listeners() -> bool {
    LISTENERS.with(|l| !l.borrow().is_empty())
}

/// The sink set registered for node `id`.
pub(crate) fn sinks_for(id: ControlId) -> Option<Rc<PointerSinks>> {
    LISTENERS.with(|l| l.borrow().get(&id).map(|e| Rc::clone(&e.sinks)))
}

/// Drop the registration for `id`. Called when the node is destroyed so a
/// leaked [`Subscription`] cannot keep sinks alive for a dead id.
pub(crate) fn forget(id: ControlId) {
    LISTENERS.with(|l| {
        l.borrow_mut().remove(&id);
    });
}

fn remove(token: i64) {
    LISTENERS.with(|l| l.borrow_mut().retain(|_, e| e.token != token));
}
