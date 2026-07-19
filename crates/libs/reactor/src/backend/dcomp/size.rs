//! Backend-agnostic element size notifications for the DirectComposition backend.
//!
//! On the WinUI backend a mounted element's size is reported by the XAML
//! `FrameworkElement.SizeChanged` event. The self-hosted DComp backend has no
//! XAML element — a node is a system-compositor `ContainerVisual` sized by the
//! Taffy layout pass. This module bridges the gap: a viz host (a
//! `SurfacePainter` / composition surface) subscribes through
//! [`ElementHandle::on_size_changed`](crate::ElementHandle::on_size_changed),
//! which lands here, and the layout pass calls [`fire_element_size`] for every
//! node it solves.
//!
//! The two ends live on different threads under the render-thread split: the
//! layout solve runs on the **front** thread while the subscribers are app
//! closures (`Rc<dyn Fn>`) registered on the **app** thread, resizing
//! app-owned viz surfaces. So the module is split the same way `pointer.rs`
//! is: the `!Send` callbacks stay in a thread-local registry on the thread
//! that registered them, the fire site consults only a `Send` id-set
//! ([`SUBSCRIBED`]) and queues plain `(id, w, h)` triples, and a host-installed
//! delivery hook ([`set_delivery`]) carries one coalesced "drain the queue"
//! job to the registering thread. With no hook installed (headless tests, a
//! single-threaded host) the fire delivers inline, exactly as it always did.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rustc_hash::{FxHashMap, FxHashSet};
use windows_core::Result;

use crate::backend::ControlId;
use crate::widgets::Subscription;

struct Entry {
    token: i64,
    cb: Rc<dyn Fn(f32, f32)>,
    /// Last delivered size; delivery fires only on a change. Seeded NaN so the
    /// first pass after registration always delivers.
    last: Cell<(f32, f32)>,
}

thread_local! {
    /// The subscriber callbacks, on the thread that registered them (the app
    /// thread once a host is live). `Rc` — never crosses.
    static LISTENERS: RefCell<FxHashMap<ControlId, Vec<Entry>>> =
        RefCell::new(FxHashMap::default());
    static NEXT_TOKEN: Cell<i64> = const { Cell::new(1) };
}

/// The ids with at least one live subscriber — the `Send` face of
/// [`LISTENERS`] the fire site filters on, so the solve queues triples only
/// for nodes somebody actually watches — typically a handful, against hundreds
/// solved.
static SUBSCRIBED: Mutex<Option<FxHashSet<ControlId>>> = Mutex::new(None);

/// One queued cross-thread notification, applied in order by
/// [`deliver_pending`].
enum Pending {
    Size(ControlId, f32, f32),
    Forget(ControlId),
}

static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());

/// Host-installed hook that schedules [`deliver_pending`] on the registering
/// (app) thread. `None` = deliver inline on the firing thread.
static DELIVERY: Mutex<Option<Arc<dyn Fn() + Send + Sync>>> = Mutex::new(None);

/// One delivery job in flight at a time: the fire site posts through
/// [`DELIVERY`] only on the queue's empty→non-empty edge.
static QUEUED: AtomicBool = AtomicBool::new(false);

fn subscribed_insert(id: ControlId) {
    if let Ok(mut s) = SUBSCRIBED.lock() {
        s.get_or_insert_default().insert(id);
    }
}

fn subscribed_remove(id: ControlId) {
    if let Ok(mut s) = SUBSCRIBED.lock()
        && let Some(set) = s.as_mut()
    {
        set.remove(&id);
    }
}

fn is_subscribed(id: ControlId) -> bool {
    SUBSCRIBED
        .lock()
        .is_ok_and(|s| s.as_ref().is_some_and(|set| set.contains(&id)))
}

/// Install (or clear) the delivery hook that carries queued size events to the
/// thread owning the subscriber registry. The hook must be cheap and
/// non-blocking (post a job, signal a queue); it fires at most once per
/// pending batch.
pub(crate) fn set_delivery(hook: Option<Arc<dyn Fn() + Send + Sync>>) {
    if let Ok(mut slot) = DELIVERY.lock() {
        *slot = hook;
    }
}

fn delivery() -> Option<Arc<dyn Fn() + Send + Sync>> {
    DELIVERY.lock().ok().and_then(|d| d.clone())
}

/// Subscribe to size changes of the node `id`. The returned [`Subscription`]
/// unregisters on drop.
///
/// The current size is not read here — that would mean reaching into the arena
/// from inside the reconcile borrow. Instead `last` is seeded `NaN`, which no
/// real size compares equal to, so the next layout pass delivers unconditionally
/// and the "fires once after the first layout pass" contract holds. Layout runs
/// on the front thread after every reconcile's commit is replayed, so a
/// subscription made while rendering is serviced with that same frame's
/// solve — one delivery hop later.
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
    subscribed_insert(id);
    Ok(Subscription::token(token, remove))
}

/// Report a node's solved size. Called by the layout pass for every node it
/// solves; cheap when the id has no subscriber (the common case — the caller
/// additionally gates on [`has_listeners`] to skip even this lookup while
/// nothing at all is subscribed).
///
/// With a delivery hook installed the triple is queued and the hook schedules
/// [`deliver_pending`] on the subscriber thread; without one it delivers
/// inline on this thread.
pub(crate) fn fire_element_size(id: ControlId, w: f32, h: f32) {
    if !is_subscribed(id) {
        return;
    }
    let Some(hook) = delivery() else {
        deliver_one(id, w, h);
        return;
    };
    if let Ok(mut q) = PENDING.lock() {
        q.push(Pending::Size(id, w, h));
    }
    if !QUEUED.swap(true, Ordering::AcqRel) {
        hook();
    }
}

/// Drain the queued notifications into this thread's subscriber registry.
/// Must run on the thread that owns [`LISTENERS`] (the app thread); the
/// host's delivery hook posts exactly this.
pub(crate) fn deliver_pending() {
    // Clear the edge first: a fire racing this drain either lands in the take
    // below or re-posts a fresh job — never lost between the two.
    QUEUED.store(false, Ordering::Release);
    let pending = match PENDING.lock() {
        Ok(mut q) if !q.is_empty() => std::mem::take(&mut *q),
        _ => return,
    };
    for p in pending {
        match p {
            Pending::Size(id, w, h) => deliver_one(id, w, h),
            Pending::Forget(id) => {
                LISTENERS.with(|l| {
                    l.borrow_mut().remove(&id);
                });
            }
        }
    }
}

/// Deliver one size to `id`'s subscribers on this thread, dropping the
/// registry borrow before invoking them (a callback marks reactor state
/// dirty; keep the borrow off the stack while user code runs). De-duplicated
/// per listener against the last delivered size, so a solve that re-reports
/// an unchanged node is a no-op.
fn deliver_one(id: ControlId, w: f32, h: f32) {
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

/// Whether any size listener is currently registered, on any thread. The
/// layout pass uses this to skip the per-node subscription lookup entirely
/// when nothing is subscribed.
pub(crate) fn has_listeners() -> bool {
    SUBSCRIBED
        .lock()
        .is_ok_and(|s| s.as_ref().is_some_and(|set| !set.is_empty()))
}

/// Drop every subscription for `id`. Called when the node is destroyed (on the
/// front thread), so a control that goes away without its subscriber dropping
/// the handle cannot leave entries behind for an id that no longer exists. The
/// registry entry itself is cleared on the subscriber thread via the queue.
pub(crate) fn forget(id: ControlId) {
    subscribed_remove(id);
    let Some(hook) = delivery() else {
        LISTENERS.with(|l| {
            l.borrow_mut().remove(&id);
        });
        return;
    };
    if let Ok(mut q) = PENDING.lock() {
        q.push(Pending::Forget(id));
    }
    if !QUEUED.swap(true, Ordering::AcqRel) {
        hook();
    }
}

/// [`Subscription`] removal hook: unregister the entry for `token`. Runs on
/// the registering thread (the handle is `!Send`), so it walks the local
/// registry directly and prunes the shared id-set for ids left empty.
fn remove(token: i64) {
    LISTENERS.with(|l| {
        let mut map = l.borrow_mut();
        for entries in map.values_mut() {
            entries.retain(|e| e.token != token);
        }
        map.retain(|id, entries| {
            if entries.is_empty() {
                subscribed_remove(*id);
                false
            } else {
                true
            }
        });
    });
}
