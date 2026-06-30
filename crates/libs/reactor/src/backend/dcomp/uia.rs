//! UI Automation provider tree for the DirectComposition backend.
//!
//! The backend renders its own controls (no WinUI element owns an automation
//! peer), so screen readers — and the project's `guishot` test harness — get
//! nothing for free. This module exposes the retained [`Node`](super::node::Node)
//! arena to UI Automation: one lightweight COM provider per [`ControlId`] (plus a
//! synthetic provider per item of a SelectorBar / ComboBox / NavigationView),
//! mirroring the logical tree, reporting Name/AutomationId/ControlType/focus, and
//! translating Invoke/Toggle/Value/RangeValue/SelectionItem/ExpandCollapse calls
//! into the **same** typed event dispatch a pointer or keyboard interaction takes
//! (the [`uia_*` action bridge](super::DCompBackend) in `input.rs`).
//!
//! ## Architecture
//! * **Providers are value objects.** [`ElementProvider`] holds only plain data
//!   (`hwnd`, `ControlId`, item index, root flag) and is recreated per query —
//!   no per-node COM caching, so idle cost is zero and there is no `!Send` state
//!   to store. UIA establishes element identity from `GetRuntimeId`, not pointer
//!   identity.
//! * **Threading.** UIA calls arrive on UIA worker threads, but the arena is
//!   `!Send` and single-threaded. Every provider method that touches the arena
//!   marshals to the UI thread through [`host::marshal_to_ui`] (a blocking
//!   request/response the message pump services); calls already on the UI thread
//!   run inline. There is one action path and one arena owner.

use std::mem::ManuallyDrop;

use super::host;
use super::{controls, theme};
use super::*;
use crate::backend::ControlKind;
use crate::system_bindings::{
    ClientToScreen, ExpandCollapseState, IExpandCollapseProvider, IExpandCollapseProvider_Impl,
    IInvokeProvider, IInvokeProvider_Impl, IRangeValueProvider, IRangeValueProvider_Impl,
    IRawElementProviderFragment, IRawElementProviderFragmentRoot,
    IRawElementProviderFragmentRoot_Impl, IRawElementProviderFragment_Impl,
    IRawElementProviderSimple, IRawElementProviderSimple_Impl, ISelectionItemProvider,
    ISelectionItemProvider_Impl, IToggleProvider, IToggleProvider_Impl, IValueProvider,
    IValueProvider_Impl, NavigateDirection, ProviderOptions, ScreenToClient, ToggleState, UiaRect,
    UiaHostProviderFromHwnd, UiaRaiseAutomationEvent, HWND, POINT, SAFEARRAY, VARIANT, VARIANT_0,
    VARIANT_0_0, VARIANT_0_0_0, UIA_AutomationFocusChangedEventId, UIA_AutomationIdPropertyId,
    UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
    UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ExpandCollapsePatternId,
    UIA_GroupControlTypeId, UIA_HasKeyboardFocusPropertyId, UIA_HelpTextPropertyId,
    UIA_ImageControlTypeId, UIA_InvokePatternId, UIA_IsContentElementPropertyId,
    UIA_IsControlElementPropertyId, UIA_IsEnabledPropertyId, UIA_IsKeyboardFocusablePropertyId,
    UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_NamePropertyId, UIA_PaneControlTypeId,
    UIA_PATTERN_ID, UIA_PROPERTY_ID, UIA_RangeValuePatternId, UIA_RangeValueValuePropertyId,
    UIA_SelectionItemPatternId, UIA_SliderControlTypeId, UIA_TabControlTypeId,
    UIA_TabItemControlTypeId, UIA_TextControlTypeId, UIA_TogglePatternId,
    UIA_ToggleToggleStatePropertyId, UIA_ValuePatternId, UIA_ValueValuePropertyId,
};
use windows_core::{
    implement_decl, Error, Interface, Result, BOOL, BSTR, HRESULT, IUnknown, PCWSTR,
};

// SAFEARRAY (runtime-id) + "is anyone listening?" — not in the generated set.
windows_core::link!("oleaut32.dll" "system" fn SafeArrayCreateVector(vt: u16, llbound: i32, celements: u32) -> *mut SAFEARRAY);
windows_core::link!("oleaut32.dll" "system" fn SafeArrayPutElement(psa: *mut SAFEARRAY, rgindices: *const i32, pv: *const core::ffi::c_void) -> HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaClientsAreListening() -> BOOL);

// `ProviderOptions_ServerSideProvider` — we are a server-side provider that
// marshals its own arena access, so we advertise nothing else.
const PROVIDER_OPTIONS_SERVER: ProviderOptions = 1;
// `UIA_E_ELEMENTNOTAVAILABLE` — returned when the node has gone (id reused/freed).
const UIA_E_ELEMENTNOTAVAILABLE: HRESULT = HRESULT(0x8004_0201u32 as i32);
// `UiaAppendRuntimeId` — first element of a fragment's runtime id.
const UIA_APPEND_RUNTIME_ID: i32 = 3;

// VARENUM tags used when building property VARIANTs.
const VT_I4: u16 = 3;
const VT_R8: u16 = 5;
const VT_BSTR: u16 = 8;
const VT_BOOL: u16 = 11;

// NavigateDirection values (uiautomationcore.h).
const NAV_PARENT: NavigateDirection = 0;
const NAV_NEXT: NavigateDirection = 1;
const NAV_PREV: NavigateDirection = 2;
const NAV_FIRST: NavigateDirection = 3;
const NAV_LAST: NavigateDirection = 4;

// ── VARIANT / runtime-id construction ────────────────────────────────────────

fn make_variant(vt: u16, inner: VARIANT_0_0_0) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: inner,
            }),
        },
    }
}

fn v_i4(v: i32) -> VARIANT {
    make_variant(VT_I4, VARIANT_0_0_0 { lVal: v })
}
fn v_r8(v: f64) -> VARIANT {
    make_variant(VT_R8, VARIANT_0_0_0 { dblVal: v })
}
fn v_bool(b: bool) -> VARIANT {
    make_variant(VT_BOOL, VARIANT_0_0_0 { boolVal: if b { -1 } else { 0 } })
}
fn v_bstr(s: String) -> VARIANT {
    make_variant(VT_BSTR, VARIANT_0_0_0 { bstrVal: ManuallyDrop::new(BSTR::from(s)) })
}

/// Build a fragment runtime id `[UiaAppendRuntimeId, control-id, item+1]` as a
/// `VT_I4` SAFEARRAY. UIA takes ownership of the array (it frees it). The triple
/// is stable per (node, item) so UIA can establish element identity across the
/// freshly-created providers this module hands out.
fn make_runtime_id(id: ControlId, item: i32) -> *mut SAFEARRAY {
    unsafe {
        let psa = SafeArrayCreateVector(VT_I4, 0, 3);
        if psa.is_null() {
            return core::ptr::null_mut();
        }
        let vals = [UIA_APPEND_RUNTIME_ID, id.get() as i32, item + 1];
        for (i, v) in vals.iter().enumerate() {
            let idx = i as i32;
            let _ = SafeArrayPutElement(psa, &idx, v as *const i32 as *const _);
        }
        psa
    }
}

// ── Kind → UIA mapping (pure) ────────────────────────────────────────────────

fn control_type(kind: ControlKind) -> i32 {
    use ControlKind::*;
    match kind {
        Button | RepeatButton | HyperlinkButton | DropDownButton | SplitButton | ToggleSwitch => {
            UIA_ButtonControlTypeId
        }
        CheckBox | ToggleButton | RadioButton => UIA_CheckBoxControlTypeId,
        TextBox | NumberBox | PasswordBox | AutoSuggestBox | RichEditBox => UIA_EditControlTypeId,
        Slider => UIA_SliderControlTypeId,
        ComboBox => UIA_ComboBoxControlTypeId,
        SelectorBar | TabView | Pivot => UIA_TabControlTypeId,
        NavigationView => UIA_ListControlTypeId,
        Expander => UIA_GroupControlTypeId,
        TextBlock | RichTextBlock => UIA_TextControlTypeId,
        Image | PersonPicture | Ellipse | Rectangle | Line => UIA_ImageControlTypeId,
        ScrollViewer | ScrollView | Canvas | SwapChainPanel => UIA_PaneControlTypeId,
        _ => UIA_GroupControlTypeId,
    }
}

fn item_control_type(parent: ControlKind) -> i32 {
    match parent {
        ControlKind::SelectorBar => UIA_TabItemControlTypeId,
        _ => UIA_ListItemControlTypeId,
    }
}

/// Containers whose selectable items are exposed as synthetic child elements.
fn is_item_container(kind: ControlKind) -> bool {
    matches!(
        kind,
        ControlKind::SelectorBar | ControlKind::ComboBox | ControlKind::NavigationView
    )
}

fn pattern_supported(kind: ControlKind, item: i32, pid: UIA_PATTERN_ID) -> bool {
    use ControlKind::*;
    if item >= 0 {
        return pid == UIA_SelectionItemPatternId;
    }
    if pid == UIA_InvokePatternId {
        matches!(kind, Button | RepeatButton | HyperlinkButton | DropDownButton | SplitButton)
    } else if pid == UIA_TogglePatternId {
        matches!(kind, ToggleSwitch | CheckBox | ToggleButton | RadioButton)
    } else if pid == UIA_ValuePatternId {
        matches!(kind, TextBox | NumberBox | PasswordBox | AutoSuggestBox)
    } else if pid == UIA_RangeValuePatternId {
        matches!(kind, Slider | NumberBox | ProgressBar | ProgressRing)
    } else if pid == UIA_ExpandCollapsePatternId {
        matches!(kind, Expander | ComboBox | DropDownButton | SplitButton)
    } else {
        false
    }
}

// ── Tree navigation result ───────────────────────────────────────────────────

/// Where a [`NavigateDirection`] step lands. `Send` (plain data) so it can be
/// returned across the UI-thread marshal.
#[derive(Clone, Copy)]
pub(crate) enum UiaNav {
    /// No element in that direction (`S_OK` + null).
    None,
    /// A real arena node.
    Node(ControlId),
    /// A synthetic item `i` of container node.
    Item(ControlId, i32),
    /// The fragment root (the reactor root node, as a root provider).
    Root,
}

// ── Backend read/navigation surface (UI thread only) ─────────────────────────
//
// These run inside the marshal on the UI thread and read the `!Send` arena
// directly. Privacy: `uia` is a child module of `dcomp`, so it can reach
// `DCompBackend`'s private fields the same way `input`/`layout` do.

impl DCompBackend {
    /// The reactor root id, used as the window's fragment root.
    pub(crate) fn uia_root(&self) -> Option<ControlId> {
        self.root
    }

    fn uia_kind(&self, id: ControlId) -> Option<ControlKind> {
        self.arena.get(id).map(|n| n.kind)
    }

    fn uia_item_count(&self, id: ControlId) -> i32 {
        match self.arena.get(id) {
            Some(n) if is_item_container(n.kind) => n.ctrl.items.len() as i32,
            _ => 0,
        }
    }

    /// The parent of `target` by DFS from the root (trees are small; no parent
    /// pointer is stored on the node).
    fn uia_parent(&self, target: ControlId) -> Option<ControlId> {
        fn rec(b: &DCompBackend, cur: ControlId, target: ControlId) -> Option<ControlId> {
            let n = b.arena.get(cur)?;
            for c in &n.children {
                if *c == target {
                    return Some(cur);
                }
                if let Some(p) = rec(b, *c, target) {
                    return Some(p);
                }
            }
            None
        }
        rec(self, self.root?, target)
    }

    pub(crate) fn uia_navigate(&self, id: ControlId, item: i32, dir: NavigateDirection) -> UiaNav {
        let Some(node) = self.arena.get(id) else {
            return UiaNav::None;
        };

        // Synthetic item: siblings within the container, parent is the container.
        if item >= 0 {
            let count = self.uia_item_count(id);
            return match dir {
                NAV_PARENT => UiaNav::Node(id),
                NAV_NEXT if item + 1 < count => UiaNav::Item(id, item + 1),
                NAV_PREV if item > 0 => UiaNav::Item(id, item - 1),
                _ => UiaNav::None,
            };
        }

        let item_count = self.uia_item_count(id);
        match dir {
            NAV_FIRST => {
                if item_count > 0 {
                    UiaNav::Item(id, 0)
                } else {
                    node.children.first().map_or(UiaNav::None, |c| UiaNav::Node(*c))
                }
            }
            NAV_LAST => {
                if item_count > 0 {
                    UiaNav::Item(id, item_count - 1)
                } else {
                    node.children.last().map_or(UiaNav::None, |c| UiaNav::Node(*c))
                }
            }
            NAV_PARENT => {
                if self.root == Some(id) {
                    UiaNav::None // the host (window frame) provides the parent
                } else {
                    match self.uia_parent(id) {
                        Some(p) if self.root == Some(p) => UiaNav::Root,
                        Some(p) => UiaNav::Node(p),
                        None => UiaNav::None,
                    }
                }
            }
            NAV_NEXT | NAV_PREV => {
                let Some(p) = self.uia_parent(id) else {
                    return UiaNav::None;
                };
                let Some(pn) = self.arena.get(p) else {
                    return UiaNav::None;
                };
                let Some(idx) = pn.children.iter().position(|c| *c == id) else {
                    return UiaNav::None;
                };
                let next = if dir == NAV_NEXT {
                    pn.children.get(idx + 1)
                } else if idx == 0 {
                    None
                } else {
                    pn.children.get(idx - 1)
                };
                next.map_or(UiaNav::None, |c| UiaNav::Node(*c))
            }
            _ => UiaNav::None,
        }
    }

    /// Accessible name: explicit AutomationName, else the visible label/text.
    fn uia_name(&self, id: ControlId, item: i32) -> String {
        let Some(n) = self.arena.get(id) else {
            return String::new();
        };
        if item >= 0 {
            return n.ctrl.items.get(item as usize).cloned().unwrap_or_default();
        }
        if let Some(a) = &n.accessibility
            && let Some(name) = &a.automation_name
            && !name.is_empty()
        {
            return name.clone();
        }
        if !n.paint.text.is_empty() {
            return n.paint.text.clone();
        }
        String::new()
    }

    fn uia_automation_id(&self, id: ControlId) -> String {
        self.arena
            .get(id)
            .and_then(|n| n.accessibility.as_ref())
            .and_then(|a| a.automation_id.clone())
            .unwrap_or_default()
    }

    fn uia_help_text(&self, id: ControlId) -> String {
        self.arena
            .get(id)
            .and_then(|n| n.accessibility.as_ref())
            .and_then(|a| a.help_text.clone())
            .unwrap_or_default()
    }

    fn uia_control_type(&self, id: ControlId, item: i32) -> i32 {
        match self.arena.get(id) {
            Some(n) if item >= 0 => item_control_type(n.kind),
            Some(n) => control_type(n.kind),
            None => UIA_GroupControlTypeId,
        }
    }

    fn uia_is_enabled(&self, id: ControlId) -> bool {
        self.arena.get(id).map_or(true, |n| n.paint.is_enabled)
    }

    fn uia_focusable(&self, id: ControlId, item: i32) -> bool {
        if item >= 0 {
            return true;
        }
        self.arena.get(id).map_or(false, |n| n.focusable)
    }

    fn uia_has_focus(&self, id: ControlId, item: i32) -> bool {
        item < 0 && self.focused_id == Some(id)
    }

    pub(crate) fn uia_pattern_supported(
        &self,
        id: ControlId,
        item: i32,
        pid: UIA_PATTERN_ID,
    ) -> bool {
        self.uia_kind(id)
            .is_some_and(|k| pattern_supported(k, item, pid))
    }

    fn uia_toggle_state(&self, id: ControlId) -> i32 {
        match self.arena.get(id) {
            Some(n) if n.ctrl.indeterminate => 2,
            Some(n) if n.ctrl.is_on || n.ctrl.is_checked => 1,
            _ => 0,
        }
    }

    fn uia_value_string(&self, id: ControlId) -> String {
        match self.arena.get(id) {
            // Never surface a password's contents.
            Some(n) if n.kind == ControlKind::PasswordBox => String::new(),
            Some(n) => n.editor.as_ref().map(|e| e.text()).unwrap_or_default(),
            None => String::new(),
        }
    }

    /// `(value, min, max, read-only, step)` for a range control.
    fn uia_range(&self, id: ControlId) -> Option<(f64, f64, f64, bool, f64)> {
        let n = self.arena.get(id)?;
        let readonly = matches!(n.kind, ControlKind::ProgressBar | ControlKind::ProgressRing);
        let step = n.ctrl.step.unwrap_or(1.0);
        Some((n.ctrl.value, n.ctrl.min, n.ctrl.max, readonly, step))
    }

    fn uia_expand_state(&self, id: ControlId) -> i32 {
        match self.arena.get(id).map(|n| n.kind) {
            Some(ControlKind::Expander) => {
                if self.arena.get(id).is_some_and(|n| n.ctrl.expanded) {
                    1
                } else {
                    0
                }
            }
            Some(ControlKind::ComboBox | ControlKind::DropDownButton | ControlKind::SplitButton) => {
                if self.popup.as_ref().is_some_and(|p| p.owner == id) {
                    1
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn uia_item_selected(&self, id: ControlId, item: i32) -> bool {
        self.arena.get(id).is_some_and(|n| n.ctrl.selected_index == item)
    }

    /// The fragment with keyboard focus, for the fragment root's `GetFocus`.
    pub(crate) fn uia_focus(&self) -> UiaNav {
        match self.focused_id {
            Some(id) if self.root == Some(id) => UiaNav::Root,
            Some(id) => UiaNav::Node(id),
            None => UiaNav::None,
        }
    }

    /// `BoundingRectangle` in screen pixels for node `id` (or item `item`).
    fn uia_bounding_rect(&self, id: ControlId, item: i32) -> Option<(f64, f64, f64, f64)> {
        let n = self.arena.get(id)?;
        let (mut x, mut y, mut w, mut h) = (n.rect.x, n.rect.y, n.rect.w, n.rect.h);
        if item >= 0 {
            match n.kind {
                ControlKind::SelectorBar => {
                    let sw = controls::segment_width(n);
                    x = n.rect.x + theme::BORDER_W + sw * item as f32;
                    w = sw;
                }
                ControlKind::NavigationView => {
                    y = n.rect.y + controls::NAV_ITEM_H * item as f32;
                    h = controls::NAV_ITEM_H;
                }
                _ => {} // ComboBox items live in a popup; report the field's box.
            }
        }
        Some(self.uia_screen_rect(x, y, w, h))
    }

    /// Convert a window-relative DIP rect to a screen-pixel UIA rect.
    fn uia_screen_rect(&self, x: f32, y: f32, w: f32, h: f32) -> (f64, f64, f64, f64) {
        let scale = self.scale();
        let mut pt = POINT {
            x: (x * scale).round() as i32,
            y: (y * scale).round() as i32,
        };
        unsafe {
            let _ = ClientToScreen(self.hwnd as HWND, &mut pt);
        }
        (pt.x as f64, pt.y as f64, (w * scale) as f64, (h * scale) as f64)
    }

    /// Deepest node containing screen point `(sx, sy)` (for `ElementProviderFromPoint`).
    fn uia_element_from_point(&self, sx: f64, sy: f64) -> UiaNav {
        let scale = self.scale();
        let mut pt = POINT { x: sx as i32, y: sy as i32 };
        unsafe {
            let _ = ScreenToClient(self.hwnd as HWND, &mut pt);
        }
        let (px, py) = (pt.x as f32 / scale, pt.y as f32 / scale);
        fn rec(b: &DCompBackend, id: ControlId, px: f32, py: f32) -> Option<ControlId> {
            let n = b.arena.get(id)?;
            if !n.rect.contains(px, py) {
                return None;
            }
            for c in &n.children {
                if let Some(found) = rec(b, *c, px, py) {
                    return Some(found);
                }
            }
            Some(id)
        }
        match self.root.and_then(|r| rec(self, r, px, py)) {
            Some(id) if self.root == Some(id) => UiaNav::Root,
            Some(id) => UiaNav::Node(id),
            None => UiaNav::Root,
        }
    }

    /// Raise an `AutomationFocusChanged` event for `id` — deferred onto the pump
    /// so it never runs inside an input borrow, and a no-op when no client is
    /// listening (idle cost stays zero). Called on the UI thread from `set_focus`.
    pub(crate) fn uia_raise_focus(&self, id: ControlId) {
        if !unsafe { UiaClientsAreListening() }.as_bool() {
            return;
        }
        let hwnd = self.hwnd;
        host::post_ui(hwnd, move || {
            let provider: IRawElementProviderSimple = ElementProvider::element(hwnd, id).into();
            unsafe {
                let _ = UiaRaiseAutomationEvent(provider.as_raw(), UIA_AutomationFocusChangedEventId);
            }
        });
    }
}

// ── Marshal helpers ──────────────────────────────────────────────────────────

fn not_available() -> Error {
    Error::from_hresult(UIA_E_ELEMENTNOTAVAILABLE)
}

/// Run `f` against the backend (UI thread, marshalled if needed). `None` only
/// when the host is gone.
fn on_backend<R, F>(hwnd: isize, f: F) -> Option<R>
where
    R: Send + 'static,
    F: FnOnce(&mut DCompBackend) -> R + Send + 'static,
{
    host::marshal_to_ui(hwnd, move || host::with_backend(f)).flatten()
}

/// Fetch a value, mapping a missing node/host to `UIA_E_ELEMENTNOTAVAILABLE`.
fn get<T, F>(hwnd: isize, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut DCompBackend) -> Option<T> + Send + 'static,
{
    on_backend(hwnd, f).flatten().ok_or_else(not_available)
}

/// Fire-and-forget an action onto the backend (Invoke/Toggle/SetValue/…).
fn act<F>(hwnd: isize, f: F)
where
    F: FnOnce(&mut DCompBackend) + Send + 'static,
{
    let _ = on_backend(hwnd, f);
}

fn root_id(hwnd: isize) -> Option<ControlId> {
    on_backend(hwnd, |b| b.uia_root()).flatten()
}

/// The window's default (host) provider for the non-client frame.
fn host_provider(hwnd: isize) -> Result<IRawElementProviderSimple> {
    let mut ptr = core::ptr::null_mut();
    unsafe { UiaHostProviderFromHwnd(hwnd as HWND, &mut ptr).ok()? };
    Ok(unsafe { IRawElementProviderSimple::from_raw(ptr) })
}

/// Build a fragment for a navigation result (`S_OK` + null for `None`).
fn nav_provider(hwnd: isize, nav: UiaNav) -> Result<IRawElementProviderFragment> {
    match nav {
        UiaNav::None => Err(Error::empty()),
        UiaNav::Node(c) => Ok(ElementProvider::element(hwnd, c).into()),
        UiaNav::Item(c, i) => Ok(ElementProvider::item(hwnd, c, i).into()),
        UiaNav::Root => {
            let rid = root_id(hwnd).ok_or_else(not_available)?;
            Ok(ElementProvider::root(hwnd, rid).into())
        }
    }
}

/// The window's root UI Automation provider (a fragment root for the reactor
/// root node). Returned from `WM_GETOBJECT`.
pub(crate) fn root_provider(hwnd: isize, root: ControlId) -> IRawElementProviderSimple {
    ElementProvider::root(hwnd, root).into()
}

// ── The provider object ──────────────────────────────────────────────────────

/// One UI Automation provider: a value object identifying an arena node (or one
/// synthetic item of a container). Agile and recreated per query; all real work
/// marshals to the UI thread.
#[derive(Clone, Copy)]
struct ElementProvider {
    hwnd: isize,
    id: ControlId,
    /// Whether this is the window's fragment root (the reactor root node).
    is_root: bool,
    /// Synthetic item index within a container, or `-1` for the node itself.
    item: i32,
}

impl ElementProvider {
    fn root(hwnd: isize, id: ControlId) -> Self {
        Self { hwnd, id, is_root: true, item: -1 }
    }
    fn element(hwnd: isize, id: ControlId) -> Self {
        Self { hwnd, id, is_root: false, item: -1 }
    }
    fn item(hwnd: isize, id: ControlId, item: i32) -> Self {
        Self { hwnd, id, is_root: false, item }
    }
    fn dup(&self) -> Self {
        *self
    }

    /// A property VARIANT for `pid` (`VT_EMPTY` for anything we don't report).
    fn property(&self, pid: UIA_PROPERTY_ID) -> VARIANT {
        let (hwnd, id, item) = (self.hwnd, self.id, self.item);
        if pid == UIA_NamePropertyId {
            v_bstr(on_backend(hwnd, move |b| b.uia_name(id, item)).unwrap_or_default())
        } else if pid == UIA_AutomationIdPropertyId {
            v_bstr(on_backend(hwnd, move |b| b.uia_automation_id(id)).unwrap_or_default())
        } else if pid == UIA_HelpTextPropertyId {
            v_bstr(on_backend(hwnd, move |b| b.uia_help_text(id)).unwrap_or_default())
        } else if pid == UIA_ControlTypePropertyId {
            v_i4(on_backend(hwnd, move |b| b.uia_control_type(id, item)).unwrap_or(UIA_GroupControlTypeId))
        } else if pid == UIA_IsEnabledPropertyId {
            v_bool(on_backend(hwnd, move |b| b.uia_is_enabled(id)).unwrap_or(true))
        } else if pid == UIA_IsKeyboardFocusablePropertyId {
            v_bool(on_backend(hwnd, move |b| b.uia_focusable(id, item)).unwrap_or(false))
        } else if pid == UIA_HasKeyboardFocusPropertyId {
            v_bool(on_backend(hwnd, move |b| b.uia_has_focus(id, item)).unwrap_or(false))
        } else if pid == UIA_IsControlElementPropertyId || pid == UIA_IsContentElementPropertyId {
            v_bool(true)
        } else if pid == UIA_ToggleToggleStatePropertyId {
            v_i4(on_backend(hwnd, move |b| b.uia_toggle_state(id)).unwrap_or(0))
        } else if pid == UIA_RangeValueValuePropertyId {
            v_r8(on_backend(hwnd, move |b| b.uia_range(id)).flatten().map_or(0.0, |r| r.0))
        } else if pid == UIA_ValueValuePropertyId {
            v_bstr(on_backend(hwnd, move |b| b.uia_value_string(id)).unwrap_or_default())
        } else {
            VARIANT::default()
        }
    }
}

implement_decl! {
    impl ElementProvider as ElementProvider_Impl: [
        IRawElementProviderSimple,
        IRawElementProviderFragment,
        IRawElementProviderFragmentRoot,
        IInvokeProvider,
        IToggleProvider,
        IValueProvider,
        IRangeValueProvider,
        ISelectionItemProvider,
        IExpandCollapseProvider
    ]
}

impl IRawElementProviderSimple_Impl for ElementProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(PROVIDER_OPTIONS_SERVER)
    }

    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        let (id, item) = (self.id, self.item);
        let supported = get(self.hwnd, move |b| Some(b.uia_pattern_supported(id, item, patternid)))?;
        if supported {
            Ok(self.dup().into())
        } else {
            Err(Error::empty()) // S_OK + null: pattern not supported
        }
    }

    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        Ok(self.property(propertyid))
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        if self.is_root {
            host_provider(self.hwnd)
        } else {
            Err(Error::empty()) // S_OK + null: only the root merges with the host
        }
    }
}

impl IRawElementProviderFragment_Impl for ElementProvider_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let (id, item) = (self.id, self.item);
        let nav = get(self.hwnd, move |b| Some(b.uia_navigate(id, item, direction)))?;
        nav_provider(self.hwnd, nav)
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Ok(make_runtime_id(self.id, self.item))
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        let (id, item) = (self.id, self.item);
        let (left, top, width, height) = get(self.hwnd, move |b| b.uia_bounding_rect(id, item))?;
        Ok(UiaRect { left, top, width, height })
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(core::ptr::null_mut())
    }

    fn SetFocus(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_focus_node(id));
        Ok(())
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        let rid = root_id(self.hwnd).ok_or_else(not_available)?;
        Ok(ElementProvider::root(self.hwnd, rid).into())
    }
}

impl IRawElementProviderFragmentRoot_Impl for ElementProvider_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let nav = get(self.hwnd, move |b| Some(b.uia_element_from_point(x, y)))?;
        nav_provider(self.hwnd, nav)
    }

    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        let nav = get(self.hwnd, |b| Some(b.uia_focus()))?;
        nav_provider(self.hwnd, nav)
    }
}

impl IInvokeProvider_Impl for ElementProvider_Impl {
    fn Invoke(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_activate(id));
        Ok(())
    }
}

impl IToggleProvider_Impl for ElementProvider_Impl {
    fn Toggle(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_activate(id));
        Ok(())
    }

    fn ToggleState(&self) -> Result<ToggleState> {
        let id = self.id;
        get(self.hwnd, move |b| Some(b.uia_toggle_state(id)))
    }
}

impl IValueProvider_Impl for ElementProvider_Impl {
    fn SetValue(&self, val: &PCWSTR) -> Result<()> {
        let s = unsafe { val.to_string() }.unwrap_or_default();
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_text(id, &s));
        Ok(())
    }

    fn Value(&self) -> Result<BSTR> {
        let id = self.id;
        Ok(BSTR::from(get(self.hwnd, move |b| Some(b.uia_value_string(id)))?))
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl IRangeValueProvider_Impl for ElementProvider_Impl {
    fn SetValue(&self, val: f64) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_range(id, val));
        Ok(())
    }

    fn Value(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.0)
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.3.into())
    }

    fn Maximum(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.2)
    }

    fn Minimum(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.1)
    }

    fn LargeChange(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.4 * 10.0)
    }

    fn SmallChange(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.4)
    }
}

impl ISelectionItemProvider_Impl for ElementProvider_Impl {
    fn Select(&self) -> Result<()> {
        let (id, item) = (self.id, self.item);
        act(self.hwnd, move |b| b.uia_select_item(id, item));
        Ok(())
    }

    fn AddToSelection(&self) -> Result<()> {
        // Single-selection containers: identical to Select.
        let (id, item) = (self.id, self.item);
        act(self.hwnd, move |b| b.uia_select_item(id, item));
        Ok(())
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Ok(()) // single-select: nothing to remove
    }

    fn IsSelected(&self) -> Result<BOOL> {
        let (id, item) = (self.id, self.item);
        Ok(get(self.hwnd, move |b| Some(b.uia_item_selected(id, item)))?.into())
    }

    fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> {
        Ok(ElementProvider::element(self.hwnd, self.id).into())
    }
}

impl IExpandCollapseProvider_Impl for ElementProvider_Impl {
    fn Expand(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_expanded(id, true));
        Ok(())
    }

    fn Collapse(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_expanded(id, false));
        Ok(())
    }

    fn ExpandCollapseState(&self) -> Result<ExpandCollapseState> {
        let id = self.id;
        get(self.hwnd, move |b| Some(b.uia_expand_state(id)))
    }
}
