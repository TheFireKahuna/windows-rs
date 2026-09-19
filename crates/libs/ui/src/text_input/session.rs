//! TSF activation, focus and key filtering. STA- and pump-bound; nothing here is `Send`.

use super::doc::Doc;
use super::store::{Inner, NOTIMPL, error, store};
use crate::bindings::*;
use core::cell::RefCell;
use core::ffi::c_void;
use std::rc::Rc;
use windows_core::{Error, GUID, HRESULT, Interface, Result};
use windows_window::{Hwnd, KeyMessage};

windows_core::link!("ole32.dll" "system" fn CoCreateInstance(class: *const GUID, outer: *mut c_void, context: u32, iid: *const GUID, out: *mut *mut c_void) -> HRESULT);

const THREAD_MGR: GUID = GUID::from_u128(0x529a9e6b_6587_4f23_ab9e_9c7d683e3c50);

pub(crate) struct Session {
    pub inner: Rc<Inner>,
    manager: ITfThreadMgr2,
    document: ITfDocumentMgr,
    keys: ITfKeystrokeMgr,
    compositions: ITfContextOwnerCompositionServices,
}

impl Session {
    /// Activates TSF on this thread and pushes one context over the shared document.
    ///
    /// Required initialization fails explicitly and by name: a host that cannot give this
    /// thread an `ITfKeystrokeMgr` has no text input at all.
    pub fn new(doc: Rc<RefCell<Doc>>, hwnd: Hwnd) -> Result<Self> {
        let inner = Inner::new(doc, hwnd);
        let mut raw = core::ptr::null_mut();
        // SAFETY: the class and interface identifiers are constants and `raw` is a local.
        unsafe {
            CoCreateInstance(
                &THREAD_MGR,
                core::ptr::null_mut(),
                1,
                &ITfThreadMgr2::IID,
                &mut raw,
            )
        }
        .ok()?;
        // SAFETY: `CoCreateInstance` succeeded, so `raw` owns one reference to the manager.
        let manager = unsafe { ITfThreadMgr2::from_raw(raw) };
        let client = unsafe { manager.Activate() }?;
        let built = (|| {
            let keys = named(manager.cast(), "ITfKeystrokeMgr")?;
            let document = unsafe { manager.CreateDocumentMgr() }?;
            let (mut context, mut cookie) = (None, 0);
            unsafe { document.CreateContext(client, 0, &store(&inner), &mut context, &mut cookie) }
                .ok()?;
            let context = context.ok_or_else(|| error(NOTIMPL))?;
            unsafe { document.Push(&context) }.ok()?;
            let compositions = named(context.cast(), "ITfContextOwnerCompositionServices")?;
            Ok((keys, document, compositions))
        })();
        match built {
            Ok((keys, document, compositions)) => Ok(Self {
                inner,
                manager,
                document,
                keys,
                compositions,
            }),
            Err(e) => {
                let _ = unsafe { manager.Deactivate() };
                Err(e)
            }
        }
    }

    /// Associates or clears text focus, finishing any open composition first.
    ///
    /// Clearing focus can finish that composition synchronously, so the caller holds no
    /// document borrow across this call.
    pub fn focus(&self, on: bool) -> Result<()> {
        if !on && self.inner.composing() {
            unsafe {
                self.compositions
                    .TerminateComposition(None::<&ITfCompositionView>)
            }
            .ok()?;
        }
        unsafe {
            match on {
                true => self.manager.SetFocus(&self.document),
                false => self.manager.SetFocus(None::<&ITfDocumentMgr>),
            }
        }
        .ok()
    }

    /// Marks the published view stale, so the next tick tells the sink to ask again.
    pub fn layout_changed(&self) {
        self.inner.dirty_layout();
    }

    /// Offers one removed keyboard message to TSF, reporting whether it was consumed.
    ///
    /// A key a TIP claims is not interpreted by the input layer, which is what keeps an
    /// active composition's Enter and Escape out of the overlay scope.
    pub fn filter(&self, key: KeyMessage) -> bool {
        if self.inner.doc.borrow().focused().is_none() {
            return false;
        }
        let down = matches!(key.message, 0x100 | 0x104);
        unsafe {
            let test = match down {
                true => self.keys.TestKeyDown(key.wparam, key.lparam),
                false => self.keys.TestKeyUp(key.wparam, key.lparam),
            };
            if !test.is_ok_and(|b| b.as_bool()) {
                return false;
            }
            match down {
                true => self.keys.KeyDown(key.wparam, key.lparam),
                false => self.keys.KeyUp(key.wparam, key.lparam),
            }
            .is_ok_and(|b| b.as_bool())
        }
    }
}

fn named<T>(cast: Result<T>, what: &str) -> Result<T> {
    cast.map_err(|e: Error| Error::new(e.code(), format!("TSF {what} initialization failed")))
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
        self.inner.release();
    }
}
