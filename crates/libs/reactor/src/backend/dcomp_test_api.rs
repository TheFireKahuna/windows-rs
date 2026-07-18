//! Test seam for the DirectComposition backend.
//!
//! The backend's internals are `pub(crate)` by design; this module is the one
//! place that reaches into them and re-publishes a *narrow, purpose-built*
//! surface for the out-of-crate test crate. It is compiled only when both
//! `dcomp-backend` and `test` are enabled, so it never exists in a shipping
//! build.
//!
//! Nothing here contains policy. Every function either forwards to the real
//! backend function or drives real backend types; a test that passes here is a
//! statement about the backend, not about this file.

use std::cell::RefCell;
use std::rc::Rc;

use super::dcomp;
use crate::backend::{
    AccessibilityModifiers, AnimationConfig, Backend, ControlId, ControlKind, Event, EventHandler,
    ImplicitTransitions, KeyboardAccelerator, LayoutAnimationConfig, PointerHandlers, Prop,
    PropValue, RichTextParagraph, SelectionMode, ThemeRef, Tooltip,
};
use crate::drag::DragHandlers;
use crate::interaction::Callback;
use dcomp::record::{CreateWithId, RecordingBackend};

// ── layout.rs ────────────────────────────────────────────────────────────────

/// [`dcomp::layout::snap`] — DIP coordinate onto the physical pixel grid.
pub fn snap(v: f32, scale: f32) -> f32 {
    dcomp::layout::snap(v, scale)
}

/// [`dcomp::layout`]'s composition z-order vocabulary. The band ladder and the
/// key's field order together ARE the ordering policy `layout::sync` applies to
/// every node's owned stack (via `derive(Ord)`), so sorting these types in a
/// test sorts the shipping comparator, not a copy of it.
pub use dcomp::layout::{Band, Slot, StackKey};

/// The exact edge-snapping arithmetic `layout::assign` performs for one node:
/// given an absolute DIP origin and extent, return the snapped `(x, w)` it
/// writes into the node's `LaidRect`. Mirrors the two lines in `assign` so a
/// test can assert the sibling-flushness property those lines exist to
/// guarantee without standing up a Taffy tree.
pub fn snap_edge(origin: f32, extent: f32, scale: f32) -> (f32, f32) {
    let s = snap(origin, scale);
    (s, snap(origin + extent, scale) - s)
}

// ── theme.rs ─────────────────────────────────────────────────────────────────

/// [`dcomp::theme::dark_wash_alpha`] — sRGB-authored alpha → linear-blend alpha
/// over the dark surface.
pub fn dark_wash_alpha(a: f32) -> f32 {
    dcomp::theme::dark_wash_alpha(a)
}

/// [`dcomp::theme::light_wash_alpha`] — the light-base counterpart.
pub fn light_wash_alpha(a: f32) -> f32 {
    dcomp::theme::light_wash_alpha(a)
}

// ── record.rs ────────────────────────────────────────────────────────────────

/// One replayed backend call, captured by [`Recorder`]'s spy.
///
/// Rendered as a string rather than a typed enum so the seam does not have to
/// re-export the backend's whole argument vocabulary; the point of the log is
/// *order* and *payload identity*, both of which survive `Debug`.
pub type AppliedLog = Vec<String>;

/// A [`Backend`] that records what the recorder replayed into it, in order.
#[derive(Default)]
struct Spy {
    log: Rc<RefCell<AppliedLog>>,
    /// Event handlers as they arrived at replay, so a test can invoke one and
    /// prove the side table round-tripped the real closure and not a copy.
    events: Rc<RefCell<Vec<(ControlId, Event, EventHandler)>>>,
    tooltips: Rc<RefCell<Vec<(ControlId, Option<Tooltip>)>>>,
}

impl Spy {
    fn note(&self, s: String) {
        self.log.borrow_mut().push(s);
    }
}

impl CreateWithId for Spy {
    fn create_with_id(&mut self, id: ControlId, kind: ControlKind) {
        self.note(format!("create {} {kind:?}", id.get()));
    }
}

impl Backend for Spy {
    fn create(&mut self, kind: ControlKind) -> ControlId {
        unreachable!("RecordingBackend mints ids and calls create_with_id ({kind:?})")
    }

    fn set_prop(&mut self, id: ControlId, prop: Prop, value: &PropValue) {
        self.note(format!("set_prop {} {prop:?} {value:?}", id.get()));
    }

    fn append_child(&mut self, parent: ControlId, child: ControlId) {
        self.note(format!("append {} {}", parent.get(), child.get()));
    }

    fn remove_child(&mut self, parent: ControlId, index: usize) {
        self.note(format!("remove {} @{index}", parent.get()));
    }

    fn replace_child(&mut self, parent: ControlId, index: usize, new: ControlId) {
        self.note(format!("replace {} @{index} {}", parent.get(), new.get()));
    }

    fn move_child(&mut self, parent: ControlId, from: usize, to: usize) {
        self.note(format!("move {} {from}->{to}", parent.get()));
    }

    fn insert_child(&mut self, parent: ControlId, index: usize, child: ControlId) {
        self.note(format!("insert {} @{index} {}", parent.get(), child.get()));
    }

    fn destroy(&mut self, id: ControlId) {
        self.note(format!("destroy {}", id.get()));
    }

    fn attach_event(&mut self, id: ControlId, event: Event, handler: EventHandler) {
        self.note(format!("attach_event {} {event:?}", id.get()));
        self.events.borrow_mut().push((id, event, handler));
    }

    fn detach_event(&mut self, id: ControlId, event: Event) {
        self.note(format!("detach_event {} {event:?}", id.get()));
    }

    fn set_tooltip(&mut self, id: ControlId, tooltip: Option<&Tooltip>) {
        self.note(format!("set_tooltip {} {tooltip:?}", id.get()));
        self.tooltips.borrow_mut().push((id, tooltip.cloned()));
    }

    fn set_pointer_handlers(&mut self, id: ControlId, handlers: Option<&PointerHandlers>) {
        self.note(format!(
            "set_pointer_handlers {} {}",
            id.get(),
            handlers.is_some()
        ));
    }

    fn set_drag_handlers(&mut self, id: ControlId, handlers: Option<&DragHandlers>) {
        self.note(format!(
            "set_drag_handlers {} {}",
            id.get(),
            handlers.is_some()
        ));
    }

    fn set_theme_bindings(
        &mut self,
        id: ControlId,
        kind: ControlKind,
        bindings: &[(Prop, ThemeRef)],
    ) {
        self.note(format!(
            "set_theme_bindings {} {kind:?} {}",
            id.get(),
            bindings.len()
        ));
    }

    fn on_theme_changed(&mut self) {
        self.note("on_theme_changed".to_string());
    }

    fn set_templated_item_count(&mut self, id: ControlId, count: usize) {
        self.note(format!("set_templated_item_count {} {count}", id.get()));
    }

    fn set_templated_selected_index(&mut self, id: ControlId, index: i32) {
        self.note(format!("set_templated_selected_index {} {index}", id.get()));
    }

    fn set_templated_selection_mode(&mut self, id: ControlId, mode: SelectionMode) {
        self.note(format!(
            "set_templated_selection_mode {} {mode:?}",
            id.get()
        ));
    }

    fn set_header_element(&mut self, id: ControlId, header_id: Option<ControlId>) {
        self.note(format!(
            "set_header_element {} {:?}",
            id.get(),
            header_id.map(ControlId::get)
        ));
    }

    fn set_keyboard_accelerators(&mut self, id: ControlId, accelerators: &[KeyboardAccelerator]) {
        self.note(format!(
            "set_keyboard_accelerators {} {}",
            id.get(),
            accelerators.len()
        ));
    }

    fn set_accessibility(&mut self, id: ControlId, accessibility: &AccessibilityModifiers) {
        let _ = accessibility;
        self.note(format!("set_accessibility {}", id.get()));
    }

    fn set_implicit_transitions(&mut self, id: ControlId, t: Option<ImplicitTransitions>) {
        self.note(format!(
            "set_implicit_transitions {} {}",
            id.get(),
            t.is_some()
        ));
    }

    fn set_layout_animation(&mut self, id: ControlId, c: Option<LayoutAnimationConfig>) {
        self.note(format!("set_layout_animation {} {}", id.get(), c.is_some()));
    }

    fn run_property_animation(&mut self, id: ControlId, c: Option<AnimationConfig>) {
        self.note(format!(
            "run_property_animation {} {}",
            id.get(),
            c.is_some()
        ));
    }

    fn set_exit_transition(&mut self, id: ControlId, c: Option<AnimationConfig>) {
        self.note(format!("set_exit_transition {} {}", id.get(), c.is_some()));
    }

    fn set_rich_text_paragraphs(&mut self, id: ControlId, paragraphs: &[RichTextParagraph]) {
        self.note(format!(
            "set_rich_text_paragraphs {} {}",
            id.get(),
            paragraphs.len()
        ));
    }

    fn attach_templated_selection_changed(&mut self, id: ControlId, _handler: Callback<i32>) {
        self.note(format!("attach_templated_selection_changed {}", id.get()));
    }

    fn attach_templated_realization(
        &mut self,
        id: ControlId,
        _realize: Rc<dyn Fn(usize)>,
        _recycle: Rc<dyn Fn(usize)>,
    ) {
        self.note(format!("attach_templated_realization {}", id.get()));
    }
}

/// The real [`RecordingBackend`] — the command-buffer seam between the
/// reconciler and the composition tree — wrapped around a spy that records
/// what replay actually applied.
///
/// [`Self::backend`] hands out the full public [`Backend`] surface, so a test
/// drives the recorder exactly as the reconciler does.
pub struct Recorder {
    inner: RecordingBackend<Spy>,
    log: Rc<RefCell<AppliedLog>>,
    events: Rc<RefCell<Vec<(ControlId, Event, EventHandler)>>>,
    tooltips: Rc<RefCell<Vec<(ControlId, Option<Tooltip>)>>>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    pub fn new() -> Self {
        let spy = Spy::default();
        let log = Rc::clone(&spy.log);
        let events = Rc::clone(&spy.events);
        let tooltips = Rc::clone(&spy.tooltips);
        Self {
            inner: RecordingBackend::new(spy),
            log,
            events,
            tooltips,
        }
    }

    /// Drive the recorder through the public backend trait.
    pub fn backend(&mut self) -> &mut dyn Backend {
        &mut self.inner
    }

    /// Commands buffered but not yet replayed.
    pub fn pending(&self) -> usize {
        self.inner.pending()
    }

    /// Replay the buffer into the spy.
    pub fn flush(&mut self) {
        self.inner.flush();
    }

    /// Everything the spy has been asked to apply, in order.
    pub fn applied(&self) -> AppliedLog {
        self.log.borrow().clone()
    }

    /// Invoke the `n`-th event handler that reached the spy at replay.
    /// Panics if there is no such handler — a test asserting a round trip
    /// wants the absence to be loud.
    pub fn invoke_replayed_event(&self, n: usize) {
        let events = self.events.borrow();
        let (_, _, handler) = &events[n];
        handler.invoke();
    }

    /// Tooltips as they arrived at replay.
    pub fn replayed_tooltips(&self) -> Vec<Option<Tooltip>> {
        self.tooltips
            .borrow()
            .iter()
            .map(|(_, t)| t.clone())
            .collect()
    }
}

/// Compile-time proof, re-stated where a test crate can see it, that the
/// command buffer's payload types are `Send`. The lib asserts this too; this
/// copy fails the *test* build if the seam ever stops holding.
pub fn assert_cmd_buffer_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<dcomp::record::Cmd>();
    assert_send::<dcomp::record::SendValue>();
}

// ── node.rs (Arena) ──────────────────────────────────────────────────────────

/// A real [`dcomp::node::Arena`] holding real [`Node`](dcomp::node::Node)s.
///
/// Node construction needs a compositor `ContainerVisual`, so this harness
/// stands up a **windowless** `Windows.UI.Composition.Compositor` (a
/// `DispatcherQueue` on the calling thread plus `Compositor::new()`) — no
/// HWND, no `DesktopWindowTarget`, no swap chain, nothing on screen.
/// [`ArenaHarness::new`] returns `Err` where that is unavailable.
pub struct ArenaHarness {
    compositor: crate::system_bindings::Compositor,
    arena: dcomp::node::Arena,
}

impl ArenaHarness {
    pub fn new() -> windows_core::Result<Self> {
        use crate::system_bindings::{
            CreateDispatcherQueueController, DQTAT_COM_ASTA, DQTYPE_THREAD_CURRENT,
            DispatcherQueueOptions,
        };
        let options = DispatcherQueueOptions {
            dwSize: size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_ASTA,
        };
        let mut controller = core::ptr::null_mut();
        unsafe {
            // Already-present controller returns an error we ignore.
            let _ = CreateDispatcherQueueController(options, &mut controller);
        }
        Ok(Self {
            compositor: crate::system_bindings::Compositor::new()?,
            arena: dcomp::node::Arena::default(),
        })
    }

    fn node(&self, kind: ControlKind) -> windows_core::Result<dcomp::node::Node> {
        Ok(dcomp::node::Node::new(
            kind,
            self.compositor.CreateContainerVisual()?,
        ))
    }

    /// `Arena::insert` — the arena mints the id.
    pub fn insert(&mut self, kind: ControlKind) -> windows_core::Result<ControlId> {
        let node = self.node(kind)?;
        Ok(self.arena.insert(node))
    }

    /// `Arena::insert_with_id` — the caller (the recorder) minted the id.
    pub fn insert_with_id(&mut self, id: ControlId, kind: ControlKind) -> windows_core::Result<()> {
        let node = self.node(kind)?;
        self.arena.insert_with_id(id, node);
        Ok(())
    }

    pub fn contains(&self, id: ControlId) -> bool {
        self.arena.get(id).is_some()
    }

    pub fn kind_of(&self, id: ControlId) -> Option<ControlKind> {
        self.arena.get(id).map(|n| n.kind)
    }

    pub fn remove(&mut self, id: ControlId) -> bool {
        self.arena.remove(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.arena.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether this node has actually ALLOCATED its [`Ctrl`](dcomp::node::Ctrl).
    ///
    /// `Ctrl` is the largest per-node payload after the Taffy style and is
    /// boxed lazily, so a node that never receives control state must not carry
    /// one. This is the observation that makes the saving real rather than
    /// nominal — hence a seam of its own.
    pub fn ctrl_allocated(&self, id: ControlId) -> Option<bool> {
        self.arena.get(id).map(|n| n.ctrl_allocated())
    }

    /// A representative slice of a node's [`Ctrl`](dcomp::node::Ctrl) as read
    /// through the normal accessor, so a test can compare what an *absent*
    /// `Ctrl` reads as against what a materialised-but-untouched one reads as.
    ///
    /// The non-zero defaults are the interesting ones: `max`, `is_active`,
    /// `selected_index`, `hot_index` and `content_align` all start at values a
    /// zeroed struct would NOT have, so they are the fields a lazily-created
    /// `Ctrl` would silently get wrong.
    pub fn ctrl_probe(&self, id: ControlId) -> Option<CtrlProbe> {
        self.arena.get(id).map(|n| {
            let c = n.ctrl();
            CtrlProbe {
                value: c.value,
                min: c.min,
                max: c.max,
                is_on: c.is_on,
                is_active: c.is_active,
                selected_index: c.selected_index,
                hot_index: c.hot_index,
                content_align: c.content_align,
                items: c.items.len(),
                menu: c.menu.len(),
                placeholder: c.placeholder.clone(),
            }
        })
    }

    /// Force this node's `Ctrl` into existence without changing any field —
    /// the "materialised but untouched" state `ctrl_probe` is compared against.
    pub fn ctrl_materialize(&mut self, id: ControlId) {
        if let Some(n) = self.arena.get_mut(id) {
            let _ = n.ctrl_mut();
        }
    }

    /// Write one `Ctrl` field (`is_on`), as `set_prop` would.
    pub fn ctrl_set_is_on(&mut self, id: ControlId, v: bool) {
        if let Some(n) = self.arena.get_mut(id) {
            n.ctrl_mut().is_on = v;
        }
    }

    /// Whether this node has allocated its `Extras` — the second lazily-boxed
    /// tier, holding the caption / nav-pane / flyout / editor-policy state.
    pub fn extras_allocated(&self, id: ControlId) -> Option<bool> {
        self.arena.get(id).map(|n| n.extras_allocated())
    }

    /// `dcomp::apply_prop` — the REAL body of `Backend::set_prop`, including
    /// its `PropValue::Unset` arm, which is the reset path.
    ///
    /// Driving the shipping function is the whole point: a test that reset a
    /// node through its own copy of the rules would agree with itself no
    /// matter how far the backend drifted.
    pub fn apply_prop(&mut self, id: ControlId, prop: Prop, value: &PropValue) {
        if let Some(n) = self.arena.get_mut(id) {
            dcomp::apply_prop(n, prop, value);
        }
    }

    /// A node's whole value state as a string: paint, Taffy style, control
    /// state (which contains `Extras`), and the loose per-node layout fields.
    ///
    /// Deliberately a broad `Debug` dump rather than a field list, so a reset
    /// that restores the field it was written for but disturbs a neighbour is
    /// caught by the same comparison. Control state is read through the
    /// `ctrl()` accessor, so an absent box and a materialised-but-default one
    /// render identically — which is exactly the equivalence a reset relies on.
    ///
    /// The dirty/invalidation flags are excluded: a reset legitimately marks
    /// the node for repaint, and a fresh node is born already dirty, so they
    /// describe scheduling rather than value.
    pub fn node_digest(&self, id: ControlId) -> Option<String> {
        self.arena.get(id).map(|n| {
            format!(
                "paint={:?}\nstyle={:?}\nctrl={:?}\nh_align={} v_align={} z={} spacing={}\n\
                 rows={:?} cols={:?}\neditor={:?}",
                n.paint,
                n.style,
                n.ctrl(),
                n.h_align,
                n.v_align,
                n.z_index,
                n.spacing,
                n.grid_rows,
                n.grid_cols,
                n.editor.as_ref().map(|e| (e.text(), e.seeded)),
            )
        })
    }
}

// ── nav.rs — the NavigationView pane ─────────────────────────────────────────

/// What a point in a nav pane lands on — [`dcomp::nav::Hit`] re-published so
/// the test crate can name the elements the backend resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavHit {
    Back,
    Toggle,
    Item(i32),
    Settings,
}

impl From<dcomp::nav::Hit> for NavHit {
    fn from(h: dcomp::nav::Hit) -> Self {
        match h {
            dcomp::nav::Hit::Back => Self::Back,
            dcomp::nav::Hit::Toggle => Self::Toggle,
            dcomp::nav::Hit::Item(i) => Self::Item(i),
            dcomp::nav::Hit::Settings => Self::Settings,
        }
    }
}

/// The pane geometry a NavigationView resolves to at a given laid-out size —
/// [`dcomp::nav::Metrics`] plus the two derived quantities that depend on the
/// node's height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavProbe {
    pub width: f32,
    pub expanded: bool,
    pub items_y: f32,
    pub back: bool,
    pub toggle: bool,
    pub settings: bool,
    pub visible_items: usize,
    /// Top of the settings row, when there is room for one.
    pub settings_y: Option<f32>,
}

impl ArenaHarness {
    /// Stamp the laid-out rect a real layout pass would have written. The pane
    /// is adaptive — its display mode and how many rows fit both depend on the
    /// node's own size — so a test that never lays out is testing the pane at
    /// 0x0 and nothing else.
    pub fn set_rect(&mut self, id: ControlId, w: f32, h: f32) {
        if let Some(n) = self.arena.get_mut(id) {
            n.rect.w = w;
            n.rect.h = h;
        }
    }

    /// Resolve the pane geometry, through the real
    /// [`dcomp::nav::metrics`] the paint and hit test both call.
    pub fn nav_probe(&self, id: ControlId) -> Option<NavProbe> {
        let n = self.arena.get(id)?;
        if n.kind != ControlKind::NavigationView {
            return None;
        }
        let has_title = n.nav_text.as_ref().is_some_and(|t| t.title.is_some());
        let m = dcomp::nav::metrics(n.extras(), n.rect.w, has_title);
        let count = n.ctrl().items.len();
        Some(NavProbe {
            width: m.width,
            expanded: m.kind.expanded(),
            items_y: m.items_y,
            back: m.back,
            toggle: m.toggle,
            settings: m.settings,
            visible_items: dcomp::nav::visible_items(&m, n.rect.h, count),
            settings_y: dcomp::nav::settings_rect(&m, n.rect.h).map(|r| r.top),
        })
    }

    /// Whether the pane's layout inset — what the content child is pushed behind
    /// — currently equals `dips`.
    ///
    /// Compared by VALUE against `taffy::length(..)` rather than destructured:
    /// `LengthPercentage` is a packed calc representation in taffy 0.11 with no
    /// public variant to match on, and equality is the only thing the assertion
    /// needs anyway.
    pub fn nav_pad_left_is(&self, id: ControlId, dips: f32) -> bool {
        self.arena
            .get(id)
            .is_some_and(|n| n.style.padding.left == taffy::style_helpers::length(dips))
    }

    /// Re-derive the pane's layout inset — [`dcomp::layout::apply_nav_metrics`],
    /// the same call `apply_prop` and the layout pass make.
    pub fn nav_apply_metrics(&mut self, id: ControlId) {
        if let Some(n) = self.arena.get_mut(id) {
            dcomp::layout::apply_nav_metrics(n);
        }
    }

    /// Resolve a node-local point against the pane, through the real
    /// [`dcomp::nav::hit`] the pointer path and the accessibility tree share.
    pub fn nav_hit(&self, id: ControlId, x: f32, y: f32) -> Option<NavHit> {
        let n = self.arena.get(id)?;
        if n.kind != ControlKind::NavigationView {
            return None;
        }
        let has_title = n.nav_text.as_ref().is_some_and(|t| t.title.is_some());
        let m = dcomp::nav::metrics(n.extras(), n.rect.w, has_title);
        dcomp::nav::hit(&m, n.rect.h, n.ctrl().items.len(), x, y).map(NavHit::from)
    }

    /// The node-local box the pane draws item `i` in — the geometry the hit test
    /// must agree with.
    pub fn nav_item_box(&self, id: ControlId, i: i32) -> Option<(f32, f32, f32, f32)> {
        let n = self.arena.get(id)?;
        let has_title = n.nav_text.as_ref().is_some_and(|t| t.title.is_some());
        let m = dcomp::nav::metrics(n.extras(), n.rect.w, has_title);
        let r = dcomp::nav::item_rect(&m, i);
        Some((r.left, r.top, r.width(), r.height()))
    }
}

/// A snapshot of the [`Ctrl`](dcomp::node::Ctrl) fields whose defaults are not
/// all-zero — see [`ArenaHarness::ctrl_probe`].
#[derive(Debug, Clone, PartialEq)]
pub struct CtrlProbe {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub is_on: bool,
    pub is_active: bool,
    pub selected_index: i32,
    pub hot_index: i32,
    pub content_align: i32,
    pub items: usize,
    pub menu: usize,
    pub placeholder: String,
}
