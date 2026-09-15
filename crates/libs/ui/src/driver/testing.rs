//! Headless verification of application declarations against the shipping app pass.
//!
//! Install a host, then create production declarations through the driver. Each flush
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
    owner: Option<crate::signal::Owner>,
    root: Option<crate::build::Mount>,
    overlays: Overlays,
    focus: Vec<FocusOp>,
}

impl LayoutDriver {
    pub fn create(create: impl FnOnce(&mut crate::build::Ui<'_>)) -> Self {
        let (owner, root) = crate::signal::Owner::scope(|| crate::build::Ui::mount_root(create));
        Self {
            owner: Some(owner),
            root: Some(root),
            overlays: Overlays::new(),
            focus: Vec::new(),
        }
    }

    /// Delivers completed edits through the shipping revision/deduplication path.
    /// The caller supplies input-owned revisions; this does not simulate TSF or IME.
    pub fn field_commit(&mut self, id: windows_scene::ControlId, revision: u64, text: &str) {
        super::app::deliver_field_commits(&[crate::text_input::Commit {
            id,
            revision,
            text: std::sync::Arc::from(text),
        }]);
    }

    /// Runs one production declaration pass and solves its geometry.
    ///
    /// The caller must clear `patch` before another flush, as with [`Host::flush`].
    /// A probe written by the solve can schedule work for the next pass.
    pub fn flush(&mut self, patch: &mut SinkPatch) {
        super::app::reconcile(&mut self.overlays, &mut self.focus);
        Host::flush(patch);
        // A native driver sends these to the input thread. Layout tests neither
        // apply them on the app thread nor claim to verify focus behavior.
        self.focus.clear();
    }
}

impl Drop for LayoutDriver {
    fn drop(&mut self) {
        _ = Host::try_with(|host| {
            self.overlays.retire(host);
            if let Some(root) = &mut self.root {
                root.retire(host);
            }
        });
        self.overlays = Overlays::new();
        self.root = None;
        self.owner = None;
    }
}
