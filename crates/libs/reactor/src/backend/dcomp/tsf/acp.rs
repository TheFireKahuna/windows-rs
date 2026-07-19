//! The `#[implement]`'d `ITextStoreACP` document, and the backend-facing
//! [`TextInput`] handle.
//!
//! This is a translator: every method converts the ACP vtable's raw pointers /
//! `HRESULT`s to and from the binding-independent core in [`store`](super::store),
//! and holds no protocol logic of its own beyond lock-scope checks and the
//! embedded-object stubs. The lock state machine, the notification discipline,
//! and the text operations are all tested in `store`.

use std::cell::RefCell;
use std::rc::Rc;

use windows_core::{implement_decl, Error, Interface, Result, BOOL, GUID, HRESULT, IUnknown, Ref};

use super::store::{
    get_selection, get_text, insert_at_selection, notify_app_selection_change,
    notify_app_text_change, run_request_lock, set_selection, set_text, AcpError, LockResult,
    StoreSink, TextChange, TextStoreCore, TsfDocument,
};
use super::{active_end, hr, layout_code, DocRect, DocSelection, TS_DEFAULT_SELECTION, TS_RT_PLAIN,
    TS_SD_READONLY, TS_SS_NOHIDDENTEXT};
use crate::system_bindings::{
    FORMATETC, HWND, IDataObject, ITextStoreACP, ITextStoreACPSink, ITextStoreACP_Impl, POINT,
    RECT, TS_ATTRID, TS_ATTRVAL, TS_RUNINFO, TS_SELECTIONSTYLE, TS_SELECTION_ACP, TS_STATUS,
    TS_TEXTCHANGE, TsViewCookie,
};

/// Standard `HRESULT`s not surfaced as named consts by the minimal bindings.
mod e {
    use windows_core::HRESULT;
    pub const NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);
    pub const INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);
    pub const FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);
    pub const UNEXPECTED: HRESULT = HRESULT(0x8000_ffffu32 as i32);
}

/// The one view cookie the store hands out — a single-line field has exactly one
/// text view. A TIP passes it back to the extent methods; we do not validate it
/// (a mismatched cookie from a misbehaving TIP is harmless here).
const VIEW_COOKIE: TsViewCookie = 1;

/// Shared store state, held by both the COM object and the backend's
/// [`TextInput`] handle. Single-threaded (front thread only): `Rc`/`RefCell`
/// throughout, never marshalled — TSF text stores are STA-affine and used only
/// on their creating thread.
struct StoreState {
    core: RefCell<TextStoreCore>,
    /// The advised `ITextStoreACPSink`, set by `AdviseSink`. TSF advises exactly
    /// one sink per store.
    sink: RefCell<Option<ITextStoreACPSink>>,
    /// The editor seam. Borrowed per call; no borrow is ever held across a sink
    /// call-out (the re-entrancy rule the core relies on).
    doc: Rc<RefCell<dyn TsfDocument>>,
    /// The owning window, returned from `GetWnd`.
    hwnd: HWND,
}

impl StoreState {
    fn require_read_lock(&self) -> Result<()> {
        if self.core.borrow().is_locked() {
            Ok(())
        } else {
            Err(Error::from_hresult(hr::TS_E_NOLOCK))
        }
    }
    fn require_write_lock(&self) -> Result<()> {
        if self.core.borrow().has_write_lock() {
            Ok(())
        } else {
            Err(Error::from_hresult(hr::TS_E_NOLOCK))
        }
    }
}

/// Adapts the advised `ITextStoreACPSink` to the core's [`StoreSink`]. Held only
/// for the duration of one lock / notification so it never outlives the borrow
/// of `StoreState::sink`.
struct ComSink<'a>(&'a ITextStoreACPSink);

impl StoreSink for ComSink<'_> {
    fn on_lock_granted(&self, flags: u32) -> i32 {
        // SAFETY: the sink is a live COM interface for the borrow's lifetime.
        unsafe { self.0.OnLockGranted(flags) }.0
    }
    fn on_text_change(&self, c: TextChange) {
        let tc = TS_TEXTCHANGE { acpStart: c.start, acpOldEnd: c.old_end, acpNewEnd: c.new_end };
        // SAFETY: `tc` outlives the synchronous call; flags 0 = a normal change.
        unsafe {
            let _ = self.0.OnTextChange(0, &tc);
        }
    }
    fn on_selection_change(&self) {
        unsafe {
            let _ = self.0.OnSelectionChange();
        }
    }
    fn on_layout_change(&self) {
        unsafe {
            let _ = self.0.OnLayoutChange(layout_code::CHANGE, VIEW_COOKIE);
        }
    }
}

/// Map a core `AcpError` to its ACP-store `HRESULT`.
fn map_err(e: AcpError) -> Error {
    let h = match e {
        AcpError::InvalidPos => hr::TS_E_INVALIDPOS,
        AcpError::NoLock => hr::TS_E_NOLOCK,
        AcpError::NoSelection => hr::TS_E_NOSELECTION,
        AcpError::ReadOnly => hr::TS_E_READONLY,
    };
    Error::from_hresult(h)
}

fn rect_of(r: DocRect) -> RECT {
    RECT { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
}

// ─────────────────────────────────────────────────────────────────────────────
// The COM object.
// ─────────────────────────────────────────────────────────────────────────────

/// The `ITextStoreACP` implementer. Construct with [`TextStore::new`]; hand the
/// resulting interface to a TSF context and keep the [`TextInput`] handle for the
/// backend.
pub(crate) struct TextStore(Rc<StoreState>);

implement_decl! {
    impl TextStore as pub(crate) TextStore_Impl: [ITextStoreACP]
}

impl TextStore {
    /// Build a store over `doc`, owned by window `hwnd`.
    pub(crate) fn new(doc: Rc<RefCell<dyn TsfDocument>>, hwnd: isize) -> Self {
        Self(Rc::new(StoreState {
            core: RefCell::new(TextStoreCore::new()),
            sink: RefCell::new(None),
            doc,
            hwnd: hwnd as HWND,
        }))
    }

    /// A backend-side handle to the same store, for change notifications and the
    /// composition guard. Clone-cheap; take it before converting the store into
    /// its `ITextStoreACP` interface.
    pub(crate) fn input(&self) -> TextInput {
        TextInput(Rc::clone(&self.0))
    }

    fn st(&self) -> &StoreState {
        &self.0
    }
}

#[allow(non_snake_case)]
impl ITextStoreACP_Impl for TextStore_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>, _dwmask: u32) -> Result<()> {
        // TSF only ever advises the ACP sink.
        if riid.is_null() || unsafe { *riid } != ITextStoreACPSink::IID {
            return Err(Error::from_hresult(e::INVALIDARG));
        }
        let unk = punk.ok().map_err(|_| Error::from_hresult(e::UNEXPECTED))?;
        let sink: ITextStoreACPSink = unk.cast()?;
        // Re-advising with a new mask keeps the same sink; a single-sink store
        // just replaces it (mask changes are a no-op — we always notify).
        *self.st().sink.borrow_mut() = Some(sink);
        Ok(())
    }

    fn UnadviseSink(&self, _punk: Ref<IUnknown>) -> Result<()> {
        // Single-sink store: clear it. (Identity re-check omitted deliberately —
        // TSF unadvises the one sink it advised; mirrors Mozilla's single slot.)
        *self.st().sink.borrow_mut() = None;
        Ok(())
    }

    fn RequestLock(&self, dwlockflags: u32) -> Result<HRESULT> {
        // No sink advised → nothing can hold a lock (`E_FAIL`, per the contract).
        let sink_guard = self.st().sink.borrow();
        let Some(sink) = sink_guard.as_ref() else {
            return Err(Error::from_hresult(e::FAIL));
        };
        let com = ComSink(sink);
        let session = match run_request_lock(&self.st().core, &com, dwlockflags) {
            LockResult::Granted { session_hr } => HRESULT(session_hr),
            LockResult::Sync => hr::TS_E_SYNCHRONOUS,
            LockResult::Queued => hr::TS_S_ASYNC,
        };
        Ok(session)
    }

    fn GetStatus(&self) -> Result<TS_STATUS> {
        let read_only =
            !self.st().doc.borrow().is_enabled() || self.st().doc.borrow().is_read_only();
        Ok(TS_STATUS {
            dwDynamicFlags: if read_only { TS_SD_READONLY } else { 0 },
            // Only ever plain, fully-visible text — no hidden runs.
            dwStaticFlags: TS_SS_NOHIDDENTEXT,
        })
    }

    fn QueryInsert(
        &self,
        acpteststart: i32,
        acptestend: i32,
        _cch: u32,
        pacpresultstart: *mut i32,
        pacpresultend: *mut i32,
    ) -> Result<()> {
        // The inserted text replaces exactly the tested range (no reconversion
        // expansion in a single-line field). Clamp defensively to the document.
        let len = self.st().doc.borrow().text_len() as i32;
        let s = acpteststart.clamp(0, len);
        let en = acptestend.clamp(s, len);
        unsafe {
            if !pacpresultstart.is_null() {
                *pacpresultstart = s;
            }
            if !pacpresultend.is_null() {
                *pacpresultend = en;
            }
        }
        Ok(())
    }

    fn GetSelection(
        &self,
        ulindex: u32,
        ulcount: u32,
        pselection: *mut TS_SELECTION_ACP,
        pcfetched: *mut u32,
    ) -> Result<()> {
        self.require_read_lock_impl()?;
        unsafe {
            if !pcfetched.is_null() {
                *pcfetched = 0;
            }
        }
        if ulcount == 0 || pselection.is_null() {
            return Ok(());
        }
        // Only the default / caret selection is ever requested (index 0 or the
        // TS_DEFAULT_SELECTION sentinel) — we hold exactly one.
        if ulindex != 0 && ulindex != TS_DEFAULT_SELECTION {
            return Ok(());
        }
        let sel = get_selection(&*self.st().doc.borrow()).map_err(map_err)?;
        let style = TS_SELECTIONSTYLE {
            ase: if sel.reversed { active_end::START } else { active_end::END },
            fInterimChar: BOOL(0),
        };
        unsafe {
            *pselection = TS_SELECTION_ACP {
                acpStart: sel.start as i32,
                acpEnd: sel.end as i32,
                style,
            };
            if !pcfetched.is_null() {
                *pcfetched = 1;
            }
        }
        Ok(())
    }

    fn SetSelection(&self, ulcount: u32, pselection: *const TS_SELECTION_ACP) -> Result<()> {
        self.require_write_lock_impl()?;
        if ulcount == 0 || pselection.is_null() {
            return Ok(());
        }
        let s = unsafe { *pselection };
        let reversed = s.style.ase == active_end::START;
        let sel = DocSelection {
            start: s.acpStart.max(0) as usize,
            end: s.acpEnd.max(0) as usize,
            reversed,
        };
        set_selection(&mut *self.st().doc.borrow_mut(), sel).map_err(map_err)
    }

    fn GetText(
        &self,
        acpstart: i32,
        acpend: i32,
        pchplain: *mut u16,
        cchplainreq: u32,
        pcchplainret: *mut u32,
        prgruninfo: *mut TS_RUNINFO,
        cruninforeq: u32,
        pcruninforet: *mut u32,
        pacpnext: *mut i32,
    ) -> Result<()> {
        self.require_read_lock_impl()?;
        let (units, next) =
            get_text(&*self.st().doc.borrow(), acpstart, acpend, cchplainreq).map_err(map_err)?;
        let n = units.len().min(cchplainreq as usize);
        unsafe {
            if !pchplain.is_null() && n > 0 {
                core::ptr::copy_nonoverlapping(units.as_ptr(), pchplain, n);
            }
            if !pcchplainret.is_null() {
                *pcchplainret = n as u32;
            }
            // One PLAIN run covering everything copied (no hidden/opaque runs).
            let mut run_ret = 0u32;
            if cruninforeq > 0 && !prgruninfo.is_null() && n > 0 {
                *prgruninfo = TS_RUNINFO { uCount: n as u32, r#type: TS_RT_PLAIN };
                run_ret = 1;
            }
            if !pcruninforet.is_null() {
                *pcruninforet = run_ret;
            }
            if !pacpnext.is_null() {
                *pacpnext = next;
            }
        }
        Ok(())
    }

    fn SetText(
        &self,
        _dwflags: u32,
        acpstart: i32,
        acpend: i32,
        pchtext: *const u16,
        cch: u32,
    ) -> Result<TS_TEXTCHANGE> {
        self.require_write_lock_impl()?;
        let text = unsafe { slice_or_empty(pchtext, cch) };
        let change = set_text(&self.st().core, &mut *self.st().doc.borrow_mut(), acpstart, acpend, text)
            .map_err(map_err)?;
        Ok(TS_TEXTCHANGE {
            acpStart: change.start,
            acpOldEnd: change.old_end,
            acpNewEnd: change.new_end,
        })
    }

    // ── Embedded objects: unsupported (plain text only). Stubbed exactly as
    //    Chromium `TSFTextStore` and Mozilla `TSFTextStore` stub them. ──────────
    fn GetFormattedText(&self, _acpstart: i32, _acpend: i32) -> Result<IDataObject> {
        Err(Error::from_hresult(e::NOTIMPL))
    }
    fn GetEmbedded(
        &self,
        _acppos: i32,
        _rguidservice: *const GUID,
        _riid: *const GUID,
        _ppunk: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        Err(Error::from_hresult(e::NOTIMPL))
    }
    fn QueryInsertEmbedded(
        &self,
        _pguidservice: *const GUID,
        _pformatetc: *const FORMATETC,
    ) -> Result<BOOL> {
        // We never accept embedded objects.
        Ok(BOOL(0))
    }
    fn InsertEmbedded(
        &self,
        _dwflags: u32,
        _acpstart: i32,
        _acpend: i32,
        _pdataobject: Ref<IDataObject>,
    ) -> Result<TS_TEXTCHANGE> {
        Err(Error::from_hresult(e::NOTIMPL))
    }
    fn InsertEmbeddedAtSelection(
        &self,
        _dwflags: u32,
        _pdataobject: Ref<IDataObject>,
        _pacpstart: *mut i32,
        _pacpend: *mut i32,
        _pchange: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        Err(Error::from_hresult(e::NOTIMPL))
    }

    fn InsertTextAtSelection(
        &self,
        dwflags: u32,
        pchtext: *const u16,
        cch: u32,
        pacpstart: *mut i32,
        pacpend: *mut i32,
        pchange: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        // A real (non query-only) insert needs a write lock; a query-only probe
        // needs only a read lock.
        if dwflags & super::insert::QUERYONLY != 0 {
            self.require_read_lock_impl()?;
        } else {
            self.require_write_lock_impl()?;
        }
        let text = unsafe { slice_or_empty(pchtext, cch) };
        let (start, end, change) =
            insert_at_selection(&self.st().core, &mut *self.st().doc.borrow_mut(), text, dwflags)
                .map_err(map_err)?;
        unsafe {
            if !pacpstart.is_null() {
                *pacpstart = start;
            }
            if !pacpend.is_null() {
                *pacpend = end;
            }
            if let (false, Some(c)) = (pchange.is_null(), change) {
                *pchange = TS_TEXTCHANGE {
                    acpStart: c.start,
                    acpOldEnd: c.old_end,
                    acpNewEnd: c.new_end,
                };
            }
        }
        Ok(())
    }

    // ── Attributes: none are modelled. A TIP asks which attrs we support, then
    //    retrieves them; the handshake is honoured (and answers "none") so a TIP
    //    never blocks waiting on it. Advertising the focused field's input scope
    //    — so a touch keyboard opens numeric for a NumberBox — means implementing
    //    `ITfInputScope` on the store, and is the natural next refinement. ──────
    fn RequestSupportedAttrs(
        &self,
        _dwflags: u32,
        _cfilterattrs: u32,
        _pafilterattrs: *const TS_ATTRID,
    ) -> Result<()> {
        Ok(())
    }
    fn RequestAttrsAtPosition(
        &self,
        _acppos: i32,
        _cfilterattrs: u32,
        _pafilterattrs: *const TS_ATTRID,
        _dwflags: u32,
    ) -> Result<()> {
        Ok(())
    }
    fn RequestAttrsTransitioningAtPosition(
        &self,
        _acppos: i32,
        _cfilterattrs: u32,
        _pafilterattrs: *const TS_ATTRID,
        _dwflags: u32,
    ) -> Result<()> {
        Ok(())
    }
    fn FindNextAttrTransition(
        &self,
        acpstart: i32,
        acphalt: i32,
        _cfilterattrs: u32,
        _pafilterattrs: *const TS_ATTRID,
        _dwflags: u32,
        pacpnext: *mut i32,
        pffound: *mut BOOL,
        plfoundoffset: *mut i32,
    ) -> Result<()> {
        // No attributes transition anywhere in a uniform plain-text field.
        unsafe {
            if !pacpnext.is_null() {
                *pacpnext = acphalt.max(acpstart);
            }
            if !pffound.is_null() {
                *pffound = BOOL(0);
            }
            if !plfoundoffset.is_null() {
                *plfoundoffset = 0;
            }
        }
        Ok(())
    }
    fn RetrieveRequestedAttrs(
        &self,
        _ulcount: u32,
        _paattrvals: *mut TS_ATTRVAL,
        pcfetched: *mut u32,
    ) -> Result<()> {
        // No attribute objects supplied yet (see the note on RequestSupportedAttrs).
        unsafe {
            if !pcfetched.is_null() {
                *pcfetched = 0;
            }
        }
        Ok(())
    }

    fn GetEndACP(&self) -> Result<i32> {
        self.require_read_lock_impl()?;
        Ok(self.st().doc.borrow().text_len() as i32)
    }

    fn GetActiveView(&self) -> Result<TsViewCookie> {
        Ok(VIEW_COOKIE)
    }

    fn GetACPFromPoint(
        &self,
        _vcview: TsViewCookie,
        _ptscreen: *const POINT,
        _dwflags: u32,
    ) -> Result<i32> {
        // Point→ACP hit-testing is only used by pen/handwriting insertion; not
        // modelled (the editor has no screen-point→index mapping surfaced here).
        // Mozilla likewise returns E_NOTIMPL when it has no view geometry.
        Err(Error::from_hresult(e::NOTIMPL))
    }

    fn GetTextExt(
        &self,
        _vcview: TsViewCookie,
        acpstart: i32,
        acpend: i32,
        prc: *mut RECT,
        pfclipped: *mut BOOL,
    ) -> Result<()> {
        let doc = self.st().doc.borrow();
        let len = doc.text_len();
        let s = acpstart.clamp(0, len as i32) as usize;
        let en = acpend.clamp(s as i32, len as i32) as usize;
        // No layout yet → TS_E_NOLAYOUT, the normal "ask again after layout"
        // answer a TIP handles gracefully.
        let r = doc.range_rect(s, en).ok_or_else(|| Error::from_hresult(hr::TS_E_NOLAYOUT))?;
        unsafe {
            if !prc.is_null() {
                *prc = rect_of(r);
            }
            if !pfclipped.is_null() {
                *pfclipped = BOOL(0);
            }
        }
        Ok(())
    }

    fn GetScreenExt(&self, _vcview: TsViewCookie) -> Result<RECT> {
        Ok(self
            .st()
            .doc
            .borrow()
            .screen_rect()
            .map(rect_of)
            .unwrap_or_default())
    }

    fn GetWnd(&self, _vcview: TsViewCookie) -> Result<HWND> {
        Ok(self.st().hwnd)
    }
}

// Lock-scope helpers on the impl wrapper (deref to `TextStore` → `StoreState`).
impl TextStore_Impl {
    fn require_read_lock_impl(&self) -> Result<()> {
        self.st().require_read_lock()
    }
    fn require_write_lock_impl(&self) -> Result<()> {
        self.st().require_write_lock()
    }
}

/// Borrow `cch` UTF-16 units at `ptr`, or an empty slice if null/zero.
///
/// # Safety
/// `ptr` must point to at least `cch` readable `u16`s (TSF's contract for the
/// text argument), or be null.
unsafe fn slice_or_empty<'a>(ptr: *const u16, cch: u32) -> &'a [u16] {
    if ptr.is_null() || cch == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(ptr, cch as usize) }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend-facing handle.
// ─────────────────────────────────────────────────────────────────────────────

/// The store as the backend sees it: push app-originated changes in, and read /
/// drive the composition guard. Holds the same `StoreState` as the COM object.
#[derive(Clone)]
pub(crate) struct TextInput(Rc<StoreState>);

impl TextInput {
    /// Whether a TIP has advised a sink (i.e. TSF is live on this store).
    pub(crate) fn has_sink(&self) -> bool {
        self.0.sink.borrow().is_some()
    }

    /// The composing run moved or resized (from the context-owner composition
    /// sink — see `comp_sink`). Marks the span on the focused editor: the
    /// underline paints over it and the §7.2 guard refuses programmatic writes
    /// while it is non-empty.
    pub(crate) fn on_composition_update(&self, start: usize, len: usize) {
        self.0.doc.borrow_mut().composition_update(start, len);
    }

    /// The composition ended (committed or cancelled): clear the span, which
    /// also lowers the §7.2 guard. Any committed text already arrived as an
    /// ordinary store edit.
    pub(crate) fn on_composition_end(&self) {
        self.0.doc.borrow_mut().composition_end();
    }

    /// Report an app-originated text change to the TIP (deferred if a lock is
    /// held). No-op when no sink is advised.
    pub(crate) fn notify_text_change(&self, change: TextChange) {
        let guard = self.0.sink.borrow();
        if let Some(sink) = guard.as_ref() {
            notify_app_text_change(&self.0.core, &ComSink(sink), change);
        }
    }

    /// Report an app-originated selection/caret move to the TIP.
    pub(crate) fn notify_selection_change(&self) {
        let guard = self.0.sink.borrow();
        if let Some(sink) = guard.as_ref() {
            notify_app_selection_change(&self.0.core, &ComSink(sink));
        }
    }
}
