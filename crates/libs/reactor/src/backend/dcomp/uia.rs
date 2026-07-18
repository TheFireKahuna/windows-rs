//! UI Automation provider tree for the DirectComposition backend.
//!
//! The backend renders its own controls (no WinUI element owns an automation
//! peer), so screen readers — and the project's `guishot` test harness — get
//! nothing for free. This module exposes the retained [`Node`](super::node::Node)
//! arena to UI Automation: one lightweight COM provider per [`ControlId`] (plus a
//! synthetic provider per item of a SelectorBar / ComboBox / NavigationView),
//! mirroring the logical tree, reporting Name/AutomationId/ControlType/focus, and
//! translating Invoke/Toggle/Value/RangeValue/Selection/SelectionItem/
//! ExpandCollapse/Scroll calls into the **same** typed event dispatch a pointer
//! or keyboard interaction takes (the [`uia_*` action bridge](super::DCompBackend)
//! in `input.rs`).
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

use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::{Mutex, OnceLock};

use super::host;
use super::{caption, controls};
use super::{layout, scroll};
use super::*;
use crate::backend::{ControlKind, Event};
use crate::system_bindings::{
    ClientToScreen, ExpandCollapseState, IExpandCollapseProvider, IExpandCollapseProvider_Impl,
    IInvokeProvider, IInvokeProvider_Impl, IRangeValueProvider, IRangeValueProvider_Impl,
    IRawElementProviderFragment, IRawElementProviderFragmentRoot,
    IRawElementProviderFragmentRoot_Impl, IRawElementProviderFragment_Impl,
    IRawElementProviderAdviseEvents, IRawElementProviderAdviseEvents_Impl,
    IRawElementProviderSimple, IRawElementProviderSimple_Impl, IScrollItemProvider,
    IScrollItemProvider_Impl, IScrollProvider,
    IScrollProvider_Impl, ISelectionItemProvider, ISelectionItemProvider_Impl, ISelectionProvider,
    ISelectionProvider_Impl, IToggleProvider, IToggleProvider_Impl, IValueProvider,
    IValueProvider_Impl, IsZoomed, NavigateDirection, PostMessageW, ProviderOptions,
    ScreenToClient, ScrollAmount, ToggleState, UiaRect,
    UiaHostProviderFromHwnd, UiaRaiseAutomationEvent, UiaRaiseAutomationPropertyChangedEvent,
    HWND, LPARAM, POINT, SAFEARRAY, VARIANT, VARIANT_0,
    VARIANT_0_0, VARIANT_0_0_0, WM_SYSCOMMAND, WPARAM, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE,
    SC_RESTORE, UIA_AutomationFocusChangedEventId, UIA_AutomationIdPropertyId,
    UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
    UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ExpandCollapsePatternId,
    UIA_ExpandCollapseExpandCollapseStatePropertyId, UIA_GroupControlTypeId,
    UIA_HasKeyboardFocusPropertyId, UIA_HelpTextPropertyId, UIA_HyperlinkControlTypeId,
    UIA_ImageControlTypeId, UIA_InvokePatternId, UIA_Invoke_InvokedEventId,
    UIA_IsContentElementPropertyId,
    UIA_IsControlElementPropertyId, UIA_IsEnabledPropertyId, UIA_IsKeyboardFocusablePropertyId,
    UIA_IsOffscreenPropertyId, UIA_IsPasswordPropertyId,
    UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_NamePropertyId, UIA_PaneControlTypeId,
    PATTERNID, PROPERTYID, UIA_ProgressBarControlTypeId, UIA_RadioButtonControlTypeId,
    UIA_RangeValuePatternId, UIA_RangeValueValuePropertyId,
    UIA_ScrollItemPatternId, UIA_ScrollPatternId, UIA_SelectionItemPatternId,
    UIA_SelectionItem_ElementSelectedEventId, UIA_SelectionPatternId,
    UIA_SliderControlTypeId, UIA_TabControlTypeId,
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

// `ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading` — the
// options the canonical Microsoft server-side provider sample advertises.
const PROVIDER_OPTIONS_SERVER: ProviderOptions = 0x1 | 0x20;
// `UIA_E_ELEMENTNOTAVAILABLE` — returned when the node has gone (id reused/freed).
const UIA_E_ELEMENTNOTAVAILABLE: HRESULT = HRESULT(0x8004_0201u32 as i32);
// `UiaAppendRuntimeId` — first element of a fragment's runtime id.
const UIA_APPEND_RUNTIME_ID: i32 = 3;

/// Synthetic-item index space: container items use their natural 0-based
/// index; the fragment root's drawn caption buttons (min/max/close) live at
/// `CAPTION_ITEM_BASE + i` so they never collide with a root that is itself an
/// item container (e.g. a NavigationView shell as the app's top element).
const CAPTION_ITEM_BASE: i32 = 1 << 20;

fn is_caption(item: i32) -> bool {
    item >= CAPTION_ITEM_BASE
}

// `ScrollAmount` values + the Scroll pattern's "no scroll" percent sentinel
// (uiautomationcore.h).
const SCROLL_LARGE_DECREMENT: ScrollAmount = 0;
const SCROLL_SMALL_DECREMENT: ScrollAmount = 1;
const SCROLL_LARGE_INCREMENT: ScrollAmount = 3;
const SCROLL_SMALL_INCREMENT: ScrollAmount = 4;
const UIA_SCROLL_NO_SCROLL: f64 = -1.0;
/// One Scroll-pattern small step — matches the wheel detent in `on_wheel`.
const SCROLL_LINE: f32 = 48.0;

// VARENUM tags used when building property VARIANTs / provider arrays.
const VT_I4: u16 = 3;
const VT_R8: u16 = 5;
const VT_BSTR: u16 = 8;
const VT_BOOL: u16 = 11;
const VT_UNKNOWN: u16 = 13;

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
        Button | RepeatButton | DropDownButton | SplitButton | ToggleSwitch => {
            UIA_ButtonControlTypeId
        }
        HyperlinkButton => UIA_HyperlinkControlTypeId,
        CheckBox | ToggleButton => UIA_CheckBoxControlTypeId,
        RadioButton => UIA_RadioButtonControlTypeId,
        TextBox | NumberBox | PasswordBox | AutoSuggestBox | RichEditBox => UIA_EditControlTypeId,
        Slider | Knob => UIA_SliderControlTypeId,
        ProgressBar | ProgressRing | Meter => UIA_ProgressBarControlTypeId,
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

fn pattern_supported(kind: ControlKind, item: i32, pid: PATTERNID) -> bool {
    use ControlKind::*;
    if item >= 0 {
        // Synthetic items select — and also invoke, so `uia:invoke;name=<label>`
        // scripts written against the old per-segment Buttons keep working.
        return pid == UIA_SelectionItemPatternId || pid == UIA_InvokePatternId;
    }
    if pid == UIA_InvokePatternId {
        matches!(kind, Button | RepeatButton | HyperlinkButton | DropDownButton | SplitButton)
    } else if pid == UIA_TogglePatternId {
        matches!(kind, ToggleSwitch | CheckBox | ToggleButton | RadioButton)
    } else if pid == UIA_ValuePatternId {
        matches!(kind, TextBox | NumberBox | PasswordBox | AutoSuggestBox)
    } else if pid == UIA_RangeValuePatternId {
        matches!(kind, Slider | Knob | NumberBox | ProgressBar | ProgressRing | Meter)
    } else if pid == UIA_ExpandCollapsePatternId {
        matches!(kind, Expander | ComboBox | DropDownButton | SplitButton)
    } else if pid == UIA_SelectionPatternId {
        is_item_container(kind)
    } else if pid == UIA_ScrollPatternId {
        matches!(kind, ScrollViewer | ScrollView)
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

    /// Drawn caption buttons under the fragment root (0 when the window has no
    /// custom caption).
    fn uia_caption_count(&self) -> i32 {
        if self.caption_rect().is_some() {
            3
        } else {
            0
        }
    }

    /// Caption button `i` (0=min, 1=max, 2=close)'s rect in window DIPs — the
    /// buttons fill the right end of the caption strip, each [`caption::BTN_W`]
    /// wide.
    fn uia_caption_button(&self, i: i32) -> Option<(f32, f32, f32, f32)> {
        if !(0..3).contains(&i) {
            return None;
        }
        let (cx, cy, cw, ch) = self.caption_rect()?;
        Some((cx + cw - (3 - i) as f32 * caption::BTN_W, cy, caption::BTN_W, ch))
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

    /// Step through the logical tree. A container's synthetic items form a
    /// *prefix* of one combined child sequence `[item 0 … item n-1, child 0 …
    /// child m-1]`, so a tree walk (screen reader, client `FindAll`) continues
    /// from the last item into content hosted inside the container — e.g. a
    /// NavigationView's body pane — instead of dead-ending on the items.
    pub(crate) fn uia_navigate(&self, id: ControlId, item: i32, dir: NavigateDirection) -> UiaNav {
        let Some(node) = self.arena.get(id) else {
            return UiaNav::None;
        };

        // Synthetic item: parent is the container; the sibling after the last
        // item is the container's first real child.
        // Caption buttons: a synthetic suffix after the root's real children.
        if is_caption(item) {
            let i = item - CAPTION_ITEM_BASE;
            return match dir {
                NAV_PARENT => UiaNav::Root,
                NAV_NEXT if i + 1 < self.uia_caption_count() => {
                    UiaNav::Item(id, CAPTION_ITEM_BASE + i + 1)
                }
                NAV_PREV if i > 0 => UiaNav::Item(id, CAPTION_ITEM_BASE + i - 1),
                NAV_PREV => match node.children.last() {
                    Some(c) => UiaNav::Node(*c),
                    None => match self.uia_item_count(id) {
                        0 => UiaNav::None,
                        n => UiaNav::Item(id, n - 1),
                    },
                },
                _ => UiaNav::None,
            };
        }

        if item >= 0 {
            let count = self.uia_item_count(id);
            return match dir {
                NAV_PARENT if self.root == Some(id) => UiaNav::Root,
                NAV_PARENT => UiaNav::Node(id),
                NAV_NEXT if item + 1 < count => UiaNav::Item(id, item + 1),
                NAV_NEXT => match node.children.first() {
                    Some(c) => UiaNav::Node(*c),
                    None if self.root == Some(id) && self.uia_caption_count() > 0 => {
                        UiaNav::Item(id, CAPTION_ITEM_BASE)
                    }
                    None => UiaNav::None,
                },
                NAV_PREV if item > 0 => UiaNav::Item(id, item - 1),
                _ => UiaNav::None,
            };
        }

        let item_count = self.uia_item_count(id);
        let caption_count = if self.root == Some(id) { self.uia_caption_count() } else { 0 };
        match dir {
            NAV_FIRST => {
                if item_count > 0 {
                    UiaNav::Item(id, 0)
                } else if let Some(c) = node.children.first() {
                    UiaNav::Node(*c)
                } else if caption_count > 0 {
                    UiaNav::Item(id, CAPTION_ITEM_BASE)
                } else {
                    UiaNav::None
                }
            }
            NAV_LAST => {
                if caption_count > 0 {
                    UiaNav::Item(id, CAPTION_ITEM_BASE + caption_count - 1)
                } else {
                    match node.children.last() {
                        Some(c) => UiaNav::Node(*c),
                        None if item_count > 0 => UiaNav::Item(id, item_count - 1),
                        None => UiaNav::None,
                    }
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
                if dir == NAV_NEXT {
                    match pn.children.get(idx + 1) {
                        Some(c) => UiaNav::Node(*c),
                        // Last real child of the root: the caption suffix follows.
                        None if self.root == Some(p) && self.uia_caption_count() > 0 => {
                            UiaNav::Item(p, CAPTION_ITEM_BASE)
                        }
                        None => UiaNav::None,
                    }
                } else if idx > 0 {
                    UiaNav::Node(pn.children[idx - 1])
                } else {
                    // First real child: preceded by the parent's last item.
                    match self.uia_item_count(p) {
                        0 => UiaNav::None,
                        n => UiaNav::Item(p, n - 1),
                    }
                }
            }
            _ => UiaNav::None,
        }
    }

    /// Accessible name: explicit AutomationName, else the visible label/text.
    fn uia_name(&self, id: ControlId, item: i32) -> String {
        if is_caption(item) {
            return match item - CAPTION_ITEM_BASE {
                0 => "Minimize",
                1 if caption::maximized() => "Restore",
                1 => "Maximize",
                _ => "Close",
            }
            .to_string();
        }
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
        if is_caption(item) {
            return UIA_ButtonControlTypeId;
        }
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
            // Caption buttons are pointer-only (Alt+Space serves the keyboard).
            return !is_caption(item);
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
        pid: PATTERNID,
    ) -> bool {
        if is_caption(item) {
            return pid == UIA_InvokePatternId; // caption buttons only invoke
        }
        if pid == UIA_ScrollItemPatternId {
            return self.uia_scroll_ancestor(id).is_some();
        }
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
        let readonly = matches!(
            n.kind,
            ControlKind::ProgressBar | ControlKind::ProgressRing | ControlKind::Meter
        );
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

    /// The selected item index of container `id`, or `None` when the index is
    /// out of range (or the node is not an item container).
    fn uia_selected_item(&self, id: ControlId) -> Option<i32> {
        let n = self.arena.get(id)?;
        if !is_item_container(n.kind) {
            return None;
        }
        let i = n.ctrl.selected_index;
        (0..n.ctrl.items.len() as i32).contains(&i).then_some(i)
    }

    /// `(offset, viewport height, content height)` in DIPs for a scroll
    /// container.
    fn uia_scroll_info(&self, id: ControlId) -> Option<(f32, f32, f32)> {
        let n = self.arena.get(id)?;
        n.is_scroll().then(|| (n.scroll_off, n.rect.h, n.ctrl.content_h))
    }

    /// UIA `Scroll`/`SetScrollPercent`: glide scroll container `id` to logical
    /// offset `off` (DIPs of content above the viewport), clamped to range —
    /// the same compositor glide + thumb sync a wheel detent takes (`on_wheel`).
    fn uia_scroll_to(&mut self, id: ControlId, off: f32) {
        let scale = self.scale();
        let Some(n) = self.arena.get_mut(id) else {
            return;
        };
        if !n.is_scroll() {
            return;
        }
        let max = (n.ctrl.content_h - n.rect.h).max(0.0);
        let target = layout::snap(off.clamp(0.0, max), scale);
        n.scroll_off = target;
        n.scroll_glide(target);
        let g = scroll::thumb_geom(n.rect.h, n.ctrl.content_h, target);
        let tx = n.rect.w - scroll::THUMB_W - scroll::THUMB_MARGIN;
        n.thumb_glide(tx, g.thumb_y);
    }

    /// Ancestors of `target`, nearest-first (root last); empty when `target`
    /// is the root or unreachable.
    fn uia_ancestors(&self, target: ControlId) -> Vec<ControlId> {
        fn rec(
            b: &DCompBackend,
            cur: ControlId,
            target: ControlId,
            path: &mut Vec<ControlId>,
        ) -> bool {
            if cur == target {
                return true;
            }
            path.push(cur);
            if let Some(n) = b.arena.get(cur) {
                for c in &n.children {
                    if rec(b, *c, target, path) {
                        return true;
                    }
                }
            }
            path.pop();
            false
        }
        let mut path = Vec::new();
        match self.root {
            Some(r) if rec(self, r, target, &mut path) => {
                path.reverse();
                path
            }
            _ => Vec::new(),
        }
    }

    /// Total DIPs `id` is shifted up by ancestor scroll offsets. Layout rects
    /// are stored unscrolled (the content-carrier visual applies the shift),
    /// so every screen-space answer must subtract this.
    fn uia_scroll_adjust(&self, id: ControlId) -> f32 {
        self.uia_ancestors(id)
            .iter()
            .filter_map(|a| self.arena.get(*a))
            .filter(|n| n.is_scroll())
            .map(|n| n.scroll_off)
            .sum()
    }

    /// The nearest scroll-container ancestor of `id`.
    fn uia_scroll_ancestor(&self, id: ControlId) -> Option<ControlId> {
        self.uia_ancestors(id)
            .into_iter()
            .find(|a| self.arena.get(*a).is_some_and(|n| n.is_scroll()))
    }

    /// Whether `id` is currently invisible: zero-area layout (a collapsed
    /// Expander body is `Display::None`) or scrolled fully outside an ancestor
    /// scroll container's viewport.
    fn uia_is_offscreen(&self, id: ControlId, item: i32) -> bool {
        if item >= 0 {
            return false; // items and caption buttons track their container
        }
        let Some(n) = self.arena.get(id) else {
            return true;
        };
        if n.rect.w <= 0.0 || n.rect.h <= 0.0 {
            return true;
        }
        let (mut top, mut bot) = (n.rect.y, n.rect.y + n.rect.h);
        for a in self.uia_ancestors(id) {
            if let Some(an) = self.arena.get(a)
                && an.is_scroll()
            {
                top -= an.scroll_off;
                bot -= an.scroll_off;
                if bot <= an.rect.y || top >= an.rect.y + an.rect.h {
                    return true;
                }
            }
        }
        false
    }

    /// UIA `ScrollIntoView`: scroll the nearest scroll ancestor the minimum
    /// distance that brings `id` (or its item) fully into the viewport.
    fn uia_scroll_into_view(&mut self, id: ControlId, item: i32) {
        let Some(sv) = self.uia_scroll_ancestor(id) else {
            return;
        };
        let Some((mut ny, mut nh)) = self.arena.get(id).map(|n| (n.rect.y, n.rect.h)) else {
            return;
        };
        if item >= 0
            && let Some(n) = self.arena.get(id)
            && n.kind == ControlKind::NavigationView
        {
            ny += controls::NAV_ITEM_H * item as f32;
            nh = controls::NAV_ITEM_H;
        }
        let Some((vy, vh, off)) = self.arena.get(sv).map(|n| (n.rect.y, n.rect.h, n.scroll_off))
        else {
            return;
        };
        // Both rects are unscrolled layout coordinates, so their difference is
        // the target's offset within the scrolled content.
        let content_y = ny - vy;
        if content_y >= off && content_y + nh <= off + vh {
            return; // already fully visible
        }
        let target = if content_y < off { content_y } else { content_y + nh - vh };
        self.uia_scroll_to(sv, target);
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
        if is_caption(item) {
            let (bx, by, bw, bh) = self.uia_caption_button(item - CAPTION_ITEM_BASE)?;
            return Some(self.uia_screen_rect(bx, by, bw, bh));
        }
        let n = self.arena.get(id)?;
        let (mut x, mut y, mut w, mut h) = (n.rect.x, n.rect.y, n.rect.w, n.rect.h);
        if item >= 0 {
            match n.kind {
                ControlKind::SelectorBar => {
                    let edges = controls::segment_edges(n);
                    if let (Some(&l), Some(&r)) =
                        (edges.get(item as usize), edges.get(item as usize + 1))
                    {
                        x = n.rect.x + l;
                        w = r - l;
                    }
                }
                ControlKind::NavigationView => {
                    y = n.rect.y + controls::NAV_ITEM_H * item as f32;
                    h = controls::NAV_ITEM_H;
                }
                _ => {} // ComboBox items live in a popup; report the field's box.
            }
        }
        y -= self.uia_scroll_adjust(id);
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
        // The caption cluster overlays the content; test its buttons first.
        if let (Some(root), Some((bx0, cy, _, ch))) = (self.root, self.uia_caption_button(0))
            && py >= cy
            && py < cy + ch
            && px >= bx0
        {
            let i = ((px - bx0) / caption::BTN_W) as i32;
            if (0..3).contains(&i) {
                return UiaNav::Item(root, CAPTION_ITEM_BASE + i);
            }
        }
        fn rec(b: &DCompBackend, id: ControlId, px: f32, py: f32) -> Option<ControlId> {
            let n = b.arena.get(id)?;
            if !n.rect.contains(px, py) {
                return None;
            }
            // Descend in content space: a scroll container's children are laid
            // out unscrolled (mirrors the pointer path's `surface_walk`).
            let cy = if n.is_scroll() { py + n.scroll_off } else { py };
            for c in &n.children {
                if let Some(found) = rec(b, *c, px, cy) {
                    return Some(found);
                }
            }
            Some(id)
        }
        match self.root.and_then(|r| rec(self, r, px, py)) {
            Some(id) if self.root == Some(id) => UiaNav::Root,
            Some(id) => match self.uia_item_at(id, px, py) {
                Some(i) => UiaNav::Item(id, i),
                None => UiaNav::Node(id),
            },
            None => UiaNav::Root,
        }
    }

    /// The synthetic item of container `id` under window-relative DIP point
    /// `(px, py)`, mirroring the pointer hit geometry in `input.rs`. `None` when
    /// the point misses the item area or the container hosts its items in a
    /// popup (ComboBox).
    fn uia_item_at(&self, id: ControlId, px: f32, py: f32) -> Option<i32> {
        // Window-space → the container's unscrolled layout space.
        let py = py + self.uia_scroll_adjust(id);
        let n = self.arena.get(id)?;
        let count = n.ctrl.items.len();
        if count == 0 {
            return None;
        }
        match n.kind {
            ControlKind::SelectorBar => {
                let edges = controls::segment_edges(n);
                let rel = px - n.rect.x;
                Some(edges[1..count].iter().take_while(|&&e| rel >= e).count() as i32)
            }
            ControlKind::NavigationView => {
                // Items occupy only the top of the rail column; elsewhere the
                // point belongs to the container (the body pane is a real child
                // and was already tried by the recursion above).
                let (rx, ry) = (px - n.rect.x, py - n.rect.y);
                let i = (ry / controls::NAV_ITEM_H).floor() as i32;
                (rx < theme::NAV_RAIL_W && (0..count as i32).contains(&i)).then_some(i)
            }
            _ => None,
        }
    }

    /// Raise an `AutomationFocusChanged` event for `id` — deferred onto the pump
    /// so it never runs inside an input borrow, and a no-op when no client is
    /// listening (idle cost stays zero). Called on the UI thread from `set_focus`.
    pub(crate) fn uia_raise_focus(&self, id: ControlId) {
        if !clients_listening() {
            return;
        }
        let hwnd = self.hwnd;
        host::post_ui(hwnd, move || {
            let provider = stable_provider(ElementProvider::element(hwnd, id));
            unsafe {
                let _ = UiaRaiseAutomationEvent(provider.as_raw(), UIA_AutomationFocusChangedEventId);
            }
        });
    }

    // ── State-change notifications ───────────────────────────────────────────
    //
    // Called from the `fire_*` event-dispatch choke points in `input.rs`, so a
    // pointer, keyboard, or UIA-initiated change announces identically to
    // screen readers. Gated on a listening client (zero idle cost) and
    // deferred through the pump so raising never re-enters the input borrow.

    pub(crate) fn uia_notify_bool(&self, id: ControlId, event: Event, v: bool) {
        if !clients_listening() {
            return;
        }
        let state = i32::from(v);
        match event {
            Event::Toggled | Event::Checked => raise_property_changed(
                self.hwnd,
                id,
                UIA_ToggleToggleStatePropertyId,
                PropVal::I4(state),
            ),
            Event::Expanding => raise_property_changed(
                self.hwnd,
                id,
                UIA_ExpandCollapseExpandCollapseStatePropertyId,
                PropVal::I4(state),
            ),
            _ => {}
        }
    }

    pub(crate) fn uia_notify_f64(&self, id: ControlId, event: Event, v: f64) {
        if !clients_listening() {
            return;
        }
        if matches!(event, Event::ValueChanged) {
            raise_property_changed(self.hwnd, id, UIA_RangeValueValuePropertyId, PropVal::R8(v));
        }
    }

    pub(crate) fn uia_notify_string(&self, id: ControlId, event: Event, v: &str) {
        if !clients_listening() {
            return;
        }
        match event {
            Event::SelectionChanged => {
                let Some(i) = self.uia_selected_item(id) else {
                    return;
                };
                let hwnd = self.hwnd;
                host::post_ui(hwnd, move || {
                    let p = stable_provider(ElementProvider::item(hwnd, id, i));
                    unsafe {
                        let _ = UiaRaiseAutomationEvent(
                            p.as_raw(),
                            UIA_SelectionItem_ElementSelectedEventId,
                        );
                    }
                });
            }
            // PasswordChanged is deliberately not announced.
            Event::TextChanged => raise_property_changed(
                self.hwnd,
                id,
                UIA_ValueValuePropertyId,
                PropVal::Bstr(v.to_string()),
            ),
            _ => {}
        }
    }
}

fn clients_listening() -> bool {
    unsafe { UiaClientsAreListening() }.as_bool()
}

/// A property's new value as plain `Send` data; the VARIANT is built on the UI
/// thread inside the deferred raise.
enum PropVal {
    I4(i32),
    R8(f64),
    Bstr(String),
}

/// Raise `AutomationPropertyChanged` for node `id` — deferred onto the pump.
/// The old value is reported empty (permitted by the pattern contracts).
fn raise_property_changed(hwnd: isize, id: ControlId, pid: PROPERTYID, val: PropVal) {
    host::post_ui(hwnd, move || {
        let provider = stable_provider(ElementProvider::element(hwnd, id));
        // For a BSTR value the VARIANT holds a non-owning alias; `_owner` keeps
        // the string alive across the synchronous raise (UIA deep-copies it),
        // then frees it — a by-value VARIANT is never dropped by anyone else.
        let mut _owner: Option<BSTR> = None;
        let newv = match val {
            PropVal::I4(v) => v_i4(v),
            PropVal::R8(v) => v_r8(v),
            PropVal::Bstr(s) => {
                let b = BSTR::from(s.as_str());
                let v = make_variant(
                    VT_BSTR,
                    VARIANT_0_0_0 {
                        bstrVal: ManuallyDrop::new(unsafe { core::mem::transmute_copy(&b) }),
                    },
                );
                _owner = Some(b);
                v
            }
        };
        unsafe {
            let _ = UiaRaiseAutomationPropertyChangedEvent(
                provider.as_raw(),
                pid,
                VARIANT::default(),
                newv,
            );
        }
    });
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
        UiaNav::Node(c) => stable_fragment(ElementProvider::element(hwnd, c)),
        UiaNav::Item(c, i) => stable_fragment(ElementProvider::item(hwnd, c, i)),
        UiaNav::Root => {
            let rid = root_id(hwnd).ok_or_else(not_available)?;
            stable_fragment(ElementProvider::root(hwnd, rid))
        }
    }
}

/// The window's root UI Automation provider (a fragment root for the reactor
/// root node). Returned from `WM_GETOBJECT`.
pub(crate) fn root_provider(hwnd: isize, root: ControlId) -> IRawElementProviderSimple {
    stable_provider(ElementProvider::root(hwnd, root))
}

// ── Stable provider identity ─────────────────────────────────────────────────
//
// UI Automation correlates elements across queries — and, critically, matches
// raised events to registered listeners — partly by the provider's COM object
// identity, not only its runtime id (see the "map of the providers that have
// raised events" in the server-side provider docs). Handing UIA a throwaway
// object per query breaks that correlation, so events raised on a fresh object
// are silently dropped even though the raise returns S_OK. We therefore mint one
// object per element identity and return that same object every time.
//
// The objects carry only plain data and marshal all real work to the UI thread,
// and `implement_decl!` makes them agile (they answer `IAgileObject`/`IMarshal`),
// so a single instance is safely shared across UIA's worker threads. The cache
// is grow-only for the process; each entry is a few words and UIA holds its own
// references besides.

/// An agile provider object, sendable because it is callable from any apartment.
struct SendProvider(IRawElementProviderSimple);
// SAFETY: `implement_decl!` providers are agile (free-threaded marshaler); the
// wrapped object may be AddRef'd/called from any thread.
unsafe impl Send for SendProvider {}

type ProviderKey = (isize, u32, i32, bool);

fn provider_cache() -> &'static Mutex<HashMap<ProviderKey, SendProvider>> {
    static CACHE: OnceLock<Mutex<HashMap<ProviderKey, SendProvider>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The one stable provider object for `p`'s element identity, created on first
/// use and reused thereafter.
fn stable_provider(p: ElementProvider) -> IRawElementProviderSimple {
    let key = (p.hwnd, p.id.get(), p.item, p.is_root);
    let mut cache = provider_cache().lock().unwrap();
    cache
        .entry(key)
        .or_insert_with(|| {
            // Only the true root is a fragment root (see the type split above).
            let obj: IRawElementProviderSimple =
                if p.is_root { RootProvider(p).into() } else { p.into() };
            SendProvider(obj)
        })
        .0
        .clone()
}

/// The stable provider for `p`, viewed as a fragment (same object).
fn stable_fragment(p: ElementProvider) -> Result<IRawElementProviderFragment> {
    stable_provider(p).cast()
}

// ── The provider object ──────────────────────────────────────────────────────

/// One UI Automation provider: a value object identifying an arena node (or one
/// synthetic item of a container). Agile; the COM object wrapping it is minted
/// once per identity and cached (see [`stable_provider`]), and all real work
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
    fn property(&self, pid: PROPERTYID) -> VARIANT {
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
        } else if pid == UIA_IsOffscreenPropertyId {
            v_bool(on_backend(hwnd, move |b| b.uia_is_offscreen(id, item)).unwrap_or(false))
        } else if pid == UIA_IsPasswordPropertyId {
            let kind = on_backend(hwnd, move |b| b.uia_kind(id)).flatten();
            v_bool(kind == Some(ControlKind::PasswordBox))
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

// Two COM object types share one data struct and one body of logic:
//
// * `ElementProvider` — a NON-root fragment element. Implements `…Fragment` +
//   the control patterns.
// * `RootProvider` — the window's fragment ROOT. Additionally implements
//   `…FragmentRoot` and `…AdviseEvents`.
//
// The split matters for events: a fragment root is an event-scope boundary. If
// every element answered `QueryInterface` for `IRawElementProviderFragmentRoot`
// (as a single all-interfaces object would), UIA treats each as its own
// fragment, and a subtree subscription rooted at the window never matches an
// event raised inside — so nothing is delivered. Only the true root is a
// fragment root, mirroring the canonical server-side provider sample.
implement_decl! {
    impl ElementProvider as ElementProvider_Impl: [
        IRawElementProviderSimple,
        IRawElementProviderFragment,
        IInvokeProvider,
        IToggleProvider,
        IValueProvider,
        IRangeValueProvider,
        ISelectionProvider,
        ISelectionItemProvider,
        IExpandCollapseProvider,
        IScrollProvider,
        IScrollItemProvider
    ]
}

implement_decl! {
    impl RootProvider as RootProvider_Impl: [
        IRawElementProviderSimple,
        IRawElementProviderFragment,
        IRawElementProviderFragmentRoot,
        IInvokeProvider,
        IToggleProvider,
        IValueProvider,
        IRangeValueProvider,
        ISelectionProvider,
        ISelectionItemProvider,
        IExpandCollapseProvider,
        IScrollProvider,
        IScrollItemProvider,
        IRawElementProviderAdviseEvents
    ]
}

/// The window's fragment root: an [`ElementProvider`] that additionally exposes
/// fragment-root and event-advise interfaces.
struct RootProvider(ElementProvider);

impl ElementProvider {
    /// The shared element data (identity for the forwarding impls).
    fn inner(&self) -> &ElementProvider {
        self
    }
}

impl RootProvider {
    fn inner(&self) -> &ElementProvider {
        &self.0
    }
}

// Emit the shared provider interfaces (everything except the root-only ones) for
// a given `_Impl` type; every method forwards to the shared logic on the inner
// `ElementProvider`.
macro_rules! forward_provider {
    ($imp:ty) => {
        impl IRawElementProviderSimple_Impl for $imp {
            fn ProviderOptions(&self) -> Result<ProviderOptions> {
                self.inner().provider_options()
            }
            fn GetPatternProvider(&self, patternid: PATTERNID) -> Result<IUnknown> {
                self.inner().pattern_provider(patternid)
            }
            fn GetPropertyValue(&self, propertyid: PROPERTYID) -> Result<VARIANT> {
                Ok(self.inner().property(propertyid))
            }
            fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
                self.inner().host_element_provider()
            }
        }
        impl IRawElementProviderFragment_Impl for $imp {
            fn Navigate(
                &self,
                direction: NavigateDirection,
            ) -> Result<IRawElementProviderFragment> {
                self.inner().navigate(direction)
            }
            fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
                self.inner().runtime_id()
            }
            fn get_BoundingRectangle(&self) -> Result<UiaRect> {
                self.inner().bounding_rect()
            }
            fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
                Ok(core::ptr::null_mut())
            }
            fn SetFocus(&self) -> Result<()> {
                self.inner().set_focus_node()
            }
            fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
                self.inner().fragment_root_provider()
            }
        }
        impl IInvokeProvider_Impl for $imp {
            fn Invoke(&self) -> Result<()> {
                self.inner().invoke()
            }
        }
        impl IToggleProvider_Impl for $imp {
            fn Toggle(&self) -> Result<()> {
                self.inner().toggle()
            }
            fn ToggleState(&self) -> Result<ToggleState> {
                self.inner().toggle_state()
            }
        }
        impl IValueProvider_Impl for $imp {
            fn SetValue(&self, val: &PCWSTR) -> Result<()> {
                self.inner().value_set(val)
            }
            fn Value(&self) -> Result<BSTR> {
                self.inner().value_get()
            }
            fn IsReadOnly(&self) -> Result<BOOL> {
                Ok(false.into())
            }
        }
        impl IRangeValueProvider_Impl for $imp {
            fn SetValue(&self, val: f64) -> Result<()> {
                self.inner().range_set(val)
            }
            fn Value(&self) -> Result<f64> {
                self.inner().range_get()
            }
            fn IsReadOnly(&self) -> Result<BOOL> {
                self.inner().range_readonly()
            }
            fn Maximum(&self) -> Result<f64> {
                self.inner().range_max()
            }
            fn Minimum(&self) -> Result<f64> {
                self.inner().range_min()
            }
            fn LargeChange(&self) -> Result<f64> {
                self.inner().range_large()
            }
            fn SmallChange(&self) -> Result<f64> {
                self.inner().range_small()
            }
        }
        impl ISelectionProvider_Impl for $imp {
            fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
                self.inner().selection_get()
            }
            fn CanSelectMultiple(&self) -> Result<BOOL> {
                Ok(false.into())
            }
            fn IsSelectionRequired(&self) -> Result<BOOL> {
                Ok(true.into())
            }
        }
        impl ISelectionItemProvider_Impl for $imp {
            fn Select(&self) -> Result<()> {
                self.inner().select_item()
            }
            fn AddToSelection(&self) -> Result<()> {
                self.inner().select_item()
            }
            fn RemoveFromSelection(&self) -> Result<()> {
                Ok(())
            }
            fn IsSelected(&self) -> Result<BOOL> {
                self.inner().is_selected()
            }
            fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> {
                self.inner().selection_container()
            }
        }
        impl IExpandCollapseProvider_Impl for $imp {
            fn Expand(&self) -> Result<()> {
                self.inner().expand()
            }
            fn Collapse(&self) -> Result<()> {
                self.inner().collapse()
            }
            fn ExpandCollapseState(&self) -> Result<ExpandCollapseState> {
                self.inner().expand_state()
            }
        }
        impl IScrollProvider_Impl for $imp {
            fn Scroll(&self, _h: ScrollAmount, v: ScrollAmount) -> Result<()> {
                self.inner().scroll(v)
            }
            fn SetScrollPercent(&self, _h: f64, v: f64) -> Result<()> {
                self.inner().scroll_set_percent(v)
            }
            fn HorizontalScrollPercent(&self) -> Result<f64> {
                Ok(UIA_SCROLL_NO_SCROLL)
            }
            fn VerticalScrollPercent(&self) -> Result<f64> {
                self.inner().scroll_v_percent()
            }
            fn HorizontalViewSize(&self) -> Result<f64> {
                Ok(100.0)
            }
            fn VerticalViewSize(&self) -> Result<f64> {
                self.inner().scroll_v_view()
            }
            fn HorizontallyScrollable(&self) -> Result<BOOL> {
                Ok(false.into())
            }
            fn VerticallyScrollable(&self) -> Result<BOOL> {
                self.inner().scroll_v_able()
            }
        }
        impl IScrollItemProvider_Impl for $imp {
            fn ScrollIntoView(&self) -> Result<()> {
                self.inner().scroll_into_view()
            }
        }
    };
}

forward_provider!(ElementProvider_Impl);
forward_provider!(RootProvider_Impl);

// Root-only interfaces.
impl IRawElementProviderFragmentRoot_Impl for RootProvider_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        self.inner().provider_from_point(x, y)
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        self.inner().focus_provider()
    }
}

impl IRawElementProviderAdviseEvents_Impl for RootProvider_Impl {
    fn AdviseEventAdded(
        &self,
        _eventid: crate::system_bindings::EVENTID,
        _propertyids: *const SAFEARRAY,
    ) -> Result<()> {
        // We use `UiaClientsAreListening` to gate raises, so no bookkeeping is
        // needed here; implementing the interface tells UIA the root wants events.
        Ok(())
    }
    fn AdviseEventRemoved(
        &self,
        _eventid: crate::system_bindings::EVENTID,
        _propertyids: *const SAFEARRAY,
    ) -> Result<()> {
        Ok(())
    }
}

// Shared provider logic (identity + patterns), called by both `ElementProvider`
// and `RootProvider` through the `forward_provider!` impls.
impl ElementProvider {
    fn provider_options(&self) -> Result<ProviderOptions> {
        Ok(PROVIDER_OPTIONS_SERVER)
    }

    fn pattern_provider(&self, patternid: PATTERNID) -> Result<IUnknown> {
        let (id, item) = (self.id, self.item);
        let supported = get(self.hwnd, move |b| Some(b.uia_pattern_supported(id, item, patternid)))?;
        if supported {
            // The same stable object answers pattern QIs (it implements them all).
            stable_provider(self.dup()).cast()
        } else {
            Err(Error::empty()) // S_OK + null: pattern not supported
        }
    }

    fn host_element_provider(&self) -> Result<IRawElementProviderSimple> {
        if self.is_root {
            host_provider(self.hwnd)
        } else {
            Err(Error::empty()) // S_OK + null: only the root merges with the host
        }
    }

    fn navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let (id, item) = (self.id, self.item);
        let nav = get(self.hwnd, move |b| Some(b.uia_navigate(id, item, direction)))?;
        nav_provider(self.hwnd, nav)
    }

    fn runtime_id(&self) -> Result<*mut SAFEARRAY> {
        if self.is_root {
            // A fragment ROOT must return null: UIA derives its identity from
            // the host HWND, which lets client subscriptions taken on the
            // window element scope-match events raised from inside the
            // fragment.
            return Ok(core::ptr::null_mut());
        }
        Ok(make_runtime_id(self.id, self.item))
    }

    fn bounding_rect(&self) -> Result<UiaRect> {
        let (id, item) = (self.id, self.item);
        let (left, top, width, height) = get(self.hwnd, move |b| b.uia_bounding_rect(id, item))?;
        Ok(UiaRect { left, top, width, height })
    }

    fn set_focus_node(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_focus_node(id));
        Ok(())
    }

    fn fragment_root_provider(&self) -> Result<IRawElementProviderFragmentRoot> {
        let rid = root_id(self.hwnd).ok_or_else(not_available)?;
        stable_provider(ElementProvider::root(self.hwnd, rid)).cast()
    }

    fn provider_from_point(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let nav = get(self.hwnd, move |b| Some(b.uia_element_from_point(x, y)))?;
        nav_provider(self.hwnd, nav)
    }

    fn focus_provider(&self) -> Result<IRawElementProviderFragment> {
        let nav = get(self.hwnd, |b| Some(b.uia_focus()))?;
        nav_provider(self.hwnd, nav)
    }

    fn invoke(&self) -> Result<()> {
        let (id, item) = (self.id, self.item);
        if is_caption(item) {
            // Caption button: post the matching system command. `IsZoomed` is
            // callable from this (UIA worker) thread, unlike the caption's
            // UI-thread hover state.
            let cmd = match item - CAPTION_ITEM_BASE {
                0 => SC_MINIMIZE,
                1 if unsafe { IsZoomed(self.hwnd as HWND) }.as_bool() => SC_RESTORE,
                1 => SC_MAXIMIZE,
                _ => SC_CLOSE,
            };
            unsafe {
                let _ = PostMessageW(self.hwnd as HWND, WM_SYSCOMMAND, cmd as WPARAM, 0 as LPARAM);
            }
            return Ok(());
        }
        if item >= 0 {
            // Invoking a synthetic container item selects it.
            act(self.hwnd, move |b| b.uia_select_item(id, item));
        } else {
            act(self.hwnd, move |b| b.uia_activate(id));
        }
        // Clients (e.g. guishot's `uia:invoke`) may wait on the Invoked event.
        let me = self.dup();
        host::post_ui(self.hwnd, move || {
            let p = stable_provider(me);
            unsafe {
                let _ = UiaRaiseAutomationEvent(p.as_raw(), UIA_Invoke_InvokedEventId);
            }
        });
        Ok(())
    }
}

impl ElementProvider {
    fn toggle(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_activate(id));
        Ok(())
    }

    fn toggle_state(&self) -> Result<ToggleState> {
        let id = self.id;
        get(self.hwnd, move |b| Some(b.uia_toggle_state(id)))
    }

    fn value_set(&self, val: &PCWSTR) -> Result<()> {
        let s = unsafe { val.to_string() }.unwrap_or_default();
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_text(id, &s));
        Ok(())
    }

    fn value_get(&self) -> Result<BSTR> {
        let id = self.id;
        Ok(BSTR::from(get(self.hwnd, move |b| Some(b.uia_value_string(id)))?))
    }

    fn range_set(&self, val: f64) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_range(id, val));
        Ok(())
    }

    fn range_get(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.0)
    }

    fn range_readonly(&self) -> Result<BOOL> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.3.into())
    }

    fn range_max(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.2)
    }

    fn range_min(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.1)
    }

    fn range_large(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.4 * 10.0)
    }

    fn range_small(&self) -> Result<f64> {
        let id = self.id;
        Ok(get(self.hwnd, move |b| b.uia_range(id))?.4)
    }
}

impl ElementProvider {
    /// The selected item, as a one-element `VT_UNKNOWN` array of its provider
    /// (`S_OK` + null for an empty selection, per the pattern contract).
    fn selection_get(&self) -> Result<*mut SAFEARRAY> {
        let id = self.id;
        let Some(i) = on_backend(self.hwnd, move |b| b.uia_selected_item(id)).flatten() else {
            return Ok(core::ptr::null_mut());
        };
        let sel = stable_provider(ElementProvider::item(self.hwnd, id, i));
        unsafe {
            let psa = SafeArrayCreateVector(VT_UNKNOWN, 0, 1);
            if !psa.is_null() {
                // SafeArrayPutElement AddRefs the interface pointer.
                let idx = 0i32;
                let _ = SafeArrayPutElement(psa, &idx, sel.as_raw());
            }
            Ok(psa)
        }
    }

    fn select_item(&self) -> Result<()> {
        let (id, item) = (self.id, self.item);
        act(self.hwnd, move |b| b.uia_select_item(id, item));
        Ok(())
    }

    fn is_selected(&self) -> Result<BOOL> {
        let (id, item) = (self.id, self.item);
        Ok(get(self.hwnd, move |b| Some(b.uia_item_selected(id, item)))?.into())
    }

    fn selection_container(&self) -> Result<IRawElementProviderSimple> {
        Ok(stable_provider(ElementProvider::element(self.hwnd, self.id)))
    }
}

impl ElementProvider {
    fn scroll(&self, v: ScrollAmount) -> Result<()> {
        let id = self.id;
        let (off, viewport, _) = get(self.hwnd, move |b| b.uia_scroll_info(id))?;
        let delta = match v {
            SCROLL_SMALL_DECREMENT => -SCROLL_LINE,
            SCROLL_SMALL_INCREMENT => SCROLL_LINE,
            SCROLL_LARGE_DECREMENT => -viewport,
            SCROLL_LARGE_INCREMENT => viewport,
            _ => return Ok(()),
        };
        act(self.hwnd, move |b| b.uia_scroll_to(id, off + delta));
        Ok(())
    }

    fn scroll_set_percent(&self, v: f64) -> Result<()> {
        if v == UIA_SCROLL_NO_SCROLL {
            return Ok(());
        }
        let id = self.id;
        let (_, viewport, content) = get(self.hwnd, move |b| b.uia_scroll_info(id))?;
        let max = (content - viewport).max(0.0);
        let off = (v.clamp(0.0, 100.0) as f32 / 100.0) * max;
        act(self.hwnd, move |b| b.uia_scroll_to(id, off));
        Ok(())
    }

    fn scroll_v_percent(&self) -> Result<f64> {
        let id = self.id;
        let (off, viewport, content) = get(self.hwnd, move |b| b.uia_scroll_info(id))?;
        let max = (content - viewport).max(0.0);
        Ok(if max > 0.0 { (off / max) as f64 * 100.0 } else { UIA_SCROLL_NO_SCROLL })
    }

    fn scroll_v_view(&self) -> Result<f64> {
        let id = self.id;
        let (_, viewport, content) = get(self.hwnd, move |b| b.uia_scroll_info(id))?;
        Ok(if content > viewport && content > 0.0 {
            (viewport / content) as f64 * 100.0
        } else {
            100.0
        })
    }

    fn scroll_v_able(&self) -> Result<BOOL> {
        let id = self.id;
        let (_, viewport, content) = get(self.hwnd, move |b| b.uia_scroll_info(id))?;
        Ok((content > viewport).into())
    }

    fn scroll_into_view(&self) -> Result<()> {
        let (id, item) = (self.id, self.item);
        act(self.hwnd, move |b| b.uia_scroll_into_view(id, item));
        Ok(())
    }

    fn expand(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_expanded(id, true));
        Ok(())
    }

    fn collapse(&self) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |b| b.uia_set_expanded(id, false));
        Ok(())
    }

    fn expand_state(&self) -> Result<ExpandCollapseState> {
        let id = self.id;
        get(self.hwnd, move |b| Some(b.uia_expand_state(id)))
    }
}
