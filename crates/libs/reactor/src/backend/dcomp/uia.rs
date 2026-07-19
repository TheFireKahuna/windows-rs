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
//! * **Provider data is plain; provider *objects* are interned.**
//!   [`ElementProvider`] carries only `Send` data (`hwnd`, `ControlId`, item
//!   index, root flag) and holds no arena state. The COM object wrapping that
//!   data is **not** recreated per query: UIA correlates elements — and matches
//!   raised events to registered listeners — partly by COM object identity, so
//!   [`stable_provider`] mints one object per element identity and hands the
//!   same object back on every query. Those objects live in a process-wide cache
//!   ([`provider_cache`]) keyed by `(hwnd, ControlId)` and then by
//!   `(item, is_root)`. The cache is **evicted by node lifetime**: `destroy`
//!   calls [`forget`], which drops the node's providers along with its `size` /
//!   `pointer` registrations. Idle cost is one small map entry per element a
//!   client has actually visited, and zero for a session no client ever queries.
//! * **Threading.** UIA calls arrive on UIA worker threads, but the arena is
//!   `!Send` and single-threaded. Every provider method that touches the arena
//!   marshals to the UI thread through [`host::marshal_to_ui`] (a blocking
//!   request/response the message pump services); calls already on the UI thread
//!   run inline. There is one action path and one arena owner.
//! * **Property batching.** A marshal is a cross-thread round trip, and a client
//!   tree walk asks for ~10 properties per element. `GetPropertyValue` therefore
//!   fills a whole [`PropSnapshot`] in **one** marshal and serves the rest of the
//!   burst from a per-UIA-thread cache of a single element. The cache is stamped
//!   with [`UIA_GEN`], a process-global counter the UI thread bumps on every
//!   mutation ([`note_tree_change`] / [`note_state_change`]), so a snapshot can
//!   never survive a change to the thing it describes. Values that move without
//!   a mutation hook — `IsOffscreen` (ancestor scroll offsets) and
//!   `BoundingRectangle` (layout) — are deliberately **not** in the snapshot and
//!   still marshal per call.

use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::{Mutex, OnceLock};

use super::host;
use super::input::HitKind;
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
    ITextProvider, ITextProvider_Impl, ITextRangeProvider, ITextRangeProvider_Impl,
    IScrollProvider_Impl, ISelectionItemProvider, ISelectionItemProvider_Impl, ISelectionProvider,
    ISelectionProvider_Impl, IToggleProvider, IToggleProvider_Impl, IValueProvider,
    IValueProvider_Impl, IsZoomed, NavigateDirection, PostMessageW, ProviderOptions,
    ScreenToClient, ScrollAmount, SupportedTextSelection, SupportedTextSelection_Single,
    TEXTATTRIBUTEID, TextPatternRangeEndpoint, TextPatternRangeEndpoint_Start, TextUnit,
    TextUnit_Character, TextUnit_Format, TextUnit_Word, ToggleState, UiaPoint, UiaRect,
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
    UIA_LiveRegionChangedEventId, UIA_LiveSettingPropertyId,
    UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_NamePropertyId, UIA_PaneControlTypeId,
    PATTERNID, PROPERTYID, UIA_ProgressBarControlTypeId, UIA_RadioButtonControlTypeId,
    UIA_RangeValuePatternId, UIA_RangeValueValuePropertyId,
    UIA_ScrollItemPatternId, UIA_ScrollPatternId, UIA_SelectionItemPatternId,
    UIA_SelectionItem_ElementSelectedEventId, UIA_SelectionPatternId,
    UIA_SliderControlTypeId, UIA_StatusBarControlTypeId, UIA_TabControlTypeId, UIA_TextPatternId,
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
// `UIA_E_INVALIDOPERATION` — the operation is not valid for this element.
const UIA_E_INVALIDOPERATION: HRESULT = HRESULT(0x8004_0200u32 as i32);
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

/// A NavigationView pane's own chrome — the back arrow, the hamburger and the
/// settings row — as synthetic items at `NAV_CHROME_BASE + i`.
///
/// They need an index space of their own for the same reason the caption
/// buttons do: the natural `0..count` range belongs to the menu items, and that
/// range is load-bearing. `ISelectionProvider` is defined over it, the
/// `uia:invoke;name=<label>` scripts the visual-parity rig runs address it, and
/// `Ctrl::selected_index` indexes straight into it. Renumbering the items to
/// make room at the front would have silently changed every one of those; a
/// disjoint base changes none of them, and a chrome item can never be mistaken
/// for a selectable page.
const NAV_CHROME_BASE: i32 = 1 << 16;

fn is_nav_chrome(item: i32) -> bool {
    (NAV_CHROME_BASE..CAPTION_ITEM_BASE).contains(&item)
}

/// An InfoBar's drawn close button, as its one synthetic child.
///
/// It shares the nav pane's chrome index space — the two are never on the same
/// node, and a second base would buy nothing but a second range to keep
/// disjoint from the caption's. `is_nav_chrome` therefore also answers for this
/// item; the two are told apart by the owning node's `ControlKind`, which every
/// consumer already has in hand.
///
/// The bar itself is not in the Tab ring (see `node::is_focusable_kind`), so
/// this element is how a keyboard-only or screen-reader user dismisses a bar at
/// all — the same arrangement the caption cluster has.
pub(crate) const INFOBAR_CLOSE_ITEM: i32 = NAV_CHROME_BASE;

/// The pane element a chrome item names, and the inverse. Both directions are
/// needed and a wrong pairing would put a screen reader's invoke on the wrong
/// button, so neither is derived from the other by arithmetic at a call site.
fn nav_chrome_hit(item: i32) -> Option<nav::Hit> {
    match item - NAV_CHROME_BASE {
        0 => Some(nav::Hit::Back),
        1 => Some(nav::Hit::Toggle),
        2 => Some(nav::Hit::Settings),
        _ => None,
    }
}

/// The pane element a synthetic item names, or `None` when the item is a menu
/// row rather than chrome. The one classifier the action path outside this
/// module asks (see `input::uia_select_item`), so the chrome index space is
/// interpreted in exactly one place.
pub(crate) fn nav_chrome_of(item: i32) -> Option<nav::Hit> {
    is_nav_chrome(item).then(|| nav_chrome_hit(item)).flatten()
}

fn nav_chrome_item(hit: nav::Hit) -> Option<i32> {
    match hit {
        nav::Hit::Back => Some(NAV_CHROME_BASE),
        nav::Hit::Toggle => Some(NAV_CHROME_BASE + 1),
        nav::Hit::Settings => Some(NAV_CHROME_BASE + 2),
        nav::Hit::Item(_) => None,
    }
}

/// UTF-16 word separator — the rule [`Editor`](super::editor::Editor)'s private
/// `is_space` uses, repeated here because the Text pattern needs word units and
/// that helper is not exported.
fn is_word_break(u: u16) -> bool {
    matches!(u, 0x20 | 0x09 | 0x0A | 0x0D | 0xA0)
}

/// Cycle guard on the parent-link walk. Real UI trees are tens of levels deep;
/// this only bounds the damage if a future mutator ever introduces a cycle,
/// since the walk runs on the UI thread inside a blocking UIA marshal.
const MAX_TREE_DEPTH: usize = 4096;

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
        // A status band, which is what a screen reader should call it — and
        // what pairs with the `LiveSetting` it advertises.
        InfoBar => UIA_StatusBarControlTypeId,
        // A badge is a short readable annotation, not a control: it has no
        // action and no value pattern, so `Text` is the honest type.
        InfoBadge => UIA_TextControlTypeId,
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

/// Whether a run is nothing but icon-font glyphs.
///
/// Icon fonts live in the Unicode private use area, where a code point has no
/// meaning outside the font that drew it. A screen reader handed one announces
/// a garbage character, which is worse than announcing nothing — an unnamed
/// element at least prompts a client to fall back to its control type. Icon
/// buttons are expected to carry a real `AutomationName` (which is checked
/// first); this only stops the absence of one from being filled with noise.
fn is_icon_text(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| matches!(c, '\u{E000}'..='\u{F8FF}') || c.is_whitespace())
}

fn pattern_supported(kind: ControlKind, item: i32, pid: PATTERNID) -> bool {
    use ControlKind::*;
    // The InfoBar's close button shares the nav pane's chrome index space, so
    // it is answered by kind BEFORE that branch — which would otherwise read
    // the same index as the pane's back arrow. Both happen to resolve to
    // Invoke-only today; stating it here means a later edit to the pane's
    // chrome patterns cannot silently change the bar's.
    if kind == InfoBar {
        return item == INFOBAR_CLOSE_ITEM && pid == UIA_InvokePatternId;
    }
    if is_nav_chrome(item) {
        // Pane chrome invokes. The settings row is also a selectable page, so
        // it carries SelectionItem alongside Invoke; the back arrow and the
        // hamburger are not part of any selection.
        return pid == UIA_InvokePatternId
            || (pid == UIA_SelectionItemPatternId
                && nav_chrome_hit(item) == Some(nav::Hit::Settings));
    }
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

    /// The container's selectable items.
    ///
    /// For a nav pane this is the rows that FIT, not the rows that exist: a
    /// pane too short for its whole menu draws a prefix of it, and the same
    /// `nav::visible_items` bound governs the paint, the hit test and this — so
    /// nothing is announced that is not on screen and cannot be clicked.
    fn uia_item_count(&self, id: ControlId) -> i32 {
        match self.arena.get(id) {
            Some(n) if n.kind == ControlKind::NavigationView => self
                .nav_metrics(id)
                .map(|(m, h, count)| nav::visible_items(&m, h, count) as i32)
                .unwrap_or(0),
            Some(n) if is_item_container(n.kind) => n.ctrl().items.len() as i32,
            _ => 0,
        }
    }

    /// Which of the pane's chrome elements are present: `(back, toggle,
    /// settings)`.
    fn nav_chrome_present(&self, id: ControlId) -> (bool, bool, bool) {
        match self.nav_metrics(id) {
            Some((m, h, _)) => (m.back, m.toggle, nav::settings_rect(&m, h).is_some()),
            None => (false, false, false),
        }
    }

    /// Total synthetic children of a nav pane — its chrome plus its items.
    fn nav_seq_len(&self, id: ControlId) -> i32 {
        let (b, t, st) = self.nav_chrome_present(id);
        i32::from(b) + i32::from(t) + self.uia_item_count(id) + i32::from(st)
    }

    /// The synthetic item at reading position `pos`. The pane reads top to
    /// bottom exactly as it is drawn: back, hamburger, the menu rows, settings.
    fn nav_seq_at(&self, id: ControlId, pos: i32) -> Option<i32> {
        let (b, t, st) = self.nav_chrome_present(id);
        if pos < 0 {
            return None;
        }
        let mut p = pos;
        for (present, hit) in [(b, nav::Hit::Back), (t, nav::Hit::Toggle)] {
            if present {
                if p == 0 {
                    return nav_chrome_item(hit);
                }
                p -= 1;
            }
        }
        let n = self.uia_item_count(id);
        if p < n {
            return Some(p);
        }
        p -= n;
        if st && p == 0 {
            return nav_chrome_item(nav::Hit::Settings);
        }
        None
    }

    /// How many synthetic children a container exposes.
    ///
    /// For every container but the nav pane this is simply the item count: the
    /// items ARE the whole synthetic sequence, and item `i` sits at position
    /// `i`. A nav pane interleaves chrome around its rows, so the two stop
    /// being the same number and the three `syn_*` helpers become the only
    /// thing the tree walk below may reason about.
    fn syn_len(&self, id: ControlId) -> i32 {
        match self.uia_kind(id) {
            Some(ControlKind::NavigationView) => self.nav_seq_len(id),
            Some(ControlKind::InfoBar) => i32::from(self.infobar_close_present(id)),
            _ => self.uia_item_count(id),
        }
    }

    /// The synthetic item at reading position `pos`.
    fn syn_at(&self, id: ControlId, pos: i32) -> Option<i32> {
        match self.uia_kind(id) {
            Some(ControlKind::NavigationView) => self.nav_seq_at(id, pos),
            Some(ControlKind::InfoBar) => {
                (pos == 0 && self.infobar_close_present(id)).then_some(INFOBAR_CLOSE_ITEM)
            }
            _ => (0..self.uia_item_count(id)).contains(&pos).then_some(pos),
        }
    }

    /// The reading position of synthetic item `item`.
    fn syn_pos(&self, id: ControlId, item: i32) -> Option<i32> {
        match self.uia_kind(id) {
            Some(ControlKind::NavigationView) => self.nav_seq_pos(id, item),
            Some(ControlKind::InfoBar) => {
                (item == INFOBAR_CLOSE_ITEM && self.infobar_close_present(id)).then_some(0)
            }
            _ => (0..self.uia_item_count(id)).contains(&item).then_some(item),
        }
    }

    /// Whether an InfoBar currently offers its close button — the bound
    /// [`syn_len`](Self::syn_len) and friends share, so nothing is announced
    /// that is not on screen and cannot be clicked.
    ///
    /// A CLOSED bar has no children of any kind: it is out of layout entirely,
    /// and exposing an invokable button inside an invisible band would let a
    /// screen reader dismiss a bar the user cannot see.
    fn infobar_close_present(&self, id: ControlId) -> bool {
        self.arena.get(id).is_some_and(|n| {
            n.kind == ControlKind::InfoBar
                && n.extras().bar_open
                && info_bar::close_rect(n.rect.w, n.rect.h, n.extras().bar_closable).is_some()
        })
    }

    /// Reading position of a synthetic item — the inverse of
    /// [`nav_seq_at`](Self::nav_seq_at).
    fn nav_seq_pos(&self, id: ControlId, item: i32) -> Option<i32> {
        let (b, t, st) = self.nav_chrome_present(id);
        let lead = i32::from(b) + i32::from(t);
        if is_nav_chrome(item) {
            return match nav_chrome_hit(item) {
                Some(nav::Hit::Back) if b => Some(0),
                Some(nav::Hit::Toggle) if t => Some(i32::from(b)),
                Some(nav::Hit::Settings) if st => Some(lead + self.uia_item_count(id)),
                _ => None,
            };
        }
        (0..self.uia_item_count(id))
            .contains(&item)
            .then_some(lead + item)
    }

    /// Drawn caption buttons under the fragment root (0 when the window has no
    /// custom caption).
    ///
    /// Four when the band also draws a back button. It takes the LAST index
    /// ([`caption::BACK_INDEX`]) rather than the first, because that index is
    /// shared with the hover/press cells and the non-client hit mapping — one
    /// numbering for all three, so a screen reader and the mouse can never
    /// disagree about which element is which. The cost is that the back button
    /// reads after the window buttons rather than before them.
    fn uia_caption_count(&self) -> i32 {
        if self.caption_rect().is_none() {
            return 0;
        }
        if self.back_button_rect().is_some() {
            caption::BACK_INDEX + 1
        } else {
            3
        }
    }

    /// Caption button `i` (0=min, 1=max, 2=close, 3=back)'s rect in window
    /// DIPs. The window buttons fill the right end of the caption strip, each
    /// [`caption::BTN_W`] wide; the back button sits at the leading edge.
    fn uia_caption_button(&self, i: i32) -> Option<(f32, f32, f32, f32)> {
        if i == caption::BACK_INDEX {
            return self.back_button_rect();
        }
        if !(0..3).contains(&i) {
            return None;
        }
        let (cx, cy, cw, ch) = self.caption_rect()?;
        Some((cx + cw - (3 - i) as f32 * caption::BTN_W, cy, caption::BTN_W, ch))
    }

    /// The parent of `target`: an O(1) read of the link every structural mutator
    /// maintains (see [`Node::parent`](super::node::Node::parent)). A parent that
    /// has since been destroyed fails the arena lookup and reads as no parent.
    fn uia_parent(&self, target: ControlId) -> Option<ControlId> {
        let p = self.arena.get(target)?.parent?;
        self.arena.get(p).is_some().then_some(p)
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
                    None => match self.syn_at(id, self.syn_len(id) - 1) {
                        Some(last) => UiaNav::Item(id, last),
                        None => UiaNav::None,
                    },
                },
                _ => UiaNav::None,
            };
        }

        if item >= 0 {
            // Siblings are the container's synthetic sequence, walked by
            // POSITION. For every kind but the nav pane position == index, so
            // this is the same walk it always was; for a nav pane it is what
            // threads the back arrow, the hamburger and the settings row into
            // the reading order between the menu rows.
            let pos = self.syn_pos(id, item);
            return match dir {
                NAV_PARENT if self.root == Some(id) => UiaNav::Root,
                NAV_PARENT => UiaNav::Node(id),
                NAV_NEXT => match pos.and_then(|p| self.syn_at(id, p + 1)) {
                    Some(next) => UiaNav::Item(id, next),
                    None => match node.children.first() {
                        Some(c) => UiaNav::Node(*c),
                        None if self.root == Some(id) && self.uia_caption_count() > 0 => {
                            UiaNav::Item(id, CAPTION_ITEM_BASE)
                        }
                        None => UiaNav::None,
                    },
                },
                NAV_PREV => match pos
                    .filter(|p| *p > 0)
                    .and_then(|p| self.syn_at(id, p - 1))
                {
                    Some(prev) => UiaNav::Item(id, prev),
                    None => UiaNav::None,
                },
                _ => UiaNav::None,
            };
        }

        let syn_count = self.syn_len(id);
        let caption_count = if self.root == Some(id) { self.uia_caption_count() } else { 0 };
        match dir {
            NAV_FIRST => {
                if let Some(first) = (syn_count > 0).then(|| self.syn_at(id, 0)).flatten() {
                    UiaNav::Item(id, first)
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
                        None => match self.syn_at(id, syn_count - 1) {
                            Some(last) => UiaNav::Item(id, last),
                            None => UiaNav::None,
                        },
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
                2 => "Close",
                _ => "Back",
            }
            .to_string();
        }
        // The InfoBar's close button shares the chrome index space, so the
        // owning kind is what tells the two apart — see `INFOBAR_CLOSE_ITEM`.
        if item == INFOBAR_CLOSE_ITEM
            && self.uia_kind(id) == Some(ControlKind::InfoBar)
        {
            return info_bar::CLOSE_LABEL.to_string();
        }
        if is_nav_chrome(item)
            && let Some(hit) = nav_chrome_hit(item)
        {
            return nav::chrome_label(hit).to_string();
        }
        let Some(n) = self.arena.get(id) else {
            return String::new();
        };
        if item >= 0 {
            return n.ctrl().items.get(item as usize).cloned().unwrap_or_default();
        }
        // An explicit AutomationName still wins (checked below); absent one,
        // the bar names itself from its severity plus the text as drawn, so a
        // screen reader is told what the status ICON conveys visually.
        let authored = n
            .accessibility
            .as_ref()
            .and_then(|a| a.automation_name.as_ref())
            .is_some_and(|s| !s.is_empty());
        if !authored {
            match n.kind {
                // The bar names itself from its severity plus the text as
                // drawn, so a screen reader is told what the status ICON
                // conveys visually.
                ControlKind::InfoBar => return info_bar::accessible_name(n),
                // A badge draws a bare number with no label of its own, so
                // without this it reaches an assistive client as an unnamed
                // element and its whole content — the count — is lost. The dot
                // form genuinely has no text; a host that means something by it
                // says so with an `automation_name`, which is why this only
                // fills in when none was authored.
                ControlKind::InfoBadge => {
                    return info_badge::label(n).unwrap_or_default();
                }
                // A switch carries its label in `on_content`/`off_content`
                // rather than `paint.text`, and hosts no child to fall back
                // to, so without this it is the one interactive control in the
                // set that reaches a screen reader unnamed. The word it is
                // currently showing is its visible content, exactly as a
                // badge's count is — and it changing when the switch flips is
                // that content changing, which `uia_notify_bool` announces.
                //
                // A switch given no content stays nameless on purpose: there
                // is nothing on screen to read, and a host that means
                // something by a bare track says so with an `automation_name`.
                ControlKind::ToggleSwitch => {
                    return controls::toggle_state_label(n).to_string();
                }
                _ => {}
            }
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
        // A Button given rich element content carries no text of its OWN — the
        // label is a TextBlock inside it. That is how every pill, chip and icon
        // button in a reactor app is built (`pressable` is exactly this), so
        // without a fallback the entire interactive surface is nameless to a
        // screen reader and to the automation harness. Fall back to the text
        // the subtree actually renders.
        //
        // Only for kinds that need a name: giving every layout container the
        // first string beneath it would name panels after whatever happened to
        // be at their top-left.
        if super::node::is_interactive_kind(n.kind) {
            return self.descendant_text(id, 0).unwrap_or_default();
        }
        String::new()
    }

    /// The first text this node's subtree renders, depth-first.
    ///
    /// Depth-bounded because a name is a short label and UIA asks for it
    /// constantly — an unbounded walk would put the cost of the whole subtree
    /// on a property fetched per element per pass.
    fn descendant_text(&self, id: ControlId, depth: u32) -> Option<String> {
        const MAX_DEPTH: u32 = 4;
        if depth > MAX_DEPTH {
            return None;
        }
        let n = self.arena.get(id)?;
        if depth > 0 && !n.paint.text.is_empty() && !is_icon_text(&n.paint.text) {
            return Some(n.paint.text.clone());
        }
        n.children
            .iter()
            .find_map(|&c| self.descendant_text(c, depth + 1))
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
        // The pane's own chrome are buttons, not list items: a screen reader
        // must not offer "select" on the hamburger, nor count it among the
        // pages.
        if is_caption(item) || is_nav_chrome(item) {
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
        if pid == UIA_TextPatternId {
            // Never on a synthetic item, never on a password field.
            return item < 0 && self.uia_text_supported(id);
        }
        // A plain Button carrying a MenuFlyout opens a popup exactly as a
        // DropDownButton does (`input`'s `activate`), so it expands and
        // collapses whatever its ControlKind says. This cannot be answered by
        // kind alone — the same kind without a menu is a pure Invoke — so it is
        // gated on the node's own state rather than in `pattern_supported`.
        if pid == UIA_ExpandCollapsePatternId && item < 0 && self.opens_a_menu(id) {
            return true;
        }
        self.uia_kind(id)
            .is_some_and(|k| pattern_supported(k, item, pid))
    }

    /// Whether this node activates by opening the menu popup.
    fn opens_a_menu(&self, id: ControlId) -> bool {
        self.arena.get(id).is_some_and(|n| {
            matches!(
                n.kind,
                ControlKind::Button | ControlKind::RepeatButton | ControlKind::ToggleButton
            ) && !n.ctrl().menu.is_empty()
        })
    }

    fn uia_toggle_state(&self, id: ControlId) -> i32 {
        match self.arena.get(id) {
            Some(n) if n.ctrl().indeterminate => 2,
            Some(n) if n.ctrl().is_on || n.ctrl().is_checked => 1,
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
        let step = n.ctrl().step.unwrap_or(1.0);
        Some((n.ctrl().value, n.ctrl().min, n.ctrl().max, readonly, step))
    }

    fn uia_expand_state(&self, id: ControlId) -> i32 {
        match self.arena.get(id).map(|n| n.kind) {
            Some(ControlKind::Expander) => {
                if self.arena.get(id).is_some_and(|n| n.ctrl().expanded) {
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
            // Same question for a menu-carrying Button, and the same answer:
            // the backend owns at most one popup, so "is it mine?" IS the
            // expanded state. Reported only when the node actually has a menu,
            // so a plain Button keeps answering 0 to a client that asks anyway.
            _ if self.opens_a_menu(id) => {
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
        let Some(n) = self.arena.get(id) else {
            return false;
        };
        let sel = n.ctrl().selected_index;
        // The settings element selects at its sentinel slot; the other chrome
        // items are never part of a selection (and their ids must not compare
        // against the sentinel, which shares the chrome index space).
        match nav_chrome_of(item) {
            Some(nav::Hit::Settings) => sel == nav::SETTINGS_INDEX,
            Some(_) => false,
            None => sel == item,
        }
    }

    /// The selected item index of container `id`, or `None` when the index is
    /// out of range (or the node is not an item container).
    ///
    /// Bounded by the EXPOSED item count, not the stored one. They differ only
    /// for a nav pane too short to draw its whole menu, and there the
    /// distinction matters: reporting a selection a client cannot then navigate
    /// to hands it an index into an element that does not exist.
    fn uia_selected_item(&self, id: ControlId) -> Option<i32> {
        let n = self.arena.get(id)?;
        if !is_item_container(n.kind) {
            return None;
        }
        let i = n.ctrl().selected_index;
        // A nav pane whose selection sits at the settings sentinel reports the
        // settings element — bounded the same way the items are: only when the
        // pane is tall enough to actually show the row.
        if n.kind == ControlKind::NavigationView && i == nav::SETTINGS_INDEX {
            let (_, _, st) = self.nav_chrome_present(id);
            return st.then(|| nav_chrome_item(nav::Hit::Settings)).flatten();
        }
        (0..self.uia_item_count(id)).contains(&i).then_some(i)
    }

    /// `(offset, viewport height, content height)` in DIPs for a scroll
    /// container.
    fn uia_scroll_info(&self, id: ControlId) -> Option<(f32, f32, f32)> {
        let n = self.arena.get(id)?;
        n.is_scroll().then(|| (n.scroll_off, n.rect.h, n.ctrl().content_h))
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
        let max = (n.ctrl().content_h - n.rect.h).max(0.0);
        let target = layout::snap(off.clamp(0.0, max), scale);
        n.scroll_off = target;
        n.scroll_glide(target);
        let g = scroll::thumb_geom(n.rect.h, n.ctrl().content_h, target);
        let tx = n.rect.w - scroll::THUMB_W - scroll::THUMB_MARGIN;
        n.thumb_glide(tx, g.thumb_y);
    }

    /// Ancestors of `target`, nearest-first (root last); empty when `target`
    /// is the root or unparented.
    ///
    /// Walks the maintained parent links, so this is O(depth). It used to be a
    /// DFS from the root per call, which made a client `FindAll` over the tree
    /// O(n²) through [`uia_is_offscreen`](Self::uia_is_offscreen).
    fn uia_ancestors(&self, target: ControlId) -> Vec<ControlId> {
        let mut path = Vec::new();
        let mut cur = target;
        // The links form a tree, so this terminates; the bound is belt-and-braces
        // against a cycle introduced by a future mutator, which would otherwise
        // hang the UI thread inside a blocking UIA marshal.
        for _ in 0..MAX_TREE_DEPTH {
            let Some(p) = self.uia_parent(cur) else {
                break;
            };
            path.push(p);
            cur = p;
        }
        path
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
            && !is_nav_chrome(item)
            && self
                .arena
                .get(id)
                .is_some_and(|n| n.kind == ControlKind::NavigationView)
            && let Some((m, _, _)) = self.nav_metrics(id)
        {
            let r = nav::item_rect(&m, item);
            ny += r.top;
            nh = r.height();
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
                // The close button's box, from the one geometry the paint used.
                ControlKind::InfoBar => {
                    if let Some(r) = info_bar::close_rect(
                        n.rect.w,
                        n.rect.h,
                        n.extras().bar_closable,
                    ) {
                        x = n.rect.x + r.left;
                        y = n.rect.y + r.top;
                        w = r.width();
                        h = r.height();
                    }
                }
                ControlKind::NavigationView => {
                    // Every pane box — rows and chrome alike — comes from the
                    // one geometry the paint used.
                    if let Some((m, nh, _)) = self.nav_metrics(id) {
                        let r = match nav_chrome_hit(item).filter(|_| is_nav_chrome(item)) {
                            Some(nav::Hit::Back) => nav::back_rect(&m),
                            Some(nav::Hit::Toggle) => nav::toggle_rect(&m),
                            Some(nav::Hit::Settings) => nav::settings_rect(&m, nh),
                            _ => Some(nav::item_rect(&m, item)),
                        };
                        if let Some(r) = r {
                            x = n.rect.x + r.left;
                            y = n.rect.y + r.top;
                            w = r.width();
                            h = r.height();
                        }
                    }
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

    /// The element at screen point `(sx, sy)` (for `ElementProviderFromPoint`).
    ///
    /// Hit-testing is delegated to [`DCompBackend::hit_test`], the single
    /// hit-test authority shared with pointer and wheel routing, so a click and
    /// an `ElementProviderFromPoint` can never resolve to different elements.
    /// [`HitKind::Any`] is the arm UIA needs: the topmost element at the point,
    /// interactive or not.
    ///
    /// Two things live outside the arena walk and are handled here:
    ///
    /// * **Caption buttons** are synthetic items in the `CAPTION_ITEM_BASE`
    ///   sentinel space, not arena nodes, so `hit_test` cannot return them. The
    ///   cluster overlays the content, so it is tested *before* delegating.
    /// * **Container items** (SelectorBar segments, NavigationView rows) are
    ///   likewise synthetic; once the walk names their container,
    ///   [`uia_item_at`](Self::uia_item_at) subdivides it — it takes the same
    ///   window-space point and applies its own scroll adjustment.
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
        match self.hit_test(px, py, HitKind::Any) {
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
        // The InfoBar subdivides into exactly one element, and it has no items
        // — so it is resolved before the item-count guard below.
        if n.kind == ControlKind::InfoBar {
            return (self.infobar_close_present(id)
                && info_bar::hit_close(n, px - n.rect.x, py - n.rect.y))
            .then_some(INFOBAR_CLOSE_ITEM);
        }
        let count = n.ctrl().items.len();
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
                // Delegated to the pane's own hit test — the same call the
                // pointer path makes, so a click and an
                // `ElementProviderFromPoint` can never name different elements.
                // Outside the pane the point belongs to the container (the body
                // is a real child and the walk already tried it).
                let (m, h, count) = self.nav_metrics(id)?;
                match nav::hit(&m, h, count, px - n.rect.x, py - n.rect.y)? {
                    nav::Hit::Item(i) => Some(i),
                    hit => nav_chrome_item(hit),
                }
            }
            _ => None,
        }
    }

    // ── Text pattern (single-line editors) ───────────────────────────────────
    //
    // The Text pattern is served straight off the [`Editor`](super::editor::Editor)
    // document: `buf` is already UTF-16, so a UIA text position IS a buffer
    // index — no transcoding, no second copy of the document.
    //
    // A `PasswordBox` supports the pattern NOWHERE: `uia_text_supported` returns
    // false for it, so `GetPatternProvider(TextPatternId)` answers null and no
    // range over its contents can be minted at all. That is stronger than
    // masking inside `GetText`, and matches the `IsPassword` contract the Value
    // pattern already honours here.

    /// Whether `id` exposes the Text pattern: an editor-backed node that is not
    /// a password field.
    fn uia_text_supported(&self, id: ControlId) -> bool {
        self.arena.get(id).is_some_and(|n| {
            n.editor.is_some() && n.kind != ControlKind::PasswordBox
        })
    }

    /// The editor of `id`, only when the Text pattern is exposed for it.
    fn uia_editor(&self, id: ControlId) -> Option<&super::editor::Editor> {
        let n = self.arena.get(id)?;
        (n.kind != ControlKind::PasswordBox).then_some(())?;
        n.editor.as_ref()
    }

    /// Document length in UTF-16 code units.
    fn uia_text_len(&self, id: ControlId) -> Option<usize> {
        Some(self.uia_editor(id)?.buf.len())
    }

    /// `[a, b)` of the document as a `String` (clamped to the buffer).
    fn uia_text_slice(&self, id: ControlId, a: usize, b: usize) -> Option<String> {
        let ed = self.uia_editor(id)?;
        let n = ed.buf.len();
        let (a, b) = (a.min(n), b.min(n));
        Some(String::from_utf16_lossy(&ed.buf[a.min(b)..b]))
    }

    /// The caret/anchor selection as an ordered `[start, end)`.
    fn uia_text_selection(&self, id: ControlId) -> Option<(usize, usize)> {
        Some(self.uia_editor(id)?.sel())
    }

    /// Point the editor's selection at `[a, b)` (caret at `b`), as UIA `Select`.
    fn uia_text_select(&mut self, id: ControlId, a: usize, b: usize) {
        let Some(n) = self.arena.get_mut(id) else {
            return;
        };
        if n.kind == ControlKind::PasswordBox {
            return;
        }
        let Some(ed) = n.editor.as_mut() else {
            return;
        };
        let len = ed.buf.len();
        ed.anchor = a.min(len);
        ed.caret = b.min(len);
        // Restart the compositor caret blink solid-first, exactly as a keyboard
        // caret move does.
        ed.caret_moved = true;
        n.mark_dirty();
    }

    /// Word boundaries around `i`: `[start, end)` of the word `i` sits in.
    ///
    /// `Editor`'s own `word_left` / `word_right` are private, so this repeats
    /// their rule (skip whitespace, then the run of non-whitespace) against the
    /// same buffer.
    fn uia_text_word(&self, id: ControlId, i: usize) -> Option<(usize, usize)> {
        let ed = self.uia_editor(id)?;
        let buf = &ed.buf;
        let n = buf.len();
        let i = i.min(n);
        let mut start = i;
        while start > 0 && !is_word_break(buf[start - 1]) {
            start -= 1;
        }
        let mut end = i;
        // A word run plus the whitespace that trails it — the unit UIA expects,
        // so `Move(Word, 1)` lands on the next word rather than on the gap.
        while end < n && !is_word_break(buf[end]) {
            end += 1;
        }
        while end < n && is_word_break(buf[end]) {
            end += 1;
        }
        Some((start, end))
    }

    /// Node-local text-draw origin `(origin_x, origin_y)` in DIPs — the same
    /// [`TextBand`](super::editor::TextBand) the painter draws the run at, so a
    /// screen reader is told where the words actually are.
    fn uia_text_origin(&self, id: ControlId) -> Option<(f32, f32)> {
        let band = editor::TextBand::of(self.arena.get(id)?)?;
        Some((band.origin_x, band.origin_y))
    }

    /// Screen-pixel rectangles covering `[a, b)` — one per line the range spans
    /// (single-line editors yield at most one). Empty for a degenerate range.
    fn uia_text_rects(&self, id: ControlId, a: usize, b: usize) -> Vec<(f64, f64, f64, f64)> {
        let Some(ed) = self.uia_editor(id) else {
            return Vec::new();
        };
        let (Some(layout), Some((ox, oy)), Some(n)) =
            (ed.layout.as_ref(), self.uia_text_origin(id), self.arena.get(id))
        else {
            return Vec::new();
        };
        let len = ed.buf.len();
        let (a, b) = (a.min(len), b.min(len));
        if a >= b {
            return Vec::new();
        }
        let scroll = self.uia_scroll_adjust(id);
        layout
            .hit_test_range(a as u32, (b - a) as u32, n.rect.x + ox, n.rect.y + oy - scroll)
            .unwrap_or_default()
            .into_iter()
            .map(|(x, y, w, h)| self.uia_screen_rect(x, y, w, h))
            .collect()
    }

    /// The document index under screen point `(sx, sy)` for editor `id`.
    fn uia_text_index_at(&self, id: ControlId, sx: f64, sy: f64) -> Option<usize> {
        let ed = self.uia_editor(id)?;
        let n = self.arena.get(id)?;
        let scale = self.scale();
        let mut pt = POINT { x: sx as i32, y: sy as i32 };
        unsafe {
            let _ = ScreenToClient(self.hwnd as HWND, &mut pt);
        }
        let (ox, _) = self.uia_text_origin(id)?;
        Some(ed.index_at_x(pt.x as f32 / scale - n.rect.x, ox))
    }

    /// Raise an `AutomationFocusChanged` event for `id` — deferred onto the pump
    /// so it never runs inside an input borrow, and a no-op when no client is
    /// listening (idle cost stays zero). Called on the UI thread from `set_focus`.
    pub(crate) fn uia_raise_focus(&self, id: ControlId) {
        // The state this describes just changed — retire the property snapshots
        // before the (client-gated) event raise.
        note_state_change();
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
        // The state this describes just changed — retire the property snapshots
        // before the (client-gated) event raise.
        note_state_change();
        if !clients_listening() {
            return;
        }
        let state = i32::from(v);
        match event {
            Event::Toggled | Event::Checked => {
                raise_property_changed(
                    self.hwnd,
                    id,
                    UIA_ToggleToggleStatePropertyId,
                    PropVal::I4(state),
                );
                // A switch that names itself from the word it shows just
                // changed that word, so the name changed with the state.
                // Raised only when it genuinely moved: an authored
                // `automation_name` outranks the content, and two identical
                // sides read the same either way — announcing a name that did
                // not change would make a screen reader repeat itself on every
                // flip.
                if let Some(n) = self.arena.get(id)
                    && n.kind == ControlKind::ToggleSwitch
                    && n.accessibility
                        .as_ref()
                        .and_then(|a| a.automation_name.as_ref())
                        .is_none_or(|s| s.is_empty())
                    && n.extras().on_content != n.extras().off_content
                {
                    let name = controls::toggle_state_label(n).to_string();
                    raise_property_changed(
                        self.hwnd,
                        id,
                        UIA_NamePropertyId,
                        PropVal::Bstr(name),
                    );
                }
            }
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
        // The state this describes just changed — retire the property snapshots
        // before the (client-gated) event raise.
        note_state_change();
        if !clients_listening() {
            return;
        }
        if matches!(event, Event::ValueChanged) {
            raise_property_changed(self.hwnd, id, UIA_RangeValueValuePropertyId, PropVal::R8(v));
        }
    }

    pub(crate) fn uia_notify_string(&self, id: ControlId, event: Event, v: &str) {
        // The state this describes just changed — retire the property snapshots
        // before the (client-gated) event raise.
        note_state_change();
        if !clients_listening() {
            return;
        }
        match event {
            Event::SelectionChanged => self.uia_raise_selection(id),
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

    /// The i32-payload arm of the notification choke point — a ComboBox (or
    /// UIA-driven) `SelectionChanged` carries the index. Announced exactly as
    /// the string-payload arm announces a bar/nav selection; the index itself
    /// is re-read from the arena (`uia_selected_item`), which the callers have
    /// already updated — state mutates first, notification fires last.
    pub(crate) fn uia_notify_i32(&self, id: ControlId, event: Event, v: i32) {
        note_state_change();
        if !clients_listening() {
            return;
        }
        let _ = v;
        if matches!(event, Event::SelectionChanged) {
            self.uia_raise_selection(id);
        }
    }

    /// Announce that an InfoBar has opened, by raising `LiveRegionChanged` on
    /// it — deferred onto the pump like every other raise.
    ///
    /// This is the entire reason an InfoBar exists for a non-visual user: the
    /// bar appears without being asked for, so nothing else in the
    /// accessibility model would ever mention it. A client reads the band's
    /// `Name` (its severity plus its text) and its
    /// [`LiveSetting`](Self::uia_live_setting) to decide how to say it.
    ///
    /// Raised on the OPENING edge only. Re-announcing on every prop write would
    /// interrupt the user each time an unrelated property of a bar that is
    /// already on screen changed.
    pub(crate) fn uia_announce_live_region(&self, id: ControlId) {
        if !clients_listening() {
            return;
        }
        let hwnd = self.hwnd;
        host::post_ui(hwnd, move || {
            let p = stable_provider(ElementProvider::element(hwnd, id));
            unsafe {
                let _ = UiaRaiseAutomationEvent(p.as_raw(), UIA_LiveRegionChangedEventId);
            }
        });
    }

    /// Raise `SelectionItem::ElementSelected` for `id`'s currently selected
    /// synthetic item — deferred onto the pump like every other raise.
    fn uia_raise_selection(&self, id: ControlId) {
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
}

fn clients_listening() -> bool {
    unsafe { UiaClientsAreListening() }.as_bool()
}

// ── Property batching ────────────────────────────────────────────────────────
//
// Every marshal is a `PostMessageW` + condvar wait against the UI thread, and
// each one drains the command buffer (`with_backend` → `flush`). A client tree
// walk reads ~10 properties per element, so one marshal per property made a
// 104-element walk ~1000 blocking round trips. Instead the FIRST property read
// of an element fills a whole `PropSnapshot` in one marshal and the rest of the
// burst is served from it.
//
// **Invalidation rule.** A snapshot is valid only while `UIA_GEN` still holds
// the value it was stamped with. The UI thread bumps `UIA_GEN` on every arena
// mutation — `set_prop` and the structural mutators call `note_state_change` /
// `note_tree_change`, and the interactive state changes funnel through the
// `uia_notify_*` / `uia_raise_focus` / `act` paths below, which bump too. The
// counter is a plain atomic, so the UIA worker validates without marshalling;
// a mismatch simply refills. The cache holds ONE element per UIA thread, so it
// is bounded by construction and cannot outlive the thread.
//
// Two properties are deliberately excluded because they move without passing a
// mutation hook: `IsOffscreen` (an ancestor's scroll offset glides from the
// wheel path) and `BoundingRectangle` (a layout pass). Both still marshal per
// call — cheap now that the ancestor walk is O(depth).

/// Monotonic stamp of "the arena as the UIA layer may describe it". Bumped by
/// the UI thread on mutation; read lock-free by UIA worker threads.
static UIA_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn uia_gen() -> u64 {
    UIA_GEN.load(std::sync::atomic::Ordering::Acquire)
}

fn bump_gen() {
    UIA_GEN.fetch_add(1, std::sync::atomic::Ordering::Release);
}

/// The logical tree changed (a child was linked or unlinked). Called from the
/// structural mutators in [`mod`](super).
pub(crate) fn note_tree_change() {
    bump_gen();
}

/// A node's observable state changed (a prop write, an interactive edit).
pub(crate) fn note_state_change() {
    bump_gen();
}

/// The properties one marshal collects for an element. Plain `Send` data.
#[derive(Clone, Default)]
struct PropSnapshot {
    name: String,
    automation_id: String,
    help_text: String,
    control_type: i32,
    value: String,
    toggle_state: i32,
    range_value: f64,
    is_enabled: bool,
    focusable: bool,
    has_focus: bool,
    is_password: bool,
    live_setting: i32,
}

impl DCompBackend {
    /// Collect every cacheable property of `(id, item)` in one pass.
    fn uia_snapshot(&self, id: ControlId, item: i32) -> PropSnapshot {
        PropSnapshot {
            name: self.uia_name(id, item),
            automation_id: self.uia_automation_id(id),
            help_text: self.uia_help_text(id),
            control_type: self.uia_control_type(id, item),
            value: self.uia_value_string(id),
            toggle_state: self.uia_toggle_state(id),
            range_value: self.uia_range(id).map_or(0.0, |r| r.0),
            is_enabled: self.uia_is_enabled(id),
            focusable: self.uia_focusable(id, item),
            has_focus: self.uia_has_focus(id, item),
            is_password: self.uia_kind(id) == Some(ControlKind::PasswordBox),
            live_setting: self.uia_live_setting(id, item),
        }
    }

    /// `LiveSetting` — how urgently a client should announce this element when
    /// its content changes (`Off` / `Polite` / `Assertive`).
    ///
    /// Only the InfoBar is a live region: it is the one control here whose
    /// whole purpose is to say something the user did not ask to hear. The
    /// severity picks the urgency, because that is the difference between "the
    /// preset saved" and "the audio device disappeared" — the first should wait
    /// for a pause in speech, the second should interrupt.
    ///
    /// Paired with the [`UIA_LiveRegionChangedEventId`] raised when a bar opens
    /// (`announce_live_region`): the property tells a client HOW to announce,
    /// the event tells it WHEN, and neither works alone.
    fn uia_live_setting(&self, id: ControlId, item: i32) -> i32 {
        use info_bar::Severity;
        if item >= 0 {
            return LIVE_OFF;
        }
        match self.arena.get(id) {
            Some(n) if n.kind == ControlKind::InfoBar && n.extras().bar_open => {
                match info_bar::severity(n.extras()) {
                    Severity::Warning | Severity::Error => LIVE_ASSERTIVE,
                    _ => LIVE_POLITE,
                }
            }
            _ => LIVE_OFF,
        }
    }
}

// `LiveSetting` values (uiautomationcore.h). Not in the generated set — the
// enum carries no constants of its own there, only the property id does.
const LIVE_OFF: i32 = 0;
const LIVE_POLITE: i32 = 1;
const LIVE_ASSERTIVE: i32 = 2;

thread_local! {
    /// One cached element per UIA worker thread: `(key, generation, snapshot)`.
    static SNAPSHOT: std::cell::RefCell<Option<((isize, u32, i32), u64, PropSnapshot)>> =
        const { std::cell::RefCell::new(None) };
}

/// The snapshot for `(hwnd, id, item)`, from this thread's cache when it is
/// still current, else refilled with a single marshal.
fn snapshot(hwnd: isize, id: ControlId, item: i32) -> Option<PropSnapshot> {
    let key = (hwnd, id.get(), item);
    let hit = SNAPSHOT.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(k, g, _)| *k == key && *g == uia_gen())
            .map(|(_, _, s)| s.clone())
    });
    if let Some(s) = hit {
        return Some(s);
    }
    // Stamp on the UI thread: bumps happen there too, so reading the counter
    // inside the same borrow pairs the snapshot with the exact generation it
    // describes. A mutation that lands while we are blocked bumps past this
    // stamp and the entry is dead on arrival — never stale, only refilled.
    let (stamp, s) = on_backend(hwnd, move |b| {
        b.arena
            .get(id)
            .is_some()
            .then(|| (uia_gen(), b.uia_snapshot(id, item)))
    })
    .flatten()?;
    SNAPSHOT.with(|c| *c.borrow_mut() = Some((key, stamp, s.clone())));
    Some(s)
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
///
/// Every action mutates, so this is also a generation bump: a client that sets a
/// value and immediately reads it back must not be answered from the snapshot
/// taken before its own write.
fn act<F>(hwnd: isize, f: F)
where
    F: FnOnce(&mut DCompBackend) + Send + 'static,
{
    let _ = on_backend(hwnd, f);
    note_state_change();
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
// so a single instance is safely shared across UIA's worker threads.
//
// Lifetime: the cache is keyed by NODE first, then by the synthetic-item/root
// variants of that node, so `destroy` can drop every provider for a node in one
// O(1) map removal ([`forget`], called from `DCompBackend::destroy` beside the
// existing `size::forget` / `pointer::forget`). Without that, a long session of
// mounts and unmounts grew the map forever. Dropping our reference does not
// yank the object out from under UIA — COM refcounting keeps any provider UIA
// still holds alive; we only stop *handing out* a dead node's provider, and
// every method on it already answers `UIA_E_ELEMENTNOTAVAILABLE` once the arena
// entry is gone.

/// An agile provider object, sendable because it is callable from any apartment.
struct SendProvider(IRawElementProviderSimple);
// SAFETY: `implement_decl!` providers are agile (free-threaded marshaler); the
// wrapped object may be AddRef'd/called from any thread.
unsafe impl Send for SendProvider {}

/// Which provider of one node: `(synthetic item index, is fragment root)`.
type VariantKey = (i32, bool);
/// Which node: `(hwnd, ControlId)`.
type NodeKey = (isize, u32);

type ProviderCache = HashMap<NodeKey, HashMap<VariantKey, SendProvider>>;

fn provider_cache() -> &'static Mutex<ProviderCache> {
    static CACHE: OnceLock<Mutex<ProviderCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drop every cached provider object (element, items, root) for node `id` of
/// window `hwnd` — one map removal, whatever the node's item count.
///
/// Called from `DCompBackend::destroy`. Ids are minted monotonically and never
/// reused, so an entry for a destroyed node could never be re-addressed — this
/// is purely about keeping the map bounded across mount/unmount churn.
pub(crate) fn forget(hwnd: isize, id: ControlId) {
    if let Ok(mut cache) = provider_cache().lock() {
        cache.remove(&(hwnd, id.get()));
    }
}

/// The one stable provider object for `p`'s element identity, created on first
/// use and reused thereafter.
fn stable_provider(p: ElementProvider) -> IRawElementProviderSimple {
    let mut cache = provider_cache().lock().unwrap();
    cache
        .entry((p.hwnd, p.id.get()))
        .or_default()
        .entry((p.item, p.is_root))
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
    ///
    /// Constants first (no marshal at all), then the two properties that must be
    /// read live, then everything else off ONE batched [`PropSnapshot`] — see the
    /// property-batching notes above.
    fn property(&self, pid: PROPERTYID) -> VARIANT {
        let (hwnd, id, item) = (self.hwnd, self.id, self.item);
        if pid == UIA_IsControlElementPropertyId || pid == UIA_IsContentElementPropertyId {
            return v_bool(true);
        }
        // Excluded from the snapshot: moves with scroll glide / layout, neither
        // of which passes a generation bump.
        if pid == UIA_IsOffscreenPropertyId {
            return v_bool(on_backend(hwnd, move |b| b.uia_is_offscreen(id, item)).unwrap_or(false));
        }
        let Some(s) = snapshot(hwnd, id, item) else {
            return VARIANT::default();
        };
        if pid == UIA_NamePropertyId {
            v_bstr(s.name)
        } else if pid == UIA_AutomationIdPropertyId {
            v_bstr(s.automation_id)
        } else if pid == UIA_HelpTextPropertyId {
            v_bstr(s.help_text)
        } else if pid == UIA_ControlTypePropertyId {
            v_i4(s.control_type)
        } else if pid == UIA_IsEnabledPropertyId {
            v_bool(s.is_enabled)
        } else if pid == UIA_IsKeyboardFocusablePropertyId {
            v_bool(s.focusable)
        } else if pid == UIA_HasKeyboardFocusPropertyId {
            v_bool(s.has_focus)
        } else if pid == UIA_IsPasswordPropertyId {
            v_bool(s.is_password)
        } else if pid == UIA_ToggleToggleStatePropertyId {
            v_i4(s.toggle_state)
        } else if pid == UIA_RangeValueValuePropertyId {
            v_r8(s.range_value)
        } else if pid == UIA_ValueValuePropertyId {
            v_bstr(s.value)
        } else if pid == UIA_LiveSettingPropertyId {
            v_i4(s.live_setting)
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
        IScrollItemProvider,
        ITextProvider
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
        ITextProvider,
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
            // The back button is an app command, not a system one — it has to
            // go back through the backend on the UI thread, like any other
            // invoke, rather than posting a `WM_SYSCOMMAND`.
            if item - CAPTION_ITEM_BASE == caption::BACK_INDEX {
                act(self.hwnd, move |b| b.raise_back_requested());
                return Ok(());
            }
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

// ── Text pattern ─────────────────────────────────────────────────────────────
//
// `ITextProvider` on the editor element, `ITextRangeProvider` on a range over
// its UTF-16 document. A text position is a code-unit index into
// `Editor::buf` — the same index space the caret, the selection and DWrite's
// hit-testing already use, so nothing is transcoded on this path.
//
// The editor is single-line by construction (it has `scroll_x` and no
// `scroll_y`), so Line / Paragraph / Page / Document are all ONE unit spanning
// the whole document, and `GetVisibleRanges` is the document range.
//
// PASSWORDS: `uia_text_supported` is false for a `PasswordBox`, so
// `GetPatternProvider` answers null and no range can ever be minted over a
// masked buffer. Every accessor additionally re-checks the kind, so a field that
// becomes a password after a range was handed out starts answering empty rather
// than leaking.

/// A range's element identity plus its `[start, end)` in code units, shared with
/// the registry below so a range handed back to us can be recognised by object
/// identity — and so a comparison can tell two ranges over DIFFERENT elements
/// apart even when their offsets coincide.
type RangeState = (isize, ControlId, usize, usize);
type SpanCell = std::sync::Arc<Mutex<RangeState>>;

type RangeRegistry = Mutex<HashMap<usize, std::sync::Weak<Mutex<RangeState>>>>;

/// Live ranges, keyed by their COM object address.
///
/// `Compare` / `CompareEndpoints` / `MoveEndpointByRange` are handed an
/// `ITextRangeProvider` and need its absolute offsets, which no standard method
/// exposes. Rather than an unchecked downcast, each range we mint registers a
/// `Weak` handle on its span under its own pointer: a hit that still upgrades is
/// provably one of ours (the `Arc` lives only inside that object), and anything
/// else — a foreign implementation, a marshalling proxy, a freed address — fails
/// the upgrade and is treated as foreign. Dead entries are pruned on insert, so
/// the map stays bounded without a `Drop` impl.
fn range_registry() -> &'static RangeRegistry {
    static REG: OnceLock<RangeRegistry> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Mint a range over `[a, b)` of editor `id` and register it.
fn new_range(hwnd: isize, id: ControlId, a: usize, b: usize) -> ITextRangeProvider {
    let span: SpanCell = std::sync::Arc::new(Mutex::new((hwnd, id, a.min(b), a.max(b))));
    let obj: ITextRangeProvider = TextRange { hwnd, id, span: span.clone() }.into();
    if let Ok(mut reg) = range_registry().lock() {
        if reg.len() > 256 {
            reg.retain(|_, w| w.strong_count() > 0);
        }
        reg.insert(obj.as_raw() as usize, std::sync::Arc::downgrade(&span));
    }
    obj
}

/// The element + span of `r`, if `r` is one of our live ranges.
fn foreign_state(r: Option<&ITextRangeProvider>) -> Option<RangeState> {
    let cell = range_registry()
        .lock()
        .ok()?
        .get(&(r?.as_raw() as usize))?
        .upgrade()?;
    let state = *cell.lock().ok()?;
    Some(state)
}

/// The span of `r` when it is a live range over the SAME element as `owner`.
/// Endpoint arithmetic against a range on another element is meaningless, so it
/// is rejected rather than silently answered.
fn sibling_span(owner: (isize, ControlId), r: Option<&ITextRangeProvider>) -> Option<(usize, usize)> {
    let (hwnd, id, a, b) = foreign_state(r)?;
    ((hwnd, id) == owner).then_some((a, b))
}

/// One UI Automation text range: an element identity plus a mutable
/// `[start, end)` over that element's document.
struct TextRange {
    hwnd: isize,
    id: ControlId,
    span: SpanCell,
}

implement_decl! {
    impl TextRange as TextRange_Impl: [ITextRangeProvider]
}

// `ITextProvider` is answered by the same stable element object every other
// pattern is answered by (see `pattern_provider`).
impl ElementProvider {
    fn text_document_range(&self) -> Result<ITextRangeProvider> {
        let id = self.id;
        let len = get(self.hwnd, move |b| b.uia_text_len(id))?;
        Ok(new_range(self.hwnd, self.id, 0, len))
    }

    /// A one-element `VT_UNKNOWN` array holding the selection range.
    fn text_selection(&self) -> Result<*mut SAFEARRAY> {
        let id = self.id;
        let (a, b) = get(self.hwnd, move |b| b.uia_text_selection(id))?;
        Ok(range_array(&[new_range(self.hwnd, self.id, a, b)]))
    }

    /// The whole document — a single-line field scrolls horizontally but never
    /// hides a line, and reporting the exact visible substring would make a
    /// screen reader announce a clipped fragment.
    fn text_visible_ranges(&self) -> Result<*mut SAFEARRAY> {
        Ok(range_array(&[self.text_document_range()?]))
    }

    /// The editor has no child elements, so any child maps to the whole document.
    fn text_range_from_child(&self) -> Result<ITextRangeProvider> {
        self.text_document_range()
    }

    fn text_range_from_point(&self, pt: &UiaPoint) -> Result<ITextRangeProvider> {
        let (id, x, y) = (self.id, pt.x, pt.y);
        let i = get(self.hwnd, move |b| b.uia_text_index_at(id, x, y))?;
        // A degenerate range at the hit position, as the pattern specifies.
        Ok(new_range(self.hwnd, self.id, i, i))
    }
}

/// Pack `ranges` into a `VT_UNKNOWN` SAFEARRAY (UIA takes ownership).
fn range_array(ranges: &[ITextRangeProvider]) -> *mut SAFEARRAY {
    unsafe {
        let psa = SafeArrayCreateVector(VT_UNKNOWN, 0, ranges.len() as u32);
        if psa.is_null() {
            return psa;
        }
        for (i, r) in ranges.iter().enumerate() {
            let idx = i as i32;
            // SafeArrayPutElement AddRefs the interface pointer.
            let _ = SafeArrayPutElement(psa, &idx, r.as_raw());
        }
        psa
    }
}

macro_rules! forward_text_provider {
    ($imp:ty) => {
        impl ITextProvider_Impl for $imp {
            fn DocumentRange(&self) -> Result<ITextRangeProvider> {
                self.inner().text_document_range()
            }
            fn SupportedTextSelection(&self) -> Result<SupportedTextSelection> {
                Ok(SupportedTextSelection_Single)
            }
            fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
                self.inner().text_selection()
            }
            fn GetVisibleRanges(&self) -> Result<*mut SAFEARRAY> {
                self.inner().text_visible_ranges()
            }
            fn RangeFromChild(
                &self,
                _childelement: windows_core::Ref<IRawElementProviderSimple>,
            ) -> Result<ITextRangeProvider> {
                self.inner().text_range_from_child()
            }
            fn RangeFromPoint(&self, point: &UiaPoint) -> Result<ITextRangeProvider> {
                self.inner().text_range_from_point(point)
            }
        }
    };
}

forward_text_provider!(ElementProvider_Impl);
forward_text_provider!(RootProvider_Impl);

// ── The range object ─────────────────────────────────────────────────────────

impl TextRange {
    fn span(&self) -> (usize, usize) {
        let (_, _, a, b) = *self.span.lock().unwrap();
        (a, b)
    }

    fn set_span(&self, a: usize, b: usize) {
        let mut g = self.span.lock().unwrap();
        *g = (g.0, g.1, a.min(b), a.max(b));
    }

    /// This range's element identity, for comparisons against another range.
    fn owner(&self) -> (isize, ControlId) {
        (self.hwnd, self.id)
    }

    fn len(&self) -> usize {
        let id = self.id;
        on_backend(self.hwnd, move |b| b.uia_text_len(id))
            .flatten()
            .unwrap_or(0)
    }

    /// The unit boundaries enclosing `i`. Line / Paragraph / Page / Document all
    /// resolve to the whole document — the editor is single-line.
    fn unit_bounds(&self, unit: TextUnit, i: usize) -> (usize, usize) {
        let len = self.len();
        let i = i.min(len);
        // Compared, not matched: the generated `TextUnit_*` constants are not
        // upper-case, and a constant pattern would trip `non_upper_case_globals`.
        if unit == TextUnit_Character {
            (i, (i + 1).min(len))
        } else if unit == TextUnit_Word || unit == TextUnit_Format {
            let id = self.id;
            on_backend(self.hwnd, move |b| b.uia_text_word(id, i))
                .flatten()
                .unwrap_or((i, len))
        } else {
            (0, len)
        }
    }

    /// Step `i` by `count` units, returning `(new index, units actually moved)`.
    fn step(&self, unit: TextUnit, i: usize, count: i32) -> (usize, i32) {
        let len = self.len();
        let mut at = i.min(len);
        let mut moved = 0i32;
        let back = count < 0;
        for _ in 0..count.unsigned_abs() {
            let next = if unit == TextUnit_Character {
                if back {
                    at.checked_sub(1)
                } else {
                    (at < len).then_some(at + 1)
                }
            } else if unit == TextUnit_Word || unit == TextUnit_Format {
                if back {
                    // Start of this word if we are inside one, else the start of
                    // the word before it.
                    let (s, _) = self.unit_bounds(unit, at);
                    let prev = if s < at {
                        s
                    } else {
                        self.unit_bounds(unit, at.saturating_sub(1)).0
                    };
                    (prev < at).then_some(prev)
                } else {
                    let (_, e) = self.unit_bounds(unit, at);
                    (e > at).then_some(e)
                }
            } else if back {
                // One document unit: at most one step, to either edge.
                (at > 0).then_some(0)
            } else {
                (at < len).then_some(len)
            };
            match next {
                Some(n) => {
                    at = n;
                    moved += if back { -1 } else { 1 };
                }
                None => break,
            }
        }
        (at, moved)
    }
}

impl ITextRangeProvider_Impl for TextRange_Impl {
    fn Clone(&self) -> Result<ITextRangeProvider> {
        let (a, b) = self.span();
        Ok(new_range(self.hwnd, self.id, a, b))
    }

    fn Compare(&self, range: windows_core::Ref<ITextRangeProvider>) -> Result<BOOL> {
        // Equal only when it is one of our ranges, over the same element, with
        // the same endpoints. Identical offsets on a different field are a
        // different range.
        let (a, b) = self.span();
        let same = foreign_state(range.as_ref()) == Some((self.hwnd, self.id, a, b));
        Ok(same.into())
    }

    fn CompareEndpoints(
        &self,
        endpoint: TextPatternRangeEndpoint,
        targetrange: windows_core::Ref<ITextRangeProvider>,
        targetendpoint: TextPatternRangeEndpoint,
    ) -> Result<i32> {
        let (a, b) = self.span();
        let (ta, tb) =
            sibling_span(self.owner(), targetrange.as_ref()).ok_or_else(not_available)?;
        let mine = if endpoint == TextPatternRangeEndpoint_Start { a } else { b };
        let theirs = if targetendpoint == TextPatternRangeEndpoint_Start { ta } else { tb };
        Ok(match mine.cmp(&theirs) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        })
    }

    fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> Result<()> {
        let (a, _) = self.span();
        let (s, e) = self.unit_bounds(unit, a);
        self.set_span(s, e);
        Ok(())
    }

    fn GetText(&self, maxlength: i32) -> Result<BSTR> {
        let (a, mut b) = self.span();
        if maxlength >= 0 {
            b = b.min(a + maxlength as usize);
        }
        let id = self.id;
        // `uia_text_slice` re-checks the password kind, so a field that turned
        // into a `PasswordBox` after this range was minted yields nothing.
        let s = on_backend(self.hwnd, move |bk| bk.uia_text_slice(id, a, b))
            .flatten()
            .ok_or_else(not_available)?;
        Ok(BSTR::from(s))
    }

    fn Move(&self, unit: TextUnit, count: i32) -> Result<i32> {
        let (a, b) = self.span();
        let degenerate = a == b;
        let (at, moved) = self.step(unit, a, count);
        if degenerate {
            self.set_span(at, at);
        } else {
            // A non-degenerate range lands on a whole unit, per the pattern.
            let (s, e) = self.unit_bounds(unit, at);
            self.set_span(s, e);
        }
        Ok(moved)
    }

    fn MoveEndpointByUnit(
        &self,
        endpoint: TextPatternRangeEndpoint,
        unit: TextUnit,
        count: i32,
    ) -> Result<i32> {
        let (a, b) = self.span();
        let start = endpoint == TextPatternRangeEndpoint_Start;
        let (at, moved) = self.step(unit, if start { a } else { b }, count);
        // Crossing the other endpoint collapses the range onto the moved one.
        if start {
            self.set_span(at, b.max(at));
        } else {
            self.set_span(a.min(at), at);
        }
        Ok(moved)
    }

    fn MoveEndpointByRange(
        &self,
        endpoint: TextPatternRangeEndpoint,
        targetrange: windows_core::Ref<ITextRangeProvider>,
        targetendpoint: TextPatternRangeEndpoint,
    ) -> Result<()> {
        let (a, b) = self.span();
        let (ta, tb) =
            sibling_span(self.owner(), targetrange.as_ref()).ok_or_else(not_available)?;
        let to = if targetendpoint == TextPatternRangeEndpoint_Start { ta } else { tb };
        if endpoint == TextPatternRangeEndpoint_Start {
            self.set_span(to, b.max(to));
        } else {
            self.set_span(a.min(to), to);
        }
        Ok(())
    }

    fn Select(&self) -> Result<()> {
        let (a, b) = self.span();
        let id = self.id;
        act(self.hwnd, move |bk| bk.uia_text_select(id, a, b));
        Ok(())
    }

    fn GetBoundingRectangles(&self) -> Result<*mut SAFEARRAY> {
        let (a, b) = self.span();
        let id = self.id;
        let rects = on_backend(self.hwnd, move |bk| bk.uia_text_rects(id, a, b)).unwrap_or_default();
        // The pattern's array is a flat run of doubles: left, top, width, height.
        unsafe {
            let psa = SafeArrayCreateVector(VT_R8, 0, (rects.len() * 4) as u32);
            if psa.is_null() {
                return Ok(psa);
            }
            for (i, r) in rects.iter().enumerate() {
                for (j, v) in [r.0, r.1, r.2, r.3].iter().enumerate() {
                    let idx = (i * 4 + j) as i32;
                    let _ = SafeArrayPutElement(psa, &idx, v as *const f64 as *const _);
                }
            }
            Ok(psa)
        }
    }

    fn GetEnclosingElement(&self) -> Result<IRawElementProviderSimple> {
        Ok(stable_provider(ElementProvider::element(self.hwnd, self.id)))
    }

    fn GetChildren(&self) -> Result<*mut SAFEARRAY> {
        // A drawn text field has no child elements.
        Ok(unsafe { SafeArrayCreateVector(VT_UNKNOWN, 0, 0) })
    }

    fn ScrollIntoView(&self, _aligntotop: BOOL) -> Result<()> {
        let id = self.id;
        act(self.hwnd, move |bk| bk.uia_scroll_into_view(id, -1));
        Ok(())
    }

    fn FindText(&self, text: &BSTR, backward: BOOL, ignorecase: BOOL) -> Result<ITextRangeProvider> {
        let (a, b) = self.span();
        let id = self.id;
        let hay = on_backend(self.hwnd, move |bk| bk.uia_text_slice(id, a, b))
            .flatten()
            .ok_or_else(not_available)?;
        let needle = text.display().to_string();
        // Search in UTF-16 units so a hit offset lands in the document's own
        // index space (a `str` byte offset would not).
        let (hay, needle) = if ignorecase.as_bool() {
            (hay.to_lowercase(), needle.to_lowercase())
        } else {
            (hay, needle)
        };
        let hay: Vec<u16> = hay.encode_utf16().collect();
        let pat: Vec<u16> = needle.encode_utf16().collect();
        if pat.is_empty() || pat.len() > hay.len() {
            return Err(Error::empty()); // S_OK + null: not found
        }
        let mut hits =
            (0..=hay.len() - pat.len()).filter(|i| hay[*i..*i + pat.len()] == pat[..]);
        let found = if backward.as_bool() { hits.next_back() } else { hits.next() };
        match found {
            Some(i) => Ok(new_range(self.hwnd, self.id, a + i, a + i + pat.len())),
            None => Err(Error::empty()),
        }
    }

    fn FindAttribute(
        &self,
        _attributeid: TEXTATTRIBUTEID,
        _val: &VARIANT,
        _backward: BOOL,
    ) -> Result<ITextRangeProvider> {
        // No text attributes are reported, so nothing can match.
        Err(Error::empty()) // S_OK + null
    }

    fn GetAttributeValue(&self, _attributeid: TEXTATTRIBUTEID) -> Result<VARIANT> {
        // The field is uniformly formatted and we report no attributes; an empty
        // VARIANT is the "not supported" answer clients tolerate.
        Ok(VARIANT::default())
    }

    fn AddToSelection(&self) -> Result<()> {
        // `SupportedTextSelection_Single` — there is only ever one selection.
        Err(Error::from_hresult(UIA_E_INVALIDOPERATION))
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Err(Error::from_hresult(UIA_E_INVALIDOPERATION))
    }
}

#[cfg(test)]
mod tests {
    use super::is_icon_text;

    /// The name fallback must not announce icon-font code points.
    ///
    /// Only the classifier is covered here: `descendant_text` walks a live
    /// arena, which needs a backend the harness does not currently hand out.
    #[test]
    fn icon_runs_are_not_names() {
        // Segoe Fluent Icons: chevron, and the power glyph the GUI's bypass
        // pill uses. Private use area — meaningless outside the font.
        assert!(is_icon_text("\u{E70D}"));
        assert!(is_icon_text("\u{E7E8}"));
        // A glyph with layout whitespace around it is still just a glyph.
        assert!(is_icon_text("  \u{E70D} "));

        // Real labels, including ones that merely contain a symbol.
        assert!(!is_icon_text("Apply"));
        assert!(!is_icon_text("+  Add Processor"));
        assert!(!is_icon_text("\u{E70D} Collapse"));
        // Empty is not an icon run — it is simply no name, and the caller
        // must not report it as one.
        assert!(!is_icon_text(""));
    }
}
