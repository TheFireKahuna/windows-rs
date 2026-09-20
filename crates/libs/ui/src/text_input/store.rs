//! The text store is the document. ACP methods read the buffer; none holds a borrow over a
//! TIP call.
//!
//! Adapted from reactor's text-store contract (50e8e569), with a coalesced asynchronous lock
//! request and posted notification delivery. No notification escapes `RequestLock`.

use super::doc::{Change, Command, Doc};
use super::{Affinity, Selection};
use crate::bindings::*;
use core::cell::{Cell, RefCell};
use core::ffi::c_void;
use std::rc::Rc;
use windows_core::{
    BOOL, BSTR, Error, GUID, HRESULT, IUnknown, IUnknownImpl, Interface, Ref, Result,
    implement_decl,
};
use windows_scene::ControlId;
use windows_window::{Hwnd, WM_FRAME};

windows_core::link!("ole32.dll" "system" fn CoTaskMemAlloc(bytes: usize) -> *mut c_void);

const INVALID: HRESULT = HRESULT(0x8007_0057u32 as i32);
const NOMEM: HRESULT = HRESULT(0x8007_000eu32 as i32);
const FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);
pub(crate) const NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);
const NOLOCK: HRESULT = HRESULT(0x8004_0201u32 as i32);
const NOLAYOUT: HRESULT = HRESULT(0x8004_0206u32 as i32);
const INVALIDPOS: HRESULT = HRESULT(0x8004_0200u32 as i32);
const READONLY: HRESULT = HRESULT(0x8004_0209u32 as i32);
const SYNCHRONOUS: HRESULT = HRESULT(0x8004_0208u32 as i32);
const ASYNC: HRESULT = HRESULT(0x0004_0300);
const INPUT_SCOPE: GUID = GUID::from_u128(0x1713dd5a_68e7_4a5b_9af6_592a595c778d);
const VIEW: TsViewCookie = 1;

pub(crate) fn error(code: HRESULT) -> Error {
    Error::from_hresult(code)
}

/// The lock protocol, held in a `Cell` so no borrow of it can span a callback.
///
/// TSF permits one outstanding asynchronous request; a second escalates the first to
/// read/write rather than queuing beside it. A nested synchronous request fails
/// synchronously, and so does any request made while notifications are being delivered.
#[derive(Clone, Copy, Default)]
struct Locks {
    held: u8,
    queued: u8,
    notifying: bool,
}

pub(crate) struct Inner {
    pub doc: Rc<RefCell<Doc>>,
    pub hwnd: Hwnd,
    locks: Cell<Locks>,
    /// What the sink was last told, so a notification is sent only on a real difference.
    seen: Cell<(u64, i32, Selection)>,
    mask: Cell<u32>,
    layout_dirty: Cell<bool>,
    posted: Cell<bool>,
    scope_asked: Cell<bool>,
    sink: RefCell<Option<ITextStoreACPSink>>,
    /// The composition's owning control and the TIP object that opened it. Identity, not
    /// range length, is what a later callback is matched against.
    composition: RefCell<Option<(ControlId, IUnknown)>>,
}

impl Inner {
    pub fn new(doc: Rc<RefCell<Doc>>, hwnd: Hwnd) -> Rc<Self> {
        Rc::new(Self {
            doc,
            hwnd,
            locks: Cell::default(),
            seen: Cell::default(),
            mask: Cell::default(),
            layout_dirty: Cell::default(),
            posted: Cell::default(),
            scope_asked: Cell::default(),
            sink: RefCell::default(),
            composition: RefCell::default(),
        })
    }

    /// Rings the window once. Notifications are delivered from that tick, never from inside
    /// a lock grant.
    pub fn wake(&self) {
        if !self.posted.replace(true) && !self.hwnd.post(WM_FRAME, 0, 0) {
            self.posted.set(false);
        }
    }

    pub fn dirty_layout(&self) {
        self.layout_dirty.set(true);
        self.wake();
    }

    /// Announces that the application, not a TIP, replaced the document.
    ///
    /// The length last reported stays, so the text change that follows names the extent the
    /// sink still believes the document has.
    pub fn changed(&self) {
        let seen = self.seen.get();
        self.seen.set((u64::MAX, seen.1, seen.2));
        self.dirty_layout();
    }

    pub fn composing(&self) -> bool {
        self.composition.borrow().is_some()
    }

    fn state(&self) -> (u64, i32, Selection) {
        let doc = self.doc.borrow();
        (doc.revision, doc.len() as i32, doc.selection())
    }

    /// TSF requires a document lock of the right access before any content method runs.
    fn lock(&self, write: bool) -> Result<()> {
        let want = if write { 4 } else { 2 };
        (self.locks.get().held & want != 0)
            .then_some(())
            .ok_or_else(|| error(NOLOCK))
    }

    /// Validates a half-open ACP range against the buffer, resolving -1 to its end.
    fn range(&self, start: i32, end: i32) -> Result<(u32, u32)> {
        let doc = self.doc.borrow();
        let len = doc
            .id
            .map(|_| doc.len() as i32)
            .ok_or_else(|| error(READONLY))?;
        let end = if end == -1 { len } else { end };
        match start >= 0 && end >= start && end <= len {
            true => Ok((start as u32, end as u32)),
            false => Err(error(INVALIDPOS)),
        }
    }

    /// Runs one TSF-originated mutation and records its result as already seen, so the
    /// application path does not echo the change back as `OnTextChange`.
    fn edit(&self, mutate: impl FnOnce(&mut Doc) -> Option<Change>) -> Result<()> {
        let change = {
            let mut doc = self.doc.borrow_mut();
            let change = mutate(&mut doc);
            doc.emit(change);
            change
        };
        self.seen.set(self.state());
        if change.is_some() {
            self.dirty_layout();
        }
        change.map(|_| ()).ok_or_else(|| error(INVALIDPOS))
    }

    /// Moves the selection through the command queue, so a key already waiting on geometry
    /// still runs first.
    fn select(&self, wanted: Selection) -> Result<()> {
        {
            let mut doc = self.doc.borrow_mut();
            if !doc.accepts(wanted) {
                return Err(error(INVALIDPOS));
            }
            doc.command(Command::Select(wanted));
        }
        self.seen.set(self.state());
        self.wake();
        Ok(())
    }

    /// Converts a control-relative DIP rectangle to screen pixels through the published view.
    fn screen(&self, r: windows_text::Rect) -> Result<RECT> {
        let view = self.doc.borrow().view;
        if view.scale <= 0.0 {
            return Err(error(NOLAYOUT));
        }
        let (x, y) = (
            view.rect.x0 + r.x * view.scale,
            view.rect.y0 + r.y * view.scale,
        );
        Ok(RECT {
            left: x.floor() as i32,
            top: y.floor() as i32,
            right: (x + r.w * view.scale).ceil() as i32,
            bottom: (y + r.h * view.scale).ceil() as i32,
        })
    }

    /// Delivers the queued notifications, then discharges the outstanding asynchronous lock
    /// request. Neither runs inside `RequestLock`.
    pub fn notify(&self) {
        self.posted.set(false);
        let Some(sink) = self.sink.borrow().clone() else {
            return;
        };
        if self.locks.get().held != 0 {
            return;
        }
        self.locks.set(Locks {
            notifying: true,
            ..self.locks.get()
        });
        let now = self.state();
        let was = self.seen.replace(now);
        let mask = self.mask.get();
        // SAFETY: the sink is an owned interface; each call goes through its own vtable slot
        // with the arguments TSF declares, and no document borrow is open across them.
        unsafe {
            if was.0 != now.0 && mask & 1 != 0 {
                let change = TS_TEXTCHANGE {
                    acpStart: 0,
                    acpOldEnd: was.1,
                    acpNewEnd: now.1,
                };
                let _ = (sink.vtable().OnTextChange)(sink.as_raw(), 0, &change);
            }
            if was.2 != now.2 && mask & 2 != 0 {
                let _ = (sink.vtable().OnSelectionChange)(sink.as_raw());
            }
            if self.layout_dirty.replace(false) && mask & 4 != 0 {
                let _ = (sink.vtable().OnLayoutChange)(sink.as_raw(), 1, VIEW);
            }
        }
        self.locks.set(Locks {
            notifying: false,
            ..self.locks.get()
        });
        // A document with keys still waiting on shaped geometry is not readable yet.
        let locks = self.locks.get();
        if locks.queued == 0 || self.doc.borrow().waiting() {
            return;
        }
        self.locks.set(Locks {
            held: locks.queued,
            queued: 0,
            notifying: false,
        });
        // SAFETY: as above. The lock word is a `Cell`, so a reentrant request reads it
        // rather than deadlocking on a borrow.
        let _ = unsafe { (sink.vtable().OnLockGranted)(sink.as_raw(), locks.queued.into()) };
        self.locks.set(Locks {
            held: 0,
            ..self.locks.get()
        });
        if self.locks.get().queued != 0 {
            self.wake();
        }
    }

    /// Drops the sink and any retained composition view outside every framework borrow.
    ///
    /// A TIP that did not send its final callback must not keep context to store to
    /// composition to context alive after the window's owner goes away.
    pub fn release(&self) {
        let composition = self.composition.borrow_mut().take();
        drop(composition);
        let sink = self.sink.borrow_mut().take();
        drop(sink);
    }
}

/// Declares the vtable methods that answer a constant or one statement, so the fixed half of
/// the table reads as a table. The signatures are the platform's; the bodies are not.
///
/// The caller names the receiver, because a `self` this macro introduced would be a
/// different binding from the `self` the bodies are written against.
macro_rules! fixed {
    ($me:ident; $($name:ident($($p:ident: $t:ty),*) $(-> $r:ty)? = $body:expr;)*) => {$(
        #[allow(unused_variables)]
        fn $name(&$me $(, $p: $t)*) -> Result<fixed!(@ty $($r)?)> { $body }
    )*};
    (@ty) => { () };
    (@ty $r:ty) => { $r };
}

fn view(cookie: TsViewCookie) -> Result<()> {
    (cookie == VIEW).then_some(()).ok_or_else(|| error(INVALID))
}

struct Store(Rc<Inner>);
implement_decl! { impl Store as Store_Impl: [ITextStoreACP, ITfContextOwnerCompositionSink, ITfInputScope] }

pub(crate) fn store(inner: &Rc<Inner>) -> ITextStoreACP {
    Store(Rc::clone(inner)).into()
}

impl ITfInputScope_Impl for Store_Impl {
    fixed! {
        self;
        GetPhrase(out: *mut *mut BSTR, count: *mut u32) = Err(error(NOTIMPL));
        GetRegularExpression() -> BSTR = Err(error(NOTIMPL));
        GetSRGS() -> BSTR = Err(error(NOTIMPL));
        GetXML() -> BSTR = Err(error(NOTIMPL));
    }

    fn GetInputScopes(&self, out: *mut *mut InputScope, count: *mut u32) -> Result<()> {
        if out.is_null() || count.is_null() {
            return Err(error(INVALID));
        }
        let scope = match self.0.doc.borrow().scope() {
            super::InputScope::Default => 0,
            super::InputScope::Url => 1,
            super::InputScope::Number => 29,
            super::InputScope::Password => 31,
            super::InputScope::Search => 50,
        };
        // SAFETY: the caller supplies both outputs. COM task memory ownership passes to it.
        unsafe {
            let value = CoTaskMemAlloc(size_of::<InputScope>()).cast::<InputScope>();
            if value.is_null() {
                return Err(error(NOMEM));
            }
            *value = scope;
            *out = value;
            *count = 1;
        }
        Ok(())
    }
}

impl ITextStoreACP_Impl for Store_Impl {
    fixed! {
        self;
        GetFormattedText(a: i32, b: i32) -> IDataObject = Err(error(NOTIMPL));
        GetEmbedded(at: i32, s: *const GUID, i: *const GUID, o: *mut *mut c_void) = Err(error(NOTIMPL));
        InsertEmbedded(f: u32, a: i32, b: i32, d: Ref<IDataObject>) -> TS_TEXTCHANGE = Err(error(NOTIMPL));
        InsertEmbeddedAtSelection(f: u32, d: Ref<IDataObject>, a: *mut i32, b: *mut i32, c: *mut TS_TEXTCHANGE) = Err(error(NOTIMPL));
        QueryInsertEmbedded(s: *const GUID, f: *const FORMATETC) -> BOOL = Ok(BOOL(0));
        GetActiveView() -> TsViewCookie = Ok(VIEW);
        GetWnd(v: TsViewCookie) -> HWND = { view(v)?; Ok(self.0.hwnd.raw()) };
        GetScreenExt(v: TsViewCookie) -> RECT = { view(v)?; let c = self.0.doc.borrow().view.clip; Ok(RECT { left: c.x0 as i32, top: c.y0 as i32, right: c.x1 as i32, bottom: c.y1 as i32 }) };
        GetStatus() -> TS_STATUS = Ok(TS_STATUS { dwDynamicFlags: u32::from(self.0.doc.borrow().id.is_none()), dwStaticFlags: 8 });
        GetEndACP() -> i32 = { self.0.lock(false)?; Ok(self.0.doc.borrow().len() as i32) };
        RequestAttrsTransitioningAtPosition(at: i32, n: u32, a: *const TS_ATTRID, f: u32) = { self.0.range(at, at)?; self.0.scope_asked.set(false); Ok(()) };
        RequestAttrsAtPosition(at: i32, n: u32, a: *const TS_ATTRID, f: u32) = { self.0.range(at, at)?; self.RequestSupportedAttrs(f, n, a) };
        UnadviseSink(object: Ref<IUnknown>) = { let previous = self.0.sink.borrow_mut().take(); drop(previous); Ok(()) };
    }

    fn AdviseSink(&self, iid: *const GUID, object: Ref<IUnknown>, mask: u32) -> Result<()> {
        // SAFETY: TSF passes the interface identifier it is advising for; a null one is the
        // only shape this cannot read.
        if iid.is_null() || unsafe { *iid } != ITextStoreACPSink::IID {
            return Err(error(INVALID));
        }
        let incoming: ITextStoreACPSink = object.ok()?.cast()?;
        // Releasing the previous sink can call out, so it leaves the slot before it drops.
        let previous = self.0.sink.borrow_mut().replace(incoming);
        drop(previous);
        self.0.mask.set(mask);
        Ok(())
    }

    fn RequestLock(&self, flags: u32) -> Result<HRESULT> {
        if !matches!(flags & 6, 2 | 6) || flags & !7 != 0 {
            return Err(error(INVALID));
        }
        let Some(sink) = self.0.sink.borrow().clone() else {
            return Err(error(FAIL));
        };
        let access = (flags & 6) as u8;
        let locks = self.0.locks.get();
        let blocked = locks.notifying || self.0.doc.borrow().waiting();
        if locks.held != 0 || blocked {
            if flags & 1 != 0 {
                return Ok(SYNCHRONOUS);
            }
            self.0.locks.set(Locks {
                queued: locks.queued | access,
                ..locks
            });
            self.0.wake();
            return Ok(ASYNC);
        }
        self.0.locks.set(Locks {
            held: access | locks.queued,
            queued: 0,
            notifying: false,
        });
        // SAFETY: no borrow of any kind survives a callback that can re-enter this vtable:
        // the lock word is a `Cell` and the sink borrow above has already ended.
        let granted =
            unsafe { (sink.vtable().OnLockGranted)(sink.as_raw(), self.0.locks.get().held.into()) };
        self.0.locks.set(Locks {
            held: 0,
            ..self.0.locks.get()
        });
        self.0.wake();
        Ok(granted)
    }

    fn QueryInsert(
        &self,
        start: i32,
        end: i32,
        count: u32,
        out_start: *mut i32,
        out_end: *mut i32,
    ) -> Result<()> {
        let (a, b) = self.0.range(start, end)?;
        if out_start.is_null()
            || out_end.is_null()
            || u64::from(a) + u64::from(count) > i32::MAX as u64
        {
            return Err(error(INVALID));
        }
        // SAFETY: both out-parameters are checked non-null just above.
        unsafe { (*out_start, *out_end) = (a as i32, b as i32) };
        Ok(())
    }

    fn GetSelection(
        &self,
        index: u32,
        count: u32,
        out: *mut TS_SELECTION_ACP,
        fetched: *mut u32,
    ) -> Result<()> {
        self.0.lock(false)?;
        if fetched.is_null() || (count > 0 && out.is_null()) {
            return Err(error(INVALID));
        }
        // SAFETY: `fetched` is checked non-null, and `out` is written only where the caller
        // declared capacity for it.
        unsafe { *fetched = 0 };
        if count == 0 || !matches!(index, 0 | u32::MAX) {
            return Ok(());
        }
        let selection = self.0.doc.borrow().selection();
        let range = selection.range();
        unsafe {
            *out = TS_SELECTION_ACP {
                acpStart: range.start as i32,
                acpEnd: range.end as i32,
                style: TS_SELECTIONSTYLE {
                    ase: if selection.caret < selection.anchor {
                        1
                    } else {
                        2
                    },
                    fInterimChar: BOOL(0),
                },
            };
            *fetched = 1;
        }
        Ok(())
    }

    fn SetSelection(&self, count: u32, input: *const TS_SELECTION_ACP) -> Result<()> {
        self.0.lock(true)?;
        if count != 1 || input.is_null() {
            return Err(error(INVALID));
        }
        // SAFETY: `input` is checked non-null and TSF declares one element for `count` 1.
        let acp = unsafe { *input };
        let (a, b) = self.0.range(acp.acpStart, acp.acpEnd)?;
        if !matches!(acp.style.ase, 0..=2) {
            return Err(error(INVALIDPOS));
        }
        let reversed = acp.style.ase == 1;
        self.0.select(Selection {
            anchor: if reversed { b } else { a },
            caret: if reversed { a } else { b },
            affinity: match reversed {
                true => Affinity::Downstream,
                false => Affinity::Upstream,
            },
        })
    }

    fn GetText(
        &self,
        start: i32,
        end: i32,
        plain: *mut u16,
        capacity: u32,
        written: *mut u32,
        runs: *mut TS_RUNINFO,
        run_capacity: u32,
        run_count: *mut u32,
        next: *mut i32,
    ) -> Result<()> {
        self.0.lock(false)?;
        let (a, b) = self.0.range(start, end)?;
        if written.is_null()
            || run_count.is_null()
            || next.is_null()
            || (capacity > 0 && plain.is_null())
            || (run_capacity > 0 && runs.is_null())
        {
            return Err(error(INVALID));
        }
        // A caller asking for runs alone still advances over the range it counted.
        let n = match (capacity, run_capacity) {
            (0, 0) => 0,
            (0, _) => b - a,
            _ => (b - a).min(capacity),
        };
        let doc = self.0.doc.borrow();
        // SAFETY: ACP bounds and caller capacities are checked; no intermediate allocation.
        unsafe {
            if capacity > 0 && n > 0 {
                core::ptr::copy_nonoverlapping(
                    doc.text().as_ptr().add(a as usize),
                    plain,
                    n as usize,
                );
            }
            (*written, *next, *run_count) = (if capacity > 0 { n } else { 0 }, (a + n) as i32, 0);
            if run_capacity > 0 && n > 0 {
                (*runs, *run_count) = (
                    TS_RUNINFO {
                        uCount: n,
                        r#type: 0,
                    },
                    1,
                );
            }
        }
        Ok(())
    }

    fn SetText(
        &self,
        flags: u32,
        start: i32,
        end: i32,
        text: *const u16,
        count: u32,
    ) -> Result<TS_TEXTCHANGE> {
        self.0.lock(true)?;
        let (a, b) = self.0.range(start, end)?;
        // SAFETY: `units` checks the pointer against the declared count.
        let text = unsafe { units(text, count)? };
        self.0.edit(|doc| doc.replace(a, b, text))?;
        let _ = flags;
        Ok(TS_TEXTCHANGE {
            acpStart: a as i32,
            acpOldEnd: b as i32,
            acpNewEnd: a as i32 + count as i32,
        })
    }

    fn InsertTextAtSelection(
        &self,
        flags: u32,
        text: *const u16,
        count: u32,
        start: *mut i32,
        end: *mut i32,
        change: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        let query_only = flags & 2 != 0;
        self.0.lock(!query_only)?;
        let range = self.0.doc.borrow().range();
        if u64::from(range.start) + u64::from(count) > i32::MAX as u64 {
            return Err(error(INVALID));
        }
        let delta = TS_TEXTCHANGE {
            acpStart: range.start as i32,
            acpOldEnd: range.end as i32,
            acpNewEnd: (range.start + count) as i32,
        };
        if !query_only {
            // SAFETY: `units` checks the pointer against the declared count.
            let text = unsafe { units(text, count)? };
            self.0
                .edit(|doc| doc.replace(range.start, range.end, text))?;
        }
        // SAFETY: every out-parameter is checked non-null before it is written.
        unsafe {
            if flags & 1 == 0 && !start.is_null() && !end.is_null() {
                (*start, *end) = (
                    delta.acpStart,
                    match query_only {
                        true => delta.acpOldEnd,
                        false => delta.acpNewEnd,
                    },
                );
            }
            if !query_only && !change.is_null() {
                *change = delta;
            }
        }
        Ok(())
    }

    fn RequestSupportedAttrs(&self, flags: u32, count: u32, attrs: *const TS_ATTRID) -> Result<()> {
        if count > 0 && attrs.is_null() {
            return Err(error(INVALID));
        }
        // SAFETY: the filter array is read only where the caller declared its length.
        let asked = count == 0
            || unsafe { core::slice::from_raw_parts(attrs, count as usize) }.contains(&INPUT_SCOPE);
        self.0.scope_asked.set(asked);
        let _ = flags;
        Ok(())
    }

    fn FindNextAttrTransition(
        &self,
        start: i32,
        halt: i32,
        count: u32,
        attrs: *const TS_ATTRID,
        flags: u32,
        next: *mut i32,
        found: *mut BOOL,
        offset: *mut i32,
    ) -> Result<()> {
        self.0.range(start, halt)?;
        if next.is_null() || found.is_null() || offset.is_null() {
            return Err(error(INVALID));
        }
        // One scope covers the whole document, so no attribute transitions inside it.
        // SAFETY: all three out-parameters are checked non-null just above.
        unsafe { (*next, *found, *offset) = (halt, BOOL(0), 0) };
        let _ = (count, attrs, flags);
        Ok(())
    }

    fn RetrieveRequestedAttrs(
        &self,
        count: u32,
        attrs: *mut TS_ATTRVAL,
        fetched: *mut u32,
    ) -> Result<()> {
        if fetched.is_null() || (count > 0 && attrs.is_null()) {
            return Err(error(INVALID));
        }
        // SAFETY: `fetched` is checked non-null; `attrs` is written only where the caller
        // declared capacity for at least one value.
        unsafe { *fetched = 0 };
        if count == 0 || !self.0.scope_asked.replace(false) {
            return Ok(());
        }
        // A context's scope is an immutable COM value, returned through TS_ATTRVAL's
        // VT_UNKNOWN. The touch keyboard reads it there; a store that only declares
        // ITfInputScope raises a QWERTY keyboard over a number field.
        let scope: IUnknown = self.to_interface::<ITfInputScope>().cast()?;
        unsafe {
            *attrs = TS_ATTRVAL {
                idAttr: INPUT_SCOPE,
                dwOverlapId: 0,
                varValue: unknown(scope),
            };
            *fetched = 1;
        }
        Ok(())
    }

    fn GetACPFromPoint(&self, v: TsViewCookie, point: *const POINT, flags: u32) -> Result<i32> {
        self.0.lock(false)?;
        view(v)?;
        if point.is_null() {
            return Err(error(INVALID));
        }
        // SAFETY: the point is checked non-null and TSF passes one screen-pixel value.
        let screen = unsafe { *point };
        let doc = self.0.doc.borrow();
        let geometry = doc.geometry().ok_or_else(|| error(NOLAYOUT))?;
        let view = doc.view;
        if view.scale <= 0.0 {
            return Err(error(NOLAYOUT));
        }
        let x = (screen.x as f32 - view.rect.x0) / view.scale - geometry.origin.x;
        let _ = flags;
        Ok(geometry.hit(x).0 as i32)
    }

    fn GetTextExt(
        &self,
        v: TsViewCookie,
        start: i32,
        end: i32,
        out: *mut RECT,
        clipped: *mut BOOL,
    ) -> Result<()> {
        self.0.lock(false)?;
        let (a, b) = self.0.range(start, end)?;
        view(v)?;
        if out.is_null() || clipped.is_null() {
            return Err(error(INVALID));
        }
        // Missing TSF geometry returns TS_E_NOLAYOUT; publication causes a layout
        // notification, which is what makes the TIP ask again.
        let (rect, inside) = {
            let doc = self.0.doc.borrow();
            let geometry = doc.geometry().ok_or_else(|| error(NOLAYOUT))?;
            let mut r = geometry.caret(Selection::at(a));
            if a != b {
                for c in geometry
                    .clusters
                    .iter()
                    .filter(|c| c.start < b && a < c.end)
                {
                    let right = (r.x + r.w).max(c.rect.x + c.rect.w);
                    r.x = r.x.min(c.rect.x);
                    r.w = right - r.x;
                }
            }
            (r.x, r.y) = (r.x + geometry.origin.x, r.y + geometry.origin.y);
            // Cut by the field's own reveal viewport: a scrolled field reports what shows.
            clamp(r, geometry.viewport)
        };
        let screen = self.0.screen(rect)?;
        let clip = self.0.doc.borrow().view.clip;
        // SAFETY: both out-parameters are checked non-null above.
        unsafe {
            *out = RECT {
                left: screen.left.max(clip.x0 as i32),
                top: screen.top.max(clip.y0 as i32),
                right: screen.right.min(clip.x1 as i32),
                bottom: screen.bottom.min(clip.y1 as i32),
            };
            *clipped = BOOL::from(!inside || *out != screen);
        }
        Ok(())
    }
}

impl ITfContextOwnerCompositionSink_Impl for Store_Impl {
    fn OnStartComposition(&self, view: Ref<ITfCompositionView>) -> Result<BOOL> {
        let Some(id) = self.0.doc.borrow().id else {
            return Ok(BOOL(0));
        };
        if self.0.composing() {
            return Ok(BOOL(0));
        }
        let view = view.ok()?;
        // A TIP can reenter during either call. Resolve its COM identity before taking
        // document state, then check that the same control generation still owns focus.
        let identity: IUnknown = view.cast()?;
        // SAFETY: `view` is an owned interface for the duration of this call.
        let Some(range) = extent(&unsafe { view.GetRange() }?) else {
            return Ok(BOOL(0));
        };
        let opened = {
            let mut doc = self.0.doc.borrow_mut();
            let change = (doc.id == Some(id))
                .then(|| doc.compose(Some(range)))
                .flatten();
            doc.emit(change);
            change.is_some()
        };
        if opened {
            *self.0.composition.borrow_mut() = Some((id, identity));
            self.0.wake();
        }
        Ok(BOOL::from(opened))
    }

    fn OnUpdateComposition(
        &self,
        view: Ref<ITfCompositionView>,
        range: Ref<ITfRange>,
    ) -> Result<()> {
        let identity: IUnknown = view.ok()?.cast()?;
        let Some(range) = range
            .as_ref()
            .and_then(extent)
            .filter(|_| self.owns(&identity))
        else {
            return Ok(());
        };
        let _ = self.0.edit(|doc| doc.compose(Some(range)));
        self.0.wake();
        Ok(())
    }

    fn OnEndComposition(&self, view: Ref<ITfCompositionView>) -> Result<()> {
        let identity: IUnknown = view.ok()?.cast()?;
        if !self.owns(&identity) {
            return Ok(());
        }
        // Release the retained TIP outside every framework borrow.
        let ended = self.0.composition.borrow_mut().take();
        drop(ended);
        let _ = self.0.edit(|doc| doc.compose(None));
        self.0.doc.borrow_mut().settle();
        self.0.wake();
        Ok(())
    }
}

impl Store_Impl {
    /// Reports whether `identity` is the TIP object that opened the live composition. A
    /// reused control slot cannot be addressed by a late callback, because the pair holds
    /// the generation-checked id it was opened for.
    fn owns(&self, identity: &IUnknown) -> bool {
        let held = self.0.composition.borrow();
        held.as_ref()
            .is_some_and(|(id, open)| open == identity && Some(*id) == self.0.doc.borrow().id)
    }
}

/// Returns `r` cut to `bounds` horizontally, and whether it was already inside them.
fn clamp(r: windows_text::Rect, bounds: windows_text::Rect) -> (windows_text::Rect, bool) {
    let inside = r.x >= bounds.x && r.x + r.w <= bounds.x + bounds.w;
    let right = (r.x + r.w).min(bounds.x + bounds.w);
    let x = r.x.max(bounds.x);
    (
        windows_text::Rect {
            x,
            w: (right - x).max(0.0),
            ..r
        },
        inside,
    )
}

fn extent(range: &ITfRange) -> Option<(u32, u32)> {
    let acp: ITfRangeACP = range.cast().ok()?;
    let (mut start, mut count) = (0, 0);
    // SAFETY: both out-parameters are local and live across the call.
    unsafe { acp.GetExtent(&mut start, &mut count) }.ok().ok()?;
    if start < 0 || count < 0 {
        return None;
    }
    let start = start as u32;
    Some((start, start.checked_add(count as u32)?))
}

/// # Safety
///
/// `pointer` must address `count` readable code units for the returned slice's lifetime.
unsafe fn units<'a>(pointer: *const u16, count: u32) -> Result<&'a [u16]> {
    match (count, pointer.is_null()) {
        (0, _) => Ok(&[]),
        (_, true) => Err(error(INVALID)),
        _ => Ok(unsafe { core::slice::from_raw_parts(pointer, count as usize) }),
    }
}

/// Wraps an interface pointer as a VT_UNKNOWN variant, which the bindings do not build.
fn unknown(value: IUnknown) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: core::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: 13,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    punkVal: core::mem::ManuallyDrop::new(Some(value)),
                },
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::{InputScope, Source};
    use super::*;

    struct Sink {
        grant: Box<dyn Fn(u32)>,
        changed: Box<dyn Fn()>,
        events: Rc<RefCell<Vec<&'static str>>>,
    }
    implement_decl! { impl Sink as Sink_Impl: [ITextStoreACPSink] }
    impl ITextStoreACPSink_Impl for Sink_Impl {
        fn OnTextChange(&self, _: u32, _: *const TS_TEXTCHANGE) -> Result<()> {
            self.events.borrow_mut().push("text");
            (self.changed)();
            Ok(())
        }
        fn OnSelectionChange(&self) -> Result<()> {
            self.events.borrow_mut().push("selection");
            Ok(())
        }
        fn OnLayoutChange(&self, _: TsLayoutCode, _: TsViewCookie) -> Result<()> {
            self.events.borrow_mut().push("layout");
            Ok(())
        }
        fn OnStatusChange(&self, _: u32) -> Result<()> {
            Ok(())
        }
        fn OnAttrsChange(&self, _: i32, _: i32, _: u32, _: *const TS_ATTRID) -> Result<()> {
            Ok(())
        }
        fn OnLockGranted(&self, access: u32) -> Result<()> {
            self.events.borrow_mut().push("grant");
            (self.grant)(access);
            Ok(())
        }
        fn OnStartEditTransaction(&self) -> Result<()> {
            Ok(())
        }
        fn OnEndEditTransaction(&self) -> Result<()> {
            Ok(())
        }
    }

    /// A store over one focused, empty field, with every notification mask bit set.
    fn inner(hwnd: Hwnd) -> Rc<Inner> {
        let id = ControlId::FIRST;
        let mut doc = Doc::default();
        doc.source(&Source {
            id,
            scope: InputScope::Default,
            based_on: 0,
            text: Vec::new().into(),
        });
        doc.focus(Some(id));
        let inner = Inner::new(Rc::new(RefCell::new(doc)), hwnd);
        inner.mask.set(7);
        inner
    }

    fn request(store: &ITextStoreACP, flags: u32) -> HRESULT {
        let mut result = HRESULT(0);
        unsafe { (store.vtable().RequestLock)(store.as_raw(), flags, &mut result) }
            .ok()
            .unwrap();
        result
    }

    #[test]
    fn native_store_coalesces_reentrant_locks_without_notifications_inside_request() {
        let window = windows_window::Window::new("TSF protocol test")
            .create()
            .unwrap();
        let inner = inner(window.handle());
        let store = store(&inner);
        let events = Rc::new(RefCell::new(Vec::new()));
        let grants = Rc::new(RefCell::new(Vec::new()));
        let sink: ITextStoreACPSink = Sink {
            events: events.clone(),
            changed: Box::new(|| {}),
            grant: Box::new({
                let store = store.clone();
                let grants = grants.clone();
                move |access| {
                    let first = grants.borrow().is_empty();
                    grants.borrow_mut().push(access);
                    if first {
                        assert_eq!(request(&store, 3), SYNCHRONOUS);
                        for flags in [2, 6, 2, 6] {
                            assert_eq!(request(&store, flags), ASYNC);
                        }
                    }
                }
            }),
        }
        .into();
        *inner.sink.borrow_mut() = Some(sink);
        assert_eq!(request(&store, 2), HRESULT(0));
        assert_eq!(*grants.borrow(), [2]);
        assert_eq!(*events.borrow(), ["grant"]);
        inner.notify();
        assert_eq!(*grants.borrow(), [2, 6]);
        inner.release();
    }

    #[test]
    fn notifications_defer_reentrant_grants_and_tsf_edits_are_not_echoed() {
        let window = windows_window::Window::new("TSF notification test")
            .create()
            .unwrap();
        let inner = inner(window.handle());
        let store = store(&inner);
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink: ITextStoreACPSink = Sink {
            events: events.clone(),
            changed: Box::new({
                let store = store.clone();
                move || {
                    assert_eq!(request(&store, 6), ASYNC);
                }
            }),
            grant: Box::new({
                let store = store.clone();
                move |_| {
                    let mut change = TS_TEXTCHANGE::default();
                    let text = [98u16];
                    unsafe {
                        (store.vtable().SetText)(
                            store.as_raw(),
                            0,
                            0,
                            1,
                            text.as_ptr(),
                            1,
                            &mut change,
                        )
                    }
                    .ok()
                    .unwrap();
                }
            }),
        }
        .into();
        *inner.sink.borrow_mut() = Some(sink);
        let change = inner.doc.borrow_mut().replace(0, 0, &[97]);
        inner.doc.borrow_mut().emit(change);
        inner.notify();
        assert_eq!(inner.doc.borrow().text(), [98]);
        assert_eq!(events.borrow().iter().filter(|e| **e == "text").count(), 1);
        inner.notify();
        assert_eq!(events.borrow().iter().filter(|e| **e == "text").count(), 1);
        inner.release();
    }

    /// Several ACP writes inside one grant leave the document holding the last of them, and
    /// none of them is echoed back to the sink as an application change.
    #[test]
    fn several_acp_writes_in_one_grant_are_not_echoed_back_to_the_sink() {
        let window = windows_window::Window::new("TSF transaction test")
            .create()
            .unwrap();
        let inner = inner(window.handle());
        let store = store(&inner);
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink: ITextStoreACPSink = Sink {
            events: events.clone(),
            changed: Box::new(|| {}),
            grant: Box::new({
                let store = store.clone();
                move |_| {
                    for (start, end, unit) in [(0i32, 0i32, 110u16), (0, 1, 105), (0, 1, 0x306b)] {
                        let mut change = TS_TEXTCHANGE::default();
                        let text = [unit];
                        unsafe {
                            (store.vtable().SetText)(
                                store.as_raw(),
                                0,
                                start,
                                end,
                                text.as_ptr(),
                                1,
                                &mut change,
                            )
                        }
                        .ok()
                        .unwrap();
                    }
                }
            }),
        }
        .into();
        *inner.sink.borrow_mut() = Some(sink);
        assert_eq!(request(&store, 6), HRESULT(0));
        assert_eq!(inner.doc.borrow().text(), [0x306b]);
        assert_eq!(*events.borrow(), ["grant"], "one grant, and no echo");
        inner.notify();
        assert_eq!(
            events.borrow().iter().filter(|e| **e == "text").count(),
            0,
            "a TSF-originated write is already the sink's own knowledge"
        );
        inner.release();
    }

    /// A composition callback naming a view the store never accepted belongs to a text service
    /// whose composition is over. It moves nothing.
    #[test]
    fn a_composition_callback_from_a_view_the_store_does_not_hold_is_ignored() {
        let window = windows_window::Window::new("TSF composition identity test")
            .create()
            .unwrap();
        let inner = inner(window.handle());
        let store = store(&inner);
        let change = inner.doc.borrow_mut().replace(0, 0, &[97]);
        inner.doc.borrow_mut().emit(change);
        let before = inner.doc.borrow().revision;

        // The store holds no composition, so no callback identity can match one.
        let sink: ITfContextOwnerCompositionSink = store.cast().unwrap();
        let foreign: IUnknown = store.cast().unwrap();
        assert!(!inner.composing());
        let ended = unsafe {
            (sink.vtable().OnEndComposition)(sink.as_raw(), core::mem::transmute_copy(&foreign))
        };
        assert!(ended.is_ok());
        assert!(!inner.composing());
        assert_eq!(inner.doc.borrow().revision, before, "nothing moved");
        inner.release();
    }
    #[test]
    fn invalid_selection_and_unlocked_edit_leave_document_unchanged() {
        let window = windows_window::Window::new("TSF ACP test")
            .create()
            .unwrap();
        let inner = inner(window.handle());
        let store = store(&inner);
        let change = inner.doc.borrow_mut().replace(0, 0, &[0xd83d, 0xde00]);
        inner.doc.borrow_mut().emit(change);
        let mut delta = TS_TEXTCHANGE::default();
        assert_eq!(
            unsafe {
                (store.vtable().SetText)(store.as_raw(), 0, 0, 0, core::ptr::null(), 0, &mut delta)
            },
            NOLOCK
        );
        inner.locks.set(Locks {
            held: 6,
            ..Locks::default()
        });
        let selection = TS_SELECTION_ACP {
            acpStart: 1,
            acpEnd: 1,
            style: TS_SELECTIONSTYLE {
                ase: 2,
                fInterimChar: BOOL(0),
            },
        };
        assert_eq!(
            unsafe { (store.vtable().SetSelection)(store.as_raw(), 1, &selection) },
            INVALIDPOS
        );
        assert_eq!(inner.doc.borrow().text(), [0xd83d, 0xde00]);
        inner.release();
    }
}
