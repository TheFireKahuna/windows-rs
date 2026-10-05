//! The hit array the input thread resolves against, reachable from the window procedure.
//!
//! One array answers the pointer, focus order, overlay dismiss, the window's own
//! caption hit test and automation's element-from-point. The procedure has to answer the
//! caption's question while an input pass is on the stack, so the array is shared rather than
//! owned by the pass — and every borrow of it here is short enough that no call-out can span
//! one, which is what keeps a nested pump's question answerable.

use core::cell::{Cell, RefCell};
use windows_scene::{ControlId, HitEntry, HitTable, NodeId};
use windows_window::CaptionHit;

#[derive(Default)]
pub struct HitView {
    table: RefCell<HitTable>,
    /// The window commands, as the scene thread last published them.
    caption: Cell<[Option<ControlId>; 3]>,
}

impl HitView {
    /// Reads the array.
    ///
    /// # Panics
    ///
    /// Panics where the array is already being read or replaced by this thread. Every caller is
    /// on the window's own thread and no borrow here outlives its statement, so the only way to
    /// reach one is to call this from inside `f`.
    pub fn with<T>(&self, f: impl FnOnce(&HitTable) -> T) -> T {
        f(&self.table.borrow())
    }

    /// Returns a copy of the entry `id` names, or `None` where the array holds none.
    #[must_use]
    pub fn entry(&self, id: ControlId) -> Option<HitEntry> {
        self.with(|hits| hits.entry(id).copied())
    }

    /// Replaces the array from the scene thread's copy and re-installs `shadows` on it.
    ///
    /// A fresh array carries no offsets, so the shadows go on whether or not the tracker set
    /// moved.
    pub fn replace(&self, from: &HitTable, shadows: &[(NodeId, std::sync::Arc<core::sync::atomic::AtomicU64>)]) {
        let mut table = self.table.borrow_mut();
        table.copy_from(from);
        table.set_shadows(shadows);
    }

    /// Re-installs `shadows` on the array in place.
    pub fn set_shadows(&self, shadows: &[(NodeId, std::sync::Arc<core::sync::atomic::AtomicU64>)]) {
        self.table.borrow_mut().set_shadows(shadows);
    }

    /// Publishes the window commands the caption's hit test resolves against.
    pub fn set_caption(&self, commands: [Option<ControlId>; 3]) {
        self.caption.set(commands);
    }

    /// Answers what is at a point in the caption band.
    ///
    /// Fallible in one direction only: an answer given from inside a read of the array is the
    /// drag strip, which is what a band with no command at that point is anyway.
    #[must_use]
    pub fn caption_hit(&self, x: f32, y: f32) -> CaptionHit {
        let commands = self.caption.get();
        self.table
            .try_borrow()
            .map_or(CaptionHit::Drag, |hits| {
                crate::caption::hit(x, y, &hits, commands)
            })
    }
}
