//! `DCompHost`: the Win32 + system-compositor host that drives a root
//! [`Component`] through `RenderHost<DCompBackend, Win32Dispatcher>`. It owns the
//! bare HWND and the blocking `GetMessageW` pump (true idle — zero CPU at rest),
//! routes input/resize/timer messages to the backend, and starts/stops the
//! self-stopping animation timer that ticks button ink springs.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Once};

use super::dispatch::{drain, LocalQueue, SendInner, WM_APP_DISPATCH};
use super::*;
use crate::engine::RenderHost;
use crate::system_bindings::*;
use crate::{Component, Element, RenderCx, WindowSize};
use windows_core::PCWSTR;

const TIMER_ID: usize = 1;

/// Per-thread state the WndProc reaches into. `render_host` is a clone (an `Rc`
/// bump) of the host's render host; `local`/`send` are the dispatcher's queues.
struct HostShared {
    render_host: RenderHost<DCompBackend, Win32Dispatcher>,
    local: Rc<LocalQueue>,
    send: Arc<SendInner>,
}

thread_local! {
    static DCOMP: RefCell<Option<Rc<HostShared>>> = const { RefCell::new(None) };
}

/// A self-hosted DirectComposition window hosting one reactor component tree.
pub struct DCompHost {
    hwnd: HWND,
}

impl DCompHost {
    /// Create the window, compositor, backend, and render host, mount `root`, and
    /// paint the first frame. Call [`run`](Self::run) to enter the message loop.
    pub fn new(title: impl AsRef<str>, root: Box<dyn Component>) -> windows_core::Result<Self> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        ensure_dispatcher_queue()?;

        let (hwnd, dpi, (pw, ph)) = create_window(title.as_ref(), 960, 640)?;
        let scale = (dpi as f32 / 96.0).max(0.01);
        let dip = (pw as f32 / scale, ph as f32 / scale);

        let comp = Compositing::new(hwnd, pw, ph, dpi as f32)?;
        let backend = DCompBackend::new(comp, dip, dpi as f32);

        let dispatcher = Win32Dispatcher::new(hwnd);
        let marshaller = dispatcher.marshaller();
        let (local, send) = dispatcher.queues();

        let render_host = RenderHost::new(backend, root, dispatcher);
        render_host.set_marshaller(Some(marshaller));
        render_host.set_inner_size(WindowSize {
            width: dip.0 as f64,
            height: dip.1 as f64,
        });
        render_host.set_dpi(dpi);

        // After every reconcile: adopt the new root, lay out, paint. If the
        // reconcile touched a sprung node, make sure the animation timer runs.
        let pr_host = render_host.clone_inner();
        render_host.set_post_render(move |root_id| {
            let animating = pr_host.with_reconciler_mut(|r| {
                r.backend.set_root(root_id);
                r.backend.relayout_and_paint();
                r.backend.is_animating()
            });
            if animating {
                unsafe {
                    SetTimer(hwnd, TIMER_ID, 16, None);
                }
            }
        });

        DCOMP.with(|c| {
            *c.borrow_mut() = Some(Rc::new(HostShared {
                render_host: render_host.clone_inner(),
                local,
                send,
            }));
        });

        // Frame-tick pump: when a canvas/viz subscriber appears (via
        // `on_frame_tick`) while the timer is idle, start it; the WM_TIMER handler
        // drives the ticks and stops the timer once no subscriber and no spring
        // remain (true idle).
        crate::set_frame_pump_wake(Some(Rc::new(move || unsafe {
            SetTimer(hwnd, TIMER_ID, 16, None);
        })));

        render_host.kick();
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        Ok(Self { hwnd })
    }

    /// Run the blocking message loop until the window closes.
    pub fn run(&self) {
        let mut msg: MSG = unsafe { core::mem::zeroed() };
        unsafe {
            while GetMessageW(&mut msg, core::ptr::null_mut(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        crate::set_frame_pump_wake(None);
        DCOMP.with(|c| *c.borrow_mut() = None);
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Convenience: build the host from a render function (the same `Fn(&mut
    /// RenderCx) -> Element` shape `App::render` takes) and run the message loop.
    pub fn render<F>(title: impl AsRef<str>, f: F) -> windows_core::Result<()>
    where
        F: Fn(&mut RenderCx) -> Element + 'static,
    {
        struct RenderFn<F>(F);
        impl<F: Fn(&mut RenderCx) -> Element + 'static> Component for RenderFn<F> {
            fn render(&self, _props: &(), cx: &mut RenderCx) -> Element {
                (self.0)(cx)
            }
        }
        let host = Self::new(title, Box::new(RenderFn(f)))?;
        host.run();
        Ok(())
    }
}

fn shared() -> Option<Rc<HostShared>> {
    DCOMP.with(|c| c.borrow().clone())
}

/// Static light/dark token table for the window backdrop, resolved to a WinRT
/// `Color` (the system compositor encodes it into the FP16 scRGB surface). Node
/// colours are the GUI's responsibility (it re-emits theme-bound `Prop`s); the
/// backend only owns the window backdrop, which flips with the system theme.
pub(crate) fn window_backdrop(dark: bool) -> Color {
    if dark {
        Color { a: 255, r: 14, g: 14, b: 17 }
    } else {
        Color { a: 255, r: 243, g: 243, b: 245 }
    }
}

/// Best-effort read of the system app theme. Detection of the live setting needs
/// a registry/uxtheme binding not yet wired here, so this defaults to dark (the
/// app's design default); the flip *mechanism* (re-resolve + repaint) is in
/// place for when detection lands.
fn system_prefers_dark() -> bool {
    true
}

/// Whether a `WM_SETTINGCHANGE` lParam names the immersive colour set (theme).
fn is_immersive_color_set(lparam: LPARAM) -> bool {
    if lparam == 0 {
        return false;
    }
    let s = unsafe { wide_str(lparam as *const u16) };
    s == "ImmersiveColorSet"
}

/// Read a NUL-terminated UTF-16 string at `ptr` (bounded) into a `String`.
unsafe fn wide_str(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
        String::from_utf16_lossy(core::slice::from_raw_parts(ptr, len))
    }
}

fn dpi_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    (if dpi == 0 { 96 } else { dpi } as f32) / 96.0
}

/// `(x, y)` from a mouse LPARAM, converted from physical pixels to DIPs.
fn dip_xy(hwnd: HWND, lparam: LPARAM) -> (f32, f32) {
    let px = (lparam & 0xFFFF) as i16 as f32;
    let py = ((lparam >> 16) & 0xFFFF) as i16 as f32;
    let scale = dpi_scale(hwnd);
    (px / scale, py / scale)
}

fn track_leave(hwnd: HWND) {
    let mut t = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut t);
    }
}

fn start_timer(hwnd: HWND, needed: bool) {
    if needed {
        unsafe {
            SetTimer(hwnd, TIMER_ID, 16, None);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1, // the compositor owns every pixel; never erase/flash.

        WM_APP_DISPATCH => {
            if let Some(s) = shared() {
                drain(&s.local, &s.send);
            }
            0
        }

        WM_MOUSEMOVE => {
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                track_leave(hwnd);
                let started = s.render_host.with_reconciler_mut(|r| r.backend.on_pointer_move(x, y));
                start_timer(hwnd, started);
            }
            0
        }

        WM_MOUSELEAVE => {
            if let Some(s) = shared() {
                let started = s.render_host.with_reconciler_mut(|r| r.backend.on_pointer_leave());
                start_timer(hwnd, started);
            }
            0
        }

        WM_LBUTTONDOWN => {
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                let (captured, timer) =
                    s.render_host.with_reconciler_mut(|r| r.backend.on_pointer_down(x, y));
                if captured {
                    unsafe {
                        SetCapture(hwnd);
                    }
                }
                start_timer(hwnd, timer);
            }
            0
        }

        WM_LBUTTONUP => {
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                unsafe {
                    let _ = ReleaseCapture();
                }
                let timer = s.render_host.with_reconciler_mut(|r| r.backend.on_pointer_up(x, y));
                start_timer(hwnd, timer);
            }
            0
        }

        WM_TIMER => {
            if wparam == TIMER_ID
                && let Some(s) = shared()
            {
                // Pace any backend frame-tick subscribers (canvas/viz) first…
                crate::drive_frame_ticks();
                // …then advance button ink springs.
                let springs = s.render_host.with_reconciler_mut(|r| r.backend.tick(1.0 / 60.0));
                // Keep the timer only while work remains; otherwise return to
                // a blocking, zero-CPU pump.
                if !springs && !crate::frame_ticks_active() {
                    unsafe {
                        let _ = KillTimer(hwnd, TIMER_ID);
                    }
                }
            }
            0
        }

        WM_DPICHANGED => {
            // lParam is the suggested new window rectangle; move/resize to it and
            // let the ensuing WM_SIZE re-fold the new DPI into layout + surfaces.
            let rc = lparam as *const RECT;
            if !rc.is_null() {
                let rc = unsafe { &*rc };
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        core::ptr::null_mut(),
                        rc.left,
                        rc.top,
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            0
        }

        WM_SETTINGCHANGE => {
            // An "ImmersiveColorSet" change flips the system light/dark theme.
            if is_immersive_color_set(lparam)
                && let Some(s) = shared()
            {
                let dark = system_prefers_dark();
                s.render_host.with_reconciler_mut(|r| {
                    r.backend.apply_theme(dark);
                    r.backend.mark_all_dirty_and_repaint();
                });
            }
            0
        }

        WM_SIZE => {
            if let Some(s) = shared() {
                let pw = (lparam & 0xFFFF) as i32;
                let ph = ((lparam >> 16) & 0xFFFF) as i32;
                if pw > 0 && ph > 0 {
                    let dpi = unsafe { GetDpiForWindow(hwnd) };
                    let dpi = if dpi == 0 { 96 } else { dpi };
                    let scale = dpi as f32 / 96.0;
                    s.render_host.set_dpi(dpi);
                    s.render_host.set_inner_size(WindowSize {
                        width: pw as f64 / scale as f64,
                        height: ph as f64 / scale as f64,
                    });
                    s.render_host
                        .with_reconciler_mut(|r| r.backend.resize(pw, ph, dpi));
                }
            }
            0
        }

        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn ensure_dispatcher_queue() -> windows_core::Result<()> {
    // The system Compositor needs a DispatcherQueue on the calling thread.
    // DQTYPE_THREAD_CURRENT = 2, DQTAT_COM_ASTA = 1.
    let options = DispatcherQueueOptions {
        dwSize: size_of::<DispatcherQueueOptions>() as u32,
        threadType: 2,
        apartmentType: 1,
    };
    let mut controller = core::ptr::null_mut();
    unsafe {
        // A controller already existing on this thread returns an error we ignore;
        // the queue we need is then already present.
        let _ = CreateDispatcherQueueController(options, &mut controller);
    }
    Ok(())
}

fn create_window(
    title: &str,
    width: i32,
    height: i32,
) -> windows_core::Result<(HWND, u32, (i32, i32))> {
    static CLASS: &[u16] = &[
        b'D' as u16, b'C' as u16, b'o' as u16, b'm' as u16, b'p' as u16, b'H' as u16, b'o' as u16,
        b's' as u16, b't' as u16, 0,
    ];
    static REGISTER: Once = Once::new();

    let hinstance = unsafe { GetModuleHandleW(PCWSTR(core::ptr::null())) } as HINSTANCE;

    REGISTER.call_once(|| {
        let cursor = unsafe { LoadCursorW(core::ptr::null_mut(), IDC_ARROW) };
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: core::ptr::null_mut(),
            hCursor: cursor,
            hbrBackground: core::ptr::null_mut(), // compositor owns the surface
            lpszMenuName: PCWSTR(core::ptr::null()),
            lpszClassName: PCWSTR(CLASS.as_ptr()),
        };
        unsafe {
            RegisterClassW(&wc);
        }
    });

    let mut title_w: Vec<u16> = title.encode_utf16().collect();
    title_w.push(0);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            PCWSTR(CLASS.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width,
            height,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            hinstance,
            core::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(windows_core::Error::empty());
    }

    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96 } else { dpi };

    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let pw = (rc.right - rc.left).max(1);
    let ph = (rc.bottom - rc.top).max(1);
    Ok((hwnd, dpi, (pw, ph)))
}
