use std::any::TypeId;
use std::rc::Rc;

use super::*;

/// A widget slot that mounts an [`Element`] tree of its own, beside its children.
///
/// A slot's subtree is a **parentless root**: it belongs to a popup or to a
/// backend-owned region rather than to the owner's box, so it takes no part in the
/// owner's layout and appears in no child list. That is what makes slots useful
/// and also what makes them dangerous — the reconciler's generic subtree walk
/// follows the child mirror, so it cannot reach a slot root. Slot lifetime has to
/// be driven from the slot bookkeeping instead.
///
/// Hence this enum. Every path that touches slots — mount, reconcile, unmount —
/// iterates [`ALL`](Self::ALL) rather than naming a slot, so a slot handled by
/// only some of them is not expressible. The compiler enforces the rest: adding a
/// variant breaks the exhaustive matches in [`Widget::slot_element`] and in the
/// reconciler's backend dispatch until the new slot is wired through both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// A complex header (e.g. `Expander`'s element header, `TitleBar`'s content).
    Header,
    /// A pane beside the content (e.g. `SplitView`'s pane, `NavigationView`'s footer).
    Pane,
    /// An attached flyout's content — live controls in a popup, not a snapshot.
    Flyout,
}

impl Slot {
    /// How many slot kinds exist; the width of a node's slot-root array.
    pub const COUNT: usize = 3;

    /// Every slot, in mount order. Iterated by the reconciler instead of naming
    /// slots one at a time.
    pub const ALL: [Slot; Self::COUNT] = [Slot::Header, Slot::Pane, Slot::Flyout];

    /// This slot's index into a node's slot-root array.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// One entry of a widget's flat declarative binding list: either a
/// property value or an event handler slot.
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    Prop(Prop, PropValue),
    Event(Event, Option<EventHandler>),
}

pub type PropBindings = Vec<Binding>;

/// Converts a widget struct field to an optional `PropValue::Str`.
///
/// Generated binding code calls `w.field.to_prop_str()` for TextBlock
/// and IReference setters. The trait lets the same generated code work
/// whether the field is `String` (always `Some`) or `Option<String>`
/// (mirrors the Option).
pub trait ToPropStr {
    fn to_prop_str(&self) -> Option<PropValue>;
}

impl ToPropStr for String {
    fn to_prop_str(&self) -> Option<PropValue> {
        Some(PropValue::Str(self.clone()))
    }
}

impl ToPropStr for Option<String> {
    fn to_prop_str(&self) -> Option<PropValue> {
        self.as_ref().map(|s| PropValue::Str(s.clone()))
    }
}

pub fn find_prop(bindings: &[Binding], prop: Prop) -> Option<&PropValue> {
    bindings.iter().find_map(|b| match b {
        Binding::Prop(p, v) if *p == prop => Some(v),
        _ => None,
    })
}

pub fn find_event(bindings: &[Binding], event: Event) -> Option<&Option<EventHandler>> {
    bindings.iter().find_map(|b| match b {
        Binding::Event(e, h) if *e == event => Some(h),
        _ => None,
    })
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct RichTextLineBreak;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct RichTextRun {
    pub text: String,
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_strikethrough: bool,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
}

impl RichTextRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct RichTextHyperlink {
    pub text: String,
    pub uri: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RichTextInline {
    Run(RichTextRun),
    LineBreak,
    Hyperlink(RichTextHyperlink),
}

impl Default for RichTextInline {
    fn default() -> Self {
        Self::Run(RichTextRun::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct RichTextParagraph {
    pub inlines: Vec<RichTextInline>,
}

impl RichTextParagraph {
    pub fn new(inlines: Vec<RichTextInline>) -> Self {
        Self { inlines }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct RichTextBlock {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub paragraphs: Vec<RichTextParagraph>,
    pub font_size: Option<f64>,
    pub is_text_selection_enabled: bool,
    pub text_wrapping: TextWrapping,
}

impl RichTextBlock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn single_paragraph(inlines: Vec<RichTextInline>) -> Self {
        Self {
            paragraphs: vec![RichTextParagraph::new(inlines)],
            ..Self::default()
        }
    }

    pub fn font_size(mut self, size: f64) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn selectable(mut self) -> Self {
        self.is_text_selection_enabled = true;
        self
    }

    pub fn wrap(mut self) -> Self {
        self.text_wrapping = TextWrapping::Wrap;
        self
    }
}

impl Widget for RichTextBlock {
    widget_header!(ControlKind::RichTextBlock);
    fn bindings(&self) -> PropBindings {
        let mut out = Vec::with_capacity(3);
        if let Some(fs) = self.font_size {
            out.push(Binding::Prop(Prop::FontSize, PropValue::F64(fs)));
        }
        if self.is_text_selection_enabled {
            out.push(Binding::Prop(
                Prop::IsTextSelectionEnabled,
                PropValue::Bool(true),
            ));
        }
        if self.text_wrapping != TextWrapping::NoWrap {
            out.push(Binding::Prop(
                Prop::TextWrappingWrap,
                PropValue::I32(self.text_wrapping.0),
            ));
        }
        out
    }
}

/// How a widget exposes its children to the child reconciler.
pub enum Children<'a> {
    None,
    PositionalSingle(&'a Element),
    Keyed(&'a [Element]),
    Tabs(&'a [TabItem]),
    PivotItems(&'a [PivotItem]),
}

/// Adapter implemented by every built-in [`Element`] widget variant; lets
/// the reconciler drive backend prop / child operations uniformly.
pub trait Widget {
    fn kind(&self) -> ControlKind;
    fn key(&self) -> Option<&str>;
    fn modifiers(&self) -> &Modifiers;
    fn attached(&self) -> Option<&AttachedProps> {
        self.modifiers().attached.as_ref()
    }
    fn bindings(&self) -> PropBindings;
    fn children(&self) -> Children<'_> {
        Children::None
    }
    /// The element tree this widget puts in `slot`, if any.
    ///
    /// The one place the slot kinds are enumerated. The reconciler mounts,
    /// reconciles and tears down slots by walking [`Slot::ALL`] and asking here,
    /// so none of those three paths names a slot — which is what stops a new slot
    /// from being wired into some of them and forgotten in the rest. Adding a
    /// [`Slot`] variant makes this match non-exhaustive, so the build fails until
    /// the accessor exists.
    fn slot_element(&self, slot: Slot) -> Option<&Element> {
        match slot {
            Slot::Header => self.header_element(),
            Slot::Pane => self.pane_element(),
            Slot::Flyout => self.flyout_element(),
        }
    }
    /// Optional element tree for a header slot (e.g. Expander complex header).
    fn header_element(&self) -> Option<&Element> {
        None
    }
    /// Optional element tree for a pane slot (e.g. SplitView pane).
    fn pane_element(&self) -> Option<&Element> {
        None
    }
    /// Optional element tree for an attached flyout's content.
    ///
    /// A slot rather than a child: the content belongs to a popup, not to the
    /// owner's box, so it must not take part in the owner's layout. Mounting it
    /// here is what makes a flyout hold real, live controls — reconciled, focus-
    /// and state-preserving, and identical to any other subtree — instead of a
    /// snapshot the backend has to re-interpret.
    fn flyout_element(&self) -> Option<&Element> {
        None
    }
    /// Optional post-mount callback. When present, the reconciler invokes it
    /// immediately after creation with the control's [`MountInfo`].
    fn on_mounted_callback(&self) -> Option<&Callback<MountInfo>> {
        None
    }
    /// Optional pre-unmount callback. When present, the reconciler invokes it
    /// just before the control is destroyed, while the control still exists,
    /// with the control's [`MountInfo`]. Owners use this to tear down external
    /// resources bound to the control (e.g. join a render thread that presents
    /// into the control's swap chain) — teardown runs regardless of whether a
    /// native element is present.
    fn on_unmounted_callback(&self) -> Option<&Callback<MountInfo>> {
        None
    }
}

/// What the reconciler hands a mount/unmount callback: the control's backend
/// id, and the native element when the backend exposes one.
///
/// [`id`](Self::id) is always present and is how app code addresses the
/// control — subscribing to its size, registering a pointer sink, parenting a
/// composition surface under it. Being `Copy` and `Send`, it is safe to retain
/// across renders and, unlike a native element, to carry to another thread.
///
/// [`native`](Self::native) is the live backend object: a XAML `UIElement` on
/// the WinUI backend, where interfaces like `ISwapChainPanelNative` are only
/// reachable through it. It is thread-affine, so retaining it pins the holder
/// to the backend's thread — address the control by `id` wherever the backend
/// offers an id-keyed path instead.
#[derive(Clone)]
pub struct MountInfo {
    pub id: ControlId,
    pub native: Option<windows_core::IInspectable>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TemplatedKind {
    ListView,
    GridView,
    FlipView,
}

/// Selection behaviour for templated lists (ListView, GridView).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    /// No selection allowed.
    None,
    /// Exactly one item may be selected at a time (default).
    #[default]
    Single,
    /// Multiple items may be selected via Ctrl+Click.
    Multiple,
    /// Range selection via Shift+Click and Ctrl+Click.
    Extended,
}

/// Erased adapter exposing the items of a [`TemplatedListElement`] to the
/// reconciler without leaking the concrete item type.
pub trait TemplatedListImpl: 'static {
    fn item_count(&self) -> usize;

    fn build_item_view(&self, index: usize) -> Element;

    fn item_key(&self, index: usize) -> Option<String>;

    fn selected_index(&self) -> i32;

    fn invoke_selection_changed(&self, index: i32);

    fn item_type_id(&self) -> TypeId;

    fn items_ptr(&self) -> *const ();

    fn has_selection_handler(&self) -> bool;

    fn raw_selection_callback(&self) -> Option<Callback<i32>>;
}

struct TemplatedListCell<T: 'static> {
    items: Rc<Vec<T>>,
    view_builder: Rc<dyn Fn(&T, usize) -> Element>,
    key_selector: Option<Rc<dyn Fn(&T) -> String>>,
    on_selection_changed: Option<Callback<i32>>,
    selected_index: i32,
}

impl<T: 'static> TemplatedListImpl for TemplatedListCell<T> {
    fn item_count(&self) -> usize {
        self.items.len()
    }

    fn build_item_view(&self, index: usize) -> Element {
        match self.items.get(index) {
            Some(item) => (self.view_builder)(item, index),
            None => Element::Empty,
        }
    }

    fn item_key(&self, index: usize) -> Option<String> {
        let item = self.items.get(index)?;
        self.key_selector.as_ref().map(|k| k(item))
    }

    fn selected_index(&self) -> i32 {
        self.selected_index
    }

    fn invoke_selection_changed(&self, index: i32) {
        if let Some(cb) = &self.on_selection_changed {
            cb.invoke(index);
        }
    }

    fn item_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn items_ptr(&self) -> *const () {
        Rc::as_ptr(&self.items) as *const ()
    }

    fn has_selection_handler(&self) -> bool {
        self.on_selection_changed.is_some()
    }

    fn raw_selection_callback(&self) -> Option<Callback<i32>> {
        self.on_selection_changed.clone()
    }
}

/// Virtualised list/grid/flip element backed by a [`TemplatedListImpl`].
pub struct TemplatedListElement {
    pub key: Option<String>,
    pub kind: TemplatedKind,
    pub selection_mode: SelectionMode,
    pub can_drag_items: bool,
    pub can_reorder_items: bool,
    pub allow_drop: bool,
    pub modifiers: Modifiers,
    pub items_impl: Rc<dyn TemplatedListImpl>,
    /// Invoked after a drag-reorder completes, with the new order expressed
    /// as the permutation of original item indices (position `i` holds the
    /// original index now shown there). Apps use it to reorder their own
    /// data so the change survives the next render.
    pub on_reorder: Option<Callback<Vec<usize>>>,
}

impl Clone for TemplatedListElement {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            kind: self.kind,
            selection_mode: self.selection_mode,
            can_drag_items: self.can_drag_items,
            can_reorder_items: self.can_reorder_items,
            allow_drop: self.allow_drop,
            modifiers: self.modifiers.clone(),
            items_impl: Rc::clone(&self.items_impl),
            on_reorder: self.on_reorder.clone(),
        }
    }
}

impl std::fmt::Debug for TemplatedListElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplatedListElement")
            .field("key", &self.key)
            .field("kind", &self.kind)
            .field("item_count", &self.items_impl.item_count())
            .finish()
    }
}

impl PartialEq for TemplatedListElement {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.kind == other.kind
            && self.selection_mode == other.selection_mode
            && self.can_drag_items == other.can_drag_items
            && self.can_reorder_items == other.can_reorder_items
            && self.allow_drop == other.allow_drop
            && self.modifiers == other.modifiers
            && Rc::ptr_eq(&self.items_impl, &other.items_impl)
    }
}

impl TemplatedListElement {
    pub fn same_items_as(&self, other: &Self) -> bool {
        self.items_impl.item_type_id() == other.items_impl.item_type_id()
            && self.items_impl.items_ptr() == other.items_impl.items_ptr()
    }

    pub fn item_count(&self) -> usize {
        self.items_impl.item_count()
    }

    pub fn build_item_view(&self, index: usize) -> Element {
        self.items_impl.build_item_view(index)
    }

    pub fn item_key(&self, index: usize) -> Option<String> {
        self.items_impl.item_key(index)
    }

    pub fn selected_index(&self) -> i32 {
        self.items_impl.selected_index()
    }

    pub fn invoke_selection_changed(&self, index: i32) {
        self.items_impl.invoke_selection_changed(index);
    }

    pub fn has_selection_handler(&self) -> bool {
        self.items_impl.has_selection_handler()
    }

    pub fn has_reorder_handler(&self) -> bool {
        self.on_reorder.is_some()
    }

    pub fn raw_reorder_callback(&self) -> Option<Callback<Vec<usize>>> {
        self.on_reorder.clone()
    }

    pub fn raw_selection_callback(&self) -> Option<Callback<i32>> {
        self.items_impl.raw_selection_callback()
    }

    #[cfg(feature = "test")]
    pub fn fully_keyed(&self) -> bool {
        (0..self.item_count()).all(|i| self.items_impl.item_key(i).is_some())
    }

    #[cfg(feature = "test")]
    pub fn collect_keys(&self) -> Vec<String> {
        (0..self.item_count())
            .map(|i| {
                self.items_impl
                    .item_key(i)
                    .unwrap_or_else(|| format!("__pos_{i}"))
            })
            .collect()
    }
}

/// Fluent builder for [`TemplatedListElement`]; pick a kind via
/// [`list_view`] / [`grid_view`] / [`flip_view`].
pub struct TemplatedListBuilder<T: 'static> {
    kind: TemplatedKind,
    items: Rc<Vec<T>>,
    view_builder: Rc<dyn Fn(&T, usize) -> Element>,
    key_selector: Option<Rc<dyn Fn(&T) -> String>>,
    on_selection_changed: Option<Callback<i32>>,
    selected_index: i32,
    selection_mode: SelectionMode,
    can_drag_items: bool,
    can_reorder_items: bool,
    allow_drop: bool,
    modifiers: Modifiers,
    element_key: Option<String>,
    on_reorder: Option<Callback<Vec<usize>>>,
}

impl<T: 'static> TemplatedListBuilder<T> {
    fn new<R: Into<Element>>(
        kind: TemplatedKind,
        items: Vec<T>,
        view: impl Fn(&T, usize) -> R + 'static,
    ) -> Self {
        Self {
            kind,
            items: Rc::new(items),
            view_builder: Rc::new(move |item, idx| view(item, idx).into()),
            key_selector: None,
            on_selection_changed: None,
            selected_index: -1,
            selection_mode: SelectionMode::Single,
            can_drag_items: false,
            can_reorder_items: false,
            allow_drop: false,
            modifiers: Modifiers::default(),
            element_key: None,
            on_reorder: None,
        }
    }

    pub fn with_key_selector(mut self, k: impl Fn(&T) -> String + 'static) -> Self {
        self.key_selector = Some(Rc::new(k));
        self
    }

    pub fn selected_index(mut self, i: i32) -> Self {
        self.selected_index = i;
        self
    }

    pub fn on_selection_changed(mut self, cb: impl IntoCallback<i32>) -> Self {
        self.on_selection_changed = Some(cb.into_callback());
        self
    }

    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    pub fn can_drag_items(mut self, v: bool) -> Self {
        self.can_drag_items = v;
        self
    }

    pub fn can_reorder_items(mut self, v: bool) -> Self {
        self.can_reorder_items = v;
        self
    }

    pub fn allow_drop(mut self, v: bool) -> Self {
        self.allow_drop = v;
        self
    }

    /// Handle drag-reorder completion. The callback receives the new order as
    /// the permutation of original item indices (position `i` holds the
    /// original index now displayed there). Reorder the backing data
    /// accordingly and re-render so the change persists. Typically paired with
    /// `can_drag_items(true)`, `can_reorder_items(true)`, and `allow_drop(true)`.
    pub fn on_reorder(mut self, cb: impl IntoCallback<Vec<usize>>) -> Self {
        self.on_reorder = Some(cb.into_callback());
        self
    }

    pub fn with_key(mut self, k: impl Into<String>) -> Self {
        self.element_key = Some(k.into());
        self
    }

    pub fn width(mut self, v: f64) -> Self {
        self.modifiers.width = Some(v);
        self
    }

    pub fn height(mut self, v: f64) -> Self {
        self.modifiers.height = Some(v);
        self
    }

    pub fn build(self) -> Element {
        let cell = TemplatedListCell::<T> {
            items: self.items,
            view_builder: self.view_builder,
            key_selector: self.key_selector,
            on_selection_changed: self.on_selection_changed,
            selected_index: self.selected_index,
        };
        Element::TemplatedList(TemplatedListElement {
            key: self.element_key,
            kind: self.kind,
            selection_mode: self.selection_mode,
            can_drag_items: self.can_drag_items,
            can_reorder_items: self.can_reorder_items,
            allow_drop: self.allow_drop,
            modifiers: self.modifiers,
            items_impl: Rc::new(cell),
            on_reorder: self.on_reorder,
        })
    }
}

impl<T: 'static> From<TemplatedListBuilder<T>> for Element {
    fn from(b: TemplatedListBuilder<T>) -> Self {
        b.build()
    }
}

pub fn list_view<T: 'static, R: Into<Element>>(
    items: Vec<T>,
    view: impl Fn(&T, usize) -> R + 'static,
) -> TemplatedListBuilder<T> {
    TemplatedListBuilder::new(TemplatedKind::ListView, items, view)
}

pub fn grid_view<T: 'static, R: Into<Element>>(
    items: Vec<T>,
    view: impl Fn(&T, usize) -> R + 'static,
) -> TemplatedListBuilder<T> {
    TemplatedListBuilder::new(TemplatedKind::GridView, items, view)
}

pub fn flip_view<T: 'static, R: Into<Element>>(
    items: Vec<T>,
    view: impl Fn(&T, usize) -> R + 'static,
) -> TemplatedListBuilder<T> {
    TemplatedListBuilder::new(TemplatedKind::FlipView, items, view)
}
