//! `ITfThreadMgr2` activation and document-manager plumbing.
//!
//! Stands up TSF on the **front thread** (the HWND / pump thread — TSF is
//! STA-affine and pump-bound, §7.5), pushes our [`TextStore`](super::TextStore)
//! into a context, advises the composition sink, and associates focus so the
//! store receives input. All of it must run *after* the front thread's
//! `CoInitializeEx(APARTMENTTHREADED)` — this module does not initialise the
//! apartment (the host already did).
//!
//! ## Focus model (HWND-per-app)
//!
//! We follow Chromium's desktop model: one thread manager, one document manager
//! per focusable surface, and `ITfThreadMgr2::SetFocus(docmgr)` to make it the
//! active document. Because the whole app is one top-level HWND that owns all
//! editable fields, a single document manager + context is sufficient; the store
//! swaps which editor it reflects as focus moves between fields (the store reads
//! the *currently focused* editor through [`TsfDocument`](super::TsfDocument)).
//! `SetFocus(None)` on blur-out of all fields deactivates input, and a
//! field-to-field move is reported as a wholesale document change by
//! [`bridge::flush`](super::bridge::flush).
//!
//! Lifetime is owned by [`bridge`](super::bridge): it creates this on window
//! create and drops it on destroy, which pops the context and deactivates the
//! thread manager.

use std::cell::Cell;

use windows_core::{Interface, Result, GUID};

use super::acp::TextStore;
use super::TextInput;
use crate::system_bindings::{
    ITextStoreACP, ITfContext, ITfDocumentMgr, ITfKeystrokeMgr, ITfThreadMgr2,
};

// `CoCreateInstance` is not in the generated system set; link it directly, as
// `uia.rs` links its ole helpers. The front thread's apartment is already
// initialised, so a bare create on this thread is correct.
windows_core::link!("ole32.dll" "system" fn CoCreateInstance(
    rclsid: *const GUID,
    punkouter: *mut core::ffi::c_void,
    dwclscontext: u32,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void
) -> windows_core::HRESULT);

/// `CLSID_TF_ThreadMgr` — the in-box TSF thread-manager coclass.
const CLSID_TF_THREAD_MGR: GUID = GUID::from_u128(0x529a9e6b_6587_4f23_ab9e_9c7d683e3c50);
/// `CLSCTX_INPROC_SERVER`.
const CLSCTX_INPROC_SERVER: u32 = 0x1;
/// `E_FAIL`, for the (impossible) case of a successful `CreateContext` that
/// leaves the out-parameter empty.
const E_FAIL: windows_core::HRESULT = windows_core::HRESULT(0x8000_4005u32 as i32);

/// A live TSF activation: the thread manager, our client id, and the document
/// manager + context our store is pushed into. Dropping it tears TSF down.
pub(crate) struct TsfActivation {
    thread_mgr: ITfThreadMgr2,
    doc_mgr: ITfDocumentMgr,
    /// Held to keep the context (and thus the store + sink advise) alive.
    context: ITfContext,
    #[allow(dead_code)] // the store the context references; kept alive here.
    store: ITextStoreACP,
    /// The backend-facing handle to the same store.
    input: TextInput,
    /// Keystroke pre-emption, so a TIP claims composition keys before the
    /// editor sees them. `None` if the thread manager does not expose it.
    keystroke: Option<ITfKeystrokeMgr>,
    focused: Cell<bool>,
}

/// Name the step a failure came from. TSF activation is a six-call chain whose
/// steps fail with overlapping `HRESULT`s (`TS_E_*` and `TF_E_*` share a
/// facility), so a bare code is not diagnosable — and the failure is invisible
/// in use, since the app runs fine with no IME. The step name is what turns a
/// bug report into a fix.
fn step<T>(what: &'static str, r: Result<T>) -> Result<T> {
    r.map_err(|e| {
        windows_core::Error::new(e.code(), format!("{what} failed: {:#010x}", e.code().0))
    })
}

impl TsfActivation {
    /// Create the thread manager, activate it, push `store` into a fresh
    /// context, and advise the composition sink on it. Must run on the front
    /// thread after apartment init.
    pub(crate) fn new(store: TextStore) -> Result<Self> {
        // Take the backend handle before the store is consumed into its interface.
        let input = store.input();
        let store_acp: ITextStoreACP = store.into();

        // Create the thread manager.
        let mut raw = core::ptr::null_mut();
        // SAFETY: standard CoCreateInstance out-parameter protocol.
        step("CoCreateInstance(CLSID_TF_ThreadMgr)", unsafe {
            CoCreateInstance(
                &CLSID_TF_THREAD_MGR,
                core::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &ITfThreadMgr2::IID,
                &mut raw,
            )
            .ok()
        })?;
        // SAFETY: `raw` is a valid `ITfThreadMgr2` on success.
        let thread_mgr = unsafe { ITfThreadMgr2::from_raw(raw) };

        // SAFETY: interface calls on live COM objects; out-parameters are valid.
        let client_id = step("ITfThreadMgr2::Activate", unsafe { thread_mgr.Activate() })?;
        let doc_mgr = step("ITfThreadMgr2::CreateDocumentMgr", unsafe {
            thread_mgr.CreateDocumentMgr()
        })?;

        let mut context: Option<ITfContext> = None;
        let mut edit_cookie: u32 = 0;
        // SAFETY: `&store_acp` is a live `Param<IUnknown>`; out-params are valid.
        step("ITfDocumentMgr::CreateContext", unsafe {
            doc_mgr
                .CreateContext(client_id, 0, &store_acp, &mut context, &mut edit_cookie)
                .ok()
        })?;
        let context = context.ok_or_else(|| windows_core::Error::from_hresult(E_FAIL))?;
        // SAFETY: push the context onto the document manager's stack.
        step("ITfDocumentMgr::Push", unsafe { doc_mgr.Push(&context).ok() })?;

        // Composition boundaries need no advise: the store handed to
        // `CreateContext` above implements `ITfContextOwnerCompositionSink`, and
        // TSF picks it up by `QueryInterface` when it opens a composition. (An
        // explicit `ITfSource::AdviseSink` for that interface is rejected with
        // `CONNECT_E_CANNOTCONNECT` — see `comp_sink`.)

        let keystroke: Option<ITfKeystrokeMgr> = thread_mgr.cast().ok();

        Ok(Self {
            thread_mgr,
            doc_mgr,
            context,
            store: store_acp,
            input,
            keystroke,
            focused: Cell::new(false),
        })
    }

    /// The backend handle for composition boundaries + change notifications.
    pub(crate) fn input(&self) -> TextInput {
        self.input.clone()
    }

    /// The keystroke manager, for the pump's pre-dispatch key filter.
    pub(crate) fn keystroke(&self) -> Option<ITfKeystrokeMgr> {
        self.keystroke.clone()
    }

    /// Associate (or clear) TSF focus with our document. Called from
    /// [`bridge::flush`](super::bridge::flush) when the focused field appears or
    /// the last one blurs — never from inside a backend borrow, because TSF
    /// calls straight back into the store from here.
    pub(crate) fn set_focus(&self, on: bool) -> Result<()> {
        if on == self.focused.get() {
            return Ok(());
        }
        self.focused.set(on);
        // SAFETY: `SetFocus` accepts the document manager or null (no focus).
        let hr = if on {
            unsafe { self.thread_mgr.SetFocus(&self.doc_mgr) }
        } else {
            unsafe { self.thread_mgr.SetFocus(None::<&ITfDocumentMgr>) }
        };
        hr.ok()
    }
}

impl Drop for TsfActivation {
    fn drop(&mut self) {
        // Pop our context and deactivate the thread manager. The composition
        // sink needs no unadvise — it was never advised, it rides the store.
        // Errors on teardown are not actionable (the objects are going away).
        // SAFETY: live COM objects; a failing teardown call is ignored.
        unsafe {
            let _ = self.doc_mgr.Pop(0);
            let _ = self.thread_mgr.SetFocus(None::<&ITfDocumentMgr>);
            let _ = self.thread_mgr.Deactivate();
        }
    }
}
