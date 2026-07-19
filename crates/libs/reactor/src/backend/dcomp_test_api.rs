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
use crate::style::PointerEventInfo;
use crate::widgets::Subscription;
use dcomp::node::PointerInterest;
use dcomp::record::{FlyoutDecl, FrontBackend, Intent, IntentPayload, RecordingBackend};
use crate::gesture::{GestureEvent, GestureInterest, GestureOutcome};

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

/// The `Send` face of an attached flyout, as it crosses the record seam — see
/// `dcomp::record::FlyoutDecl`. Re-published so a test can assert what does
/// (and does not) reach the front.
pub use dcomp::record::FlyoutDecl as ReplayedFlyout;

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

// ── input.rs (§7.3 keyboard decision policy) ─────────────────────────────────
//
// The three consumption decisions §7.3 requires stay synchronous, re-published
// as the pure functions they are. A WndProc is unreachable headless, so a test
// exercises the shipping decision directly rather than the message plumbing.

/// [`dcomp::editor_claims_key`] — the fixed editor-vs-accelerator conflict rule:
/// a focused editor keeps its Ctrl+A/C/X/V and unmodified printable/editing
/// keys; chorded bindings and F-keys win over it.
pub fn editor_claims_key(vk: u32, ctrl: bool, alt: bool) -> bool {
    dcomp::input::editor_claims_key(vk, ctrl, alt)
}

/// [`dcomp::input::is_function_key`] — F1..F24.
pub fn is_function_key(vk: u32) -> bool {
    dcomp::input::is_function_key(vk)
}

/// [`dcomp::input::sys_key_falls_through`] — whether an unconsumed sys-key must
/// reach `DefWindowProc` (the Alt+F4 / F10 / Alt+Space fix).
pub fn sys_key_falls_through(is_sys: bool, consumed: bool) -> bool {
    dcomp::input::sys_key_falls_through(is_sys, consumed)
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
    tooltips: Rc<RefCell<Vec<(ControlId, Option<Tooltip>)>>>,
    flyouts: Rc<RefCell<Vec<(ControlId, Option<FlyoutDecl>)>>>,
    /// Intents a test has staged (via [`Recorder::queue_*`]) for the next
    /// [`FrontBackend::take_intents`] — the headless stand-in for the input
    /// paths that queue them in the real backend.
    intents: Rc<RefCell<Vec<Intent>>>,
}

impl Spy {
    fn note(&self, s: String) {
        self.log.borrow_mut().push(s);
    }
}

impl FrontBackend for Spy {
    fn declare_event(&mut self, id: ControlId, event: Event) {
        self.note(format!("declare_event {} {event:?}", id.get()));
    }

    fn set_pointer_interest(&mut self, id: ControlId, interest: PointerInterest) {
        self.note(format!("set_pointer_interest {} {interest:?}", id.get()));
    }

    fn set_value_stamped(&mut self, id: ControlId, value: f64, based_on: u64) {
        self.note(format!("set_value {} {value} based_on={based_on}", id.get()));
    }

    fn set_text_stamped(&mut self, id: ControlId, text: &str, based_on: u64) {
        self.note(format!("set_text {} {text:?} based_on={based_on}", id.get()));
    }

    fn set_keybindings(
        &mut self,
        id: ControlId,
        keys: Vec<(crate::VirtualKey, crate::VirtualKeyModifiers)>,
    ) {
        self.note(format!("set_keybindings {} {}", id.get(), keys.len()));
    }

    fn set_flyout(&mut self, id: ControlId, decl: Option<FlyoutDecl>) {
        self.note(format!("set_flyout {} {decl:?}", id.get()));
        self.flyouts.borrow_mut().push((id, decl));
    }

    fn take_intents(&mut self) -> Vec<Intent> {
        std::mem::take(&mut *self.intents.borrow_mut())
    }
}

impl Backend for Spy {
    fn create(&mut self, id: ControlId, kind: ControlKind) {
        self.note(format!("create {} {kind:?}", id.get()));
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

    /// Never reached through the recorder — replay declares via
    /// [`FrontBackend::declare_event`]; the handler stays app-side. Logged so
    /// a regression that routes the closure back into the backend is visible.
    fn attach_event(&mut self, id: ControlId, event: Event, handler: EventHandler) {
        let _ = handler;
        self.note(format!("attach_event {} {event:?} (closure reached backend!)", id.get()));
    }

    fn detach_event(&mut self, id: ControlId, event: Event) {
        self.note(format!("detach_event {} {event:?}", id.get()));
    }

    /// Never reached through the recorder — the full `Tooltip` stays in the
    /// recorder's app-side map and only a `Send` declaration rides the buffer.
    /// Logged (and collected) so a regression that routes the payload back
    /// into the backend is visible.
    fn set_tooltip(&mut self, id: ControlId, tooltip: Option<&Tooltip>) {
        self.note(format!("set_tooltip {} (payload reached backend!)", id.get()));
        self.tooltips.borrow_mut().push((id, tooltip.cloned()));
    }

    fn set_pointer_handlers(&mut self, id: ControlId, handlers: Option<&PointerHandlers>) {
        self.note(format!(
            "set_pointer_handlers {} {}",
            id.get(),
            handlers.is_some()
        ));
    }

    /// Never reached through the recorder — drag closures stay app-side and
    /// the buffer carries only `DragInterest` bits.
    fn set_drag_handlers(&mut self, id: ControlId, handlers: Option<&DragHandlers>) {
        self.note(format!(
            "set_drag_handlers {} {} (payload reached backend!)",
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

/// The real [`RecordingBackend`] — the app half of the record seam — beside a
/// spy front half that records what replay actually applied, held apart
/// exactly as the host holds them: the recorder never touches the spy except
/// through a taken `Send` buffer, and the spy's intents reach the recorder
/// only through [`Self::drain_and_run`].
///
/// [`Self::backend`] hands out the full public [`Backend`] surface, so a test
/// drives the recorder exactly as the reconciler does. The intent half of the
/// seam is driven from the other side: [`Self::queue_*`] stage intents where
/// the real backend's input paths would, and [`Self::drain_and_run`] performs
/// the recorder's app-side resolution + invocation exactly as the host does.
pub struct Recorder {
    rec: RecordingBackend,
    spy: Spy,
    log: Rc<RefCell<AppliedLog>>,
    tooltips: Rc<RefCell<Vec<(ControlId, Option<Tooltip>)>>>,
    flyouts: Rc<RefCell<Vec<(ControlId, Option<FlyoutDecl>)>>>,
    intents: Rc<RefCell<Vec<Intent>>>,
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
        let tooltips = Rc::clone(&spy.tooltips);
        let flyouts = Rc::clone(&spy.flyouts);
        let intents = Rc::clone(&spy.intents);
        Self {
            rec: RecordingBackend::new(),
            spy,
            log,
            tooltips,
            flyouts,
            intents,
        }
    }

    /// Drive the recorder through the public backend trait.
    pub fn backend(&mut self) -> &mut dyn Backend {
        &mut self.rec
    }

    /// Commands buffered but not yet replayed.
    pub fn pending(&self) -> usize {
        self.rec.pending()
    }

    /// Take the buffer and replay it into the spy — what `post_render` does
    /// into the real backend.
    pub fn flush(&mut self) {
        let cmds = self.rec.take_cmds();
        dcomp::record::replay(&mut self.spy, cmds);
    }

    /// Everything the spy has been asked to apply, in order.
    pub fn applied(&self) -> AppliedLog {
        self.log.borrow().clone()
    }

    /// Flyout declarations as they arrived at replay — the `Send` face only;
    /// the `Element` and the `on_closed` callback never cross.
    pub fn replayed_flyouts(&self) -> Vec<Option<FlyoutDecl>> {
        self.flyouts.borrow().iter().map(|(_, d)| d.clone()).collect()
    }

    /// Tooltips as they arrived at replay.
    pub fn replayed_tooltips(&self) -> Vec<Option<Tooltip>> {
        self.tooltips
            .borrow()
            .iter()
            .map(|(_, t)| t.clone())
            .collect()
    }

    /// Stage a unit-payload event intent (`Click`, `BackRequested`), as the
    /// backend's input paths would queue it.
    pub fn queue_unit_event(&mut self, id: ControlId, event: Event) {
        self.intents.borrow_mut().push(Intent::Event {
            id,
            event,
            payload: IntentPayload::Unit,
        });
    }

    /// Stage a `ValueChanged` intent carrying revision `rev`.
    pub fn queue_value_changed(&mut self, id: ControlId, value: f64, rev: u64) {
        self.intents.borrow_mut().push(Intent::ValueChanged { id, value, rev });
    }

    /// Stage an editor-text intent (`TextChanged` / `QuerySubmitted` / …)
    /// carrying buffer revision `rev`, as the backend's `fire_editor_text`
    /// would (§7.2, text half).
    pub fn queue_editor_text(&mut self, id: ControlId, event: Event, text: &str, rev: u64) {
        self.intents.borrow_mut().push(Intent::EditorText {
            id,
            event,
            text: text.to_string(),
            rev,
        });
    }

    /// Stage an `on_tapped` intent.
    pub fn queue_tapped(&mut self, id: ControlId) {
        self.intents.borrow_mut().push(Intent::Tapped { id });
    }

    /// Run a registered gesture with one transition and stage the app
    /// notification if it asked for one — exactly what the input router's
    /// `deliver_gesture` does, and in the same order.
    ///
    /// Returns whether an [`Intent::Gesture`] was staged. That is the coalescing
    /// signal: a burst of moves must stage **one**, not one per move.
    pub fn deliver_gesture(&mut self, id: ControlId, event: GestureEvent) -> bool {
        let notify = dcomp::dispatch_gesture(id, event) == Some(GestureOutcome::Notify);
        if notify {
            self.intents.borrow_mut().push(Intent::Gesture { id });
        }
        notify
    }

    /// Stage a flyout-dismissed intent, as `close_popup` would queue it for a
    /// popup whose owner declared an `on_closed` callback.
    pub fn queue_flyout_closed(&mut self, id: ControlId) {
        self.intents
            .borrow_mut()
            .push(Intent::FlyoutClosed { id });
    }

    /// Stage an accelerator-fired intent, as the input router's
    /// `match_accelerator` → `fire_accelerator` would for a matched `(key,
    /// mods)` chord at position `index` in the node's declared list (§7.3).
    pub fn queue_accelerator(&mut self, id: ControlId, index: usize) {
        self.intents
            .borrow_mut()
            .push(Intent::Accelerator { id, index });
    }

    /// Drain the staged intents through the recorder's real app-side
    /// resolution and run the resulting handler jobs — the exact sequence the
    /// host performs after an input dispatch. Returns how many handlers ran.
    pub fn drain_and_run(&mut self) -> usize {
        let intents = self.spy.take_intents();
        let jobs = self.rec.resolve_intents(intents);
        let n = jobs.len();
        for job in jobs {
            job.run();
        }
        n
    }

}

/// A registered gesture, standing in for what
/// [`PointerSurface::on_gesture`](crate::PointerSurface::on_gesture) installs:
/// the `Send` handler the router runs inline, plus the app-side drain an
/// [`Intent::Gesture`] resolves against.
///
/// Registration goes through the shipping ops queue and is serviced
/// immediately, so a test sees the same map the router reads. Dropping this
/// forgets the gesture and unregisters the drain.
/// Serializes gesture harnesses across the test runner's threads.
///
/// Declarations cross app→front through a process-wide `OPS` queue, but
/// `service_ops` drains it into the **calling thread's** registry — correct in a
/// shipping build, where exactly one front thread ever services it. The test
/// runner threads its tests, so two harnesses on two threads would race for one
/// queue and one could swallow the other's declaration. Holding this for a
/// harness's lifetime keeps the queue single-consumer, as the design assumes.
static HARNESS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct GestureHarness {
    id: ControlId,
    _sub: Subscription,
    /// Held for the harness's lifetime — see [`HARNESS_LOCK`].
    _serialized: std::sync::MutexGuard<'static, ()>,
}

impl GestureHarness {
    /// Declare `gesture` for `id` with `interest`, and `on_action` as its
    /// app-side drain.
    pub fn register<G>(
        id: ControlId,
        interest: GestureInterest,
        gesture: G,
        on_action: impl Fn() + 'static,
    ) -> Self
    where
        G: FnMut(GestureEvent) -> GestureOutcome + Send + 'static,
    {
        let serialized = HARNESS_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        dcomp::declare_gesture(id, interest, Box::new(gesture));
        dcomp::service_gesture_ops();
        let sub = dcomp::register_gesture_action(id, Callback::new(move |()| on_action()));
        Self { id, _sub: sub, _serialized: serialized }
    }

    /// The routing bits the router would read for this gesture.
    pub fn interest(&self) -> Option<GestureInterest> {
        dcomp::gesture_interest_for(self.id)
    }
}

impl Drop for GestureHarness {
    fn drop(&mut self) {
        dcomp::forget_gesture(self.id);
        dcomp::service_gesture_ops();
    }
}

/// Compile-time proof, re-stated where a test crate can see it, that the
/// command buffer's payload types are `Send`. The lib asserts this too; this
/// copy fails the *test* build if the seam ever stops holding.
pub fn assert_cmd_buffer_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<dcomp::record::Cmd>();
    assert_send::<dcomp::record::SendValue>();
    assert_send::<Intent>();
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
    /// Monotonic id mint for [`Self::insert`] (the reconciler's role in the
    /// shipping pipeline).
    next_id: u32,
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
            next_id: 0,
        })
    }

    fn node(&self, kind: ControlKind) -> windows_core::Result<dcomp::node::Node> {
        Ok(dcomp::node::Node::new(
            kind,
            self.compositor.CreateContainerVisual()?,
        ))
    }

    /// Insert a node under a harness-minted id.
    ///
    /// The arena no longer mints ids at all — the reconciler is the single
    /// minter in the shipping pipeline — so the harness plays that role here,
    /// with the same contract: monotonic, and never reusing any id this arena
    /// has seen, caller-provided ones included (the watermark below).
    pub fn insert(&mut self, kind: ControlKind) -> windows_core::Result<ControlId> {
        self.next_id += 1;
        let id = ControlId::new(self.next_id);
        self.insert_with_id(id, kind)?;
        Ok(id)
    }

    /// `Arena::insert_with_id` — the caller minted the id. Advances the
    /// harness mint's watermark past it, as any correct single minter must.
    pub fn insert_with_id(&mut self, id: ControlId, kind: ControlKind) -> windows_core::Result<()> {
        let node = self.node(kind)?;
        self.arena.insert_with_id(id, node);
        self.next_id = self.next_id.max(id.get());
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

    /// Write the committed `Ctrl` value (`ctrl().value`), as a numeric commit
    /// would — the pre-edit value a §7.3 Escape-revert restores to the buffer.
    pub fn ctrl_set_value(&mut self, id: ControlId, v: f64) {
        if let Some(n) = self.arena.get_mut(id) {
            n.ctrl_mut().value = v;
        }
    }

    /// Replace a node's editor buffer text directly, simulating an in-progress
    /// (uncommitted) edit. A no-op for a node with no editor.
    pub fn set_editor_text(&mut self, id: ControlId, s: &str) {
        if let Some(n) = self.arena.get_mut(id)
            && let Some(e) = &mut n.editor
        {
            e.set_text(s);
            e.seeded = true;
        }
    }

    /// A node's editor buffer text, if it has an editor.
    pub fn editor_text(&self, id: ControlId) -> Option<String> {
        self.arena
            .get(id)
            .and_then(|n| n.editor.as_ref().map(|e| e.text()))
    }

    /// Run the shipping §7.3 NumberBox Escape-revert
    /// ([`dcomp::revert_number_text`]) against this node — restore the committed
    /// value into the buffer. Fires nothing; the caller inspects the buffer and
    /// the untouched `ctrl().value` through the other probes.
    pub fn number_escape_revert(&mut self, id: ControlId) {
        if let Some(n) = self.arena.get_mut(id) {
            dcomp::revert_number_text(n);
        }
    }

    /// Whether this node has allocated its `Extras` — the second lazily-boxed
    /// tier, holding the caption / nav-pane / flyout / editor-policy state.
    pub fn extras_allocated(&self, id: ControlId) -> Option<bool> {
        self.arena.get(id).map(|n| n.extras_allocated())
    }

    /// Bump the node's input value revision, as the backend's
    /// `fire_value_changed` does on every input-originated value write.
    /// Returns the new revision.
    pub fn bump_value_rev(&mut self, id: ControlId) -> u64 {
        match self.arena.get_mut(id) {
            Some(n) => {
                n.value_rev += 1;
                n.value_rev
            }
            None => 0,
        }
    }

    /// The §7.2 echo gate for control values — the shipping
    /// `Node::accepts_value_echo` predicate `set_value_stamped` applies.
    pub fn accepts_value_echo(&self, id: ControlId, based_on: u64) -> Option<bool> {
        self.arena.get(id).map(|n| n.accepts_value_echo(based_on))
    }

    /// Bump the editor's §7.2 text revision, as the backend's
    /// `editor_after_edit` does on every user-originated buffer edit.
    /// Returns the new revision (0 for a node with no editor).
    pub fn bump_text_rev(&mut self, id: ControlId) -> u64 {
        match self.arena.get_mut(id).and_then(|n| n.editor.as_mut()) {
            Some(e) => {
                e.text_rev += 1;
                e.text_rev
            }
            None => 0,
        }
    }

    /// The editor's `(anchor, caret)` selection endpoints.
    pub fn editor_caret(&self, id: ControlId) -> Option<(usize, usize)> {
        self.arena
            .get(id)
            .and_then(|n| n.editor.as_ref().map(|e| (e.anchor, e.caret)))
    }

    /// Place the editor caret (collapsed selection) at a code-unit index.
    pub fn set_editor_caret(&mut self, id: ControlId, caret: usize) {
        if let Some(e) = self.arena.get_mut(id).and_then(|n| n.editor.as_mut()) {
            e.caret = caret;
            e.anchor = caret;
        }
    }

    /// Toggle an active IME composition on the editor (the §7.2 composition
    /// guard's signal), as the IMM32/TSF path does around a composition.
    pub fn set_composition_active(&mut self, id: ControlId, active: bool) {
        if let Some(e) = self.arena.get_mut(id).and_then(|n| n.editor.as_mut()) {
            e.comp_start = 0;
            e.comp_len = if active { e.buf.len().max(1) } else { 0 };
        }
    }

    /// `dcomp::apply_text_stamped` — the REAL §7.2 arrival rules a replayed
    /// [`Cmd::SetText`] goes through (composition guard → echo-identical
    /// no-op → stale-revision drop → caret-mapped apply).
    pub fn apply_text_stamped(&mut self, id: ControlId, text: &str, based_on: u64) {
        if let Some(n) = self.arena.get_mut(id) {
            dcomp::apply_text_stamped(n, text, based_on);
        }
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

/// Where a button's content landed — the three boxes
/// [`dcomp::controls::button_boxes`] resolves, each as
/// `(left, top, right, bottom)` in node-local DIPs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonBoxProbe {
    pub icon: Option<(f32, f32, f32, f32)>,
    pub badge: Option<(f32, f32, f32, f32)>,
    pub label: (f32, f32, f32, f32),
}

impl ArenaHarness {
    /// Shape this node's cached runs, through the real
    /// [`dcomp::layout::rebuild_text`] the layout pass calls.
    ///
    /// Geometry that reads a MEASURED run — which is all of a button's content
    /// row — is testing a degenerate empty-label case until this has run, and
    /// would pass while asserting nothing.
    pub fn rebuild_text(&mut self, id: ControlId) {
        dcomp::layout::rebuild_text(&mut self.arena, id);
    }

    /// Resolve a button's content geometry through the real
    /// [`dcomp::controls::button_boxes`] the placement, the badge plate and the
    /// measure all call.
    pub fn button_boxes(&self, id: ControlId) -> Option<ButtonBoxProbe> {
        let n = self.arena.get(id)?;
        let r = windows_canvas_core::Rect::from_xywh(0.0, 0.0, n.rect.w, n.rect.h);
        let b = dcomp::controls::button_boxes(n, r);
        let t = |r: windows_canvas_core::Rect| (r.left, r.top, r.right, r.bottom);
        Some(ButtonBoxProbe {
            icon: b.icon.map(t),
            badge: b.badge.map(t),
            label: t(b.label),
        })
    }

    /// The badge's intrinsic size, or `None` when the button carries none.
    pub fn badge_size(&self, id: ControlId) -> Option<(f32, f32)> {
        dcomp::controls::badge_size(self.arena.get(id)?)
    }

    /// Whether this node would be given a paint surface — the test-visible form
    /// of "does it ever reach a `BeginDraw`".
    pub fn has_chrome(&self, id: ControlId) -> Option<bool> {
        Some(self.arena.get(id)?.has_chrome())
    }

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

// ── backend/dcomp/tsf ────────────────────────────────────────────────────────

/// The raw TSF text store's binding-independent core, re-published for the
/// headless protocol tests (the lock state machine and the ACP text/selection
/// ops). TSF itself cannot run headless; this surface is exactly the part that
/// can, and the part that historically breaks.
pub mod tsf {
    use crate::backend::dcomp::tsf;

    // Data the document trait exchanges.
    pub use tsf::store::{
        get_selection, get_text, insert_at_selection, notify_app_selection_change,
        notify_app_text_change, run_request_lock, set_selection, set_text, AcpError, LockResult,
        StoreSink, TextChange, TextStoreCore, TsfDocument,
    };
    pub use tsf::{DocRect, DocSelection};

    // Lock / insert flag words, so a test names the same constants the shipping
    // code does.
    pub use tsf::{insert, lock};
}
