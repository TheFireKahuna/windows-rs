//! `ITfThreadMgr2` activation and document-manager plumbing.
//!
//! Stands up TSF on the **front thread** (the HWND / pump thread — TSF is
//! STA-affine and pump-bound, §7.5), pushes our [`TextStore`](super::TextStore)
//! into a context, and associates focus so the store receives input. All of it
//! must run *after* the front thread's `CoInitializeEx(APARTMENTTHREADED)` — this
//! module does not initialise the apartment (the host already did).
//!
//! ## Focus model (HWND-per-app)
//!
//! We follow Chromium's desktop model: one thread manager, one document manager
//! per focusable surface, and `ITfThreadMgr2::SetFocus(docmgr)` to make it the
//! active document. Because the whole app is one top-level HWND that owns all
//! editable fields, a single document manager + context is sufficient; the store
//! swaps which editor it reflects as focus moves between fields (the store reads
//! the *currently focused* editor through [`TsfDocument`](super::TsfDocument)).
//! `SetFocus(None)` on blur-out of all fields deactivates input.
//!
//! ## What the WndProc / focus path must call (integration hooks)
//!
//! This module is not wired into `host.rs`; when it is, the front thread needs to:
//!
//! * **Message pump** — before `DispatchMessage`, give keystrokes to TSF. The
//!   simplest correct path is `ITfKeystrokeMgr::TestKeyDown` → `KeyDown` (and the
//!   Up pair) and let its `pfEaten` decide consumption, i.e. a
//!   `on_key_down(wparam, lparam) -> bool /* eaten */` hook the WndProc consults
//!   *before* the editor's own `on_key` (so the TIP claims composition keys). If
//!   `ITfKeystrokeMgr` is not advised, TSF still drives composition through the
//!   thread manager's own message filtering, but keystroke pre-emption is the
//!   robust path and is why `ITfKeystrokeMgr` is already in the bindings.
//! * **Focus** — call [`TsfActivation::set_focus`] from `set_focus` in `input.rs`
//!   when a text field gains focus, and `set_focus(false)` when the last field
//!   blurs.
//! * **Composition boundaries** — advise an `ITfContextOwnerCompositionSink` on
//!   the context and route its `OnStartComposition`/`OnEndComposition` to
//!   [`TextInput::on_composition_started`](super::acp::TextInput) /
//!   `on_composition_ended`. That sink is the origin of the §7.2 guard signal.
//!   (Chromium's `TSFTextStore` implements this sink directly; we keep it beside
//!   the store so the guard flag lives with the lock state.)
//! * **Teardown** — drop [`TsfActivation`] on window destroy (it pops the context
//!   and deactivates the thread manager).

use windows_core::{Interface, Result, GUID};

use super::acp::TextStore;
use super::TextInput;
use crate::system_bindings::{ITextStoreACP, ITfContext, ITfDocumentMgr, ITfThreadMgr2};

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

/// A live TSF activation: the thread manager, our client id, and the document
/// manager + context our store is pushed into. Dropping it tears TSF down.
pub(crate) struct TsfActivation {
    thread_mgr: ITfThreadMgr2,
    doc_mgr: ITfDocumentMgr,
    #[allow(dead_code)] // held to keep the context (and thus the store) alive.
    context: ITfContext,
    #[allow(dead_code)] // the store the context references; kept alive here.
    store: ITextStoreACP,
    /// The backend-facing handle to the same store.
    input: TextInput,
    client_id: u32,
    focused: bool,
}

impl TsfActivation {
    /// Create the thread manager, activate it, and push `store` into a fresh
    /// context. Must run on the front thread after apartment init.
    pub(crate) fn new(store: TextStore) -> Result<Self> {
        // Take the backend handle before the store is consumed into its interface.
        let input = store.input();
        let store_acp: ITextStoreACP = store.into();

        // Create the thread manager.
        let mut raw = core::ptr::null_mut();
        // SAFETY: standard CoCreateInstance out-parameter protocol.
        unsafe {
            CoCreateInstance(
                &CLSID_TF_THREAD_MGR,
                core::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &ITfThreadMgr2::IID,
                &mut raw,
            )
            .ok()?;
        }
        // SAFETY: `raw` is a valid `ITfThreadMgr2` on success.
        let thread_mgr = unsafe { ITfThreadMgr2::from_raw(raw) };

        // SAFETY: interface calls on live COM objects; out-parameters are valid.
        let client_id = unsafe { thread_mgr.Activate()? };
        let doc_mgr = unsafe { thread_mgr.CreateDocumentMgr()? };

        let mut context: Option<ITfContext> = None;
        let mut edit_cookie: u32 = 0;
        // SAFETY: `&store_acp` is a live `Param<IUnknown>`; out-params are valid.
        unsafe {
            doc_mgr
                .CreateContext(client_id, 0, &store_acp, &mut context, &mut edit_cookie)
                .ok()?;
        }
        let context = context.ok_or_else(|| {
            windows_core::Error::from_hresult(windows_core::HRESULT(0x8000_4005u32 as i32)) // E_FAIL
        })?;
        // SAFETY: push the context onto the document manager's stack.
        unsafe { doc_mgr.Push(&context).ok()? };

        Ok(Self {
            thread_mgr,
            doc_mgr,
            context,
            store: store_acp,
            input,
            client_id,
            focused: false,
        })
    }

    /// The backend handle for change notifications + the composition guard.
    pub(crate) fn input(&self) -> TextInput {
        self.input.clone()
    }

    /// The TSF client id (needed to advise sinks / keystroke managers).
    pub(crate) fn client_id(&self) -> u32 {
        self.client_id
    }

    /// Associate (or clear) TSF focus with our document. Call from the focus path
    /// when a text field gains focus (`on = true`) or the last one blurs
    /// (`on = false`).
    pub(crate) fn set_focus(&mut self, on: bool) -> Result<()> {
        if on == self.focused {
            return Ok(());
        }
        self.focused = on;
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
        // Pop our context and deactivate the thread manager. Errors on teardown
        // are not actionable (the objects are going away regardless).
        // SAFETY: live COM objects; a failing teardown call is ignored.
        unsafe {
            let _ = self.doc_mgr.Pop(0);
            let _ = self.thread_mgr.SetFocus(None::<&ITfDocumentMgr>);
            let _ = self.thread_mgr.Deactivate();
        }
    }
}
