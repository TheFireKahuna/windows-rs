//! UI Automation.
//!
//! The tree is published rather than queried on demand. A client's call reads an immutable
//! snapshot from automation's own worker thread and never enters the window's message pump,
//! so a provider method cannot block a screen reader and a client walking the tree at idle
//! costs no front-thread wakes.
//!
//! Commands cross the other way. `Invoke`, `Toggle`, `Select` and `SetValue` must return
//! without blocking, so each queues an [`Action`] and posts the front thread's frame
//! message; the tick then runs the widget's own handler, publishes its pixels and queues
//! its intent, through the same code a tap runs.
//!
//! # What is where
//!
//! | | |
//! |---|---|
//! | [`tree`] | the published snapshot: the hit array, four columns, one UTF-16 blob |
//! | [`live`] | what an immutable snapshot cannot hold — values, state, scroll, focus, the window's own origin |
//! | [`slot`] | the hand-off, versioned so a reader never blocks |
//! | [`element`] | the provider object, and the state it reaches |
//! | [`patterns`] | the control patterns |
//! | [`text`] | `TextPattern` over the blob |
//! | [`region`] | joining a presentation region's published geometry to what it means |
//! | [`roles`] | one `const` row per role |
//! | [`action`] / [`events`] | the two queues that cross back |

mod action;
mod element;
mod events;
mod live;
mod patterns;
mod region;
mod roles;
mod slot;
mod text;
mod tree;
mod variant;

pub use action::Action;
pub use events::{Property, Raise, Val};
pub use live::State;
pub use region::{PartDecl, RegionPeer};
pub use roles::Patterns;
pub use tree::{Col, ColFlags, Part, Seed, Seeds, Text, Tree, Value};

use crate::bindings::{HWND, LPARAM, LRESULT, WPARAM};
use crate::front::FrontHandle;
use crate::widget::UiaRole;
use element::Shared;
use events::Pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use windows_numerics::Vector2;
use windows_scene::{ControlId, HitEntry, NodeId};

/// The step a live region's value is quantized to before it is announced.
///
/// A read-out can move every frame, so a value is announced only once it crosses a step.
///
/// Quantization is the only bound: a value oscillating across a step boundary announces on
/// every tick. Rate-limiting it would need a timestamp, and the tick this runs on is not
/// periodic and carries none.
const LIVE_QUANTUM: f64 = 0.5;

/// The front thread's half of automation.
///
/// Owns the publish, the event queue and the window's identity. Everything a client can
/// reach lives behind the `Arc` and is `Send + Sync`; `Uia` itself is neither, so the
/// publish runs on the thread that owns the tree.
pub struct Uia {
    shared: FrontHandle<Arc<Shared>>,
    /// The published tree, held so a republish can carry the live half forward.
    current: Arc<Tree>,
    pending: Pending,
    /// The value each live region last announced, so a value that lands on the same step
    /// announces nothing.
    announced: Vec<(ControlId, f64)>,
    /// Presentation regions whose parts this tick may have to re-join.
    regions: Vec<region::Watched>,
}

impl Default for Uia {
    fn default() -> Self {
        Self::new()
    }
}

impl Uia {
    /// Creates automation state with no window attached and an empty tree.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: FrontHandle::new(Arc::new(Shared::default())),
            current: Arc::new(Tree::empty()),
            pending: Pending::default(),
            announced: Vec::new(),
            regions: Vec::new(),
        }
    }

    /// Records the window every provider answers for.
    pub fn attach(&mut self, hwnd: HWND) {
        self.shared.attach(hwnd);
    }

    /// Answers `WM_GETOBJECT` with the fragment root, or `None` for an object id that names
    /// something else.
    ///
    /// The only automation call that arrives on the pump; everything a client asks
    /// afterwards is answered off the front thread.
    pub fn get_object(&mut self, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
        element::get_object(&self.shared, w, l)
    }

    /// Releases automation's cache for the window and empties the tree.
    ///
    /// Must be called from `WM_DESTROY`, while the handle is still valid: the release names
    /// the handle, so it cannot be deferred to [`Drop`](Self::drop). Dropping our own
    /// references does not release the cache automation keeps per window.
    pub fn detach(&mut self) {
        element::disconnect(&self.shared);
        self.adopt(Arc::new(Tree::empty()));
        self.pending = Pending::default();
        self.regions.clear();
    }

    /// Returns whether a client has asked for a provider, which is what gates building the
    /// tree.
    ///
    /// The gate is the `WM_GETOBJECT` latch alone. `UiaClientsAreListening` answers `true`
    /// on a bare Windows 11 desktop with no screen reader running, so gating the build on
    /// it would build the tree on every machine. The latch is also sufficient:
    /// `WM_GETOBJECT` is the only way into this tree, so nothing can query before it is
    /// set.
    ///
    /// Event raising gates on `UiaClientsAreListening` instead, where a false positive
    /// costs one call rather than a whole tree.
    #[must_use]
    pub fn listening(&self) -> bool {
        self.shared.queried.load(Relaxed)
    }

    /// Returns whether a client has appeared since the last publish and would walk nothing.
    ///
    /// A window that is not laid out again does not republish on its own, so a client
    /// attaching to an idle window would see an empty tree until something moved. The tick
    /// asks this alongside its layout-changed check; the `WM_GETOBJECT` that set the latch
    /// has already posted the frame message that tick runs in.
    #[must_use]
    pub fn wants_tree(&self) -> bool {
        self.listening() && self.current.is_empty()
    }

    /// Publishes a new tree, built from the adopted hit array and the seeds the
    /// application thread produced alongside it.
    ///
    /// Called where the hit array is adopted and nowhere else, so entries and seeds
    /// describe the same layout by construction rather than by an ordering rule.
    pub fn publish(&mut self, entries: &[HitEntry], seeds: &Seeds) {
        if !self.listening() {
            // With no client latched, neither the string blob nor the columns pass is
            // built: the tree is not merely smaller, it is not constructed at all.
            if !self.current.is_empty() {
                self.adopt(Arc::new(Tree::empty()));
            }
            return;
        }
        let tree = Tree::build(entries, seeds);
        self.adopt(Arc::new(tree));
        self.pending.push(Raise::Structure);
    }

    fn adopt(&mut self, tree: Arc<Tree>) {
        // The live half carries forward before the publish, so no client can observe the
        // new tree with the old tree's values missing. A layout change disables no control
        // and moves no slider; without this a resize would report every toggle as reset.
        tree.live
            .carry(&self.current.live, tree.remap(&self.current));
        self.shared.evict(&tree);
        self.shared.slot.publish(Arc::clone(&tree));
        self.current = tree;
    }

    /// Publishes the window's client origin in physical pixels, and its DIP scale.
    ///
    /// Every bounding rectangle a provider reports is computed from these, because
    /// automation reports screen pixels. Call on every move, resize and DPI change: a stale
    /// origin reports every control at the wrong place.
    pub fn set_window(&mut self, origin: Vector2, scale: f32) {
        self.current.live.set_window(origin, scale);
    }

    /// Publishes the offset of one scroll container, which its descendants' bounds are
    /// resolved through. Does nothing for a node the published tree scrolls nothing by.
    pub fn set_scroll(&mut self, node: NodeId, offset: Vector2) {
        self.current.live.set_scroll(node, offset);
    }

    /// Records where a control's value now stands, and queues the property change.
    ///
    /// One relaxed store, which is why the router can call it per pointer sample. The
    /// event is coalesced to one per element per tick, so a drag announces once.
    pub fn set_value(&mut self, id: ControlId, value: f64) {
        let Some(at) = self.current.index_of(id) else {
            return;
        };
        let was = self.current.live.value(at);
        if was == Some(value) {
            return;
        }
        self.current.live.set_value(at, value);
        self.pending.push(Raise::Property {
            id,
            what: Property::Range,
            // A first value has no predecessor. Reporting the range's floor as where it
            // came from keeps automation from reading the event as no change at all.
            from: Val::Number(was.unwrap_or_else(|| self.floor(at))),
            to: Val::Number(value),
        });
        self.announce(id, at, value);
    }

    /// Returns the bottom of the element's range, or zero where it carries none.
    fn floor(&self, at: usize) -> f64 {
        match self.current.col(at).map(|col| col.value) {
            Some(Value::Range(range)) => range.min,
            _ => 0.0,
        }
    }

    /// Records one state flag — enabled, toggled, selected or expanded — and queues the
    /// matching property event for all but `ENABLED`.
    pub fn set_state(&mut self, id: ControlId, flag: State, on: bool) {
        let Some(at) = self.current.index_of(id) else {
            return;
        };
        if self.current.live.state(at).has(flag) == on {
            return;
        }
        self.current.live.set_state(at, flag, on);
        let what = match flag {
            State::TOGGLED => Some(Property::Toggle),
            State::SELECTED => Some(Property::Selected),
            State::EXPANDED => Some(Property::Expanded),
            _ => None,
        };
        if let Some(what) = what {
            self.pending.push(Raise::Property {
                id,
                what,
                from: what.of(!on),
                to: what.of(on),
            });
        }
    }

    /// Records which control holds keyboard focus, and raises a focus event when one does.
    pub fn set_focus(&mut self, id: Option<ControlId>) {
        let packed = id.map_or(u64::MAX, element::packed);
        if self.current.live.focused() == packed {
            return;
        }
        self.current.live.set_focused(packed);
        if let Some(id) = id {
            self.pending.push(Raise::Focus(id));
        }
    }

    /// Queues an overlay event, such as a menu or tooltip opening or closing.
    pub fn overlay(&mut self, raise: Raise) {
        self.pending.push(raise);
    }

    /// Binds a producer-owned value cell to a control.
    ///
    /// A presentation region's number is written by the thread that drew the pixels it
    /// describes, so it cannot live in a snapshot the front thread replaces. Creates no
    /// visual and touches no pixels, and takes effect without a republish.
    pub fn bind_value(&mut self, id: ControlId, cell: Arc<AtomicU64>) {
        self.shared.regions.declare(id, None, Some(cell));
    }

    /// Declares what is nameable inside a presentation region.
    ///
    /// Parts carry region-local rects and are restated by whoever owns the region whenever
    /// its mapping moves — a range change, a band added, a resize, a band dragged. They
    /// live beside the tree rather than in it, so none of those republishes every element
    /// on the screen. Each part's name and role travel with it, because they do not move
    /// when its geometry does.
    pub fn set_parts(&mut self, id: ControlId, parts: &[Part]) {
        self.shared.regions.declare(id, Some(parts), None);
    }

    /// Watches a presentation region, replacing any earlier watch on the same control.
    ///
    /// The region's renderer publishes the geometry, this side owns what that geometry
    /// means, and [`sync_regions`](Self::sync_regions) joins the two.
    pub fn watch_region(&mut self, peer: RegionPeer) {
        let id = peer.id;
        self.regions.retain(|watched| watched.id() != id);
        self.regions.push(region::Watched::new(peer));
    }

    /// Re-joins every watched region whose renderer has moved. Called once per tick.
    ///
    /// A region whose geometry version has not moved costs one version read and nothing
    /// else, so this can sit on the tick unconditionally.
    pub fn sync_regions(&mut self) {
        for watched in &mut self.regions {
            let id = watched.id();
            if let Some(parts) = watched.join() {
                self.shared.regions.declare(id, Some(parts), None);
            }
        }
    }

    /// Forgets everything a control declared: its parts, its bound value, its last
    /// announcement and its region watch. Called where the control table drops the control.
    pub fn release(&mut self, id: ControlId) {
        self.shared.regions.forget(id);
        self.announced.retain(|(key, _)| *key != id);
        self.regions.retain(|watched| watched.id() != id);
    }

    // ── what the tick hands over ────────────────────────────────────────────────

    /// Records the value changes and taps an interaction produced.
    ///
    /// Reads the intents the front side already builds rather than observing the
    /// interaction a second time: a slider that moved its own thumb queues one, and this
    /// takes the number out of it. A committed value and a changed one both report a value
    /// change, and [`set_value`](Self::set_value) drops a value equal to the one held, so
    /// neither is announced twice.
    pub fn observe(&mut self, intents: &[crate::widget::Intent]) {
        for intent in intents {
            match intent.what {
                crate::widget::What::Changed(v) | crate::widget::What::Committed(v) => {
                    self.set_value(intent.target, v);
                }
                crate::widget::What::Tapped => {
                    self.pending.push(Raise::Invoked(intent.target));
                }
            }
        }
    }

    /// Records a model-state change, resolving what it means from the element's role.
    ///
    /// A checkbox reports the same fact as a toggle and every other role as a selection, so
    /// a client hears "checked" or "3 of 5". Resolving it here rather than at each call
    /// site keeps the two from disagreeing.
    pub fn set_model(&mut self, id: ControlId, state: crate::widget::ModelState) {
        use crate::widget::ModelState;
        let Some(at) = self.current.index_of(id) else {
            return;
        };
        let role = self.current.col(at).map_or(UiaRole::None, |col| col.role);
        self.set_state(id, State::ENABLED, state != ModelState::Disabled);
        let on = state == ModelState::Selected;
        match role {
            UiaRole::CheckBox => self.set_state(id, State::TOGGLED, on),
            _ => self.set_state(id, State::SELECTED, on),
        }
    }

    /// Queues an invoked event for `id`.
    ///
    /// Raised by the application rather than by the provider: `Invoke` returns before the
    /// work happens, and the event is owed after the control has completed its action,
    /// which only this side knows.
    pub fn invoked(&mut self, id: ControlId) {
        self.pending.push(Raise::Invoked(id));
    }

    /// Records whether an overlay is open on `id`, as both state and an expand-collapse
    /// event.
    pub fn set_expanded(&mut self, id: ControlId, open: bool) {
        self.set_state(id, State::EXPANDED, open);
    }

    /// Moves every action clients queued since the last tick into `out`.
    pub fn drain(&mut self, out: &mut Vec<Action>) {
        self.shared.actions.drain(out);
    }

    /// Raises every pending event. Called as the last step of a tick, so the tree a client
    /// reads back is the one the event describes and no raise re-enters an input handler
    /// that is still running.
    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        self.pending.flush(|id| element::provider_for(&shared, id));
    }

    /// Queues a live-region announcement, quantized to [`LIVE_QUANTUM`] and only when the
    /// step the value lands on differs from the one last announced.
    fn announce(&mut self, id: ControlId, at: usize, value: f64) {
        let live = self.current.col(at).is_some_and(|col| {
            col.flags.has(ColFlags::LIVE_POLITE) || col.flags.has(ColFlags::LIVE_ASSERTIVE)
        });
        if !live {
            return;
        }
        let step = (value / LIVE_QUANTUM).round() * LIVE_QUANTUM;
        match self.announced.iter_mut().find(|(key, _)| *key == id) {
            Some((_, last)) if *last == step => return,
            Some((_, last)) => *last = step,
            None => self.announced.push((id, step)),
        }
        self.pending.push(Raise::Live(id));
    }
}

impl Drop for Uia {
    fn drop(&mut self) {
        // The backstop for a window dropped without a `WM_DESTROY`. Providers a client
        // still holds stop resolving from here on, which is what
        // `UIA_E_ELEMENTNOTAVAILABLE` reports; dropping our references stops new ones.
        element::disconnect(&self.shared);
    }
}

#[cfg(test)]
impl Uia {
    /// Returns the published tree.
    fn tree(&self) -> &Tree {
        &self.current
    }

    /// Sets the client-asked latch without a `WM_GETOBJECT` having arrived.
    fn latch_for_test(&mut self) {
        self.shared.queried.store(true, Relaxed);
    }

    fn queue_for_test(&self, action: Action) {
        self.shared.act(action);
    }

    fn take_pending_for_test(&mut self, out: &mut Vec<Raise>) {
        self.pending.take(out);
    }

    /// Returns the published tree by identity, so a caller can compare two publishes for
    /// sameness.
    fn tree_arc_for_test(&self) -> Arc<Tree> {
        Arc::clone(&self.current)
    }

    /// Returns how many parts `id` declared, and the second part's value.
    fn parts_for_test(&self, id: ControlId) -> (usize, Option<f64>) {
        self.shared.regions.with_parts(id, |parts| {
            (parts.len(), parts.get(1).and_then(|p| p.value))
        })
    }

    /// Returns the part covering a region-local point.
    fn part_at_for_test(&self, id: ControlId, x: f32, y: f32) -> Option<u32> {
        self.shared.regions.with_parts(id, |parts| {
            parts
                .iter()
                .find(|part| {
                    x >= part.rect.0 && x <= part.rect.2 && y >= part.rect.1 && y <= part.rect.3
                })
                .map(|part| part.sub)
        })
    }

    fn root_for_test(&self) -> crate::bindings::IRawElementProviderSimple {
        element::provider_for(&self.shared, ControlId::NONE).expect("the root always resolves")
    }
}

#[cfg(test)]
mod com_tests;
#[cfg(test)]
mod tests;
