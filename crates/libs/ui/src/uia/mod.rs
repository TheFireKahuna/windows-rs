//! UI Automation.
//!
//! # The one decision
//!
//! **The tree is published, not asked for.** A client's query reads an immutable snapshot
//! from automation's own worker thread and never touches the window's. The reference this
//! is adapted from could not do that — its tree was the `!Send` node arena — so every
//! provider method made a blocking round trip onto the pump, and the machinery that made
//! that affordable (a batched property snapshot, a process-global generation counter to
//! invalidate it, a per-thread one-element cache, and a carve-out for the two properties
//! that move without a mutation hook) is most of what this port does not contain.
//!
//! Two consequences beyond the size. `WM_GETOBJECT` can no longer hang a screen reader,
//! because a query never enters the pump — not by being careful, but by construction. And
//! a client walking the tree at idle costs zero front-thread wakes, so the window's
//! zero-cost-at-idle property survives having a screen reader attached.
//!
//! Commands are the mirror image, and the platform agrees: `Invoke` "is an asynchronous
//! call and must return immediately without blocking", as is `Toggle`, `Select` and
//! `SetValue`. They queue and ring the front thread's existing doorbell, so an invoke runs
//! the widget's own handler, publishes its pixels and queues its intent — exactly as a tap
//! does, through the same code.
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
//! | [`roles`] | one `const` row per role, replacing a cross-product |
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
/// A loudness read-out moves every frame and a client that speaks every frame is unusable,
/// so only a change a listener could act on is worth interrupting for — the same
/// conclusion the draw path reached about a read-out whose digits change every frame,
/// applied to the announcement instead.
///
/// **Quantized, not rate-limited.** A value that oscillates across a step boundary still
/// announces every tick, and this stack has no clock here to bound it with — the tick is
/// not periodic. Bounding it properly needs a timestamp, which is a thing to add when a
/// real producer shows the behaviour rather than a thing to guess at now.
const LIVE_QUANTUM: f64 = 0.5;

/// The front thread's half of automation.
///
/// Owns the publish, the event queue and the window's identity. Everything a client can
/// reach lives behind the `Arc` and is `Send + Sync`; this is not, which is what says the
/// publish happens on the thread that owns the tree.
pub struct Uia {
    shared: FrontHandle<Arc<Shared>>,
    /// Held so a republish can carry the live half forward.
    current: Arc<Tree>,
    pending: Pending,
    /// The value each live region last announced, so an announcement is a change and not a
    /// heartbeat.
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

    /// Names the window every provider answers for.
    pub fn attach(&mut self, hwnd: HWND) {
        self.shared.attach(hwnd);
    }

    /// `WM_GETOBJECT`. `None` for an object id that is somebody else's.
    ///
    /// The only automation call that arrives on the pump, and all it does is hand back the
    /// fragment root.
    pub fn get_object(&mut self, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
        element::get_object(&self.shared, w, l)
    }

    /// The window is going. **Called from `WM_DESTROY`**, while the handle is still valid.
    ///
    /// Releases automation's own cache for the window, which our references going away does
    /// not do — and cannot be deferred to [`Drop`](Self::drop), because by then the handle
    /// names nothing.
    pub fn detach(&mut self) {
        element::disconnect(&self.shared);
        self.adopt(Arc::new(Tree::empty()));
        self.pending = Pending::default();
        self.regions.clear();
    }

    /// Whether the tree is worth building.
    ///
    /// **The latch alone.** `UiaClientsAreListening` was the obvious gate and it is
    /// useless: measured on a bare Windows 11 desktop with no screen reader running, it
    /// answers `true`. Gating on it would build the tree on every machine forever, which
    /// is the opposite of what it looks like it does. Having actually been asked for a
    /// provider is not a hint, and it is also sufficient — `WM_GETOBJECT` is the only way
    /// into this tree, so nothing can query before the latch is set.
    ///
    /// The hint still gates *event raising*, where a false positive costs one call rather
    /// than a whole tree.
    #[must_use]
    pub fn listening(&self) -> bool {
        self.shared.queried.load(Relaxed)
    }

    /// Whether a client has appeared since the last publish and is looking at nothing.
    ///
    /// A window that is not laid out again will not republish on its own, so a client that
    /// attaches to an idle window would walk an empty tree forever. The tick asks this
    /// alongside its layout-changed check; the `WM_GETOBJECT` that set the latch has
    /// already posted for a tick to ask in.
    #[must_use]
    pub fn wants_tree(&self) -> bool {
        self.listening() && self.current.is_empty()
    }

    /// Publishes a new tree, built from the adopted hit array and the seeds the
    /// application thread produced alongside it.
    ///
    /// Called where the hit array is adopted and nowhere else, so the two are the same
    /// data by construction rather than by an ordering rule.
    pub fn publish(&mut self, entries: &[HitEntry], seeds: &Seeds) {
        if !self.listening() {
            // Not merely a smaller tree: the string blob is never built and the columns
            // pass never runs, so a machine with no client attached pays nothing at all.
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
        // Carried before the publish, so no client can observe the new tree with the old
        // tree's values missing. A layout change does not disable a control or move a
        // slider, and without this a resize would report every toggle as reset.
        tree.live
            .carry(&self.current.live, tree.remap(&self.current));
        self.shared.evict(&tree);
        self.shared.slot.publish(Arc::clone(&tree));
        self.current = tree;
    }

    /// The window's client origin in physical pixels, and its DIP scale.
    ///
    /// Automation speaks screen pixels. Published on every move, resize and DPI change,
    /// because a stale origin is indistinguishable from an application placing its
    /// controls in the wrong place.
    pub fn set_window(&mut self, origin: Vector2, scale: f32) {
        self.current.live.set_window(origin, scale);
    }

    pub fn set_scroll(&mut self, node: NodeId, offset: Vector2) {
        self.current.live.set_scroll(node, offset);
    }

    /// Where a control's value now stands.
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
            // A first value has no predecessor, so it reports itself as having come from
            // its own range's floor rather than from nothing — which automation would read
            // as no change at all.
            from: Val::Number(was.unwrap_or_else(|| self.floor(at))),
            to: Val::Number(value),
        });
        self.announce(id, at, value);
    }

    /// The bottom of an element's range, or zero where it names none.
    fn floor(&self, at: usize) -> f64 {
        match self.current.col(at).map(|col| col.value) {
            Some(Value::Range(range)) => range.min,
            _ => 0.0,
        }
    }

    /// A model-state change: enabled, toggled, selected, expanded.
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

    /// Which control has keyboard focus, if any.
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

    /// Records that an overlay opened or closed.
    pub fn overlay(&mut self, raise: Raise) {
        self.pending.push(raise);
    }

    /// Binds a producer-owned value to a control.
    ///
    /// A presentation region's number is written by the thread that drew the pixels it
    /// describes, so it cannot live in a snapshot the front thread replaces. This is the
    /// whole of what makes a region readable: no visual is created and no pixel is
    /// involved. Takes effect immediately.
    pub fn bind_value(&mut self, id: ControlId, cell: Arc<AtomicU64>) {
        self.shared.regions.declare(id, None, Some(cell));
    }

    /// Declares what is nameable inside a presentation region.
    ///
    /// Region-local rects, restated by whoever owns the region whenever its mapping moves —
    /// a range change, a band added, a resize, or a band being dragged. That is why they do
    /// **not** go into the tree: each of those would otherwise be a republish of every
    /// element on the screen. What a part *means* does not move with the geometry, which is
    /// why the name and the role travel with it rather than being looked up elsewhere.
    pub fn set_parts(&mut self, id: ControlId, parts: &[Part]) {
        self.shared.regions.declare(id, Some(parts), None);
    }

    /// Watches a presentation region: its renderer publishes geometry, this side owns what
    /// that geometry means, and [`sync_regions`](Self::sync_regions) joins them.
    ///
    /// The alternative is the application forwarding parts by hand every time its renderer
    /// moves one, which is the same join written once per region instead of once.
    pub fn watch_region(&mut self, peer: RegionPeer) {
        let id = peer.id;
        self.regions.retain(|watched| watched.id() != id);
        self.regions.push(region::Watched::new(peer));
    }

    /// Re-joins every watched region whose renderer has moved. **Called once per tick.**
    ///
    /// One acquire load per region when nothing changed, which is the common case and the
    /// reason this can sit on the tick unconditionally.
    pub fn sync_regions(&mut self) {
        for watched in &mut self.regions {
            let id = watched.id();
            if let Some(parts) = watched.join() {
                self.shared.regions.declare(id, Some(parts), None);
            }
        }
    }

    /// Forgets everything a control declared. Called where the control table does.
    pub fn release(&mut self, id: ControlId) {
        self.shared.regions.forget(id);
        self.announced.retain(|(key, _)| *key != id);
        self.regions.retain(|watched| watched.id() != id);
    }

    // ── what the tick hands over ────────────────────────────────────────────────

    /// Records the value changes an interaction produced.
    ///
    /// Fed the intents the front side already builds, so this is not a second observation
    /// of the same fact: a slider that moved its own thumb queues one, and this reads the
    /// number out of it. A committed value and a changed one say the same thing to a
    /// client, which is why both land here and neither is announced twice.
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
    /// A checkbox reports it as a toggle and a radio button as a selection — the same fact,
    /// announced as "checked" or as "3 of 5". Deciding it here rather than at the call site
    /// is what keeps the two from disagreeing.
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

    /// Says an invoke has completed.
    ///
    /// Raised by the application rather than by the provider, because the provider returned
    /// before the work happened — that is the pattern's own contract. "After the control has
    /// completed its associated action" is a fact only this side knows.
    pub fn invoked(&mut self, id: ControlId) {
        self.pending.push(Raise::Invoked(id));
    }

    /// Whether an overlay is open on `id`, which is both a state and an announcement.
    pub fn set_expanded(&mut self, id: ControlId, open: bool) {
        self.set_state(id, State::EXPANDED, open);
    }

    /// Takes what clients have asked for since the last tick.
    pub fn drain(&mut self, out: &mut Vec<Action>) {
        self.shared.actions.drain(out);
    }

    /// Raises everything pending. **The last thing a tick does**, so the tree a client
    /// reads back is the one the event describes, and so no raise re-enters an input
    /// handler that is still running.
    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        self.pending.flush(|id| element::provider_for(&shared, id));
    }

    /// Announces a live region, quantized and only on a change.
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
        // The backstop for a window that went without saying so. Providers a client still
        // holds resolve to nothing from here on, which is what `UIA_E_ELEMENTNOTAVAILABLE`
        // means; dropping our own references is what stops handing them out.
        element::disconnect(&self.shared);
    }
}

#[cfg(test)]
impl Uia {
    /// The tree as published. Read by the tests, which is the whole query surface.
    fn tree(&self) -> &Tree {
        &self.current
    }

    /// Latches "a client asked", without one having.
    fn latch_for_test(&mut self) {
        self.shared.queried.store(true, Relaxed);
    }

    fn queue_for_test(&self, action: Action) {
        self.shared.act(action);
    }

    fn take_pending_for_test(&mut self, out: &mut Vec<Raise>) {
        self.pending.take(out);
    }

    /// The object a client would be handed by `WM_GETOBJECT`.
    /// The published tree by identity, so a test can say "the same tree".
    fn tree_arc_for_test(&self) -> Arc<Tree> {
        Arc::clone(&self.current)
    }

    /// How many parts `id` declared, and the second one's value.
    fn parts_for_test(&self, id: ControlId) -> (usize, Option<f64>) {
        self.shared.regions.with_parts(id, |parts| {
            (parts.len(), parts.get(1).and_then(|p| p.value))
        })
    }

    /// Which part covers a region-local point.
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
