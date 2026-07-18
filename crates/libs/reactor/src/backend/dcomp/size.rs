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
//! Subscriptions are keyed by [`ControlId`], not by the container's COM
//! identity: the layout pass already carries the id, so matching costs a hash
//! rather than a `QueryInterface` per node per pass, and the handle handed back
//! to app code holds no COM — nothing pins the subscriber to this thread.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustc_hash::FxHashMap;
use windows_core::Result;

use crate::backend::ControlId;
use crate::widgets::Subscription;

struct Entry {
    token: i64,
    cb: Rc<dyn Fn(f32, f32)>,
    /// Last delivered size; the layout pass fires only on a change. Seeded NaN
    /// so the first pass after registration always delivers.
    last: Cell<(f32, f32)>,
}

thread_local! {
    static LISTENERS: RefCell<FxHashMap<ControlId, Vec<Entry>>> =
        RefCell::new(FxHashMap::default());
    static NEXT_TOKEN: Cell<i64> = const { Cell::new(1) };
}

/// Subscribe to size changes of the node `id`. The returned [`Subscription`]
/// unregisters on drop.
///
/// The current size is not read here — that would mean reaching into the arena
/// from inside the reconcile borrow. Instead `last` is seeded `NaN`, which no
/// real size compares equal to, so the next layout pass delivers unconditionally
/// and the "fires once after the first layout pass" contract holds. Layout runs
/// at the end of every reconcile, so a subscription made while rendering is
/// always serviced within the same frame.
pub(crate) fn register_element_size(
    id: ControlId,
    f: impl Fn(f32, f32) + 'static,
) -> Result<Subscription> {
    let token = NEXT_TOKEN.with(|t| {
        let v = t.get();
        t.set(v + 1);
        v
    });
    LISTENERS.with(|l| {
        l.borrow_mut().entry(id).or_default().push(Entry {
            token,
            cb: Rc::new(f),
            last: Cell::new((f32::NAN, f32::NAN)),
        })
    });
    Ok(Subscription::token(token, remove))
}

/// Deliver a size change for node `id`. Called by the layout pass for every
/// node it assigns; cheap when nothing is subscribed (the common case).
pub(crate) fn fire_element_size(id: ControlId, w: f32, h: f32) {
    // Collect the callbacks to run, then drop the borrow before invoking them
    // (a callback marks reactor state dirty; keep the registry borrow off the
    // stack while user code runs).
    let to_run: Vec<Rc<dyn Fn(f32, f32)>> = LISTENERS.with(|l| {
        let map = l.borrow();
        let Some(entries) = map.get(&id) else {
            return Vec::new();
        };
        entries
            .iter()
            .filter(|e| e.last.get() != (w, h))
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
/// to skip the per-node lookup entirely when nothing is subscribed.
pub(crate) fn has_listeners() -> bool {
    LISTENERS.with(|l| !l.borrow().is_empty())
}

/// Drop every subscription for `id`. Called when the node is destroyed, so a
/// control that goes away without its subscriber dropping the handle cannot
/// leave entries behind for an id that no longer exists.
pub(crate) fn forget(id: ControlId) {
    LISTENERS.with(|l| {
        l.borrow_mut().remove(&id);
    });
}

/// [`Subscription`] removal hook: unregister the entry for `token`.
fn remove(token: i64) {
    LISTENERS.with(|l| {
        let mut map = l.borrow_mut();
        for entries in map.values_mut() {
            entries.retain(|e| e.token != token);
        }
        map.retain(|_, entries| !entries.is_empty());
    });
}
