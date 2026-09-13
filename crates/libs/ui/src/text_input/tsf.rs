//! ACP translation and STA lifetime. The only calls into a TIP are outside document borrows.
//!
//! Adapted from reactor's text-store contract (50e8e569), with a coalesced asynchronous
//! lock request and posted notification delivery. No notification escapes RequestLock.

use super::{
    Affinity, Command, InputScope as Scope, Selection,
    protocol::{Locks, Request},
    runtime::Documents,
};
use crate::bindings::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows_core::{
    BOOL, BSTR, Error, GUID, HRESULT, IUnknown, Interface, Ref, Result, implement_decl,
};
use windows_window::{Hwnd, KeyMessage, WM_FRAME};

windows_core::link!("ole32.dll" "system" fn CoCreateInstance(class: *const GUID, outer: *mut core::ffi::c_void, context: u32, iid: *const GUID, out: *mut *mut core::ffi::c_void) -> HRESULT);
windows_core::link!("ole32.dll" "system" fn CoTaskMemAlloc(bytes: usize) -> *mut core::ffi::c_void);
const INVALID: HRESULT = HRESULT(0x80070057u32 as i32);
const NOLOCK: HRESULT = HRESULT(0x80040201u32 as i32);
const NOLAYOUT: HRESULT = HRESULT(0x80040206u32 as i32);
const INVALIDPOS: HRESULT = HRESULT(0x80040200u32 as i32);
const READONLY: HRESULT = HRESULT(0x80040209u32 as i32);
const NOTIMPL: HRESULT = HRESULT(0x80004001u32 as i32);
const INPUT_SCOPE: GUID = GUID::from_u128(0x1713dd5a_68e7_4a5b_9af6_592a595c778d);
fn error(code: HRESULT) -> Error {
    Error::from_hresult(code)
}

struct State {
    docs: Rc<RefCell<Documents>>,
    locks: RefCell<Locks>,
    sink: RefCell<Option<Rc<ITextStoreACPSink>>>,
    mask: Cell<u32>,
    requested_scope: Cell<bool>,
    composition: RefCell<Option<(windows_scene::ControlId, IUnknown)>>,
    hwnd: Hwnd,
    posted: Cell<bool>,
    layout_dirty: Cell<bool>,
    seen_revision: Cell<u64>,
    seen_len: Cell<i32>,
    seen_selection: Cell<Selection>,
}

impl State {
    fn wake(&self) {
        if !self.posted.replace(true) && !self.hwnd.post(WM_FRAME, 0, 0) {
            self.posted.set(false);
        }
    }
    fn lock(&self, write: bool) -> Result<()> {
        let held = self.locks.borrow().held;
        if held & if write { 4 } else { 2 } == 0 {
            Err(error(NOLOCK))
        } else {
            Ok(())
        }
    }
    fn seen(&self) {
        if let Some(e) = self.docs.borrow().active() {
            self.seen_revision.set(e.revision);
            self.seen_len.set(e.text.len() as i32);
            self.seen_selection.set(e.selection());
        }
    }
    fn range(&self, start: i32, end: i32) -> Result<(u32, u32)> {
        let docs = self.docs.borrow();
        let e = docs.active().ok_or_else(|| error(READONLY))?;
        let end = if end == -1 { e.text.len() as i32 } else { end };
        if start < 0 || end < start || end as usize > e.text.len() {
            return Err(error(INVALIDPOS));
        }
        Ok((start as u32, end as u32))
    }
    fn replace(&self, start: u32, end: u32, text: &[u16]) -> Result<TS_TEXTCHANGE> {
        {
            let mut docs = self.docs.borrow_mut();
            let e = docs.active_mut().ok_or_else(|| error(READONLY))?;
            if !e.replace(start, end, text) {
                return Err(error(INVALIDPOS));
            }
        }
        self.seen();
        self.layout_dirty.set(true);
        self.wake();
        Ok(TS_TEXTCHANGE {
            acpStart: start as i32,
            acpOldEnd: end as i32,
            acpNewEnd: (start as usize + text.len()) as i32,
        })
    }
    fn screen(&self, rect: windows_text::Rect) -> Result<RECT> {
        let docs = self.docs.borrow();
        let origin = docs.origin.ok_or_else(|| error(NOLAYOUT))?;
        Ok(RECT {
            left: origin.x as i32 + (rect.x * docs.scale).floor() as i32,
            top: origin.y as i32 + (rect.y * docs.scale).floor() as i32,
            right: origin.x as i32 + ((rect.x + rect.w) * docs.scale).ceil() as i32,
            bottom: origin.y as i32 + ((rect.y + rect.h) * docs.scale).ceil() as i32,
        })
    }
}

struct Store(Rc<State>);
implement_decl! { impl Store as Store_Impl: [ITextStoreACP, ITfContextOwnerCompositionSink] }

/// A context's scope is an immutable COM value, returned through TS_ATTRVAL's VT_UNKNOWN.
struct ScopeValue(i32);
implement_decl! { impl ScopeValue as ScopeValue_Impl: [ITfInputScope] }
impl ITfInputScope_Impl for ScopeValue_Impl {
    fn GetInputScopes(&self, out: *mut *mut InputScope, count: *mut u32) -> Result<()> {
        if out.is_null() || count.is_null() {
            return Err(error(INVALID));
        }
        // SAFETY: the caller supplies both outputs. COM task memory ownership passes to it.
        unsafe {
            *out = core::ptr::null_mut();
            *count = 0;
            let value = CoTaskMemAlloc(size_of::<i32>()).cast::<i32>();
            if value.is_null() {
                return Err(error(HRESULT(0x8007000eu32 as i32)));
            }
            *value = self.0;
            *out = value;
            *count = 1;
        }
        Ok(())
    }
    fn GetPhrase(&self, _out: *mut *mut BSTR, _count: *mut u32) -> Result<()> {
        Err(error(NOTIMPL))
    }
    fn GetRegularExpression(&self) -> Result<BSTR> {
        Err(error(NOTIMPL))
    }
    fn GetSRGS(&self) -> Result<BSTR> {
        Err(error(NOTIMPL))
    }
    fn GetXML(&self) -> Result<BSTR> {
        Err(error(NOTIMPL))
    }
}

impl ITextStoreACP_Impl for Store_Impl {
    fn AdviseSink(&self, iid: *const GUID, object: Ref<IUnknown>, mask: u32) -> Result<()> {
        if iid.is_null() || unsafe { *iid } != ITextStoreACPSink::IID {
            return Err(error(INVALID));
        }
        let incoming: ITextStoreACPSink = object.ok()?.cast()?;
        let previous = self.0.sink.borrow().clone();
        if let Some(old) = previous
            && old.cast::<IUnknown>()? != incoming.cast::<IUnknown>()?
        {
            return Err(error(HRESULT(0x80040204u32 as i32)));
        }
        let previous = self.0.sink.borrow_mut().replace(Rc::new(incoming));
        drop(previous); // Release outside the registry borrow.
        self.0.mask.set(mask);
        Ok(())
    }
    fn UnadviseSink(&self, object: Ref<IUnknown>) -> Result<()> {
        let previous = self.0.sink.borrow().clone();
        if !previous.is_some_and(|s| s.cast::<IUnknown>().ok().as_ref() == object.as_ref()) {
            return Err(error(HRESULT(0x80040204u32 as i32)));
        }
        let previous = self.0.sink.borrow_mut().take();
        drop(previous); // Release can call out; the slot borrow is already gone.
        Ok(())
    }
    fn RequestLock(&self, flags: u32) -> Result<HRESULT> {
        if !matches!(flags & 6, 2 | 6) || flags & !7 != 0 {
            return Err(error(INVALID));
        }
        let sink = self
            .0
            .sink
            .borrow()
            .clone()
            .ok_or_else(|| error(HRESULT(0x80004005u32 as i32)))?;
        let blocked = self.0.docs.borrow().active().is_some_and(|e| e.waiting());
        let request = {
            let mut locks = self.0.locks.borrow_mut();
            let notifying = locks.notifying;
            locks.notifying |= blocked;
            let request = locks.request(flags);
            locks.notifying = notifying;
            request
        };
        Ok(match request {
            Request::Synchronous => HRESULT(0x80040208u32 as i32),
            Request::Async => {
                self.0.wake();
                HRESULT(0x00040300)
            }
            Request::Grant(access) => {
                // No RefCell borrow survives a callback that can re-enter this vtable.
                let result = unsafe { (sink.vtable().OnLockGranted)(sink.as_raw(), access) };
                self.0.locks.borrow_mut().release();
                self.0.wake();
                result
            }
        })
    }
    fn GetStatus(&self) -> Result<TS_STATUS> {
        Ok(TS_STATUS {
            dwDynamicFlags: if self.0.docs.borrow().active().is_none() {
                1
            } else {
                0
            },
            dwStaticFlags: 8,
        })
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
        if out_start.is_null() || out_end.is_null() || a as u64 + count as u64 > i32::MAX as u64 {
            return Err(error(INVALID));
        }
        unsafe {
            *out_start = a as i32;
            *out_end = b as i32;
        }
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
        unsafe {
            *fetched = 0;
        }
        if count == 0 || !matches!(index, 0 | u32::MAX) {
            return Ok(());
        }
        let docs = self.0.docs.borrow();
        let e = docs.active().ok_or_else(|| error(READONLY))?;
        let s = e.selection();
        let range = s.range();
        unsafe {
            *out = TS_SELECTION_ACP {
                acpStart: range.start as i32,
                acpEnd: range.end as i32,
                style: TS_SELECTIONSTYLE {
                    ase: if s.caret < s.anchor { 1 } else { 2 },
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
        let s = unsafe { *input };
        let (a, b) = self.0.range(s.acpStart, s.acpEnd)?;
        let selection = if s.style.ase == 1 {
            Selection {
                anchor: b,
                caret: a,
                affinity: Affinity::Downstream,
            }
        } else {
            Selection {
                anchor: a,
                caret: b,
                affinity: Affinity::Downstream,
            }
        };
        {
            let mut docs = self.0.docs.borrow_mut();
            let e = docs.active_mut().ok_or_else(|| error(READONLY))?;
            if !e.accepts_selection(selection) || !matches!(s.style.ase, 0..=2) {
                return Err(error(INVALIDPOS));
            }
            e.command(Command::Select(selection));
        }
        self.0.seen();
        self.0.wake();
        Ok(())
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
        let n = if capacity == 0 {
            (b - a).min(if run_capacity > 0 { u32::MAX } else { 0 })
        } else {
            (b - a).min(capacity)
        };
        let docs = self.0.docs.borrow();
        let e = docs.active().ok_or_else(|| error(READONLY))?;
        // SAFETY: ACP bounds and caller capacities are checked; no intermediate allocation.
        unsafe {
            if capacity > 0 {
                core::ptr::copy_nonoverlapping(e.text.as_ptr().add(a as usize), plain, n as usize);
            }
            *written = if capacity > 0 { n } else { 0 };
            *next = (a + n) as i32;
            *run_count = 0;
            if run_capacity > 0 && n > 0 {
                *runs = TS_RUNINFO {
                    uCount: n,
                    r#type: 0,
                };
                *run_count = 1;
            }
        }
        Ok(())
    }
    fn SetText(
        &self,
        _flags: u32,
        start: i32,
        end: i32,
        text: *const u16,
        count: u32,
    ) -> Result<TS_TEXTCHANGE> {
        self.0.lock(true)?;
        let (a, b) = self.0.range(start, end)?;
        self.0.replace(a, b, unsafe { units(text, count)? })
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
        self.0.lock(flags & 2 == 0)?;
        let range = self
            .0
            .docs
            .borrow()
            .active()
            .ok_or_else(|| error(READONLY))?
            .selection()
            .range();
        if range.start as u64 + count as u64 > i32::MAX as u64 {
            return Err(error(INVALID));
        }
        let delta = if flags & 2 == 0 {
            Some(
                self.0
                    .replace(range.start, range.end, unsafe { units(text, count)? })?,
            )
        } else {
            None
        };
        unsafe {
            if flags & 1 == 0 {
                if !start.is_null() {
                    *start = range.start as i32;
                }
                if !end.is_null() {
                    *end = (range.start + count) as i32;
                }
            }
            if !change.is_null()
                && let Some(delta) = delta
            {
                *change = delta;
            }
        }
        Ok(())
    }
    fn GetFormattedText(&self, _start: i32, _end: i32) -> Result<IDataObject> {
        Err(error(NOTIMPL))
    }
    fn GetEmbedded(
        &self,
        _at: i32,
        _service: *const GUID,
        _iid: *const GUID,
        _out: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        Err(error(NOTIMPL))
    }
    fn QueryInsertEmbedded(
        &self,
        _service: *const GUID,
        _format: *const FORMATETC,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
    }
    fn InsertEmbedded(
        &self,
        _flags: u32,
        _start: i32,
        _end: i32,
        _data: Ref<IDataObject>,
    ) -> Result<TS_TEXTCHANGE> {
        Err(error(NOTIMPL))
    }
    fn InsertEmbeddedAtSelection(
        &self,
        _flags: u32,
        _data: Ref<IDataObject>,
        _start: *mut i32,
        _end: *mut i32,
        _change: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        Err(error(NOTIMPL))
    }
    fn RequestSupportedAttrs(
        &self,
        _flags: u32,
        count: u32,
        attrs: *const TS_ATTRID,
    ) -> Result<()> {
        if count > 0 && attrs.is_null() {
            return Err(error(INVALID));
        }
        self.0.requested_scope.set(
            count == 0
                || unsafe { core::slice::from_raw_parts(attrs, count as usize) }
                    .contains(&INPUT_SCOPE),
        );
        Ok(())
    }
    fn RequestAttrsAtPosition(
        &self,
        at: i32,
        count: u32,
        attrs: *const TS_ATTRID,
        flags: u32,
    ) -> Result<()> {
        self.0.range(at, at)?;
        self.RequestSupportedAttrs(flags, count, attrs)
    }
    fn RequestAttrsTransitioningAtPosition(
        &self,
        at: i32,
        _count: u32,
        _attrs: *const TS_ATTRID,
        _flags: u32,
    ) -> Result<()> {
        self.0.range(at, at)?;
        self.0.requested_scope.set(false);
        Ok(())
    }
    fn FindNextAttrTransition(
        &self,
        start: i32,
        halt: i32,
        _count: u32,
        _attrs: *const TS_ATTRID,
        _flags: u32,
        next: *mut i32,
        found: *mut BOOL,
        offset: *mut i32,
    ) -> Result<()> {
        self.0.range(start, halt)?;
        if next.is_null() || found.is_null() || offset.is_null() {
            return Err(error(INVALID));
        }
        unsafe {
            *next = halt;
            *found = BOOL(0);
            *offset = 0;
        }
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
        unsafe {
            *fetched = 0;
        }
        if count == 0 || !self.0.requested_scope.replace(false) {
            return Ok(());
        }
        let scope = self
            .0
            .docs
            .borrow()
            .active()
            .map_or(Scope::Default, |e| e.scope);
        let value: ITfInputScope = ScopeValue(match scope {
            Scope::Default => 0,
            Scope::Url => 1,
            Scope::Number => 29,
            Scope::Password => 31,
            Scope::Search => 50,
        })
        .into();
        let unknown = value.cast::<IUnknown>()?;
        unsafe {
            *attrs = TS_ATTRVAL {
                idAttr: INPUT_SCOPE,
                dwOverlapId: 0,
                varValue: VARIANT {
                    Anonymous: VARIANT_0 {
                        Anonymous: core::mem::ManuallyDrop::new(VARIANT_0_0 {
                            vt: 13,
                            wReserved1: 0,
                            wReserved2: 0,
                            wReserved3: 0,
                            Anonymous: VARIANT_0_0_0 {
                                punkVal: core::mem::ManuallyDrop::new(Some(unknown)),
                            },
                        }),
                    },
                },
            };
            *fetched = 1;
        }
        Ok(())
    }
    fn GetEndACP(&self) -> Result<i32> {
        self.0.lock(false)?;
        Ok(self
            .0
            .docs
            .borrow()
            .active()
            .map_or(0, |e| e.text.len() as i32))
    }
    fn GetActiveView(&self) -> Result<TsViewCookie> {
        Ok(1)
    }
    fn GetWnd(&self, view: TsViewCookie) -> Result<HWND> {
        if view != 1 {
            return Err(error(INVALID));
        }
        Ok(self.0.hwnd.raw())
    }
    fn GetScreenExt(&self, view: TsViewCookie) -> Result<RECT> {
        if view != 1 {
            return Err(error(INVALID));
        }
        let r = self.0.docs.borrow().clip;
        self.0.screen(windows_text::Rect {
            x: r.x0,
            y: r.y0,
            w: r.width(),
            h: r.height(),
        })
    }
    fn GetTextExt(
        &self,
        view: TsViewCookie,
        start: i32,
        end: i32,
        out: *mut RECT,
        clipped: *mut BOOL,
    ) -> Result<()> {
        self.0.lock(false)?;
        let (a, b) = self.0.range(start, end)?;
        if view != 1 || out.is_null() || clipped.is_null() {
            return Err(error(INVALID));
        }
        let (rect, was_clipped) = {
            let docs = self.0.docs.borrow();
            let e = docs.active().ok_or_else(|| error(NOLAYOUT))?;
            let g = e
                .geometry
                .as_ref()
                .filter(|g| g.revision == e.revision)
                .ok_or_else(|| error(NOLAYOUT))?;
            let mut r = g.caret(Selection {
                caret: a,
                ..Selection::default()
            });
            if a != b {
                for c in g.clusters.iter() {
                    if c.start < b && a < c.end {
                        let right = (r.x + r.w).max(c.rect.x + c.rect.w);
                        r.x = r.x.min(c.rect.x);
                        r.w = right - r.x;
                    }
                }
            }
            r.x += g.origin.x;
            r.y += g.origin.y;
            let clipped = r.x < g.viewport.x || r.x + r.w > g.viewport.x + g.viewport.w;
            let right = (r.x + r.w).min(g.viewport.x + g.viewport.w);
            r.x = r.x.max(g.viewport.x);
            r.w = (right - r.x).max(0.0);
            r.x += docs.rect.x0;
            r.y += docs.rect.y0;
            let clip = docs.clip;
            let outside =
                r.x < clip.x0 || r.y < clip.y0 || r.x + r.w > clip.x1 || r.y + r.h > clip.y1;
            let right = (r.x + r.w).min(clip.x1);
            let bottom = (r.y + r.h).min(clip.y1);
            r.x = r.x.max(clip.x0);
            r.y = r.y.max(clip.y0);
            r.w = (right - r.x).max(0.0);
            r.h = (bottom - r.y).max(0.0);
            (r, clipped || outside)
        };
        unsafe {
            *out = self.0.screen(rect)?;
            *clipped = BOOL::from(was_clipped);
        }
        Ok(())
    }
    fn GetACPFromPoint(&self, view: TsViewCookie, point: *const POINT, _flags: u32) -> Result<i32> {
        self.0.lock(false)?;
        if view != 1 || point.is_null() {
            return Err(error(INVALID));
        }
        let screen = unsafe { *point };
        let docs = self.0.docs.borrow();
        let origin = docs.origin.ok_or_else(|| error(NOLAYOUT))?;
        let e = docs.active().ok_or_else(|| error(NOLAYOUT))?;
        let g = e
            .geometry
            .as_ref()
            .filter(|g| g.revision == e.revision)
            .ok_or_else(|| error(NOLAYOUT))?;
        Ok(
            g.hit((screen.x as f32 - origin.x) / docs.scale - docs.rect.x0 - g.origin.x)
                .0 as i32,
        )
    }
}

impl ITfContextOwnerCompositionSink_Impl for Store_Impl {
    fn OnStartComposition(&self, view: Ref<ITfCompositionView>) -> Result<BOOL> {
        let Some(id) = self.0.docs.borrow().active else {
            return Ok(BOOL(0));
        };
        if self.0.composition.borrow().is_some() {
            return Ok(BOOL(0));
        }
        let view = view.ok()?;
        // A TIP can reenter during either call. Resolve its COM identity before taking
        // document state, then check that the same control generation still owns focus.
        let identity: IUnknown = view.cast()?;
        let range = unsafe { view.GetRange() }?;
        let Some(range) = extent(&range) else {
            return Ok(BOOL(0));
        };
        if self.0.composition.borrow().is_some() {
            return Ok(BOOL(0));
        }
        let mut docs = self.0.docs.borrow_mut();
        if docs.active != Some(id) {
            return Ok(BOOL(0));
        }
        let Some(e) = docs.active_mut() else {
            return Ok(BOOL(0));
        };
        if range.end as usize > e.text.len() {
            return Ok(BOOL(0));
        }
        e.compose(Some(range));
        *self.0.composition.borrow_mut() = Some((id, identity));
        drop(docs);
        self.0.wake();
        Ok(BOOL(1))
    }
    fn OnUpdateComposition(
        &self,
        view: Ref<ITfCompositionView>,
        range: Ref<ITfRange>,
    ) -> Result<()> {
        let identity: IUnknown = view.ok()?.cast()?;
        let range = range.as_ref().and_then(extent);
        let id = self
            .0
            .composition
            .borrow()
            .as_ref()
            .filter(|(_, active)| active == &identity)
            .map(|(id, _)| *id);
        let mut docs = self.0.docs.borrow_mut();
        if let Some(id) = id
            && let Some(e) = docs.editors.get_mut(id)
            && let Some(range) = range.filter(|r| r.end as usize <= e.text.len())
        {
            e.compose(Some(range));
        }
        drop(docs);
        self.0.wake();
        Ok(())
    }
    fn OnEndComposition(&self, view: Ref<ITfCompositionView>) -> Result<()> {
        let identity: IUnknown = view.ok()?.cast()?;
        let matches = self
            .0
            .composition
            .borrow()
            .as_ref()
            .is_some_and(|(_, active)| active == &identity);
        if matches {
            let ended = self.0.composition.borrow_mut().take();
            if let Some((id, _)) = ended.as_ref()
                && let Some(e) = self.0.docs.borrow_mut().editors.get_mut(*id)
            {
                e.compose(None);
            }
            // Release the retained TIP outside every framework borrow.
            drop(ended);
            self.0.wake();
        }
        Ok(())
    }
}

fn extent(range: &ITfRange) -> Option<core::ops::Range<u32>> {
    let acp: ITfRangeACP = range.cast().ok()?;
    let (mut start, mut count) = (0, 0);
    unsafe { acp.GetExtent(&mut start, &mut count) }.ok().ok()?;
    if start < 0 || count < 0 {
        return None;
    }
    Some(start as u32..(start as u32).checked_add(count as u32)?)
}
unsafe fn units<'a>(pointer: *const u16, count: u32) -> Result<&'a [u16]> {
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(error(INVALID));
    }
    Ok(unsafe { core::slice::from_raw_parts(pointer, count as usize) })
}

pub(crate) struct Session {
    state: Rc<State>,
    manager: ITfThreadMgr2,
    document: ITfDocumentMgr,
    keys: ITfKeystrokeMgr,
    compositions: ITfContextOwnerCompositionServices,
}

impl Session {
    pub fn new(docs: Rc<RefCell<Documents>>, hwnd: Hwnd) -> Result<Self> {
        let state = Rc::new(State {
            docs,
            locks: RefCell::default(),
            sink: RefCell::default(),
            mask: Cell::new(0),
            requested_scope: Cell::new(false),
            composition: RefCell::new(None),
            hwnd,
            posted: Cell::new(false),
            layout_dirty: Cell::new(false),
            seen_revision: Cell::new(0),
            seen_len: Cell::new(0),
            seen_selection: Cell::default(),
        });
        let store: ITextStoreACP = Store(state.clone()).into();
        let mut raw = core::ptr::null_mut();
        unsafe {
            CoCreateInstance(
                &GUID::from_u128(0x529a9e6b_6587_4f23_ab9e_9c7d683e3c50),
                core::ptr::null_mut(),
                1,
                &ITfThreadMgr2::IID,
                &mut raw,
            )
        }
        .ok()?;
        let manager = unsafe { ITfThreadMgr2::from_raw(raw) };
        let client = unsafe { manager.Activate() }?;
        let initialized = (|| {
            let keys = manager.cast().map_err(|e: Error| {
                Error::new(e.code(), "TSF ITfKeystrokeMgr initialization failed")
            })?;
            let document = unsafe { manager.CreateDocumentMgr() }?;
            let (mut context, mut cookie) = (None, 0);
            unsafe { document.CreateContext(client, 0, &store, &mut context, &mut cookie) }.ok()?;
            let context = context.ok_or_else(|| error(HRESULT(0x80004005u32 as i32)))?;
            unsafe { document.Push(&context) }.ok()?;
            let compositions = context.cast().map_err(|e: Error| {
                Error::new(
                    e.code(),
                    "TSF ITfContextOwnerCompositionServices initialization failed",
                )
            })?;
            Ok((keys, document, compositions))
        })();
        match initialized {
            Ok((keys, document, compositions)) => Ok(Self {
                state,
                manager,
                document,
                keys,
                compositions,
            }),
            Err(e) => {
                unsafe {
                    let _ = manager.Deactivate();
                }
                Err(e)
            }
        }
    }
    pub fn focus(&self, on: bool) -> Result<()> {
        if !on
            && self
                .state
                .docs
                .borrow()
                .active()
                .is_some_and(|e| e.composition.is_some())
        {
            unsafe {
                self.compositions
                    .TerminateComposition(None::<&ITfCompositionView>)
            }
            .ok()?;
        }
        unsafe {
            if on {
                self.manager.SetFocus(&self.document)
            } else {
                self.manager.SetFocus(None::<&ITfDocumentMgr>)
            }
        }
        .ok()
    }
    pub fn document_changed(&self) {
        self.state.seen_revision.set(u64::MAX);
        self.state.layout_dirty.set(true);
        self.flush();
    }
    pub fn layout_changed(&self) {
        self.state.layout_dirty.set(true);
    }
    pub fn filter(&self, key: KeyMessage) -> bool {
        if self.state.docs.borrow().active.is_none() {
            return false;
        }
        let down = matches!(key.message, 0x100 | 0x104);
        unsafe {
            let test = if down {
                self.keys.TestKeyDown(key.wparam, key.lparam)
            } else {
                self.keys.TestKeyUp(key.wparam, key.lparam)
            };
            if !test.is_ok_and(|b| b.as_bool()) {
                return false;
            }
            let eaten = if down {
                self.keys.KeyDown(key.wparam, key.lparam)
            } else {
                self.keys.KeyUp(key.wparam, key.lparam)
            };
            eaten.is_ok_and(|b| b.as_bool())
        }
    }
    pub fn flush(&self) {
        self.state.flush();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .compositions
                .TerminateComposition(None::<&ITfCompositionView>);
            let _ = self.manager.SetFocus(None::<&ITfDocumentMgr>);
            let _ = self.document.Pop(1);
            let _ = self.manager.Deactivate();
        }
        // A TIP that did not send its final callback must not keep context -> store ->
        // composition -> context alive after the window's owner goes away.
        let composition = self.state.composition.borrow_mut().take();
        drop(composition);
        let sink = self.state.sink.borrow_mut().take();
        drop(sink);
    }
}

impl State {
    pub fn flush(&self) {
        self.posted.set(false);
        if self.locks.borrow().held != 0 {
            return;
        }
        let sink = self.sink.borrow().clone();
        let Some(sink) = sink else {
            return;
        };
        let current = self
            .docs
            .borrow()
            .active()
            .map(|e| (e.revision, e.text.len() as i32, e.selection()));
        self.locks.borrow_mut().notifying = true;
        if let Some((revision, len, selection)) = current {
            let old = self.seen_len.replace(len);
            if self.seen_revision.replace(revision) != revision && self.mask.get() & 1 != 0 {
                let change = TS_TEXTCHANGE {
                    acpStart: 0,
                    acpOldEnd: old,
                    acpNewEnd: len,
                };
                unsafe {
                    let _ = (sink.vtable().OnTextChange)(sink.as_raw(), 0, &change);
                }
            }
            if self.seen_selection.replace(selection) != selection && self.mask.get() & 2 != 0 {
                unsafe {
                    let _ = (sink.vtable().OnSelectionChange)(sink.as_raw());
                }
            }
        }
        if self.layout_dirty.replace(false) && self.mask.get() & 4 != 0 {
            unsafe {
                let _ = (sink.vtable().OnLayoutChange)(sink.as_raw(), 1, 1);
            }
        }
        self.locks.borrow_mut().notifying = false;
        if self.docs.borrow().active().is_some_and(|e| e.waiting()) {
            return;
        }
        let pending = self.locks.borrow_mut().take_pending();
        if let Some(access) = pending {
            unsafe {
                let _ = (sink.vtable().OnLockGranted)(sink.as_raw(), access);
            }
            self.locks.borrow_mut().release();
            if self.locks.borrow().pending() {
                self.wake();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_input::{Editor, InputScope};
    use windows_scene::{Control, Ids};

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
    fn state(hwnd: Hwnd) -> Rc<State> {
        let mut docs = Documents::default();
        let id = Ids::<Control>::new().mint();
        docs.editors
            .place(id, Editor::new(id, InputScope::Default, &[]));
        docs.active = Some(id);
        Rc::new(State {
            docs: Rc::new(RefCell::new(docs)),
            locks: RefCell::default(),
            sink: RefCell::default(),
            mask: Cell::new(7),
            requested_scope: Cell::new(false),
            composition: RefCell::new(None),
            hwnd,
            posted: Cell::new(false),
            layout_dirty: Cell::new(false),
            seen_revision: Cell::new(0),
            seen_len: Cell::new(0),
            seen_selection: Cell::default(),
        })
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
        let state = state(window.handle());
        let store: ITextStoreACP = Store(state.clone()).into();
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
                        assert_eq!(request(&store, 3), HRESULT(0x80040208u32 as i32));
                        for flags in [2, 6, 2, 6] {
                            assert_eq!(request(&store, flags), HRESULT(0x40300));
                        }
                    }
                }
            }),
        }
        .into();
        *state.sink.borrow_mut() = Some(Rc::new(sink));
        assert_eq!(request(&store, 2), HRESULT(0));
        assert_eq!(*grants.borrow(), [2]);
        assert_eq!(*events.borrow(), ["grant"]);
        state.flush();
        assert_eq!(*grants.borrow(), [2, 6]);
        let previous = state.sink.borrow_mut().take();
        drop(previous);
    }
    #[test]
    fn notifications_defer_reentrant_grants_and_tsf_edits_are_not_echoed() {
        let window = windows_window::Window::new("TSF notification test")
            .create()
            .unwrap();
        let state = state(window.handle());
        let store: ITextStoreACP = Store(state.clone()).into();
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink: ITextStoreACPSink = Sink {
            events: events.clone(),
            changed: Box::new({
                let store = store.clone();
                move || {
                    assert_eq!(request(&store, 6), HRESULT(0x40300));
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
        *state.sink.borrow_mut() = Some(Rc::new(sink));
        state
            .docs
            .borrow_mut()
            .active_mut()
            .unwrap()
            .replace(0, 0, &[97]);
        state.flush();
        assert_eq!(state.docs.borrow().active().unwrap().text, [98]);
        assert_eq!(events.borrow().iter().filter(|e| **e == "text").count(), 1);
        state.flush();
        assert_eq!(events.borrow().iter().filter(|e| **e == "text").count(), 1);
        let previous = state.sink.borrow_mut().take();
        drop(previous);
    }
    #[test]
    fn invalid_selection_and_unlocked_edit_leave_document_unchanged() {
        let window = windows_window::Window::new("TSF ACP test")
            .create()
            .unwrap();
        let state = state(window.handle());
        let store: ITextStoreACP = Store(state.clone()).into();
        state
            .docs
            .borrow_mut()
            .active_mut()
            .unwrap()
            .replace(0, 0, &[0xd83d, 0xde00]);
        let mut change = TS_TEXTCHANGE::default();
        assert_eq!(
            unsafe {
                (store.vtable().SetText)(store.as_raw(), 0, 0, 0, core::ptr::null(), 0, &mut change)
            },
            NOLOCK
        );
        state.locks.borrow_mut().held = 6;
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
        assert_eq!(state.docs.borrow().active().unwrap().text, [0xd83d, 0xde00]);
    }
}
