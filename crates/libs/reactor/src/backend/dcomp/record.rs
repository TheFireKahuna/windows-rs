//! Command-buffer seam between the reconciler and the DComp backend.
//!
//! [`RecordingBackend`] wraps a concrete backend and implements [`Backend`] by
//! encoding every call as a plain-data [`Cmd`] instead of mutating the
//! composition tree directly. A later [`RecordingBackend::flush`] replays the
//! buffer into the wrapped backend. Today replay is synchronous and on the same
//! thread, so behaviour is identical to calling through; the point of the seam
//! is that the buffer is **`Send`**, which is what lets the reconciler
//! eventually run on a thread that is not the one owning the HWND, the message
//! pump and the compositor.
//!
//! Two properties are load-bearing and are enforced by the compiler rather than
//! by convention:
//!
//! * **[`Cmd`] is `Send`.** Anything thread-affine — `Rc<dyn Fn>` handlers, COM
//!   image sources — is kept out of the buffer and parked in a side table keyed
//!   by [`SideId`]. The buffer references it by id. The assertion at the bottom
//!   of this module fails the build if a `!Send` payload ever leaks in.
//! * **[`SendValue::from_prop`] matches [`PropValue`] exhaustively.** A new
//!   `PropValue` variant upstream breaks this build instead of being silently
//!   dropped from the wire, forcing a deliberate send-or-side-table decision.
//!
//! Control ids are minted here rather than by the wrapped backend. They are a
//! monotonic counter that is never reused (see the `Arena` docs in [`node`] for
//! why reuse would corrupt the reconciler's graft check), so the reconciler can
//! mint them itself and `create`'s synchronous read-back dissolves. The replay
//! side inserts at the recorded id via `DCompBackend::create_with_id`; this
//! recorder therefore owns the *sole* id counter for the process.
//!
//! [`node`]: super::node

use std::collections::HashMap;
use std::rc::Rc;

use rustc_hash::FxHashMap;

use crate::backend::{
    AccessibilityModifiers, AnimationConfig, Backend, Color, CommandBarCommandDef, ControlId,
    ControlKind, Event, EventHandler, GridLength, ImplicitTransitions, KeyboardAccelerator,
    LayoutAnimationConfig, LineEndpoints, MenuBarItemDef, MenuItemDef, NavViewItem,
    PointerHandlers, Prop, PropValue, RichTextParagraph, SelectionMode, SelectorBarItemDef,
    Thickness, ThemeRef, Tooltip, TreeNodeDef,
};
use crate::drag::DragHandlers;
use crate::interaction::Callback;

/// Key into the recorder's side table of thread-affine payloads.
///
/// The buffer carries these instead of the payload itself, which is what keeps
/// [`Cmd`] `Send`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) struct SideId(u32);

/// The `Send` subset of [`PropValue`].
///
/// Three `PropValue` variants hold thread-affine payloads — `SurfaceImageSource`
/// and `VirtualSurfaceImageSource` wrap COM interfaces, `FlyoutDef` holds a
/// `Box<Element>` and a callback — so they cannot ride the buffer. All three are
/// inert on this backend today (no `Prop::ImageSource` arm exists in
/// `DCompBackend::set_prop`; flyouts are a WinUI feature), so they are parked in
/// the side table and replayed verbatim rather than being dropped: the recorder
/// stays faithful even where the backend currently ignores the value.
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

/// A thread-affine payload parked out of the buffer, retrieved at replay by
/// [`SideId`].
///
/// Every variant here holds an `Rc<dyn Fn>`, a COM interface, or an `Element`
/// tree. When replay moves to the front thread these do not travel with it:
/// handlers stay on the app thread and are invoked from input intents, and the
/// COM-bearing prop values become front-side creation commands. Until then the
/// side table simply keeps [`Cmd`] honest.
enum SidePayload {
    Event(EventHandler),
    Pointer(PointerHandlers),
    Drag(DragHandlers),
    Tooltip(Tooltip),
    Accelerators(Vec<KeyboardAccelerator>),
    Realization(Rc<dyn Fn(usize)>, Rc<dyn Fn(usize)>),
    SelectionChanged(Callback<i32>),
    Prop(PropValue),
}

/// One recorded [`Backend`] call, as plain data.
///
/// One variant per trait method. Order in the buffer is the order the
/// reconciler issued the calls and must be preserved exactly: the child ops are
/// positional against the backend's live child vector, so reordering or
/// coalescing corrupts the tree.
#[derive(Debug)]
pub(crate) enum Cmd {
    /// Not currently recorded — `create` is applied eagerly so that
    /// `get_native_element` stays exact (see [`Backend::create`] on
    /// [`RecordingBackend`]). The variant is the encoding this call takes once
    /// the id-token protocol removes that read-back and creation joins the
    /// buffer like everything else.
    #[allow(dead_code, reason = "recorded once create stops being eager")]
    Create {
        id: ControlId,
        kind: ControlKind,
    },
    SetProp {
        id: ControlId,
        prop: Prop,
        value: SendValue,
    },
    /// `set_prop` whose value is thread-affine; the value is in the side table.
    SetPropSide {
        id: ControlId,
        prop: Prop,
        side: SideId,
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
    AttachEvent {
        id: ControlId,
        event: Event,
        side: SideId,
    },
    DetachEvent {
        id: ControlId,
        event: Event,
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
    AttachTemplatedSelectionChanged {
        id: ControlId,
        side: SideId,
    },
    AttachTemplatedRealization {
        id: ControlId,
        side: SideId,
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
    SetKeyboardAccelerators {
        id: ControlId,
        side: SideId,
    },
    SetTooltip {
        id: ControlId,
        side: Option<SideId>,
    },
    SetPointerHandlers {
        id: ControlId,
        side: Option<SideId>,
    },
    SetDragHandlers {
        id: ControlId,
        side: Option<SideId>,
    },
}

/// Records [`Backend`] calls into a `Send` buffer, then replays them into the
/// wrapped backend on [`flush`](Self::flush).
///
/// Derefs to the wrapped backend so the host's inherent-method call sites
/// (input dispatch, UIA, layout, caption) reach through unchanged. Those sites
/// are exactly the surface that stays on the front thread when replay goes
/// cross-thread, at which point the `Deref` is removed deliberately and each
/// one is re-pointed.
pub(crate) struct RecordingBackend<B: Backend> {
    inner: B,
    cmds: Vec<Cmd>,
    side: FxHashMap<SideId, SidePayload>,
    next_side: u32,
    next_id: u32,
}

impl<B: CreateWithId> RecordingBackend<B> {
    pub(crate) fn new(inner: B) -> Self {
        Self {
            inner,
            cmds: Vec::new(),
            side: FxHashMap::default(),
            next_side: 0,
            next_id: 0,
        }
    }

    /// Park a thread-affine payload and return its buffer-safe key.
    fn park(&mut self, payload: SidePayload) -> SideId {
        self.next_side += 1;
        let key = SideId(self.next_side);
        self.side.insert(key, payload);
        key
    }

    fn push(&mut self, cmd: Cmd) {
        self.cmds.push(cmd);
    }

    /// Number of commands currently buffered. Used by the host to assert the
    /// buffer was drained before layout reads the arena.
    pub(crate) fn pending(&self) -> usize {
        self.cmds.len()
    }

    /// Replay every buffered command into the wrapped backend, in issue order.
    ///
    /// Commands are applied literally — never reordered, coalesced or repaired.
    /// The reconciler is the authority on tree shape (it updates its own
    /// `children_mirror` *before* issuing each call), so the backend is a
    /// follower and any "fix-up" here would desynchronise the two. In
    /// particular the buffer legitimately contains transient states the
    /// reconciler itself produces — a child destroyed before it is unparented,
    /// and the reverse order on the tab/pivot paths — so replay must not
    /// validate intermediate consistency.
    pub(crate) fn flush(&mut self) {
        if self.cmds.is_empty() {
            return;
        }
        for cmd in std::mem::take(&mut self.cmds) {
            self.apply(cmd);
        }
    }
}

impl<B: Backend> std::ops::Deref for RecordingBackend<B> {
    type Target = B;

    fn deref(&self) -> &B {
        &self.inner
    }
}

impl<B: Backend> std::ops::DerefMut for RecordingBackend<B> {
    fn deref_mut(&mut self) -> &mut B {
        &mut self.inner
    }
}

impl<B: CreateWithId> Backend for RecordingBackend<B> {
    /// Mint the id here and create eagerly.
    ///
    /// `create` is the one call whose effect is synchronously observable
    /// through the trait: [`get_native_element`](Backend::get_native_element)
    /// takes `&self` and is invoked mid-reconcile by `on_mounted`, so a control
    /// whose creation was still buffered would report `None` and silently break
    /// every host that parents a composition surface under a node. Creating
    /// eagerly keeps that observation exact while the id — the part the
    /// reconciler actually needs back — is still minted on this side, which is
    /// what dissolves the read-back. Everything else defers.
    ///
    /// This eager path disappears with the id-token protocol that replaces
    /// `get_native_element`, at which point `Cmd::Create` rides the buffer like
    /// every other command.
    fn create(&mut self, kind: ControlKind) -> ControlId {
        self.next_id += 1;
        let id = ControlId::new(self.next_id);
        self.inner.create_with_id(id, kind);
        id
    }

    fn set_prop(&mut self, id: ControlId, prop: Prop, value: &PropValue) {
        match SendValue::from_prop(value) {
            Some(value) => self.push(Cmd::SetProp { id, prop, value }),
            None => {
                let side = self.park(SidePayload::Prop(value.clone()));
                self.push(Cmd::SetPropSide { id, prop, side });
            }
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
        self.push(Cmd::Destroy { id });
    }

    fn attach_event(&mut self, id: ControlId, event: Event, handler: EventHandler) {
        let side = self.park(SidePayload::Event(handler));
        self.push(Cmd::AttachEvent { id, event, side });
    }

    fn detach_event(&mut self, id: ControlId, event: Event) {
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
        let side = self.park(SidePayload::SelectionChanged(handler));
        self.push(Cmd::AttachTemplatedSelectionChanged { id, side });
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
        let side = self.park(SidePayload::Realization(realize, recycle));
        self.push(Cmd::AttachTemplatedRealization { id, side });
    }

    fn set_accessibility(&mut self, id: ControlId, accessibility: &AccessibilityModifiers) {
        self.push(Cmd::SetAccessibility {
            id,
            accessibility: accessibility.clone(),
        });
    }

    fn set_keyboard_accelerators(&mut self, id: ControlId, accelerators: &[KeyboardAccelerator]) {
        let side = self.park(SidePayload::Accelerators(accelerators.to_vec()));
        self.push(Cmd::SetKeyboardAccelerators { id, side });
    }

    fn set_tooltip(&mut self, id: ControlId, tooltip: Option<&Tooltip>) {
        let side = tooltip.map(|t| self.park(SidePayload::Tooltip(t.clone())));
        self.push(Cmd::SetTooltip { id, side });
    }

    fn set_pointer_handlers(&mut self, id: ControlId, handlers: Option<&PointerHandlers>) {
        let side = handlers.map(|h| self.park(SidePayload::Pointer(h.clone())));
        self.push(Cmd::SetPointerHandlers { id, side });
    }

    fn set_drag_handlers(&mut self, id: ControlId, handlers: Option<&DragHandlers>) {
        let side = handlers.map(|h| self.park(SidePayload::Drag(h.clone())));
        self.push(Cmd::SetDragHandlers { id, side });
    }

    /// Reads straight through: `create` is eager, so every live control exists
    /// in the wrapped backend even with a non-empty buffer. This is the one
    /// remaining synchronous read-back on the seam.
    fn get_native_element(&self, id: ControlId) -> Option<windows_core::IInspectable> {
        self.inner.get_native_element(id)
    }
}

impl<B: CreateWithId> RecordingBackend<B> {
    /// Apply one recorded command to the wrapped backend.
    ///
    /// Side-table payloads are *taken*, not cloned: a command is replayed once,
    /// so the entry is dead afterwards and leaving it would grow the table
    /// without bound. A missing entry means the same command was replayed
    /// twice, which is a bug in the buffer plumbing rather than a recoverable
    /// state, so it is asserted in debug and skipped in release.
    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Create { id, kind } => self.inner.create_with_id(id, kind),
            Cmd::SetProp { id, prop, value } => self.inner.set_prop(id, prop, &value.into_prop()),
            Cmd::SetPropSide { id, prop, side } => {
                if let Some(SidePayload::Prop(value)) = self.take_side(side) {
                    self.inner.set_prop(id, prop, &value);
                }
            }
            Cmd::AppendChild { parent, child } => self.inner.append_child(parent, child),
            Cmd::RemoveChild { parent, index } => self.inner.remove_child(parent, index),
            Cmd::ReplaceChild { parent, index, new } => {
                self.inner.replace_child(parent, index, new)
            }
            Cmd::MoveChild { parent, from, to } => self.inner.move_child(parent, from, to),
            Cmd::InsertChild {
                parent,
                index,
                child,
            } => self.inner.insert_child(parent, index, child),
            Cmd::Destroy { id } => self.inner.destroy(id),
            Cmd::AttachEvent { id, event, side } => {
                if let Some(SidePayload::Event(handler)) = self.take_side(side) {
                    self.inner.attach_event(id, event, handler);
                }
            }
            Cmd::DetachEvent { id, event } => self.inner.detach_event(id, event),
            Cmd::SetTemplatedItemCount { id, count } => {
                self.inner.set_templated_item_count(id, count)
            }
            Cmd::SetTemplatedRowContent {
                list_id,
                row_idx,
                content,
            } => self.inner.set_templated_row_content(list_id, row_idx, content),
            Cmd::SetTemplatedSelectedIndex { id, index } => {
                self.inner.set_templated_selected_index(id, index)
            }
            Cmd::SetTemplatedSelectionMode { id, mode } => {
                self.inner.set_templated_selection_mode(id, mode)
            }
            Cmd::SetTemplatedCanDragItems { id, value } => {
                self.inner.set_templated_can_drag_items(id, value)
            }
            Cmd::SetTemplatedCanReorderItems { id, value } => {
                self.inner.set_templated_can_reorder_items(id, value)
            }
            Cmd::SetTemplatedAllowDrop { id, value } => {
                self.inner.set_templated_allow_drop(id, value)
            }
            Cmd::SetHeaderElement { id, header_id } => self.inner.set_header_element(id, header_id),
            Cmd::SetPaneElement { id, pane_id } => self.inner.set_pane_element(id, pane_id),
            Cmd::ScrollTemplatedToIndex { id, index } => {
                self.inner.scroll_templated_to_index(id, index)
            }
            Cmd::AttachTemplatedSelectionChanged { id, side } => {
                if let Some(SidePayload::SelectionChanged(handler)) = self.take_side(side) {
                    self.inner.attach_templated_selection_changed(id, handler);
                }
            }
            Cmd::AttachTemplatedRealization { id, side } => {
                if let Some(SidePayload::Realization(realize, recycle)) = self.take_side(side) {
                    self.inner.attach_templated_realization(id, realize, recycle);
                }
            }
            Cmd::SetThemeBindings { id, kind, bindings } => {
                self.inner.set_theme_bindings(id, kind, &bindings)
            }
            Cmd::OnThemeChanged => self.inner.on_theme_changed(),
            Cmd::SetImplicitTransitions { id, transitions } => {
                self.inner.set_implicit_transitions(id, transitions)
            }
            Cmd::SetLayoutAnimation { id, config } => self.inner.set_layout_animation(id, config),
            Cmd::RunPropertyAnimation { id, config } => {
                self.inner.run_property_animation(id, config)
            }
            Cmd::SetExitTransition { id, config } => self.inner.set_exit_transition(id, config),
            Cmd::SetRichTextParagraphs { id, paragraphs } => {
                self.inner.set_rich_text_paragraphs(id, &paragraphs)
            }
            Cmd::SetAccessibility { id, accessibility } => {
                self.inner.set_accessibility(id, &accessibility)
            }
            Cmd::SetKeyboardAccelerators { id, side } => {
                if let Some(SidePayload::Accelerators(accelerators)) = self.take_side(side) {
                    self.inner.set_keyboard_accelerators(id, &accelerators);
                }
            }
            Cmd::SetTooltip { id, side } => {
                let tooltip = side.and_then(|s| match self.take_side(s) {
                    Some(SidePayload::Tooltip(t)) => Some(t),
                    _ => None,
                });
                self.inner.set_tooltip(id, tooltip.as_ref());
            }
            Cmd::SetPointerHandlers { id, side } => {
                let handlers = side.and_then(|s| match self.take_side(s) {
                    Some(SidePayload::Pointer(h)) => Some(h),
                    _ => None,
                });
                self.inner.set_pointer_handlers(id, handlers.as_ref());
            }
            Cmd::SetDragHandlers { id, side } => {
                let handlers = side.and_then(|s| match self.take_side(s) {
                    Some(SidePayload::Drag(h)) => Some(h),
                    _ => None,
                });
                self.inner.set_drag_handlers(id, handlers.as_ref());
            }
        }
    }

    fn take_side(&mut self, side: SideId) -> Option<SidePayload> {
        let payload = self.side.remove(&side);
        debug_assert!(
            payload.is_some(),
            "side-table entry {side:?} missing — command replayed twice?"
        );
        payload
    }
}

/// Accepts a control id minted by the caller instead of minting its own.
///
/// Splitting the id out of `create` is what lets the reconciler address
/// controls without waiting on the backend. The contract the implementor must
/// uphold is the arena's: ids arrive monotonically and are never reused, so a
/// destroyed control's id can never alias a live one (see the `Arena` docs in
/// [`node`] for what breaks otherwise).
///
/// [`node`]: super::node
pub(crate) trait CreateWithId: Backend {
    fn create_with_id(&mut self, id: ControlId, kind: ControlKind);
}

/// The buffer must stay `Send`: it is the payload that will cross to the app
/// thread. If a thread-affine value ever leaks into a [`Cmd`] variant instead of
/// the side table, this fails to compile.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Cmd>();
    assert_send::<SendValue>();
};
