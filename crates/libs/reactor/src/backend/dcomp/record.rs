//! Command-buffer seam between the reconciler and the DComp backend.
//!
//! [`RecordingBackend`] is the **app half** of that seam: it implements
//! [`Backend`] by encoding every call as a plain-data [`Cmd`] instead of
//! mutating the composition tree directly, and it owns every app closure input
//! can ever invoke. The **front half** is the concrete [`FrontBackend`] (the
//! real `DCompBackend`, or a test spy) into which the host [`replay`]s the
//! taken buffer. Today replay is synchronous and on the same thread, so
//! behaviour is identical to calling through; the point of the seam is that
//! the buffer is **`Send`** and the recorder holds no backend reference at
//! all, which is what lets the reconciler eventually run on a thread that is
//! not the one owning the HWND, the message pump and the compositor.
//!
//! Two properties are load-bearing and are enforced by the compiler rather than
//! by convention:
//!
//! * **[`Cmd`] and [`Intent`] are `Send`.** Anything thread-affine — `Rc<dyn Fn>`
//!   handlers, COM image sources, `Element` trees — never enters either buffer:
//!   closures live in this recorder's app-side maps and cross as declarations
//!   (presence bits, key lists) the front consults without ever holding them.
//!   The assertion at the bottom of this module fails the build if a `!Send`
//!   payload ever leaks in.
//! * **[`SendValue::from_prop`] matches [`PropValue`] exhaustively.** A new
//!   `PropValue` variant upstream breaks this build instead of being silently
//!   dropped from the wire, forcing a deliberate send-or-declare decision.
//!
//! The reconciler mints control ids, so `create` takes one rather than
//! returning one and the trait has no call that must be answered synchronously.
//! That is what makes the buffer a complete encoding: every method is a pure
//! command, and replaying the stream reproduces the tree exactly.
//!
//! **Events flow the other way as intents.** App event callbacks are never
//! replayed into the backend: [`Cmd::AttachEvent`] is a pure `{id, event}`
//! declaration and the [`EventHandler`] stays in this recorder's app-side
//! handler map. When input needs app logic, the backend queues a typed, plain-
//! data [`Intent`] carrying everything the app needs; the host drains the queue
//! through [`RecordingBackend::drain_intents`] after each input dispatch and
//! invokes the mapped handlers outside the backend borrow. Today the drain runs
//! on the same thread within the same message, so behaviour matches the old
//! synchronous fire — the queue is the seam that lets the two halves later live
//! on different threads.
//!
//! **Viz pointer surfaces ride the same queue.** A knob/slider/EQ surface's
//! sinks (`on_down`/`on_move`/`on_up`/`on_wheel`/`on_exit`) are the one place
//! immediate feedback used to run an app closure inline in the input router.
//! Now the router routes on plain presence bits (`pointer::SurfaceInterest`) and
//! queues an [`Intent::Surface`]/[`Intent::SurfaceExit`]; the drain resolves it
//! against the app-side sink closures (`pointer::sinks_for`) — kept out of the
//! recorder's own maps only because the surface is registered imperatively from
//! an effect, not through the [`Backend`] trait, but owned by the same app half.
//! The drag path's synchronous `drive_frame_ticks()` moves to the host: after
//! running the jobs, a surface job that ran drives one tick so the drag preview
//! repaints in the same message ([`IntentJob::drives_frame_tick`]).

use std::collections::HashMap;
use std::rc::Rc;

use rustc_hash::FxHashMap;

use super::node::PointerInterest;
use crate::backend::{
    AccessibilityModifiers, AnimationConfig, Backend, Color, CommandBarCommandDef, ControlId,
    ControlKind, Event, EventHandler, GridLength, ImplicitTransitions, KeyboardAccelerator,
    LayoutAnimationConfig, LineEndpoints, MenuBarItemDef, MenuItemDef, NavViewItem,
    PointerHandlers, Prop, PropValue, RichTextParagraph, SelectionMode, SelectorBarItemDef,
    Thickness, ThemeRef, Tooltip, TreeNodeDef,
};
use crate::drag::DragHandlers;
use crate::interaction::Callback;
use crate::style::{PointerEventInfo, TooltipContent, TooltipPlacement};

/// The `Send` subset of [`PropValue`].
///
/// Three `PropValue` variants hold thread-affine payloads — `SurfaceImageSource`
/// and `VirtualSurfaceImageSource` wrap COM interfaces, `FlyoutDef` holds a
/// `Box<Element>` and a callback — so they cannot ride the buffer. All three are
/// inert on this backend (no `Prop::ImageSource` arm exists in
/// `DCompBackend::set_prop`; flyouts are a WinUI feature), so `set_prop` drops
/// them at record time — see the note there.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum SendValue {
    Str(String),
    F64(f64),
    U16(u16),
    Bool(bool),
    I32(i32),
    Thickness(Thickness),
    Color(Color),
    Unset,
    GridLengths(Vec<GridLength>),
    LineEndpoints(LineEndpoints),
    NavMenuItems(Vec<NavViewItem>),
    StrList(Vec<String>),
    MenuBarItems(Vec<MenuBarItemDef>),
    MenuFlyoutItems(Vec<MenuItemDef>),
    TreeViewNodes(Vec<TreeNodeDef>),
    CommandBarCommands(Vec<CommandBarCommandDef>),
    CommandBarFlyoutDef {
        primary: Vec<CommandBarCommandDef>,
        secondary: Vec<CommandBarCommandDef>,
    },
    SelectorBarItems(Vec<SelectorBarItemDef>),
    Resources(HashMap<String, String>),
    GradientStops(Vec<(f64, Color)>),
    F64List(Vec<f64>),
    ValueLabels(Vec<(f64, String)>),
}

impl SendValue {
    /// Encode a [`PropValue`] for the buffer, or `None` if it holds a
    /// thread-affine payload and must go to the side table instead.
    ///
    /// This match is deliberately exhaustive — do **not** add a `_` arm. A new
    /// `PropValue` variant upstream must break this build so it gets a
    /// deliberate send-or-side-table decision rather than being silently
    /// dropped from the wire.
    fn from_prop(value: &PropValue) -> Option<Self> {
        Some(match value {
            PropValue::Str(v) => Self::Str(v.clone()),
            PropValue::F64(v) => Self::F64(*v),
            PropValue::U16(v) => Self::U16(*v),
            PropValue::Bool(v) => Self::Bool(*v),
            PropValue::I32(v) => Self::I32(*v),
            PropValue::Thickness(v) => Self::Thickness(*v),
            PropValue::Color(v) => Self::Color(*v),
            PropValue::Unset => Self::Unset,
            PropValue::GridLengths(v) => Self::GridLengths(v.clone()),
            PropValue::LineEndpoints(v) => Self::LineEndpoints(*v),
            PropValue::NavMenuItems(v) => Self::NavMenuItems(v.clone()),
            PropValue::StrList(v) => Self::StrList(v.clone()),
            PropValue::MenuBarItems(v) => Self::MenuBarItems(v.clone()),
            PropValue::MenuFlyoutItems(v) => Self::MenuFlyoutItems(v.clone()),
            PropValue::TreeViewNodes(v) => Self::TreeViewNodes(v.clone()),
            PropValue::CommandBarCommands(v) => Self::CommandBarCommands(v.clone()),
            PropValue::CommandBarFlyoutDef { primary, secondary } => Self::CommandBarFlyoutDef {
                primary: primary.clone(),
                secondary: secondary.clone(),
            },
            PropValue::SelectorBarItems(v) => Self::SelectorBarItems(v.clone()),
            PropValue::Resources(v) => Self::Resources(v.clone()),
            PropValue::GradientStops(v) => Self::GradientStops(v.clone()),
            PropValue::F64List(v) => Self::F64List(v.clone()),
            PropValue::ValueLabels(v) => Self::ValueLabels(v.clone()),
            // Thread-affine: COM interfaces and an Element/callback tree.
            PropValue::SurfaceImageSource(_)
            | PropValue::VirtualSurfaceImageSource(_)
            | PropValue::FlyoutDef(_) => return None,
        })
    }

    /// Rebuild the [`PropValue`] for replay. Consumes `self` so the round trip
    /// through the buffer costs exactly one clone (taken at record time).
    fn into_prop(self) -> PropValue {
        match self {
            Self::Str(v) => PropValue::Str(v),
            Self::F64(v) => PropValue::F64(v),
            Self::U16(v) => PropValue::U16(v),
            Self::Bool(v) => PropValue::Bool(v),
            Self::I32(v) => PropValue::I32(v),
            Self::Thickness(v) => PropValue::Thickness(v),
            Self::Color(v) => PropValue::Color(v),
            Self::Unset => PropValue::Unset,
            Self::GridLengths(v) => PropValue::GridLengths(v),
            Self::LineEndpoints(v) => PropValue::LineEndpoints(v),
            Self::NavMenuItems(v) => PropValue::NavMenuItems(v),
            Self::StrList(v) => PropValue::StrList(v),
            Self::MenuBarItems(v) => PropValue::MenuBarItems(v),
            Self::MenuFlyoutItems(v) => PropValue::MenuFlyoutItems(v),
            Self::TreeViewNodes(v) => PropValue::TreeViewNodes(v),
            Self::CommandBarCommands(v) => PropValue::CommandBarCommands(v),
            Self::CommandBarFlyoutDef { primary, secondary } => {
                PropValue::CommandBarFlyoutDef { primary, secondary }
            }
            Self::SelectorBarItems(v) => PropValue::SelectorBarItems(v),
            Self::Resources(v) => PropValue::Resources(v),
            Self::GradientStops(v) => PropValue::GradientStops(v),
            Self::F64List(v) => PropValue::F64List(v),
            Self::ValueLabels(v) => PropValue::ValueLabels(v),
        }
    }
}

/// Which drag callbacks the app declared for a node — the drag analogue of
/// [`PointerInterest`]: the `DragHandlers` closures stay in the recorder's
/// app-side map, and these bits are all the front will ever consult when the
/// backend grows an OLE `IDropTarget` (today it has none, so replay records
/// nothing — but the declaration shape is settled now, so the drop-target work
/// starts from bits + intents instead of re-deriving this split).
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub(crate) struct DragInterest {
    pub enter: bool,
    pub leave: bool,
    pub over: bool,
    pub drop: bool,
}

impl DragInterest {
    fn of(h: &DragHandlers) -> Self {
        Self {
            enter: h.on_drag_enter.is_some(),
            leave: h.on_drag_leave.is_some(),
            over: h.on_drag_over.is_some(),
            drop: h.on_drag_drop.is_some(),
        }
    }
}

/// The `Send` face of a [`Tooltip`]: what the front needs to *show* one. A
/// rich tooltip's `Element` tree cannot cross (and could only be realized
/// through the reconciler anyway), so it declares its presence and nothing
/// more; the full `Tooltip` stays in the recorder's app-side map for whichever
/// future path realizes it.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum TooltipDecl {
    Text {
        text: String,
        placement: Option<TooltipPlacement>,
    },
    Rich,
}

impl TooltipDecl {
    fn of(t: &Tooltip) -> Self {
        match &t.content {
            TooltipContent::Text(s) => Self::Text {
                text: s.clone(),
                placement: t.placement,
            },
            TooltipContent::Rich(_) => Self::Rich,
        }
    }
}

/// The typed payload of an [`Intent::Event`], mirroring the [`EventHandler`]
/// variants this backend can address. The backend no longer holds the handler
/// at fire time, so the payload type travels with the intent and is checked
/// against the mapped handler at drain — a mismatch is dropped with a
/// diagnostic instead of reaching the panicking `invoke_*` accessors.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum IntentPayload {
    Unit,
    Bool(bool),
    Str(String),
    F64(f64),
    I32(i32),
}

/// Which positional pointer callback an [`Intent::Pointer`] addresses.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum PointerIntentKind {
    Pressed,
    Released,
    Moved,
}

/// Which viz-surface sink an [`Intent::Surface`] addresses (the drag/scrub/wheel
/// transitions; hover-exit is [`Intent::SurfaceExit`]).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum SurfaceIntentKind {
    Down,
    Move,
    Up,
    Wheel,
}

/// One queued app notification: everything the app needs to run the matching
/// handler, as plain `Send` data — no fire site's closure reads backend state,
/// so the payload is complete by construction.
///
/// Queue order is invocation order. That FIFO property carries a documented
/// contract: a node with both a `Click` handler and `on_tapped` queues them
/// from one activation in that order, so the app observes them exactly as the
/// old synchronous fire delivered them.
#[derive(Debug)]
pub(crate) enum Intent {
    /// A typed control event, addressed to the app-side `(id, event)` map.
    Event {
        id: ControlId,
        event: Event,
        payload: IntentPayload,
    },
    /// `Event::ValueChanged`, carrying the node's input revision. The drain
    /// records `rev` as the latest the app has been consulted about, and the
    /// app's echo of the value comes back stamped against it
    /// ([`Cmd::SetValue`]) so a stale echo can be dropped.
    ValueChanged {
        id: ControlId,
        value: f64,
        rev: u64,
    },
    /// A positional pointer callback (`on_pointer_pressed/released/moved`).
    Pointer {
        id: ControlId,
        kind: PointerIntentKind,
        info: PointerEventInfo,
    },
    /// `on_tapped` — rides the same queue as `Event::Click`, preserving the
    /// Click-then-tapped order within one activation.
    Tapped { id: ControlId },
    /// `on_right_tapped`.
    RightTapped { id: ControlId },
    /// A viz pointer-surface transition (knob/slider/EQ drag or wheel),
    /// addressed to the app-side sink map (`pointer::sinks_for`). Rides the same
    /// FIFO queue as everything else, so a gesture's down → moves → up reach the
    /// app in that order.
    Surface {
        id: ControlId,
        kind: SurfaceIntentKind,
        info: PointerEventInfo,
    },
    /// A viz surface's hover-exit sink (`on_exit`): the hover left this surface
    /// for another, for none, or the window edge. Queued where the old
    /// synchronous `fire_surface_exit` fired, so its order relative to the next
    /// surface's `Move` is preserved.
    SurfaceExit { id: ControlId },
    /// A keyboard accelerator fired: a declared `(key, mods)` chord matched on
    /// keydown (§7.3). `index` is the position in the node's declared
    /// accelerator list — the same order [`Cmd::SetKeybindings`] carried — so
    /// the drain addresses the right `on_invoked` in the app-side `accels` map
    /// without the front ever holding the callback.
    Accelerator { id: ControlId, index: usize },
}

/// A handler invocation resolved from an [`Intent`] at drain time: the cloned
/// app callback plus its payload. Held by the host across the end of the
/// reconciler borrow and [`run`](Self::run) with the borrow released, so a
/// handler that re-enters the pump (or the backend) finds nothing held.
pub(crate) enum IntentJob {
    Unit(Callback<()>),
    Bool(Callback<bool>, bool),
    Str(Callback<String>, String),
    F64(Callback<f64>, f64),
    I32(Callback<i32>, i32),
    Pointer(Callback<PointerEventInfo>, PointerEventInfo),
    /// A viz surface sink (`on_down`/`on_move`/`on_up`/`on_wheel`), cloned out of
    /// the app-side [`PointerSinks`](super::PointerSinks) cell.
    Surface(Rc<dyn Fn(PointerEventInfo)>, PointerEventInfo),
    /// A viz surface's hover-exit sink (`on_exit`).
    SurfaceExit(Rc<dyn Fn()>),
}

impl IntentJob {
    /// Invoke the handler. Type agreement was already checked at drain.
    pub(crate) fn run(self) {
        match self {
            Self::Unit(cb) => cb.invoke(()),
            Self::Bool(cb, v) => cb.invoke(v),
            Self::Str(cb, v) => cb.invoke(v),
            Self::F64(cb, v) => cb.invoke(v),
            Self::I32(cb, v) => cb.invoke(v),
            Self::Pointer(cb, info) => cb.invoke(info),
            Self::Surface(cb, info) => cb(info),
            Self::SurfaceExit(cb) => cb(),
        }
    }

    /// Whether running this job should drive a frame tick promptly rather than
    /// waiting for the next paced `WM_APP_FRAME`. True for a surface drag/scrub
    /// sink: that is the EQ/knob drag path, whose preview must repaint from the
    /// value the sink just streamed within this same input message (the tightest
    /// latency coupling in the backend). Hover-exit alone advances no preview.
    pub(crate) fn drives_frame_tick(&self) -> bool {
        matches!(self, Self::Surface(..))
    }
}

/// Resolve one typed event intent against its mapped handler, or `None` (with
/// a debug diagnostic) when the payload type does not match the handler
/// variant. This is the checked replacement for the panicking `invoke_*`
/// accessors: the backend no longer holds the variant to validate against at
/// fire time, so a widget-glue mismatch must surface here as a dropped intent,
/// never as a production panic.
fn event_job(handler: &EventHandler, payload: IntentPayload) -> Option<IntentJob> {
    match (handler, payload) {
        (EventHandler::Unit(cb), IntentPayload::Unit) => Some(IntentJob::Unit(cb.clone())),
        (EventHandler::Bool(cb), IntentPayload::Bool(v)) => Some(IntentJob::Bool(cb.clone(), v)),
        (EventHandler::Str(cb), IntentPayload::Str(v)) => Some(IntentJob::Str(cb.clone(), v)),
        (EventHandler::F64(cb), IntentPayload::F64(v)) => Some(IntentJob::F64(cb.clone(), v)),
        (EventHandler::I32(cb), IntentPayload::I32(v)) => Some(IntentJob::I32(cb.clone(), v)),
        (handler, payload) => {
            debug_assert!(
                false,
                "intent payload {payload:?} does not match handler {handler:?} — dropped"
            );
            None
        }
    }
}

/// One recorded [`Backend`] call, as plain data.
///
/// One variant per trait method. Order in the buffer is the order the
/// reconciler issued the calls and must be preserved exactly: the child ops are
/// positional against the backend's live child vector, so reordering or
/// coalescing corrupts the tree.
#[derive(Debug)]
pub(crate) enum Cmd {
    Create {
        id: ControlId,
        kind: ControlKind,
    },
    SetProp {
        id: ControlId,
        prop: Prop,
        value: SendValue,
    },
    AppendChild {
        parent: ControlId,
        child: ControlId,
    },
    RemoveChild {
        parent: ControlId,
        index: usize,
    },
    ReplaceChild {
        parent: ControlId,
        index: usize,
        new: ControlId,
    },
    MoveChild {
        parent: ControlId,
        from: usize,
        to: usize,
    },
    InsertChild {
        parent: ControlId,
        index: usize,
        child: ControlId,
    },
    Destroy {
        id: ControlId,
    },
    /// A pure declaration — the [`EventHandler`] itself never rides the
    /// buffer; it stays in the recorder's app-side handler map and is invoked
    /// from queued [`Intent`]s. Replay only tells the backend the event exists
    /// (interactivity flags).
    AttachEvent {
        id: ControlId,
        event: Event,
    },
    DetachEvent {
        id: ControlId,
        event: Event,
    },
    /// `set_prop(Prop::Value, F64)`, stamped with the input revision the app
    /// had been consulted about when the write was recorded. Replay drops the
    /// write when the node's revision has moved past `based_on` — the app is
    /// echoing a value the gesture already superseded (§7.2 applied to control
    /// values; the gesture-time half lives in `apply_prop`'s pressed gate).
    SetValue {
        id: ControlId,
        value: f64,
        based_on: u64,
    },
    SetTemplatedItemCount {
        id: ControlId,
        count: usize,
    },
    SetTemplatedRowContent {
        list_id: ControlId,
        row_idx: usize,
        content: Option<ControlId>,
    },
    SetTemplatedSelectedIndex {
        id: ControlId,
        index: i32,
    },
    SetTemplatedSelectionMode {
        id: ControlId,
        mode: SelectionMode,
    },
    SetTemplatedCanDragItems {
        id: ControlId,
        value: bool,
    },
    SetTemplatedCanReorderItems {
        id: ControlId,
        value: bool,
    },
    SetTemplatedAllowDrop {
        id: ControlId,
        value: bool,
    },
    SetHeaderElement {
        id: ControlId,
        header_id: Option<ControlId>,
    },
    SetPaneElement {
        id: ControlId,
        pane_id: Option<ControlId>,
    },
    ScrollTemplatedToIndex {
        id: ControlId,
        index: i32,
    },
    /// Declarations only — the `Callback`/`Rc<dyn Fn>` payloads stay in the
    /// recorder's app-side maps, exactly like event handlers. The backend does
    /// not consume templated lists today; when it does, these become presence
    /// bits it routes on and the closures fire as intents.
    AttachTemplatedSelectionChanged {
        id: ControlId,
    },
    AttachTemplatedRealization {
        id: ControlId,
    },
    AttachTemplatedReorder {
        id: ControlId,
    },
    SetThemeBindings {
        id: ControlId,
        kind: ControlKind,
        bindings: Vec<(Prop, ThemeRef)>,
    },
    OnThemeChanged,
    SetImplicitTransitions {
        id: ControlId,
        transitions: Option<ImplicitTransitions>,
    },
    SetLayoutAnimation {
        id: ControlId,
        config: Option<LayoutAnimationConfig>,
    },
    RunPropertyAnimation {
        id: ControlId,
        config: Option<AnimationConfig>,
    },
    SetExitTransition {
        id: ControlId,
        config: Option<AnimationConfig>,
    },
    SetRichTextParagraphs {
        id: ControlId,
        paragraphs: Vec<RichTextParagraph>,
    },
    SetAccessibility {
        id: ControlId,
        accessibility: AccessibilityModifiers,
    },
    /// The plain `(key, modifiers)` list of the app's accelerators; the
    /// `on_invoked` callbacks stay in the recorder's app-side map. Matching a
    /// chord is front-side work against exactly this list (§7.3's
    /// `SetKeybindings`); firing is an intent resolved against the map.
    SetKeybindings {
        id: ControlId,
        keys: Vec<(crate::VirtualKey, crate::VirtualKeyModifiers)>,
    },
    SetTooltip {
        id: ControlId,
        decl: Option<TooltipDecl>,
    },
    /// The presence bits of the app's pointer callbacks — the closures stay in
    /// the recorder's app-side map, invoked from [`Intent::Pointer`]/
    /// [`Intent::Tapped`]; the backend keeps only what input consults
    /// synchronously (hit-testability, pointer capture).
    SetPointerInterest {
        id: ControlId,
        interest: PointerInterest,
    },
    /// The presence bits of the app's drag callbacks — see [`DragInterest`].
    SetDragInterest {
        id: ControlId,
        interest: DragInterest,
    },
}

/// What replay needs from the wrapped (front-side) backend beyond the
/// [`Backend`] trait: the declaration halves of the event/pointer/value
/// protocols whose closure halves stay in this recorder, and the intent queue
/// input fills for the recorder to drain. Implemented by
/// [`DCompBackend`](super::DCompBackend); a test spy can implement it to
/// observe replay headlessly.
pub(crate) trait FrontBackend: Backend {
    /// [`Cmd::AttachEvent`]: note that `event` has a handler (app-side).
    fn declare_event(&mut self, id: ControlId, event: Event);
    /// [`Cmd::SetPointerInterest`]: note which pointer callbacks exist.
    fn set_pointer_interest(&mut self, id: ControlId, interest: PointerInterest);
    /// [`Cmd::SetValue`]: the revision-gated `Prop::Value` write.
    fn set_value_stamped(&mut self, id: ControlId, value: f64, based_on: u64);
    /// [`Cmd::SetKeybindings`]: the `(key, mods)` chords the front matches
    /// against on keydown (§7.3). The `on_invoked` callbacks stay app-side in
    /// the recorder's `accels` map, addressed by the matched index.
    fn set_keybindings(
        &mut self,
        id: ControlId,
        keys: Vec<(crate::VirtualKey, crate::VirtualKeyModifiers)>,
    );
    /// Intents queued by input since the last drain, in fire order.
    fn take_intents(&mut self) -> Vec<Intent>;
}

/// The app half of the seam: records [`Backend`] calls into a `Send` buffer
/// the host [`take_cmds`](Self::take_cmds) and [`replay`]s into the front
/// backend, and keeps every input-invoked closure — the `(id, event)` handler
/// map, the per-node pointer callbacks, and the declared-but-dormant payloads
/// (accelerators, drag, tooltips, templated-list callbacks). It holds **no**
/// backend reference: the front half is reached only through the buffer going
/// one way and [`Intent`]s coming back, which is the whole point — the two
/// halves share nothing but `Send` data, so they can live on different
/// threads.
pub(crate) struct RecordingBackend {
    cmds: Vec<Cmd>,
    /// App-side event handlers, keyed by control. The inner `Vec` mirrors the
    /// shape the backend node used to hold — a handful of events per control.
    handlers: FxHashMap<ControlId, Vec<(Event, EventHandler)>>,
    /// App-side pointer callbacks (`on_tapped` / `on_pointer_*`), whole.
    pointer: FxHashMap<ControlId, PointerHandlers>,
    /// App-side accelerator callbacks; the front sees only the `(key, mods)`
    /// list ([`Cmd::SetKeybindings`]).
    accels: FxHashMap<ControlId, Vec<KeyboardAccelerator>>,
    /// App-side drag callbacks; the front sees only [`DragInterest`].
    drag: FxHashMap<ControlId, DragHandlers>,
    /// Full tooltips (a rich one holds an `Element` tree); the front sees only
    /// [`TooltipDecl`].
    tooltips: FxHashMap<ControlId, Tooltip>,
    /// Templated-list callbacks — realize/recycle, selection, reorder. Declared
    /// to the front by presence only; nothing consumes them on this backend
    /// yet, but they are held here so the templated work starts from the same
    /// closures-stay-app-side split as everything else.
    realization: FxHashMap<ControlId, (Rc<dyn Fn(usize)>, Rc<dyn Fn(usize)>)>,
    selection_changed: FxHashMap<ControlId, Callback<i32>>,
    reorder: FxHashMap<ControlId, Callback<Vec<usize>>>,
    /// Per control, the latest [`Intent::ValueChanged`] revision the drain has
    /// handed the app — what a subsequent `Prop::Value` echo is stamped
    /// `based_on`. Advanced at drain whether or not a handler is mapped: the
    /// drain marks "the app has run past this input", which is what keeps
    /// purely app-driven value writes (a meter, a follower) applying even
    /// while nobody listens for `ValueChanged`.
    delivered_value_rev: FxHashMap<ControlId, u64>,
}

impl RecordingBackend {
    pub(crate) fn new() -> Self {
        Self {
            cmds: Vec::new(),
            handlers: FxHashMap::default(),
            pointer: FxHashMap::default(),
            accels: FxHashMap::default(),
            drag: FxHashMap::default(),
            tooltips: FxHashMap::default(),
            realization: FxHashMap::default(),
            selection_changed: FxHashMap::default(),
            reorder: FxHashMap::default(),
            delivered_value_rev: FxHashMap::default(),
        }
    }

    /// Resolve the front backend's queued intents into ready-to-run handler
    /// jobs, in fire order. The host takes the intents from the front half and
    /// brings them here (the only place that knows what the app subscribed
    /// to); the returned jobs must be [`run`](IntentJob::run) **after** every
    /// borrow is released, which is what lets a handler pump messages or
    /// re-enter the backend without deadlocking.
    ///
    /// An intent whose control has no mapped handler resolves to nothing —
    /// firing is unconditional backend-side precisely because only this map
    /// knows what the app subscribed to.
    pub(crate) fn resolve_intents(&mut self, intents: Vec<Intent>) -> Vec<IntentJob> {
        if intents.is_empty() {
            return Vec::new();
        }
        let mut jobs = Vec::with_capacity(intents.len());
        for intent in intents {
            match intent {
                Intent::Event { id, event, payload } => {
                    if let Some(h) = self.handler(id, event)
                        && let Some(job) = event_job(h, payload)
                    {
                        jobs.push(job);
                    }
                }
                Intent::ValueChanged { id, value, rev } => {
                    self.delivered_value_rev.insert(id, rev);
                    if let Some(h) = self.handler(id, Event::ValueChanged)
                        && let Some(job) = event_job(h, IntentPayload::F64(value))
                    {
                        jobs.push(job);
                    }
                }
                Intent::Pointer { id, kind, info } => {
                    let cb = self.pointer.get(&id).and_then(|p| match kind {
                        PointerIntentKind::Pressed => p.on_pointer_pressed.as_ref(),
                        PointerIntentKind::Released => p.on_pointer_released.as_ref(),
                        PointerIntentKind::Moved => p.on_pointer_moved.as_ref(),
                    });
                    if let Some(cb) = cb {
                        jobs.push(IntentJob::Pointer(cb.clone(), info));
                    }
                }
                Intent::Tapped { id } => {
                    if let Some(cb) = self.pointer.get(&id).and_then(|p| p.on_tapped.as_ref()) {
                        jobs.push(IntentJob::Unit(cb.clone()));
                    }
                }
                Intent::RightTapped { id } => {
                    if let Some(cb) =
                        self.pointer.get(&id).and_then(|p| p.on_right_tapped.as_ref())
                    {
                        jobs.push(IntentJob::Unit(cb.clone()));
                    }
                }
                Intent::Surface { id, kind, info } => {
                    if let Some(sinks) = super::pointer::sinks_for(id) {
                        let cell = match kind {
                            SurfaceIntentKind::Down => &sinks.down,
                            SurfaceIntentKind::Move => &sinks.moved,
                            SurfaceIntentKind::Up => &sinks.up,
                            SurfaceIntentKind::Wheel => &sinks.wheel,
                        };
                        if let Some(cb) = cell.borrow().as_ref() {
                            jobs.push(IntentJob::Surface(cb.clone(), info));
                        }
                    }
                }
                Intent::SurfaceExit { id } => {
                    if let Some(sinks) = super::pointer::sinks_for(id)
                        && let Some(cb) = sinks.exited.borrow().as_ref()
                    {
                        jobs.push(IntentJob::SurfaceExit(cb.clone()));
                    }
                }
                Intent::Accelerator { id, index } => {
                    // The front matched the chord against the `(key, mods)`
                    // list and named its position; this is the only map that
                    // holds the `on_invoked` callback, so resolution is a plain
                    // index into the declared list. A destroyed node dropped its
                    // entry (see `destroy`), so a stale accelerator resolves to
                    // nothing.
                    if let Some(accel) = self.accels.get(&id).and_then(|list| list.get(index)) {
                        jobs.push(IntentJob::Unit(accel.on_invoked.clone()));
                    }
                }
            }
        }
        jobs
    }

    fn handler(&self, id: ControlId, event: Event) -> Option<&EventHandler> {
        self.handlers
            .get(&id)?
            .iter()
            .find(|(e, _)| *e == event)
            .map(|(_, h)| h)
    }

    fn push(&mut self, cmd: Cmd) {
        self.cmds.push(cmd);
    }

    /// Take the buffered commands for the host to [`replay`] into the front
    /// backend, leaving the buffer empty. The take is atomic — a command can
    /// never be observed both here and in a replay — so "everything recorded
    /// has been replayed" holds by construction after the host applies the
    /// returned batch.
    pub(crate) fn take_cmds(&mut self) -> Vec<Cmd> {
        std::mem::take(&mut self.cmds)
    }

    /// Number of commands currently buffered (test observability — the
    /// headless harness is this count's only consumer).
    #[cfg(feature = "test")]
    pub(crate) fn pending(&self) -> usize {
        self.cmds.len()
    }
}

/// Replay a taken command batch into the front backend, in issue order.
///
/// Commands are applied literally — never reordered, coalesced or repaired.
/// The reconciler is the authority on tree shape (it updates its own
/// `children_mirror` *before* issuing each call), so the backend is a
/// follower and any "fix-up" here would desynchronise the two. In
/// particular the buffer legitimately contains transient states the
/// reconciler itself produces — a child destroyed before it is unparented,
/// and the reverse order on the tab/pivot paths — so replay must not
/// validate intermediate consistency.
pub(crate) fn replay<B: FrontBackend>(backend: &mut B, cmds: Vec<Cmd>) {
    for cmd in cmds {
        apply(backend, cmd);
    }
}

/// Apply one recorded command to the front backend.
fn apply<B: FrontBackend>(backend: &mut B, cmd: Cmd) {
    match cmd {
        Cmd::Create { id, kind } => backend.create(id, kind),
        Cmd::SetProp { id, prop, value } => backend.set_prop(id, prop, &value.into_prop()),
        Cmd::AppendChild { parent, child } => backend.append_child(parent, child),
        Cmd::RemoveChild { parent, index } => backend.remove_child(parent, index),
        Cmd::ReplaceChild { parent, index, new } => backend.replace_child(parent, index, new),
        Cmd::MoveChild { parent, from, to } => backend.move_child(parent, from, to),
        Cmd::InsertChild {
            parent,
            index,
            child,
        } => backend.insert_child(parent, index, child),
        Cmd::Destroy { id } => backend.destroy(id),
        Cmd::AttachEvent { id, event } => backend.declare_event(id, event),
        Cmd::DetachEvent { id, event } => backend.detach_event(id, event),
        Cmd::SetValue { id, value, based_on } => backend.set_value_stamped(id, value, based_on),
        Cmd::SetTemplatedItemCount { id, count } => backend.set_templated_item_count(id, count),
        Cmd::SetTemplatedRowContent {
            list_id,
            row_idx,
            content,
        } => backend.set_templated_row_content(list_id, row_idx, content),
        Cmd::SetTemplatedSelectedIndex { id, index } => {
            backend.set_templated_selected_index(id, index)
        }
        Cmd::SetTemplatedSelectionMode { id, mode } => {
            backend.set_templated_selection_mode(id, mode)
        }
        Cmd::SetTemplatedCanDragItems { id, value } => {
            backend.set_templated_can_drag_items(id, value)
        }
        Cmd::SetTemplatedCanReorderItems { id, value } => {
            backend.set_templated_can_reorder_items(id, value)
        }
        Cmd::SetTemplatedAllowDrop { id, value } => backend.set_templated_allow_drop(id, value),
        Cmd::SetHeaderElement { id, header_id } => backend.set_header_element(id, header_id),
        Cmd::SetPaneElement { id, pane_id } => backend.set_pane_element(id, pane_id),
        Cmd::ScrollTemplatedToIndex { id, index } => backend.scroll_templated_to_index(id, index),
        // Declarations whose closures live in the recorder's maps and whose
        // features the DComp backend has not grown yet: nothing consumes them
        // front-side, so replay records nothing. Each names its future
        // consumer — templated-list virtualization, the tooltip presenter, an
        // OLE drop target — and lands here, not in a trait method, so growing
        // the feature is an additive edit. The payloads are bound and dropped
        // (not wildcarded) so the variants stay honest data, not vestigial
        // fields the lint would flag.
        Cmd::AttachTemplatedSelectionChanged { id } => {
            let _ = id;
        }
        Cmd::AttachTemplatedRealization { id } => {
            let _ = id;
        }
        Cmd::AttachTemplatedReorder { id } => {
            let _ = id;
        }
        // The §7.3 accelerator table: the front records the `(key, mods)` list
        // it matches keydowns against; the `on_invoked` callbacks stay in the
        // recorder's `accels` map and fire as `Intent::Accelerator`.
        Cmd::SetKeybindings { id, keys } => backend.set_keybindings(id, keys),
        Cmd::SetTooltip { id, decl } => {
            let _ = (id, decl);
        }
        Cmd::SetDragInterest { id, interest } => {
            let _ = (id, interest);
        }
        Cmd::SetThemeBindings { id, kind, bindings } => {
            backend.set_theme_bindings(id, kind, &bindings)
        }
        Cmd::OnThemeChanged => backend.on_theme_changed(),
        Cmd::SetImplicitTransitions { id, transitions } => {
            backend.set_implicit_transitions(id, transitions)
        }
        Cmd::SetLayoutAnimation { id, config } => backend.set_layout_animation(id, config),
        Cmd::RunPropertyAnimation { id, config } => backend.run_property_animation(id, config),
        Cmd::SetExitTransition { id, config } => backend.set_exit_transition(id, config),
        Cmd::SetRichTextParagraphs { id, paragraphs } => {
            backend.set_rich_text_paragraphs(id, &paragraphs)
        }
        Cmd::SetAccessibility { id, accessibility } => {
            backend.set_accessibility(id, &accessibility)
        }
        Cmd::SetPointerInterest { id, interest } => backend.set_pointer_interest(id, interest),
    }
}

impl Backend for RecordingBackend {
    fn create(&mut self, id: ControlId, kind: ControlKind) {
        self.push(Cmd::Create { id, kind });
    }

    fn set_prop(&mut self, id: ControlId, prop: Prop, value: &PropValue) {
        // A numeric `Value` write is the app echoing (or setting) a control
        // value input may also be driving, so it is stamped with the input
        // revision the app had been consulted about when this render ran —
        // the framework half of the §7.2 revision protocol; app code never
        // sees the counters.
        if prop == Prop::Value
            && let PropValue::F64(v) = value
        {
            let based_on = self.delivered_value_rev.get(&id).copied().unwrap_or(0);
            self.push(Cmd::SetValue { id, value: *v, based_on });
            return;
        }
        match SendValue::from_prop(value) {
            Some(value) => self.push(Cmd::SetProp { id, prop, value }),
            // Thread-affine prop values (XAML image sources, flyout Element
            // trees) have no consumer on this backend — `DCompBackend::set_prop`
            // has no arm for any of them — so there is nothing to declare and
            // nothing to keep. If the backend ever grows one, `from_prop`'s
            // exhaustive match is where the decision resurfaces.
            None => {}
        }
    }

    fn append_child(&mut self, parent: ControlId, child: ControlId) {
        self.push(Cmd::AppendChild { parent, child });
    }

    fn remove_child(&mut self, parent: ControlId, index: usize) {
        self.push(Cmd::RemoveChild { parent, index });
    }

    fn replace_child(&mut self, parent: ControlId, index: usize, new: ControlId) {
        self.push(Cmd::ReplaceChild { parent, index, new });
    }

    fn move_child(&mut self, parent: ControlId, from: usize, to: usize) {
        self.push(Cmd::MoveChild { parent, from, to });
    }

    fn insert_child(&mut self, parent: ControlId, index: usize, child: ControlId) {
        self.push(Cmd::InsertChild {
            parent,
            index,
            child,
        });
    }

    fn destroy(&mut self, id: ControlId) {
        // The app-side closures are scoped to the control's own lifetime;
        // dropping them at record time is safe because any intent still queued
        // for this id resolves against the map at drain, and a drain always
        // runs before the reconcile that recorded this destroy.
        self.handlers.remove(&id);
        self.pointer.remove(&id);
        self.accels.remove(&id);
        self.drag.remove(&id);
        self.tooltips.remove(&id);
        self.realization.remove(&id);
        self.selection_changed.remove(&id);
        self.reorder.remove(&id);
        self.delivered_value_rev.remove(&id);
        self.push(Cmd::Destroy { id });
    }

    fn attach_event(&mut self, id: ControlId, event: Event, handler: EventHandler) {
        let slot = self.handlers.entry(id).or_default();
        slot.retain(|(e, _)| *e != event);
        slot.push((event, handler));
        self.push(Cmd::AttachEvent { id, event });
    }

    fn detach_event(&mut self, id: ControlId, event: Event) {
        if let Some(slot) = self.handlers.get_mut(&id) {
            slot.retain(|(e, _)| *e != event);
            if slot.is_empty() {
                self.handlers.remove(&id);
            }
        }
        self.push(Cmd::DetachEvent { id, event });
    }

    fn set_templated_item_count(&mut self, id: ControlId, count: usize) {
        self.push(Cmd::SetTemplatedItemCount { id, count });
    }

    fn set_templated_row_content(
        &mut self,
        list_id: ControlId,
        row_idx: usize,
        content: Option<ControlId>,
    ) {
        self.push(Cmd::SetTemplatedRowContent {
            list_id,
            row_idx,
            content,
        });
    }

    fn set_templated_selected_index(&mut self, id: ControlId, index: i32) {
        self.push(Cmd::SetTemplatedSelectedIndex { id, index });
    }

    fn set_templated_selection_mode(&mut self, id: ControlId, mode: SelectionMode) {
        self.push(Cmd::SetTemplatedSelectionMode { id, mode });
    }

    fn set_templated_can_drag_items(&mut self, id: ControlId, value: bool) {
        self.push(Cmd::SetTemplatedCanDragItems { id, value });
    }

    fn set_templated_can_reorder_items(&mut self, id: ControlId, value: bool) {
        self.push(Cmd::SetTemplatedCanReorderItems { id, value });
    }

    fn set_templated_allow_drop(&mut self, id: ControlId, value: bool) {
        self.push(Cmd::SetTemplatedAllowDrop { id, value });
    }

    fn set_header_element(&mut self, id: ControlId, header_id: Option<ControlId>) {
        self.push(Cmd::SetHeaderElement { id, header_id });
    }

    fn set_pane_element(&mut self, id: ControlId, pane_id: Option<ControlId>) {
        self.push(Cmd::SetPaneElement { id, pane_id });
    }

    fn scroll_templated_to_index(&mut self, id: ControlId, index: i32) {
        self.push(Cmd::ScrollTemplatedToIndex { id, index });
    }

    fn attach_templated_selection_changed(&mut self, id: ControlId, handler: Callback<i32>) {
        self.selection_changed.insert(id, handler);
        self.push(Cmd::AttachTemplatedSelectionChanged { id });
    }

    fn attach_templated_reorder(&mut self, id: ControlId, handler: Callback<Vec<usize>>) {
        self.reorder.insert(id, handler);
        self.push(Cmd::AttachTemplatedReorder { id });
    }

    fn set_theme_bindings(&mut self, id: ControlId, kind: ControlKind, bindings: &[(Prop, ThemeRef)]) {
        self.push(Cmd::SetThemeBindings {
            id,
            kind,
            bindings: bindings.to_vec(),
        });
    }

    fn on_theme_changed(&mut self) {
        self.push(Cmd::OnThemeChanged);
    }

    fn set_implicit_transitions(
        &mut self,
        id: ControlId,
        transitions: Option<ImplicitTransitions>,
    ) {
        self.push(Cmd::SetImplicitTransitions { id, transitions });
    }

    fn set_layout_animation(&mut self, id: ControlId, config: Option<LayoutAnimationConfig>) {
        self.push(Cmd::SetLayoutAnimation { id, config });
    }

    fn run_property_animation(&mut self, id: ControlId, config: Option<AnimationConfig>) {
        self.push(Cmd::RunPropertyAnimation { id, config });
    }

    fn set_exit_transition(&mut self, id: ControlId, config: Option<AnimationConfig>) {
        self.push(Cmd::SetExitTransition { id, config });
    }

    fn set_rich_text_paragraphs(&mut self, id: ControlId, paragraphs: &[RichTextParagraph]) {
        self.push(Cmd::SetRichTextParagraphs {
            id,
            paragraphs: paragraphs.to_vec(),
        });
    }

    fn attach_templated_realization(
        &mut self,
        id: ControlId,
        realize: Rc<dyn Fn(usize)>,
        recycle: Rc<dyn Fn(usize)>,
    ) {
        self.realization.insert(id, (realize, recycle));
        self.push(Cmd::AttachTemplatedRealization { id });
    }

    fn set_accessibility(&mut self, id: ControlId, accessibility: &AccessibilityModifiers) {
        self.push(Cmd::SetAccessibility {
            id,
            accessibility: accessibility.clone(),
        });
    }

    fn set_keyboard_accelerators(&mut self, id: ControlId, accelerators: &[KeyboardAccelerator]) {
        let keys = accelerators.iter().map(|a| (a.key, a.modifiers)).collect();
        if accelerators.is_empty() {
            self.accels.remove(&id);
        } else {
            self.accels.insert(id, accelerators.to_vec());
        }
        self.push(Cmd::SetKeybindings { id, keys });
    }

    fn set_tooltip(&mut self, id: ControlId, tooltip: Option<&Tooltip>) {
        let decl = tooltip.map(TooltipDecl::of);
        match tooltip {
            Some(t) => {
                self.tooltips.insert(id, t.clone());
            }
            None => {
                self.tooltips.remove(&id);
            }
        }
        self.push(Cmd::SetTooltip { id, decl });
    }

    fn set_pointer_handlers(&mut self, id: ControlId, handlers: Option<&PointerHandlers>) {
        let interest = match handlers {
            Some(h) => {
                self.pointer.insert(id, h.clone());
                PointerInterest::of(h)
            }
            None => {
                self.pointer.remove(&id);
                PointerInterest::default()
            }
        };
        self.push(Cmd::SetPointerInterest { id, interest });
    }

    fn set_drag_handlers(&mut self, id: ControlId, handlers: Option<&DragHandlers>) {
        let interest = match handlers {
            Some(h) => {
                self.drag.insert(id, h.clone());
                DragInterest::of(h)
            }
            None => {
                self.drag.remove(&id);
                DragInterest::default()
            }
        };
        self.push(Cmd::SetDragInterest { id, interest });
    }

    /// The DirectComposition backend exposes no native element — its controls
    /// are addressed by id — so the recorder answers `None` without consulting
    /// anything: there is nothing to consult, it holds no backend.
    fn get_native_element(&self, _id: ControlId) -> Option<windows_core::IInspectable> {
        None
    }
}


/// The buffers must stay `Send`: [`Cmd`] is the payload that will cross to the
/// app thread, and [`Intent`] is the payload that will cross back from the
/// front thread. If a thread-affine value ever leaks into either — instead of
/// the side table or the app-side handler maps — this fails to compile.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Cmd>();
    assert_send::<SendValue>();
    assert_send::<Intent>();
};
