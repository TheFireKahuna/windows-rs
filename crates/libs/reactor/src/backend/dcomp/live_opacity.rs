//! A retained OPACITY a producer thread eases on a declarative node — the thinnest
//! possible live facet, and the counterpart of [`bar_field`](super::bar_field) with
//! the geometry removed.
//!
//! The Simple preview's lit spans are declarative `Shape::path` elements sharing the
//! response stroke's geometry (so their shape can never diverge from the curve). Only
//! their per-span OPACITY moves at display cadence while the pointer hovers, and
//! routing that through the reactor would re-render the tree every frame. So the
//! opacity is a compositor property the producer writes directly onto the node that
//! already exists — one `set_opacity` per push, no geometry, no reconcile.
//!
//! The ease itself lives app-side (the producer already ticks at display rate while
//! hovering): this primitive is "dumb", taking an opacity that is ALREADY eased and
//! writing it. It never touches the visual tree's shape, so a declarative reshape and
//! a live opacity write cannot fight — they are different properties of the same node.
//!
//! ## The seam
//!
//! [`bar_field`](super::bar_field)'s, minus the geometry: the visual tree is
//! thread-affine, so the producer is handed a control id. Opacities are queued from
//! whatever thread computed them, coalesced per control (a producer that outruns the
//! front thread overwrites its own pending value), and applied on the front thread as
//! one `Visual::SetOpacity` on the node's container.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::backend::ControlId;

/// Pending opacity per control. Coalesced — the latest value wins.
static PENDING: Mutex<Option<HashMap<ControlId, f32>>> = Mutex::new(None);

/// Whether a service call is already on its way to the front thread. Gates the post
/// so a producer running at display rate leaves at most one message in flight.
static POSTED: AtomicBool = AtomicBool::new(false);

/// A handle to one node's live opacity, writable from any thread.
///
/// Obtained from a mounted element. Cheap to `Copy` and `Send`, because it holds no
/// COM: the control id names a node the front thread owns. A handle outliving its
/// control is harmless — the update is dropped when the id no longer resolves.
#[derive(Clone, Copy, Debug)]
pub struct LiveOpacity {
    id: ControlId,
}

impl LiveOpacity {
    pub(crate) fn new(id: ControlId) -> Self {
        Self { id }
    }

    /// Set this node's opacity (`0.0..=1.0`), already eased by the caller. The front
    /// thread writes it directly onto the node's container — one property set,
    /// nothing rasterizes, no reconcile.
    pub fn set(&self, opacity: f32) {
        {
            let Ok(mut q) = PENDING.lock() else { return };
            let map = q.get_or_insert_with(HashMap::new);
            map.insert(self.id, opacity.clamp(0.0, 1.0));
        }
        // One wake in flight, and the claim is the WHOLE gate — see the same reasoning
        // (and the bug that motivated it) in `bar_field::enqueue`. This map is DRAINED
        // each service, so an empty map means no pending work and the wake claim can
        // key on it having been empty; a stale id simply fails to resolve and its
        // value is dropped.
        if !POSTED.swap(true, Ordering::AcqRel) {
            let hwnd = super::live_text::front_hwnd();
            if hwnd != 0 {
                super::host::post_ui(hwnd, || {
                    if let Some(s) = super::host::shared() {
                        s.backend.borrow_mut().service_live_opacity();
                    }
                });
            } else {
                POSTED.store(false, Ordering::Release);
            }
        }
    }
}

/// Drain the pending opacities into `out` (an id→opacity list the caller applies), and
/// release the wake claim so the next push posts again. The claim is released BEFORE
/// the caller applies, so a push landing during the apply schedules another service.
pub(crate) fn drain_into(out: &mut Vec<(ControlId, f32)>) {
    POSTED.store(false, Ordering::Release);
    out.clear();
    let Ok(mut q) = PENDING.lock() else { return };
    if let Some(map) = q.as_mut() {
        out.extend(map.drain());
    }
}
