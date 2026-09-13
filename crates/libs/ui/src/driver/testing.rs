//! Headless verification of application declarations against the shipping app pass.
//!
//! Install a host and mount production views before creating the driver. Each flush
//! reconciles signals and overlays through the app thread's own function, then runs
//! the real host solver and emits its scene patch. No window or compositor is needed.
//! Input routing, focus application and animation playback require native driving;
//! this driver checks declarations and geometry only.

use crate::build::Host;
use crate::overlay::Overlays;
use crate::seam::FocusOp;
use windows_scene::SinkPatch;

/// Owns the app-side overlay lifecycle for a mounted layout under test.
#[derive(Default)]
pub struct LayoutDriver {
    overlays: Overlays,
    focus: Vec<FocusOp>,
}

impl LayoutDriver {
    /// Runs one production declaration pass and solves its geometry.
    ///
    /// The caller must clear `patch` before another flush, as with [`Host::flush`].
    /// A probe written by the solve can schedule work for the next pass.
    pub fn flush(&mut self, patch: &mut SinkPatch) {
        super::app::reconcile(&mut self.overlays, &mut self.focus);
        Host::with(|host| host.flush(patch));
        // A native driver sends these to the input thread. Layout tests neither
        // apply them on the app thread nor claim to verify focus behavior.
        self.focus.clear();
    }
}
