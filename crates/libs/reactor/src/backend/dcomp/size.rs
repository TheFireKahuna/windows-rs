//! Backend-agnostic element size notifications for the DirectComposition backend.
//!
//! On the WinUI backend a mounted element's size is reported by the XAML
//! `FrameworkElement.SizeChanged` event. The self-hosted DComp backend has no
//! XAML element — a node is a system-compositor `ContainerVisual` sized by the
//! Taffy layout pass. This module bridges the gap: a viz host (a
//! `SurfacePainter` / composition surface) subscribes through
//! [`ElementHandle::on_size_changed`](crate::ElementHandle::on_size_changed),
//! which lands here, and the layout pass calls [`fire_element_size`] whenever a
//! node's laid-out size changes.
//!
//! The subscription is keyed by the node container's canonical `IUnknown`
//! identity, so the value returned to the caller is an ordinary
//! [`EventRevoker`](windows_core::EventRevoker) that unregisters on drop — the
//! call site is identical to the XAML path.

use core::ffi::c_void;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows_core::{Error, EventRevoker, Interface, Result, HRESULT, IUnknown};

use crate::system_bindings::ContainerVisual;

const S_OK: HRESULT = HRESULT(0);

struct Entry {
    token: i64,
    /// Canonical `IUnknown` pointer of the node's container visual (the identity
    /// key shared with the layout pass).
    key: *mut c_void,
    cb: Rc<dyn Fn(f32, f32)>,
    /// Last delivered size; the layout pass fires only on a change.
    last: Cell<(f32, f32)>,
}

thread_local! {
    static LISTENERS: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
    static NEXT_TOKEN: Cell<i64> = const { Cell::new(1) };
}

/// Canonical COM identity pointer for `obj` (the value the layout pass and the
/// registration both key on). `QueryInterface(IUnknown)` returns the same pointer
/// regardless of which interface the object was reached through.
fn identity(obj: &impl Interface) -> Option<*mut c_void> {
    obj.cast::<IUnknown>().ok().map(|u| u.as_raw())
}

/// Subscribe to size changes of the node whose container is `cv`. The returned
/// [`EventRevoker`] unregisters on drop.
pub(crate) fn register_element_size(
    cv: &ContainerVisual,
    f: impl Fn(f32, f32) + 'static,
) -> Result<EventRevoker> {
    let key = identity(cv).ok_or_else(Error::empty)?;
    let token = NEXT_TOKEN.with(|t| {
        let v = t.get();
        t.set(v + 1);
        v
    });
    let cb: Rc<dyn Fn(f32, f32)> = Rc::new(f);
    // The node may already be laid out (registration runs from a mount callback,
    // which can land after the layout pass that sized it). Deliver the current
    // size immediately — the "fires once after the first layout pass" contract —
    // and seed `last` with it so the next layout pass doesn't re-deliver. A
    // not-yet-laid-out node reads (0,0): keep the NaN seed so the first real
    // layout pass delivers.
    let current = cv
        .cast::<crate::system_bindings::IVisual>()
        .ok()
        .and_then(|v| v.Size().ok())
        .map(|s| (s.x, s.y))
        .filter(|&(w, h)| w > 0.0 && h > 0.0);
    LISTENERS.with(|l| {
        l.borrow_mut().push(Entry {
            token,
            key,
            cb: cb.clone(),
            last: Cell::new(current.unwrap_or((f32::NAN, f32::NAN))),
        })
    });
    if let Some((w, h)) = current {
        cb(w, h);
    }
    // The revoker holds a ref to the container as its `source` (kept alive for the
    // subscription); `remove` ignores it and unregisters purely by `token`.
    let source: IUnknown = cv.cast()?;
    Ok(EventRevoker::new(source, token, remove))
}

/// Deliver a size change for the node whose container has canonical identity
/// `key`. Called by the layout pass when a node's laid-out size changes. Cheap
/// when there are no listeners (the common case).
pub(crate) fn fire_element_size(key: *mut c_void, w: f32, h: f32) {
    // Collect the callbacks to run, then drop the borrow before invoking them
    // (a callback marks reactor state dirty; keep the registry borrow off the
    // stack while user code runs).
    let to_run: Vec<Rc<dyn Fn(f32, f32)>> = LISTENERS.with(|l| {
        l.borrow()
            .iter()
            .filter(|e| e.key == key && e.last.get() != (w, h))
            .map(|e| {
                e.last.set((w, h));
                e.cb.clone()
            })
            .collect()
    });
    for cb in to_run {
        cb(w, h);
    }
}

/// Whether any size listener is currently registered. The layout pass uses this
/// to skip the per-node identity cast entirely when nothing is subscribed.
pub(crate) fn has_listeners() -> bool {
    LISTENERS.with(|l| !l.borrow().is_empty())
}

/// Canonical identity of a node container, for the layout pass to match against
/// registered keys.
pub(crate) fn container_key(cv: &ContainerVisual) -> Option<*mut c_void> {
    identity(cv)
}

/// `EventRevoker` removal hook: unregister the entry for `token`. The `source`
/// pointer (the container) is unused — the token is the identity.
unsafe extern "system" fn remove(_source: *mut c_void, token: i64) -> HRESULT {
    LISTENERS.with(|l| l.borrow_mut().retain(|e| e.token != token));
    S_OK
}
