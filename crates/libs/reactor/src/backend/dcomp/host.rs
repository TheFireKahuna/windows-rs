//! `DCompHost`: the Win32 + system-compositor host that drives a root
//! [`Component`] through `RenderHost<DCompBackend, Win32Dispatcher>`. It owns the
//! bare HWND and the blocking `GetMessageW` pump (true idle — zero CPU at rest),
//! routes input/resize/timer messages to the backend, and starts/stops the
//! self-stopping animation timer that ticks button ink springs.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once};

use super::dispatch::{drain, LocalQueue, SendInner, WM_APP_DISPATCH};
use super::uia;
use super::*;
use crate::engine::RenderHost;
use crate::system_bindings::*;
use crate::{Component, Element, RenderCx, WindowSize};
use windows_core::{Interface, PCWSTR};

/// `WM_GETOBJECT` `lParam` value that asks for the window's root UI Automation
/// provider (`UiaRootObjectId`, defined as `-25` in `uiautomationcore.h`).
const UIA_ROOT_OBJECT_ID: i32 = -25;

/// App message used to marshal a single UI-Automation provider call onto the UI
/// thread. `wParam` is a `*mut Box<dyn FnOnce() + Send>` the WndProc runs and
/// frees. See [`marshal_to_ui`].
pub(crate) const WM_APP_UIA: u32 = WM_APP + 0x43;

/// The thread id of the UI (message-pump) thread, captured at host creation.
/// UIA provider methods arrive on UIA's own worker threads; they compare against
/// this to take the in-thread fast path or marshal (see [`marshal_to_ui`]).
static UI_THREAD_ID: AtomicU32 = AtomicU32::new(0);

windows_core::link!("kernel32.dll" "system" fn GetCurrentThreadId() -> u32);

/// Whether the caller is already running on the UI thread.
pub(crate) fn on_ui_thread() -> bool {
    UI_THREAD_ID.load(Ordering::Relaxed) == unsafe { GetCurrentThreadId() }
}

thread_local! {
    /// Set while a `with_backend` call holds the reconciler borrow, so a
    /// re-entrant call (e.g. a UIA event raised synchronously from inside a
    /// backend mutation) refuses rather than double-borrowing the `RefCell`.
    static IN_BACKEND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f` against the backend on the UI thread. Returns `None` when there is no
/// live host on this thread, or when called re-entrantly (the outer borrow is
/// still active). Must be called on the UI thread (the UIA layer reaches it
/// through [`marshal_to_ui`]).
pub(crate) fn with_backend<R>(f: impl FnOnce(&mut DCompBackend) -> R) -> Option<R> {
    if IN_BACKEND.with(|c| c.get()) {
        return None;
    }
    IN_BACKEND.with(|c| c.set(true));
    let r = shared().map(|s| s.render_host.with_reconciler_mut(|r| f(&mut r.backend)));
    IN_BACKEND.with(|c| c.set(false));
    r
}

/// Marshal `f` onto the UI thread and block until it completes, returning its
/// result. When already on the UI thread it runs inline (re-entrancy-safe — a
/// provider call triggered synchronously from our own thread never deadlocks on
/// the pump). Otherwise it posts [`WM_APP_UIA`] and waits on a condvar the
/// WndProc signals. `None` means the post failed (window gone) — surfaced as
/// "element not available" by the provider.
pub(crate) fn marshal_to_ui<R, F>(hwnd: isize, f: F) -> Option<R>
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    if on_ui_thread() {
        return Some(f());
    }
    let slot: Arc<(Mutex<Option<R>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
    let slot2 = Arc::clone(&slot);
    let job: Box<dyn FnOnce() + Send> = Box::new(move || {
        let r = f();
        let (m, c) = &*slot2;
        *m.lock().unwrap() = Some(r);
        c.notify_one();
    });
    // Double-box: the message carries one pointer-sized `*mut Box<dyn FnOnce()>`.
    let raw = Box::into_raw(Box::new(job));
    let ok = unsafe {
        PostMessageW(hwnd as HWND, WM_APP_UIA, raw as WPARAM, 0 as LPARAM).as_bool()
    };
    if !ok {
        // Nothing will run the job; reclaim it so it isn't leaked.
        drop(unsafe { Box::from_raw(raw) });
        return None;
    }
    let (m, c) = &*slot;
    let mut g = m.lock().unwrap();
    while g.is_none() {
        g = c.wait(g).unwrap();
    }
    g.take()
}

/// Post a closure to run on the UI thread without waiting (fire-and-forget). Runs
/// outside any backend borrow, so it is safe for deferred UIA event raising that
/// must not re-enter an in-progress input handler.
pub(crate) fn post_ui(hwnd: isize, f: impl FnOnce() + Send + 'static) {
    let job: Box<dyn FnOnce() + Send> = Box::new(f);
    let raw = Box::into_raw(Box::new(job));
    let ok = unsafe { PostMessageW(hwnd as HWND, WM_APP_UIA, raw as WPARAM, 0 as LPARAM).as_bool() };
    if !ok {
        drop(unsafe { Box::from_raw(raw) });
    }
}

const TIMER_ID: usize = 1;
/// Caret-blink timer — the one allowed at-rest timer, running only while a text
/// field holds focus (and the window is active), killed on blur.
const BLINK_TIMER_ID: usize = 2;
/// `GetCaretBlinkTime` sentinel meaning "blinking disabled" (solid caret).
const BLINK_INFINITE: u32 = u32::MAX;

thread_local! {
    /// Whether the caret-blink timer is currently scheduled (avoids resetting
    /// the blink phase on unrelated input).
    static BLINK_RUNNING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

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
        // Record the UI thread so UIA provider calls can detect the in-thread
        // fast path versus needing to marshal (see `marshal_to_ui`).
        UI_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
        ensure_dispatcher_queue()?;

        let (hwnd, dpi, (pw, ph)) = create_window(title.as_ref(), 960, 640)?;
        let scale = (dpi as f32 / 96.0).max(0.01);
        let dip = (pw as f32 / scale, ph as f32 / scale);

        let comp = Compositing::new(hwnd, pw, ph, dpi as f32)?;
        let mut backend = DCompBackend::new(comp, dip, dpi as f32, hwnd as isize);
        // Honour the OS light/dark app theme for the window backdrop at startup
        // (the same flip the `WM_SETTINGCHANGE` handler applies when it changes).
        backend.apply_theme(system_prefers_dark());

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

windows_core::link!("advapi32.dll" "system" fn RegGetValueW(
    hkey: *mut core::ffi::c_void,
    lpsubkey: PCWSTR,
    lpvalue: PCWSTR,
    dwflags: u32,
    pdwtype: *mut u32,
    pvdata: *mut core::ffi::c_void,
    pcbdata: *mut u32,
) -> i32);

/// Live read of the system **app** theme: the per-user
/// `…\Themes\Personalize\AppsUseLightTheme` DWORD (`0` = dark, `1` = light) the OS
/// Settings UI writes and broadcasts (`WM_SETTINGCHANGE` "ImmersiveColorSet") when
/// the user flips it. Defaults to dark — the app's design default — if the value
/// is absent or unreadable.
fn system_prefers_dark() -> bool {
    // HKEY_CURRENT_USER, sign-extended to a pointer as the Win32 headers define it.
    const HKEY_CURRENT_USER: *mut core::ffi::c_void =
        (0x8000_0001u32 as i32) as isize as *mut core::ffi::c_void;
    // RRF_RT_REG_DWORD: only succeed if the value really is a DWORD.
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;

    let subkey: Vec<u16> =
        "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
            .encode_utf16()
            .collect();
    let value: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            core::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    // ERROR_SUCCESS == 0; AppsUseLightTheme == 0 means dark. Unreadable → dark.
    if status == 0 {
        data == 0
    } else {
        true
    }
}

/// Whether a `WM_SETTINGCHANGE` lParam names the immersive colour set (theme).
fn is_immersive_color_set(lparam: LPARAM) -> bool {
    if lparam == 0 {
        return false;
    }
    let s = unsafe { wide_str(lparam as *const u16) };
    s == "ImmersiveColorSet"
}

/// Read an IMM composition string (`GCS_COMPSTR` / `GCS_RESULTSTR`) into a
/// `String`. Two calls: size probe, then fetch.
unsafe fn imm_string(himc: HIMC, gcs: u32) -> Option<String> {
    let bytes = unsafe { ImmGetCompositionStringW(himc, gcs, core::ptr::null_mut(), 0) };
    if bytes <= 0 {
        return None;
    }
    let mut buf = vec![0u16; (bytes as usize).div_ceil(2)];
    let got = unsafe {
        ImmGetCompositionStringW(himc, gcs, buf.as_mut_ptr().cast(), bytes as u32)
    };
    if got <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..(got as usize) / 2]))
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

/// Start or stop the caret-blink timer to match whether a text field is
/// focused. Only toggles on a change of state so the blink phase is preserved
/// across unrelated input. A `GetCaretBlinkTime` of 0 / INFINITE (blinking
/// disabled) keeps the timer off — the caret then stays solid.
fn sync_blink(hwnd: HWND) {
    let want = shared()
        .map(|s| {
            s.render_host
                .with_reconciler_mut(|r| r.backend.wants_blink_timer())
        })
        .unwrap_or(false);
    let interval = unsafe { GetCaretBlinkTime() };
    let blink = want && interval != 0 && interval != BLINK_INFINITE;
    BLINK_RUNNING.with(|c| {
        if blink && !c.get() {
            unsafe {
                SetTimer(hwnd, BLINK_TIMER_ID, interval, None);
            }
            c.set(true);
        } else if !blink && c.get() {
            unsafe {
                let _ = KillTimer(hwnd, BLINK_TIMER_ID);
            }
            c.set(false);
        }
    });
}

/// Force the caret-blink timer off (window deactivated).
fn stop_blink(hwnd: HWND) {
    BLINK_RUNNING.with(|c| {
        if c.get() {
            unsafe {
                let _ = KillTimer(hwnd, BLINK_TIMER_ID);
            }
            c.set(false);
        }
    });
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

        // A UIA worker thread asked us to run a provider call on the UI thread.
        WM_APP_UIA => {
            let raw = wparam as *mut Box<dyn FnOnce() + Send>;
            if !raw.is_null() {
                let job = unsafe { Box::from_raw(raw) };
                job();
                // A UIA-driven action (Invoke/Toggle/SetValue) may have started a
                // spring; keep the animation timer running if so.
                if let Some(s) = shared() {
                    let anim =
                        s.render_host.with_reconciler_mut(|r| r.backend.is_animating());
                    start_timer(hwnd, anim);
                }
            }
            0
        }

        // UI Automation root request: hand back our window's root fragment
        // provider. Any other object id (MSAA client, etc.) falls through to the
        // default handler.
        WM_GETOBJECT => {
            if lparam as i32 == UIA_ROOT_OBJECT_ID
                && let Some(s) = shared()
                && let Some(root) = s.render_host.with_reconciler_mut(|r| r.backend.uia_root())
            {
                let provider = uia::root_provider(hwnd as isize, root);
                return unsafe {
                    UiaReturnRawElementProvider(hwnd, wparam, lparam, provider.as_raw())
                };
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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
                sync_blink(hwnd);
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

        WM_MOUSEWHEEL => {
            if let Some(s) = shared() {
                // wheel lParam carries SCREEN coords; convert to client DIPs.
                let mut pt = POINT {
                    x: (lparam & 0xFFFF) as i16 as i32,
                    y: ((lparam >> 16) & 0xFFFF) as i16 as i32,
                };
                unsafe {
                    let _ = ScreenToClient(hwnd, &mut pt);
                }
                let scale = dpi_scale(hwnd);
                let delta = ((wparam >> 16) & 0xFFFF) as i16 as i32;
                let (x, y) = (pt.x as f32 / scale, pt.y as f32 / scale);
                let started =
                    s.render_host.with_reconciler_mut(|r| r.backend.on_wheel(x, y, delta));
                start_timer(hwnd, started);
            }
            0
        }

        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if let Some(s) = shared() {
                let vk = (wparam & 0xFFFF) as u32;
                let shift = unsafe { GetKeyState(VK_SHIFT as i32) } < 0;
                let ctrl = unsafe { GetKeyState(VK_CONTROL as i32) } < 0;
                let started = s
                    .render_host
                    .with_reconciler_mut(|r| r.backend.on_key(vk, shift, ctrl));
                start_timer(hwnd, started);
                sync_blink(hwnd);
            }
            0
        }

        WM_CHAR => {
            if let Some(s) = shared() {
                let ch = (wparam & 0xFFFF) as u16;
                s.render_host.with_reconciler_mut(|r| r.backend.on_char(ch));
                sync_blink(hwnd);
            }
            0
        }

        // ── IME (IMM32 composition fallback) ─────────────────────────────
        WM_IME_STARTCOMPOSITION => {
            if let Some(s) = shared()
                && s.render_host.with_reconciler_mut(|r| r.backend.ime_begin())
            {
                // A text field owns composition: suppress the default IME window.
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_IME_COMPOSITION => {
            if let Some(s) = shared()
                && s.render_host.with_reconciler_mut(|r| r.backend.has_text_focus())
            {
                let himc = unsafe { ImmGetContext(hwnd) };
                if !himc.is_null() {
                    let flags = lparam as u32;
                    if flags & GCS_RESULTSTR != 0
                        && let Some(res) = unsafe { imm_string(himc, GCS_RESULTSTR) }
                    {
                        s.render_host
                            .with_reconciler_mut(|r| r.backend.ime_commit(&res));
                    }
                    if flags & GCS_COMPSTR != 0 {
                        let comp = unsafe { imm_string(himc, GCS_COMPSTR) }.unwrap_or_default();
                        s.render_host
                            .with_reconciler_mut(|r| r.backend.ime_update(&comp));
                    }
                    unsafe {
                        let _ = ImmReleaseContext(hwnd, himc);
                    }
                }
                sync_blink(hwnd);
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_IME_ENDCOMPOSITION => {
            if let Some(s) = shared()
                && s.render_host.with_reconciler_mut(|r| r.backend.has_text_focus())
            {
                s.render_host.with_reconciler_mut(|r| r.backend.ime_end());
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_SETFOCUS => {
            if let Some(s) = shared() {
                s.render_host
                    .with_reconciler_mut(|r| r.backend.window_focus_changed(true));
            }
            sync_blink(hwnd);
            0
        }

        WM_KILLFOCUS => {
            if let Some(s) = shared() {
                s.render_host.with_reconciler_mut(|r| {
                    r.backend.window_focus_changed(false);
                    r.backend.on_focus_lost();
                });
            }
            stop_blink(hwnd);
            0
        }

        WM_TIMER => {
            if wparam == BLINK_TIMER_ID {
                if let Some(s) = shared() {
                    s.render_host.with_reconciler_mut(|r| r.backend.blink_tick());
                }
                return 0;
            }
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
