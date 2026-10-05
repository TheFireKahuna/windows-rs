//! The content window: a child that always covers its frame's client area and is the window
//! the application's input, keyboard focus, text services and automation address.
//!
//! The frame keeps what belongs to a top-level window — the caption, sizing, DPI, visibility
//! and the frame gate. The content window exists because the system compositor attaches the
//! win32k input sink that routes wheel and precision-touchpad input to a
//! `VisualInteractionSource` only to a `DesktopWindowTarget` whose window has `WS_CHILD`; a
//! target on a top-level window gets no sink, and no wheel or touchpad gesture over it reaches a
//! tracker. A sink's window is the window the input is delivered to, so the application's input
//! lives here too.

use crate::bindings::*;
use crate::caption::Caption;
use core::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;
use windows_core::*;

/// Receives the raw window handle, message code, and `wparam`/`lparam`. Return
/// `Some(result)` to handle the message, or `None` to fall through to default processing.
pub(crate) type MessageHandler =
    Box<dyn FnMut(*mut core::ffi::c_void, u32, usize, isize) -> Option<isize>>;

/// What the content window's procedure owns.
struct State {
    message: RefCell<Option<MessageHandler>>,
    /// The frame's caption, whose points this window passes through to the frame.
    caption: Option<Rc<Caption>>,
}

/// Creates the content window: a child of `frame` covering its client area.
///
/// # Errors
///
/// The class could not be registered or the window could not be created.
pub(crate) fn create(
    frame: HWND,
    message: Option<MessageHandler>,
    caption: Option<Rc<Caption>>,
) -> Result<HWND> {
    register_class()?;
    let mut client = RECT::default();
    // SAFETY: `frame` is live and owned by this thread; the class is registered and every
    // other handle argument is null.
    let hwnd = unsafe {
        _ = GetClientRect(frame, &mut client);
        CreateWindowExW(
            0,
            class_name(),
            PCWSTR::null(),
            (WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS) as u32,
            0,
            0,
            client.right - client.left,
            client.bottom - client.top,
            frame,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(Error::from_thread());
    }
    let state = Box::new(State {
        message: RefCell::new(message),
        caption,
    });
    // SAFETY: `hwnd` is live; the procedure takes ownership of the box until `WM_NCDESTROY`.
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as _) };
    Ok(hwnd)
}

/// Sizes the content window to the client area a `WM_SIZE` reports.
///
/// A zero dimension is a minimize, and the window keeps its size until the restore reports the
/// real one.
pub(crate) fn fit(hwnd: HWND, lparam: LPARAM) {
    let width = (lparam & 0xffff) as i32;
    let height = ((lparam >> 16) & 0xffff) as i32;
    if width == 0 || height == 0 {
        return;
    }
    // SAFETY: `hwnd` is the frame's child, destroyed only with it, on this thread.
    unsafe {
        _ = SetWindowPos(
            hwnd,
            core::ptr::null_mut(),
            0,
            0,
            width,
            height,
            (SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) as u32,
        );
    }
}

fn class_name() -> PCWSTR {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    let name = NAME.get_or_init(|| "windows-window.Content\0".encode_utf16().collect());
    PCWSTR(name.as_ptr())
}

/// Registers the content window's class once per process, caching the outcome.
///
/// # Errors
///
/// The class name is already registered to another window procedure.
fn register_class() -> Result<()> {
    static ATOM: OnceLock<core::result::Result<ATOM, HRESULT>> = OnceLock::new();
    ATOM.get_or_init(|| {
        // No background brush and no class cursor: the content is the compositor target, and
        // the cursor is the application's to set for whatever is under the pointer.
        // SAFETY: the descriptor is a stack local, and its class name outlives the process.
        let atom = unsafe {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hCursor: LoadCursorW(core::ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name(),
                ..Default::default()
            };
            RegisterClassW(&wc)
        };
        match atom {
            0 => Err(Error::from_thread().code()),
            atom => Ok(atom),
        }
    })
    .map(|_| ())
    .map_err(|code| Error::new(code, "the content window class could not be registered"))
}

/// Runs `f` against the content window's state, or answers `None` before it is installed or
/// after it is freed.
///
/// # Safety
///
/// `hwnd` must be a window of the content class, on its own thread.
unsafe fn with_state<R>(hwnd: HWND, f: impl FnOnce(&State) -> R) -> Option<R> {
    // SAFETY: the caller guarantees the class, so any state on it is a `State` installed by
    // `create`, and the procedure runs on the thread that owns it.
    unsafe {
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        (!state.is_null()).then(|| f(&*state))
    }
}

/// Answers every message delivered to the content window.
///
/// # Safety
///
/// The caller must be the system's message dispatch.
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // The caption band and the resize edges lie inside the client area this window covers, so
    // their points are passed through to the frame, whose caption answers them. Nothing a
    // handler returns can take them.
    if message == WM_NCHITTEST as u32 {
        // SAFETY: this is the content class's own procedure.
        let caption = unsafe { with_state(hwnd, |state| state.caption.clone()) }.flatten();
        if let Some(caption) = caption
            // SAFETY: the parent is the frame that created this window, on this thread.
            && caption.pass_through(unsafe { GetParent(hwnd) }, lparam)
        {
            return HTTRANSPARENT as LRESULT;
        }
        return HTCLIENT as LRESULT;
    }

    // Detached across the call, as the frame's handler is, so a handler that re-enters this
    // procedure finds the slot empty and falls through rather than aliasing itself.
    // SAFETY: as above.
    let handler = unsafe { with_state(hwnd, |state| state.message.borrow_mut().take()) }.flatten();
    let mut handled = None;
    if let Some(mut handler) = handler {
        handled = handler(hwnd, message, wparam, lparam);
        // SAFETY: as above.
        unsafe {
            with_state(hwnd, move |state| {
                let mut slot = state.message.borrow_mut();
                if slot.is_none() {
                    *slot = Some(handler);
                }
            })
        };
    }

    if message == WM_NCDESTROY as u32 {
        // SAFETY: as above. Cleared before the box is freed, on this thread.
        unsafe {
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
            if !state.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(state));
            }
        }
    }

    if let Some(result) = handled {
        return result;
    }
    match message as i32 {
        // It draws nothing of its own: its content is the compositor target bound to it.
        WM_ERASEBKGND => 1,
        // SAFETY: the arguments are the ones just received.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
