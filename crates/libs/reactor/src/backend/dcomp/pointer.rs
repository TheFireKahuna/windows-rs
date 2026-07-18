//! Capture-capable pointer subscriptions for viz surfaces on the
//! DirectComposition backend.
//!
//! On the WinUI backend a custom-drawn control (knob / slider / EQ canvas)
//! opens a [`PointerSurface`](crate::PointerSurface) over its hosting XAML
//! `UIElement` and receives `PointerPressed/Moved/Released/WheelChanged` with
//! capture. The self-hosted backend has no XAML element — the mounted native
//! object is the node's system-compositor `ContainerVisual`. This module is the
//! bridge, mirroring `size.rs`: `PointerSurface` registers a sink set keyed by
//! the container's canonical `IUnknown` identity, and the backend's input
//! router (`input.rs`) delivers element-relative pointer events to the deepest
//! registered surface under the pointer, with implicit capture for the
//! press-to-release span of a drag.

use core::ffi::c_void;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows_core::{Error, EventRevoker, Interface, Result, HRESULT, IUnknown};

use crate::style::PointerEventInfo;
use crate::system_bindings::ContainerVisual;

const S_OK: HRESULT = HRESULT(0);

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
    /// Canonical `IUnknown` pointer of the surface's container visual (matches
    /// `Node::ident`).
    key: *mut c_void,
    sinks: Rc<PointerSinks>,
}

thread_local! {
    static LISTENERS: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
    static NEXT_TOKEN: Cell<i64> = const { Cell::new(1) };
}

/// Register a pointer-sink set for the node whose container is `cv`. Returns
/// the (initially empty) sinks to fill and an [`EventRevoker`] that
/// unregisters on drop.
pub(crate) fn register_element_pointer(
    cv: &ContainerVisual,
) -> Result<(Rc<PointerSinks>, EventRevoker)> {
    let key = cv
        .cast::<IUnknown>()
        .ok()
        .map(|u| u.as_raw())
        .ok_or_else(Error::empty)?;
    let sinks = Rc::new(PointerSinks::default());
    let token = NEXT_TOKEN.with(|t| {
        let v = t.get();
        t.set(v + 1);
        v
    });
    LISTENERS.with(|l| {
        l.borrow_mut().push(Entry {
            token,
            key,
            sinks: Rc::clone(&sinks),
        })
    });
    let source: IUnknown = cv.cast()?;
    Ok((sinks, EventRevoker::new(source, token, remove)))
}

/// Whether any pointer surface is registered — lets the input router skip the
/// surface walk entirely in the common case.
pub(crate) fn has_listeners() -> bool {
    LISTENERS.with(|l| !l.borrow().is_empty())
}

/// The sink set registered for the container with canonical identity `key`.
pub(crate) fn sinks_for(key: *mut c_void) -> Option<Rc<PointerSinks>> {
    LISTENERS.with(|l| {
        l.borrow()
            .iter()
            .find(|e| e.key == key)
            .map(|e| Rc::clone(&e.sinks))
    })
}

unsafe extern "system" fn remove(_source: *mut c_void, token: i64) -> HRESULT {
    LISTENERS.with(|l| l.borrow_mut().retain(|e| e.token != token));
    S_OK
}
