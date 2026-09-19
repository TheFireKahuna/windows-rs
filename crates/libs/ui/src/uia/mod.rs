//! UI Automation.
//!
//! The tree is published rather than queried on demand. A client's call reads an immutable
//! snapshot from automation's own worker thread and never enters the window's message pump, so
//! a provider method cannot block a screen reader and a client walking the tree at idle costs
//! no front-thread wakes.
//!
//! Commands cross the other way. `Invoke`, `Toggle`, `Select` and `SetValue` must return
//! without blocking, so each queues an [`Action`] and posts the front thread's frame message;
//! the tick then runs the widget's own handler, publishes its pixels and queues its intent,
//! through the same code a tap runs.
//!
//! # What is where
//!
//! | | |
//! |---|---|
//! | [`snapshot`] | the published table, the string pool, the live column and the hand-off |
//! | [`provider`] | the two COM objects and every interface they answer |
//! | [`text`] | `TextPattern` over the pool |
//! | [`regions`] | what a presentation region declares, and the join with its renderer |
//! | [`roles`] | one `const` row per role |
//! | [`action`] and [`events`] | the two queues that cross back |

pub(crate) mod action;
mod events;
mod provider;
mod regions;
mod roles;
pub(crate) mod snapshot;
mod text;
mod variant;

pub use action::Action;
pub use events::{Property, Raise, Val};
pub use regions::{PartDecl, RegionPeer};
pub use roles::Patterns;
pub use snapshot::{ColFlags, Entry, NONE, Part, Snapshot, State, Tree};

use crate::bindings::{HWND, LPARAM, LRESULT, WPARAM};
use crate::front::FrontHandle;
use crate::widget::{Intent, ModelState, UiaRole, What};
use provider::Shared;
use std::sync::{Arc, atomic::AtomicU64};
use windows_numerics::Vector2;
use windows_scene::{ControlId, NodeId};

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
/// Owns the publish, the event queue and the window's identity. Everything a client can reach
/// lives behind the `Arc` and is `Send + Sync`; `Uia` itself is neither, so the publish runs on
/// the thread that owns the tree.
pub struct Uia {
    /// Thread-affine by construction, which is the invariant that lets the publish write the
    /// snapshot without a second lock.
    shared: FrontHandle<Arc<Shared>>,
    /// The snapshot last published, held so a republish carries its live column forward and so
    /// a raise can read what a property now says.
    current: Arc<Tree>,
    pending: events::Pending,
    /// The step each live region last announced, so a value landing on the same step announces
    /// nothing. One row per live region, which is a handful per screen.
    announced: Vec<(ControlId, f64)>,
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
            pending: events::Pending::default(),
            announced: Vec::new(),
        }
    }

    /// Records the window every provider answers for.
    pub fn attach(&mut self, hwnd: HWND) {
        self.shared.attach(hwnd);
    }

    /// Answers `WM_GETOBJECT` with the fragment root, or `None` for an object id that names
    /// something else.
    ///
    /// The only automation call that arrives on the pump; everything a client asks afterwards
    /// is answered off the front thread.
    pub fn get_object(&mut self, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
        provider::get_object(&self.shared, w, l)
    }

    /// Releases automation's cache for the window and empties the tree.
    ///
    /// Must be called from `WM_DESTROY`, while the handle is still valid: the release names the
    /// handle, so it cannot be deferred to a drop. Dropping our own references does not release
    /// the cache automation keeps per window.
    pub fn detach(&mut self) {
        provider::disconnect(&self.shared);
        self.adopt(Arc::new(Tree::empty()));
    }

    /// Returns whether a client has asked for a provider, which is what gates building the tree.
    ///
    /// The gate is the `WM_GETOBJECT` latch alone. `UiaClientsAreListening` answers `true` on a
    /// bare Windows 11 desktop with no screen reader running, so gating the build on it would
    /// build the tree on every machine. The latch is also sufficient: `WM_GETOBJECT` is the
    /// only way into this tree, so nothing can query before it is set.
    ///
    /// Event raising gates on `UiaClientsAreListening` instead, where a false positive costs one
    /// call rather than a whole tree.
    #[must_use]
    pub fn listening(&self) -> bool {
        self.shared
            .asked
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// Returns whether a client has appeared since the last publish and would walk nothing.
    ///
    /// A window that is not laid out again does not republish on its own, so a client attaching
    /// to an idle window would see an empty tree until something moved. The tick asks this
    /// alongside its layout-changed check; the `WM_GETOBJECT` that set the latch has already
    /// posted the frame message that tick runs in.
    #[must_use]
    pub fn wants_tree(&self) -> bool {
        self.listening() && self.current.is_empty()
    }

    /// Publishes a new tree from the rows the application thread produced.
    ///
    /// Called where the hit array is adopted and nowhere else, so the entries and the array
    /// describe the same layout by construction rather than by an ordering rule.
    pub fn publish(&mut self, snapshot: &Snapshot) {
        if !self.listening() {
            // With no client latched, neither the string pool nor the link pass is built: the
            // tree is not merely smaller, it is not constructed at all.
            if !self.current.is_empty() {
                self.adopt(Arc::new(Tree::empty()));
            }
            return;
        }
        // A document that changed body or selection owes an event, and only the outgoing
        // snapshot knows what it held. A password field publishes neither.
        for field in snapshot.fields.iter().filter(|field| !field.password) {
            let Some(was) = self.current.field(field.id) else {
                continue;
            };
            if was.text != field.text {
                let from = Val::Text(Arc::clone(&was.text));
                self.pending
                    .push(Raise::Property(field.id, Property::Text, from));
                self.pending.push(Raise::text_changed(field.id));
            }
            if was.selection != field.selection {
                self.pending.push(Raise::selection_changed(field.id));
            }
        }
        self.adopt(Arc::new(Tree::adopt(snapshot)));
        self.pending.push(Raise::Structure);
    }

    /// Carries the live column forward and publishes `tree` in its place.
    ///
    /// The carry runs before the publish, so no client can observe the new tree with the old
    /// tree's values missing. A layout change disables no control and moves no slider; without
    /// this a resize would report every toggle as reset.
    fn adopt(&mut self, tree: Arc<Tree>) {
        tree.carry(&self.current);
        self.shared.evict(&tree);
        self.shared.tree.write(|held| *held = Arc::clone(&tree));
        self.current = tree;
    }

    /// Publishes the window's client origin in physical pixels, and its DIP scale.
    ///
    /// Every bounding rectangle a provider reports is computed from these, because automation
    /// reports screen pixels. Call on every move, resize and DPI change: a stale origin reports
    /// every control at the wrong place.
    pub fn set_window(&mut self, origin: Vector2, scale: f32) {
        self.current.set_window(origin, scale);
    }

    /// Publishes the offset of one scroll container, which its descendants' bounds are resolved
    /// through. Does nothing for a node the published tree scrolls nothing by.
    pub fn set_scroll(&mut self, node: NodeId, offset: Vector2) {
        self.current.set_scroll(node, offset);
    }

    /// Records where a control's value now stands, and queues the property change.
    ///
    /// One relaxed store, which is why the router can call it per pointer sample. The event
    /// folds to one per element per tick, so a drag announces once.
    pub fn set_value(&mut self, id: ControlId, value: f64) {
        let Some(at) = self.current.index_of(id) else {
            return;
        };
        let was = self.current.value(at);
        if was == Some(value) {
            return;
        }
        self.current.set_value(at, value);
        let from = was.map_or(Val::Empty, Val::Number);
        self.pending
            .push(Raise::Property(id, Property::Range, from));
        self.announce(id, at, value);
    }

    /// Records one state flag — enabled, toggled, selected or expanded — and queues the
    /// matching property event for all but `ENABLED`.
    pub fn set_state(&mut self, id: ControlId, flag: State, on: bool) {
        let Some(at) = self.current.index_of(id) else {
            return;
        };
        if self.current.state(at).has(flag) == on {
            return;
        }
        self.current.set_state(at, flag, on);
        let what = match flag {
            State::TOGGLED => Property::Toggle,
            State::SELECTED => Property::Selected,
            State::EXPANDED => Property::Expanded,
            // `IsEnabled` is not one of the properties this stack raises changes for.
            _ => return,
        };
        let from = match what {
            Property::Selected => Val::Bool(!on),
            _ => Val::Int(i32::from(!on)),
        };
        self.pending.push(Raise::Property(id, what, from));
    }

    /// Records which control holds keyboard focus, and raises a focus event when one does.
    pub fn set_focus(&mut self, id: Option<ControlId>) {
        let now = events::packed_focus(id);
        if self.current.focused() == now {
            return;
        }
        self.current.set_focused(now);
        if let Some(id) = id {
            self.pending.push(Raise::focus(id));
        }
    }

    /// Queues an overlay event, such as a menu or tooltip opening or closing.
    pub fn overlay(&mut self, raise: Raise) {
        self.pending.push(raise);
    }

    /// Binds a producer-owned value cell to a control.
    ///
    /// A presentation region's number is written by the thread that drew the pixels it
    /// describes, so it cannot live in a snapshot the front thread replaces. Creates no visual
    /// and touches no pixels, and takes effect without a republish.
    pub fn bind_value(&mut self, id: ControlId, cell: Arc<AtomicU64>) {
        self.shared.regions.bind_value(id, cell);
    }

    /// Declares what is nameable inside a presentation region.
    ///
    /// Parts carry region-local rects and are restated by whoever owns the region whenever its
    /// mapping moves — a range change, a band added, a resize, a band dragged. They live beside
    /// the tree rather than in it, so none of those republishes every element on the screen.
    /// Each part's name and role travel with it, because they do not move when its geometry
    /// does.
    pub fn set_parts(&mut self, id: ControlId, parts: &[Part]) {
        self.shared.regions.set_parts(id, parts);
    }

    /// Watches a presentation region, replacing any earlier watch on the same control.
    ///
    /// The region's renderer publishes the geometry, this side owns what that geometry means,
    /// and [`sync_regions`](Self::sync_regions) joins the two.
    pub fn watch_region(&mut self, peer: RegionPeer) {
        self.shared.regions.watch(peer);
    }

    /// Re-joins every watched region whose renderer has moved. Called once per tick.
    ///
    /// A region whose geometry version has not moved costs one acquire load and nothing else,
    /// so this can sit on the tick unconditionally.
    pub fn sync_regions(&mut self) {
        _ = self.shared.regions.sync();
    }

    /// Forgets everything a control declared: its parts, its bound value, its last announcement
    /// and its region watch. Called where the control table drops the control.
    pub fn release(&mut self, id: ControlId) {
        self.shared.regions.forget(id);
        self.announced.retain(|&(held, _)| held != id);
    }

    /// Records the value changes and taps an interaction produced.
    ///
    /// Reads the intents the front side already builds rather than observing the interaction a
    /// second time: a slider that moved its own thumb queues one, and this takes the number out
    /// of it. A moved value and a committed one both report a value change, and
    /// [`set_value`](Self::set_value) drops a value equal to the one held, so neither is
    /// announced twice. A two-axis drag carries no value and invokes nothing: what it moves is
    /// the application's own subject.
    pub fn observe(&mut self, intents: &[Intent]) {
        for intent in intents {
            match intent.what {
                What::Scalar { value, .. } => self.set_value(intent.target, value),
                What::Tapped => self.invoked(intent.target),
                _ => {}
            }
        }
    }

    /// Records a model-state change, resolving what it means from the element's role.
    ///
    /// A checkbox reports the same fact as a toggle and every other role as a selection, so a
    /// client hears "checked" or "3 of 5". Resolving it here rather than at each call site
    /// keeps the two from disagreeing.
    pub fn set_model(&mut self, id: ControlId, state: ModelState) {
        let Some(at) = self.current.index_of(id) else {
            return;
        };
        let role = self
            .current
            .at(at)
            .map_or(UiaRole::None, |entry| entry.role);
        self.set_state(id, State::ENABLED, state != ModelState::Disabled);
        let on = state == ModelState::Selected;
        match role {
            UiaRole::CheckBox => self.set_state(id, State::TOGGLED, on),
            _ => self.set_state(id, State::SELECTED, on),
        }
    }

    /// Queues an invoked event for `id`.
    ///
    /// Raised by the application rather than by the provider: `Invoke` returns before the work
    /// happens, and the event is owed after the control has completed its action, which only
    /// this side knows.
    pub fn invoked(&mut self, id: ControlId) {
        self.pending.push(Raise::invoked(id));
    }

    /// Records whether an overlay is open on `id`, as both state and an expand-collapse event.
    pub fn set_expanded(&mut self, id: ControlId, open: bool) {
        self.set_state(id, State::EXPANDED, open);
    }

    /// Moves every action clients queued since the last tick into `out`.
    pub fn drain(&mut self, out: &mut Vec<Action>) {
        self.shared.actions.drain(out);
    }

    /// Moves every editing request clients queued since the last tick into `out`.
    pub(crate) fn text_actions(&self, out: &mut Vec<action::TextAction>) {
        self.shared.edits.drain(out);
    }

    /// Raises every pending event. Called as the last step of a tick, so the tree a client reads
    /// back is the one the event describes and no raise re-enters an input handler that is still
    /// running.
    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        self.pending.flush(&shared, &self.current);
    }

    /// Queues a live-region announcement, quantized to [`LIVE_QUANTUM`] and only when the step
    /// the value lands on differs from the one last announced.
    fn announce(&mut self, id: ControlId, at: u16, value: f64) {
        if !events::is_live(&self.current, at) {
            return;
        }
        let step = (value / LIVE_QUANTUM).round() * LIVE_QUANTUM;
        match self.announced.iter_mut().find(|(held, _)| *held == id) {
            Some((_, last)) if *last == step => return,
            Some((_, last)) => *last = step,
            None => self.announced.push((id, step)),
        }
        self.pending.push(Raise::live(id));
    }
}

impl Drop for Uia {
    fn drop(&mut self) {
        // The backstop for a window dropped without a `WM_DESTROY`. Providers a client still
        // holds stop resolving from here on, which is what `UIA_E_ELEMENTNOTAVAILABLE` reports;
        // dropping our references stops new ones.
        provider::disconnect(&self.shared);
    }
}

#[cfg(test)]
impl Uia {
    /// Returns the published tree.
    fn tree(&self) -> &Tree {
        &self.current
    }

    /// Returns the published tree by identity, so a caller can compare two publishes for
    /// sameness.
    fn tree_arc_for_test(&self) -> Arc<Tree> {
        Arc::clone(&self.current)
    }

    /// Sets the client-asked latch without a `WM_GETOBJECT` having arrived.
    fn latch_for_test(&mut self) {
        self.shared
            .asked
            .store(true, core::sync::atomic::Ordering::Relaxed);
    }

    fn queue_for_test(&self, action: Action) {
        self.shared.act(action);
    }

    fn take_pending_for_test(&mut self, out: &mut Vec<Raise>) {
        self.pending.take(out);
    }

    /// Returns how many parts `id` declared, and the number the second part's slot holds.
    fn parts_for_test(&self, id: ControlId) -> (usize, Option<f64>) {
        (
            self.shared.regions.with_subs(id, <[Part]>::len),
            self.shared.regions.value(id, 1),
        )
    }

    /// Returns the part covering a region-local point.
    fn part_at_for_test(&self, id: ControlId, x: f32, y: f32) -> Option<u32> {
        let found = self.shared.regions.pick(id, windows_scene::Point { x, y });
        (found != regions::NO_PART).then_some(found)
    }

    fn root_for_test(&self) -> crate::bindings::IRawElementProviderSimple {
        provider::provider_for(&self.shared, ControlId::NONE).expect("the root always resolves")
    }
}

#[cfg(test)]
mod com_tests;
#[cfg(test)]
mod tests;
