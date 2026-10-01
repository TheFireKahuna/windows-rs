//! Headless verification of application declarations against the shipping app pass.
//!
//! Install a host, then create production declarations through the driver. Each flush
//! reconciles signals and overlays through the app thread's own function, then runs
//! the real host solver and emits its scene patch. No window or compositor is needed.
//! Input routing, focus application and animation playback require native driving;
//! this driver checks declarations and geometry only.

use core::cell::RefCell;

use crate::build::Host;
use crate::overlay::Overlays;
use crate::role::Scope;
use crate::seam::FocusOp;
use crate::uia::{Snapshot, Tree};
use windows_core::Result;
use windows_scene::{Env, SinkPatch};
use windows_text::FontLadder;

/// Installs this thread's host as the app thread's start does: `env` and `scope` for the
/// host, and a text engine over `ladder` for its table.
pub fn install(env: Env, scope: Scope, ladder: FontLadder) -> Result<()> {
    Host::install(env, scope);
    Host::with(|h| h.text.install(ladder))
}

/// Returns the sink identity for a retained node used by a native scene probe.
pub fn node_id<K>(node: crate::build::Node<K>) -> windows_scene::NodeId {
    node.id
}

/// Returns the automation tree as a client adopts it, over the host as it stands.
///
/// The walk builds into the host's own patch, so it is answered between flushes and the
/// buffers it filled are dropped rather than published.
#[must_use]
pub fn uia_tree() -> Tree {
    let mut snapshot = Snapshot::default();
    Host::with(|h| {
        h.uia_entries(&mut snapshot);
        h.pending.clear();
    });
    Tree::adopt(&snapshot, &[])
}

/// Owns the app-side overlay lifecycle for a mounted layout under test.
#[derive(Default)]
pub struct LayoutDriver {
    owner: Option<crate::signal::Owner>,
    root: Option<crate::build::Mount>,
    overlays: Overlays,
    focus: Vec<FocusOp>,
}

impl LayoutDriver {
    /// Applies retained declarations through the shipping scene/control adoption path.
    /// Callers pass the dispatch result when flushing an acknowledged release.
    /// This drives no OS input, text-services rendering, scrolling or presentation regions.
    pub fn flush_controls(&mut self, controls: &mut crate::widget::Controls,
        accepted: Option<windows_scene::ControlId>, front: &mut crate::widget::Front<'_>) -> Result<()> {
        let mut down = crate::seam::Down::default();
        self.flush(&mut down.patch);
        Host::with(|h| h.fill(&mut down));
        down.preview_done = controls.take_preview_release()
            .map(|epoch| (epoch, accepted.unwrap_or(windows_scene::ControlId::NONE)));
        super::pass::apply_control_patch(controls, &mut down, front)
    }

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
        super::pass::deliver_field_commits(&[crate::text_input::Commit {
            id,
            revision,
            text: std::sync::Arc::from(text),
        }]);
    }

    /// Delivers explicitly supplied compositor events through the production overlay path.
    /// This checks completion handling, not animation playback or elapsed time.
    pub fn scene_events(&mut self, events: &[windows_scene::SceneEvent]) {
        self.overlays.scene(events, &mut self.focus);
    }

    /// Delivers an activation through the shipping handlers and overlay lifecycle.
    /// This does not simulate input routing or native focus application.
    pub fn tap(&mut self, target: windows_scene::ControlId) {
        let intents = [crate::widget::Intent { target, what: crate::widget::What::Tapped }];
        Host::dispatch(&intents);
        self.overlays.settle(&[], &intents, &mut self.focus);
        self.overlays.after_dispatch(&mut self.focus);
    }

    /// Delivers an unconsumed key report through the shipping app-side command path.
    /// The caller supplies focus; this does not simulate Win32, TSF or input routing.
    pub fn key(&mut self, target: Option<windows_scene::ControlId>, event: crate::input::KeyEvent) {
        let reports = [crate::input::Report::Key { target, event }];
        let mut intents = Vec::new();
        super::pass::key_intents(&mut self.overlays, &reports, &mut self.focus, &mut intents);
        Host::dispatch(&intents);
        self.overlays.settle(&reports, &intents, &mut self.focus);
        self.overlays.after_dispatch(&mut self.focus);
    }

    /// Runs one production declaration pass and solves its geometry.
    ///
    /// The caller must clear `patch` before another flush, as with [`Host::flush`].
    /// A probe written by the solve can schedule work for the next pass.
    pub fn flush(&mut self, patch: &mut SinkPatch) {
        super::pass::reconcile(&mut self.overlays, &mut self.focus);
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

// ── driving the two paths a nested pump and a docked occlusion take ─────────────

thread_local! {
    /// What runs where the pass makes its deepest call-out, for a harness that needs a real
    /// nested pump on a real pass's stack.
    static AT_CALL_OUT: RefCell<Option<Box<dyn FnMut()>>> = const { RefCell::new(None) };
}

/// Installs a function run inside every input pass, at the point a text service's own message
/// pump opens.
///
/// A store-level reentrancy test cannot reach this: the property under test is that the pass
/// holds nothing a nested pump needs, and only a pass can demonstrate that. Installed on the
/// window's own thread, before the window runs.
pub fn on_call_out(f: impl FnMut() + 'static) {
    AT_CALL_OUT.with(|slot| *slot.borrow_mut() = Some(Box::new(f)));
}

/// Runs the installed call-out, where one is installed and is not already running.
pub(super) fn at_call_out() {
    // Fallibly, so a hook that re-enters the pass is a skipped call rather than a panic inside
    // the window procedure.
    _ = AT_CALL_OUT.try_with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(f) = slot.as_mut()
        {
            f();
        }
    });
}

pub use crate::text_input::Occluder;

/// The running window's handle, reachable from any thread.
///
/// Process-wide rather than thread-local: the system's own occlusion callback arrives on a
/// pool thread, so a harness reporting one has to be able to do the same.
static OCCLUDER: std::sync::Mutex<Option<Occluder>> = std::sync::Mutex::new(None);

/// Publishes the running window's occlusion handle. Called once, as the window is built.
pub(super) fn publish_occluder(occluder: Occluder) {
    *OCCLUDER.lock().unwrap_or_else(|e| e.into_inner()) = Some(occluder);
}

/// Returns the running window's occlusion handle, once its window exists.
#[must_use]
pub fn occluder() -> Option<Occluder> {
    OCCLUDER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}
