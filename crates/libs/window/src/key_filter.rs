//! One pretranslation hook, including messages removed by a nested system pump.

use crate::{Hwnd, Window, bindings::MSG};
use core::{ffi::c_void, marker::PhantomData};
use std::{cell::RefCell, rc::Rc};
use windows_core::{BOOL, Error, Result};

windows_core::link!("user32.dll" "system" fn SetWindowsHookExW(id: i32, proc: Option<unsafe extern "system" fn(i32, usize, isize) -> isize>, module: *mut c_void, thread: u32) -> *mut c_void);
windows_core::link!("user32.dll" "system" fn UnhookWindowsHookEx(hook: *mut c_void) -> BOOL);
windows_core::link!("user32.dll" "system" fn CallNextHookEx(hook: *mut c_void, code: i32, w: usize, l: isize) -> isize);
windows_core::link!("kernel32.dll" "system" fn GetCurrentThreadId() -> u32);

/// A queued keyboard message, before TranslateMessage can generate its character.
#[derive(Clone, Copy, Debug)]
pub struct KeyMessage {
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
}

type Filter = Rc<dyn Fn(KeyMessage) -> bool>;
thread_local! { static FILTER: RefCell<Option<(Hwnd, Filter)>> = const { RefCell::new(None) }; }

/// Keeps the hook on the window's thread and removes it before the window can be dropped.
#[must_use]
pub struct KeyFilter<'w> {
    hook: *mut c_void,
    _window: PhantomData<&'w Window>,
    _thread: PhantomData<Rc<()>>,
}

impl Window {
    /// Installs the thread's single key filter. Returning true consumes a queued key.
    ///
    /// The callback runs without a window borrow and may be re-entered by text services.
    /// It must keep its own borrows short and must not unwind across this callback.
    pub fn key_filter(
        &self,
        filter: impl Fn(KeyMessage) -> bool + 'static,
    ) -> Result<KeyFilter<'_>> {
        if FILTER.with(|slot| slot.borrow().is_some()) {
            return Err(Error::new(
                windows_core::HRESULT(0x800700b7u32 as i32),
                "a key filter is already installed",
            ));
        }
        // SAFETY: a thread-local hook with a static callback; no injected/global hook.
        let hook = unsafe {
            SetWindowsHookExW(
                3,
                Some(receive),
                core::ptr::null_mut(),
                GetCurrentThreadId(),
            )
        };
        if hook.is_null() {
            return Err(Error::from_thread());
        }
        // Keys are delivered to the window input goes to, which is the content window where
        // there is one.
        FILTER.with(|slot| *slot.borrow_mut() = Some((self.input_window(), Rc::new(filter))));
        Ok(KeyFilter {
            hook,
            _window: PhantomData,
            _thread: PhantomData,
        })
    }
}

unsafe extern "system" fn receive(code: i32, removed: usize, data: isize) -> isize {
    if code >= 0 && removed == 1 && data != 0 {
        // SAFETY: WH_GETMESSAGE supplies a writable MSG for this callback's duration.
        let message = unsafe { &mut *(data as *mut MSG) };
        if matches!(message.message, 0x100 | 0x101 | 0x104 | 0x105) {
            let filter = FILTER.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .filter(|(hwnd, _)| hwnd.raw() == message.hwnd)
                    .map(|(_, f)| Rc::clone(f))
            });
            if let Some(filter) = filter {
                let key = KeyMessage {
                    message: message.message,
                    wparam: message.wParam,
                    lparam: message.lParam,
                };
                // A panic must never cross user32. Abort rather than silently dropping an edit.
                let consumed =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| filter(key)))
                        .unwrap_or_else(|_| std::process::abort());
                if consumed {
                    message.message = 0;
                    message.wParam = 0;
                    message.lParam = 0;
                }
            }
        }
    }
    // SAFETY: preserve the hook chain, including codes this hook does not handle.
    unsafe { CallNextHookEx(core::ptr::null_mut(), code, removed, data) }
}

impl Drop for KeyFilter<'_> {
    fn drop(&mut self) {
        // SAFETY: the hook is owned by this guard on its installing thread.
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
        }
        FILTER.with(|slot| *slot.borrow_mut() = None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::PeekMessageW;
    use std::cell::Cell;

    #[test]
    fn removed_keys_are_filtered_once_in_a_nested_pump_and_guard_removes_hook() {
        let window = Window::new("key filter contract")
            .hidden()
            .create()
            .unwrap();
        let count = Rc::new(Cell::new(0));
        let nested = Rc::new(Cell::new(0));
        let hwnd = window.handle();
        let hook = window
            .key_filter({
                let count = count.clone();
                let nested = nested.clone();
                move |key| {
                    count.set(count.get() + 1);
                    if key.wparam == 65 {
                        let mut next = MSG::default();
                        // Simulates a TIP/system call-out running its own removed-message pump.
                        assert!(
                            unsafe { PeekMessageW(&mut next, hwnd.raw(), 0x100, 0x105, 1) }
                                .as_bool()
                        );
                        assert_eq!(next.wParam, 66);
                        nested.set(next.message);
                        true
                    } else {
                        false
                    }
                }
            })
            .unwrap();
        assert!(window.key_filter(|_| false).is_err(), "one hook per thread");
        assert!(hwnd.post(0x100, 65, 0));
        assert!(hwnd.post(0x100, 66, 0));
        let mut message = MSG::default();
        assert!(unsafe { PeekMessageW(&mut message, hwnd.raw(), 0x100, 0x105, 0) }.as_bool());
        assert_eq!(
            count.get(),
            0,
            "peeking without removal is not pretranslation"
        );
        assert!(unsafe { PeekMessageW(&mut message, hwnd.raw(), 0x100, 0x105, 1) }.as_bool());
        assert_eq!(
            message.message, 0,
            "consumed keys become WM_NULL before TranslateMessage"
        );
        assert_eq!(nested.get(), 0x100, "unconsumed key is delivered unchanged");
        assert_eq!(count.get(), 2);
        assert!(!unsafe { PeekMessageW(&mut message, hwnd.raw(), 0x100, 0x105, 1) }.as_bool());
        drop(hook);
        assert!(hwnd.post(0x100, 65, 0));
        assert!(unsafe { PeekMessageW(&mut message, hwnd.raw(), 0x100, 0x105, 1) }.as_bool());
        assert_eq!(message.message, 0x100);
        assert_eq!(count.get(), 2, "teardown removed the callback");
    }
}
