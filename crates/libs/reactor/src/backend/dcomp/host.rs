//! `DCompHost`: the Win32 + system-compositor host that drives a root
//! [`Component`] through the two halves of the record seam: the reconciler
//! drives a [`RecordingBackend`] (the app half — a `Send` command buffer plus
//! the app-side closure maps), and the host owns the real [`DCompBackend`]
//! outright (the front half), replaying taken buffers into it after each
//! reconcile and feeding its queued intents back to the recorder (see
//! [`record`](super::record)). Input, UIA, caption and resize paths borrow the
//! backend directly — never the reconciler — which is the decoupling the
//! render-thread split needs. It owns the bare HWND and the blocking
//! `GetMessageW` pump (true idle — zero CPU at rest), and wakes/parks the
//! vsync [`FramePacer`] that paces canvas/viz frame ticks.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once};

use super::dispatch::{drain, LocalQueue, SendInner, WM_APP_DISPATCH};
use super::pacer::{FramePacer, WM_APP_FRAME};
use super::record::{self, FrontBackend, RecordingBackend};
use super::uia;
use super::*;
use crate::engine::RenderHost;
use crate::style::{set_current_color_scheme, set_theme_applier, requested_theme};
use crate::system_bindings::*;
use crate::{ColorScheme, Component, Element, RenderCx, RequestedTheme, WindowSize};
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

/// Run `f` against the backend on the UI thread, then deliver any app intents
/// it queued. Returns `None` when there is no live host on this thread, or
/// when either half is already borrowed — a re-entrant call, e.g. a
/// composition scoped batch whose `Completed` fires synchronously from inside
/// a backend mutation (the `try_` borrows refuse instead of panicking, and
/// callers defer through the pump). Must be called on the UI thread (the UIA
/// layer reaches it through [`marshal_to_ui`]).
pub(crate) fn with_backend<R>(f: impl FnOnce(&mut DCompBackend) -> R) -> Option<R> {
    let s = shared()?;
    let (out, intents) = {
        // Backend first: only once it is secured may the buffer be taken, so a
        // refusal can never strand taken commands unreplayed.
        let mut b = s.backend.try_borrow_mut().ok()?;
        // Input, UIA and caption reads all hit-test against laid-out
        // geometry, so any buffered commands must be replayed first. The
        // buffer is normally already empty here (`post_render` replays every
        // reconcile); this is the guarantee, not the common path.
        let cmds = s
            .render_host
            .try_with_reconciler_mut(|r| r.backend.take_cmds())?;
        record::replay(&mut *b, cmds);
        let out = f(&mut b);
        // UIA actions (Invoke/Toggle/SetValue) fire the same intents input
        // does...
        (out, b.take_intents())
    };
    // ...resolved app-side and run here, with every borrow released, so a
    // handler that pumps messages or re-enters the backend finds nothing held.
    run_intents(&s, intents);
    Some(out)
}

/// Route one input entry point into the front backend, then deliver the
/// intents it queued: the recorder resolves them against its app-side handler
/// maps, and the handlers run with every borrow released — same thread, same
/// message, so app-visible behaviour matches the old synchronous fire while
/// the queue stays the seam. Every wndproc arm that can make the backend fire
/// an event goes through here. Input never touches the reconciler: it borrows
/// the backend the host owns, which is exactly the coupling the render-thread
/// split removes.
fn dispatch_input<R>(s: &HostShared, f: impl FnOnce(&mut DCompBackend) -> R) -> R {
    let (out, intents) = {
        let mut b = s.backend.borrow_mut();
        let out = f(&mut b);
        (out, b.take_intents())
    };
    run_intents(s, intents);
    out
}

/// Resolve queued intents against the recorder's app-side maps and run the
/// jobs, with no borrow held across a job.
fn run_intents(s: &HostShared, intents: Vec<record::Intent>) {
    if intents.is_empty() {
        return;
    }
    let jobs = s
        .render_host
        .with_reconciler_mut(|r| r.backend.resolve_intents(intents));
    // A viz surface drag/scrub sink drove `drive_frame_ticks()` synchronously
    // when it ran inline; now that it runs a hop later as a job, the tick moves
    // here — after the sink has streamed its value, before the message returns —
    // so the drag preview repaints in the same message instead of waiting for
    // the next paced frame.
    let tick = jobs.iter().any(|j| j.drives_frame_tick());
    for job in jobs {
        job.run();
    }
    if tick {
        crate::drive_frame_ticks();
    }
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

/// Per-thread state the WndProc reaches into. `render_host` is a clone (an `Rc`
/// bump) of the host's render host; `local`/`send` are the dispatcher's queues.
struct HostShared {
    render_host: RenderHost<RecordingBackend, Win32Dispatcher>,
    /// The front half of the record seam: the real backend, owned by the host
    /// — not by the reconciler. Input, UIA, caption and resize paths borrow it
    /// here directly; the reconciler reaches it only through the taken command
    /// buffer `post_render` replays.
    backend: Rc<RefCell<DCompBackend>>,
    local: Rc<LocalQueue>,
    send: Arc<SendInner>,
    /// Vsync pacer for the canvas/viz frame ticks; dropping `HostShared` (end of
    /// [`DCompHost::run`]) tells its worker to exit.
    pacer: FramePacer,
    /// The host window (as `isize`; `HWND` is a raw pointer), for posting
    /// deferred UI-thread work and the DWM frame attribute.
    hwnd: isize,
    /// The last effective light/dark state applied end-to-end, so redundant
    /// triggers (a system flip under a forced override, a repeated request)
    /// skip the repaint/re-render fan-out.
    applied_dark: Cell<Option<bool>>,
}

thread_local! {
    static DCOMP: RefCell<Option<Rc<HostShared>>> = const { RefCell::new(None) };

    /// Set for the duration of our own [`release_capture_self`] call.
    ///
    /// `ReleaseCapture` delivers `WM_CAPTURECHANGED` **synchronously**, and a
    /// normal button-up releases capture before dispatching the release. Without
    /// this flag the capture-lost handler would tear the press down a moment
    /// before `on_pointer_up` went looking for it, and no click would ever
    /// activate. Only a capture change we did NOT initiate is a steal.
    static RELEASING_CAPTURE: Cell<bool> = const { Cell::new(false) };
}

/// Release pointer capture without the resulting synchronous
/// `WM_CAPTURECHANGED` being mistaken for the OS stealing it.
fn release_capture_self() {
    RELEASING_CAPTURE.with(|c| c.set(true));
    unsafe {
        let _ = ReleaseCapture();
    }
    RELEASING_CAPTURE.with(|c| c.set(false));
}

/// A self-hosted DirectComposition window hosting one reactor component tree.
pub struct DCompHost {
    hwnd: HWND,
}

impl DCompHost {
    /// Create the window, compositor, backend, and render host, mount `root`, and
    /// paint the first frame. Call [`run`](Self::run) to enter the message loop.
    pub fn new(title: impl AsRef<str>, root: Box<dyn Component>) -> windows_core::Result<Self> {
        // Default client size scales with the display: 80% of the monitor's work
        // area (floored to a usable minimum), so the window opens proportionate
        // on anything from a laptop panel to a 4K desktop. Explicit sizes go
        // through [`new_sized`](Self::new_sized).
        Self::new_impl(title, None, root)
    }

    /// Like [`new`](Self::new) but opens at a specific client size (DIPs).
    pub fn new_sized(
        title: impl AsRef<str>,
        client_w_dip: f64,
        client_h_dip: f64,
        root: Box<dyn Component>,
    ) -> windows_core::Result<Self> {
        Self::new_impl(title, Some((client_w_dip, client_h_dip)), root)
    }

    fn new_impl(
        title: impl AsRef<str>,
        client_dip: Option<(f64, f64)>,
        root: Box<dyn Component>,
    ) -> windows_core::Result<Self> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        // Record the UI thread so UIA provider calls can detect the in-thread
        // fast path versus needing to marshal (see `marshal_to_ui`).
        UI_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
        ensure_dispatcher_queue()?;

        let (hwnd, dpi, (pw, ph)) = create_window(title.as_ref(), client_dip)?;
        let scale = (dpi as f32 / 96.0).max(0.01);
        let dip = (pw as f32 / scale, ph as f32 / scale);

        let comp = Compositing::new(hwnd, pw, ph, dpi as f32)?;
        let mut backend = DCompBackend::new(comp, dip, dpi as f32, hwnd as isize);
        // Resolve the effective theme (app override, else the OS app theme)
        // before the first render, so `use_color_scheme`, the backdrop, and the
        // DWM frame are correct from frame one. Later changes — the app calling
        // `set_requested_theme`, or the OS flipping — go through
        // `apply_effective_theme`.
        let dark = effective_dark();
        set_current_color_scheme(scheme_for(dark));
        backend.apply_theme(dark);
        apply_frame_dark_mode(hwnd, dark);

        let dispatcher = Win32Dispatcher::new(hwnd);
        let marshaller = dispatcher.marshaller();
        let (local, send) = dispatcher.queues();

        // The two halves of the record seam: the host owns the real backend
        // outright, and the reconciler drives a recorder that holds no backend
        // reference at all — its calls become a `Send` command buffer that
        // `post_render` replays below. Replay is synchronous and on this
        // thread, so behaviour is unchanged; the ownership split is what lets
        // the reconciler later run off the thread that owns the HWND, the pump
        // and the compositor.
        let backend = Rc::new(RefCell::new(backend));
        let render_host = RenderHost::new(RecordingBackend::new(), root, dispatcher);
        render_host.set_marshaller(Some(marshaller));
        render_host.set_inner_size(WindowSize {
            width: dip.0 as f64,
            height: dip.1 as f64,
        });
        render_host.set_dpi(dpi);

        // After every reconcile: replay the buffer, adopt the new root, lay
        // out, paint. All control motion plays on the system compositor — no
        // timer to arm.
        let pr_host = render_host.clone_inner();
        let pr_backend = Rc::clone(&backend);
        render_host.set_post_render(move |root_id| {
            // Take the reconcile's commands, then release the reconciler
            // before touching the backend: the two borrows never overlap.
            let cmds = pr_host.with_reconciler_mut(|r| r.backend.take_cmds());
            let intents = {
                let mut b = pr_backend.borrow_mut();
                // Replay before anything reads the arena: `set_root` looks the
                // new root up and silently no-ops if it is absent, and layout
                // walks the whole tree.
                record::replay(&mut *b, cmds);
                // After the buffer, so a surface requested in the same frame its
                // host mounts finds the control already in the arena; before
                // layout, so the sprite is parented when sizes are pushed.
                b.service_surface_ops();
                // Apply the frame's pointer-surface presence declarations into
                // the front-side interest map so a bit filled during this render
                // is visible to the next input message (one frame ahead, by
                // design — the router routes on these bits, never the closures).
                pointer::service_ops();
                b.set_root(root_id);
                b.relayout_and_paint();
                // Replay fires no events today, so this is normally empty —
                // taken anyway so an intent can never sit in the queue until
                // the next input message if a replayed path ever gains one.
                b.take_intents()
            };
            if !intents.is_empty() {
                let jobs = pr_host.with_reconciler_mut(|r| r.backend.resolve_intents(intents));
                for job in jobs {
                    job.run();
                }
            }
        });

        let pacer = FramePacer::new(hwnd as isize);
        let pump_wake = pacer.wake_handle();
        DCOMP.with(|c| {
            *c.borrow_mut() = Some(Rc::new(HostShared {
                render_host: render_host.clone_inner(),
                backend,
                local,
                send,
                pacer,
                hwnd: hwnd as isize,
                applied_dark: Cell::new(Some(dark)),
            }));
        });

        // Route `set_requested_theme` into this host. The setter typically fires
        // from inside event dispatch, where a backend borrow is still held — so
        // the apply is deferred through the message pump rather than run
        // synchronously (the hook only pokes; `requested_theme()` is re-read at
        // apply time, so coalesced posts are harmless). The hwnd is captured by
        // value, not read from this thread's host state: the hook must keep
        // working when the setter fires on the app thread, where `shared()` has
        // nothing — the posted job re-resolves everything on the pump thread.
        let theme_hwnd = hwnd as isize;
        set_theme_applier(Some(std::sync::Arc::new(move |_| {
            post_ui(theme_hwnd, || {
                if let Some(s) = shared() {
                    apply_effective_theme(&s);
                }
            });
        })));

        // Frame-tick pump: when a canvas/viz subscriber appears (via
        // `on_frame_tick`) while the pacer is parked, wake it; the WM_APP_FRAME
        // handler drives the ticks and parks the pacer once no subscriber
        // remains (true idle). This is the pacer's ONLY client — control motion
        // never runs on it. The hook holds a cross-thread wake handle rather
        // than reading this thread's host state: a subscriber may register on
        // the app thread once the reconciler moves off the pump thread.
        crate::set_frame_pump_wake(Some(std::sync::Arc::new(move || pump_wake.wake())));

        render_host.kick();
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW as i32);
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
        set_theme_applier(None);
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

    /// Like [`render`](Self::render) but opens at a specific client size (DIPs).
    pub fn render_sized<F>(
        title: impl AsRef<str>,
        client_w_dip: f64,
        client_h_dip: f64,
        f: F,
    ) -> windows_core::Result<()>
    where
        F: Fn(&mut RenderCx) -> Element + 'static,
    {
        struct RenderFn<F>(F);
        impl<F: Fn(&mut RenderCx) -> Element + 'static> Component for RenderFn<F> {
            fn render(&self, _props: &(), cx: &mut RenderCx) -> Element {
                (self.0)(cx)
            }
        }
        let host = Self::new_sized(title, client_w_dip, client_h_dip, Box::new(RenderFn(f)))?;
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
    // Raw WinRT `Windows.UI.Color` (8-bit sRGB) for the compositor's backdrop brush,
    // not a reactor theming `Color` — plain ARGB literals.
    if dark {
        Color { a: 255, r: 14, g: 14, b: 17 }
    } else {
        Color { a: 255, r: 243, g: 243, b: 245 }
    }
}

/// The effective light/dark state: the app's [`requested_theme`] override when
/// forced, else the live OS app theme.
fn effective_dark() -> bool {
    match requested_theme() {
        RequestedTheme::Dark => true,
        RequestedTheme::Light => false,
        RequestedTheme::Default => system_prefers_dark(),
    }
}

fn scheme_for(dark: bool) -> ColorScheme {
    if dark {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}

/// Resolve and apply the effective theme end-to-end: the `use_color_scheme`
/// signal, the DWM frame, the compositor backdrop, a chrome repaint, and a
/// component re-render. No-op when the effective state hasn't changed (e.g. a
/// system flip under a forced override). Both triggers — the app calling
/// `set_requested_theme` and a `WM_SETTINGCHANGE` "ImmersiveColorSet" — land
/// here, so override precedence lives in exactly one place.
fn apply_effective_theme(s: &HostShared) {
    let dark = effective_dark();
    if s.applied_dark.get() == Some(dark) {
        return;
    }
    s.applied_dark.set(Some(dark));
    // Scheme first: the re-render below must already read the new value.
    set_current_color_scheme(scheme_for(dark));
    apply_frame_dark_mode(s.hwnd as HWND, dark);
    {
        let mut b = s.backend.borrow_mut();
        b.apply_theme(dark);
        b.mark_all_dirty_and_repaint();
    }
    // Every component re-renders on the next pass: value-color props are
    // recomputed only inside render functions, so memoised components with
    // unchanged props would otherwise keep the old palette.
    s.render_host
        .with_reconciler_mut(|r| r.invalidate_all_components());
    // Re-run component render functions so scheme-derived values (colors,
    // `use_color_scheme`) are re-read (the repaint above only covers
    // backend-drawn chrome).
    s.render_host.request_render();
}

windows_core::link!("dwmapi.dll" "system" fn DwmSetWindowAttribute(
    hwnd: HWND,
    dwattribute: u32,
    pvattribute: *const core::ffi::c_void,
    cbattribute: u32,
) -> i32);

/// Keep the DWM-owned window chrome (frame border, snap-layout flyout) on the
/// effective theme via `DWMWA_USE_IMMERSIVE_DARK_MODE`. The caption strip is
/// drawn in-client, but DWM still renders the outer frame around it.
fn apply_frame_dark_mode(hwnd: HWND, dark: bool) {
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    // The attribute is a Win32 BOOL (4 bytes).
    let value: i32 = dark as i32;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&value as *const i32).cast(),
            size_of::<i32>() as u32,
        );
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

/// Window DPI from `GetDpiForWindow`, falling back to 96 (100%) when it is
/// unavailable (e.g. the window is not yet on a monitor).
///
/// A `NEWAPO_DCOMP_FORCE_DPI` env override forces the whole DPI→pixel pipeline
/// to a chosen value (e.g. `144` = 150%), so high-DPI layout can be exercised on a
/// 100%-scaled development monitor. It is read only when the variable is present,
/// so it has zero effect on a normal run.
fn effective_dpi(hwnd: HWND) -> u32 {
    if let Ok(v) = std::env::var("NEWAPO_DCOMP_FORCE_DPI")
        && let Ok(d) = v.trim().parse::<u32>()
        && d >= 48
    {
        return d;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 96 } else { dpi }
}

fn dpi_scale(hwnd: HWND) -> f32 {
    effective_dpi(hwnd) as f32 / 96.0
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

/// Physical height of the resize frame at this DPI (also the maximized
/// client-top overhang the frame extension must pad back in).
fn frame_y_px(hwnd: HWND) -> i32 {
    let dpi = effective_dpi(hwnd);
    unsafe {
        GetSystemMetricsForDpi(SM_CYFRAME as i32, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER as i32, dpi)
    }
}

/// Repaint the caption band after a hover/maximize state flip.
fn repaint_caption() {
    if let Some(s) = shared() {
        s.backend.borrow_mut().repaint_caption();
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1, // the compositor owns every pixel; never erase/flash.

        // ── Extended frame: remove the native caption, keep resize borders ──
        WM_NCCALCSIZE if wparam != 0 => {
            let params = lparam as *mut NCCALCSIZE_PARAMS;
            if params.is_null() {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
            let top = unsafe { (*params).rgrc[0].top };
            let r = unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            // DefWindowProc reserved borders + caption; restore the top so the
            // client extends into the caption (the reactor TitleBar band is the
            // caption). A maximized window overhangs the monitor by the frame,
            // so pad that back in to keep the band on-screen.
            let pad = if unsafe { IsZoomed(hwnd) }.as_bool() {
                frame_y_px(hwnd)
            } else {
                0
            };
            unsafe {
                (*params).rgrc[0].top = top + pad;
            }
            r
        }

        // Frame hit-testing: resize borders from DefWindowProc; then the top
        // resize band, the drawn min/max/close cluster (HTMAXBUTTON is what
        // summons Win11 snap layouts), interactive content, and finally the
        // caption drag region.
        WM_NCHITTEST => {
            let def = unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            if def as u32 != HTCLIENT {
                return def;
            }
            let mut pt = POINT {
                x: (lparam & 0xFFFF) as i16 as i32,
                y: ((lparam >> 16) & 0xFFFF) as i16 as i32,
            };
            unsafe {
                let _ = ScreenToClient(hwnd, &mut pt);
            }
            if !unsafe { IsZoomed(hwnd) }.as_bool() && pt.y < frame_y_px(hwnd) {
                return HTTOP as LRESULT;
            }
            let scale = dpi_scale(hwnd);
            let (x, y) = (pt.x as f32 / scale, pt.y as f32 / scale);
            let Some(s) = shared() else {
                return HTCLIENT as LRESULT;
            };
            if let Some((cx, cy, cw, ch)) = s.backend.borrow_mut().caption_rect()
                && x >= cx
                && x < cx + cw
                && y >= cy
                && y < cy + ch
            {
                // The drawn back button sits at the LEADING edge, so it is
                // tested before the trailing cluster. `HTSYSMENU` is the
                // non-client code for that corner, which buys the whole
                // hover/press pipeline the window buttons already use — see
                // `caption::index_for_hit` for the double-click hazard it
                // brings and the `WM_NCLBUTTONDBLCLK` arm that defuses it.
                if s.backend.borrow_mut().back_button_active()
                    && let Some((bx, by, bw, bh)) = s.backend.borrow_mut().back_button_rect()
                    && x >= bx
                    && x < bx + bw
                    && y >= by
                    && y < by + bh
                {
                    return caption::HTSYSMENU as LRESULT;
                }
                let from_right = (cx + cw) - x;
                if from_right <= caption::BTN_W {
                    return HTCLOSE as LRESULT;
                }
                if from_right <= 2.0 * caption::BTN_W {
                    return HTMAXBUTTON as LRESULT;
                }
                if from_right <= 3.0 * caption::BTN_W {
                    return HTMINBUTTON as LRESULT;
                }
                let keep_client = s.backend.borrow_mut().wants_client_at(x, y);
                if !keep_client {
                    return HTCAPTION as LRESULT;
                }
            }
            HTCLIENT as LRESULT
        }

        // Hover feedback for the drawn caption buttons (non-client mouse).
        WM_NCMOUSEMOVE => {
            if caption::set_hover(caption::index_for_hit(wparam as u32)) {
                repaint_caption();
            }
            let mut t = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE | TME_NONCLIENT,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            unsafe {
                let _ = TrackMouseEvent(&mut t);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_NCMOUSELEAVE => {
            if caption::set_hover(-1) {
                repaint_caption();
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        // Swallow presses on the drawn buttons (DefWindowProc would render the
        // classic non-client chrome); dispatch the command on release.
        WM_NCLBUTTONDOWN => {
            let idx = caption::index_for_hit(wparam as u32);
            if idx >= 0 {
                caption::set_pressed(idx);
                // Only the back button draws a distinct pressed state, so it
                // is the only one whose arming needs a repaint.
                if idx == caption::BACK_INDEX {
                    repaint_caption();
                }
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        // `DefWindowProc` closes the window on a double-click over `HTSYSMENU`
        // — the classic system-menu gesture. The back button borrows that hit
        // code (see `caption::index_for_hit`), so a fast double tap on it must
        // be swallowed here or it would close the app. Each click has already
        // been dispatched as its own press/release pair.
        caption::WM_NCLBUTTONDBLCLK => {
            if caption::index_for_hit(wparam as u32) == caption::BACK_INDEX {
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_NCLBUTTONUP => {
            let idx = caption::index_for_hit(wparam as u32);
            let pressed = caption::pressed();
            caption::set_pressed(-1);
            if idx >= 0 && idx == pressed {
                // The back button is an APP command, not a system one: it
                // raises the TitleBar's `BackRequested` rather than posting a
                // `WM_SYSCOMMAND`.
                if idx == caption::BACK_INDEX {
                    if let Some(s) = shared() {
                        dispatch_input(&s, |b| b.raise_back_requested());
                    }
                    repaint_caption();
                    return 0;
                }
                let cmd = match idx {
                    0 => SC_MINIMIZE,
                    1 if unsafe { IsZoomed(hwnd) }.as_bool() => SC_RESTORE,
                    1 => SC_MAXIMIZE,
                    _ => SC_CLOSE,
                };
                unsafe {
                    let _ = PostMessageW(hwnd, WM_SYSCOMMAND, cmd as WPARAM, 0);
                }
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_APP_DISPATCH => {
            if let Some(s) = shared() {
                drain(&s.local, &s.send);
            }
            0
        }

        // The pacer worker's vsync tick: drive the canvas/viz frame-tick
        // subscribers, then park the pacer once none remain (true idle).
        WM_APP_FRAME => {
            if let Some(s) = shared() {
                s.pacer.begin_tick();
                crate::drive_frame_ticks();
                if !crate::frame_ticks_active() {
                    s.pacer.park();
                }
            }
            0
        }

        // A UIA worker thread asked us to run a provider call on the UI thread.
        WM_APP_UIA => {
            let raw = wparam as *mut Box<dyn FnOnce() + Send>;
            if !raw.is_null() {
                let job = unsafe { Box::from_raw(raw) };
                job();
            }
            0
        }

        // UI Automation root request: hand back our window's root fragment
        // provider. Any other object id (MSAA client, etc.) falls through to the
        // default handler.
        WM_GETOBJECT => {
            if lparam as i32 == UIA_ROOT_OBJECT_ID
                && let Some(s) = shared()
                && let Some(root) = s.backend.borrow_mut().uia_root()
            {
                let provider = uia::root_provider(hwnd as isize, root);
                return unsafe {
                    UiaReturnRawElementProvider(hwnd, wparam, lparam, provider.as_raw())
                };
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_MOUSEMOVE => {
            // The cursor is in the client area — any caption-button hover ends.
            if caption::set_hover(-1) {
                repaint_caption();
            }
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                track_leave(hwnd);
                dispatch_input(&s, |b| b.on_pointer_move(x, y));
            }
            0
        }

        WM_MOUSELEAVE => {
            if let Some(s) = shared() {
                dispatch_input(&s, |b| b.on_pointer_leave());
            }
            0
        }

        WM_LBUTTONDOWN => {
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                let captured = dispatch_input(&s, |b| b.on_pointer_down(x, y));
                if captured {
                    unsafe {
                        SetCapture(hwnd);
                    }
                }
            }
            0
        }

        WM_LBUTTONUP => {
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                release_capture_self();
                dispatch_input(&s, |b| b.on_pointer_up(x, y));
            }
            0
        }

        // Capture was taken from us: a system modal dialog, Alt+Tab, Win+D, a
        // debugger break. No WM_LBUTTONUP will follow, so the gesture has to be
        // cancelled here or its state stays live forever — a stuck global
        // `scrubbing` flag leaves every slider, knob and meter in the window
        // snapping instead of springing until the next clean press/release.
        //
        // Our own ReleaseCapture also lands here (synchronously, from the
        // button-up arm above); `release_capture_self` marks those so a normal
        // click is not cancelled out from under itself.
        WM_CAPTURECHANGED => {
            if !RELEASING_CAPTURE.with(|c| c.get())
                && let Some(s) = shared()
            {
                dispatch_input(&s, |b| b.on_pointer_cancel());
            }
            0
        }

        // Right button: no capture and no press ink (see
        // `on_right_pointer_down`) — it only reports a right-tap, which is what
        // a context menu hangs off. Unhandled presses fall through to
        // DefWindowProc so the system context-menu path still works.
        WM_RBUTTONDOWN => {
            let (x, y) = dip_xy(hwnd, lparam);
            let consumed =
                shared().is_some_and(|s| dispatch_input(&s, |b| b.on_right_pointer_down(x, y)));
            if consumed {
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_RBUTTONUP => {
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                dispatch_input(&s, |b| b.on_right_pointer_up(x, y));
            }
            0
        }

        // Both wheel axes decode identically: SCREEN coords in lParam, a signed
        // 120-per-detent delta in the high word of wParam. They differ only in
        // what the sign MEANS — WM_MOUSEWHEEL is positive away from the user,
        // WM_MOUSEHWHEEL is positive to the RIGHT. Neither is remapped here;
        // the delta goes through raw and the backend tags it with its axis, so
        // a sink reads each with the convention Windows documents for it.
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
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
                let horizontal = msg == WM_MOUSEHWHEEL;
                dispatch_input(&s, |b| {
                    if horizontal {
                        b.on_wheel_h(x, y, delta)
                    } else {
                        b.on_wheel(x, y, delta)
                    }
                });
            }
            0
        }

        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let mut consumed = false;
            if let Some(s) = shared() {
                let vk = (wparam & 0xFFFF) as u32;
                let mut mods = crate::VirtualKeyModifiers::None;
                if unsafe { GetKeyState(VK_SHIFT as i32) } < 0 {
                    mods |= crate::VirtualKeyModifiers::Shift;
                }
                if unsafe { GetKeyState(VK_CONTROL as i32) } < 0 {
                    mods |= crate::VirtualKeyModifiers::Control;
                }
                if unsafe { GetKeyState(VK_MENU as i32) } < 0 {
                    mods |= crate::VirtualKeyModifiers::Menu;
                }
                consumed = dispatch_input(&s, |b| b.on_key(vk, mods));
            }
            // A sys-key (Alt-chord, F10) the backend did not consume must reach
            // `DefWindowProc`, or Alt+F4 / F10 / Alt+Space are swallowed (§7.3).
            if input::sys_key_falls_through(msg == WM_SYSKEYDOWN, consumed) {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
            0
        }

        // Ends the held/auto-repeat state started by the matching key-down.
        WM_KEYUP | WM_SYSKEYUP => {
            let mut consumed = false;
            if let Some(s) = shared() {
                let vk = (wparam & 0xFFFF) as u32;
                consumed = dispatch_input(&s, |b| b.on_key_up(vk));
            }
            // Sys-key releases (Alt-tap-to-menu, F10) likewise fall through.
            if input::sys_key_falls_through(msg == WM_SYSKEYUP, consumed) {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
            0
        }

        WM_CHAR => {
            if let Some(s) = shared() {
                let ch = (wparam & 0xFFFF) as u16;
                dispatch_input(&s, |b| b.on_char(ch));
            }
            0
        }

        // ── IME (IMM32 composition fallback) ─────────────────────────────
        WM_IME_STARTCOMPOSITION => {
            if let Some(s) = shared()
                && dispatch_input(&s, |b| b.ime_begin())
            {
                // A text field owns composition: suppress the default IME window.
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_IME_COMPOSITION => {
            if let Some(s) = shared()
                && s.backend.borrow_mut().has_text_focus()
            {
                let himc = unsafe { ImmGetContext(hwnd) };
                if !himc.is_null() {
                    let flags = lparam as u32;
                    if flags & GCS_RESULTSTR != 0
                        && let Some(res) = unsafe { imm_string(himc, GCS_RESULTSTR) }
                    {
                        dispatch_input(&s, |b| b.ime_commit(&res));
                    }
                    if flags & GCS_COMPSTR != 0 {
                        let comp = unsafe { imm_string(himc, GCS_COMPSTR) }.unwrap_or_default();
                        dispatch_input(&s, |b| b.ime_update(&comp));
                    }
                    unsafe {
                        let _ = ImmReleaseContext(hwnd, himc);
                    }
                }
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_IME_ENDCOMPOSITION => {
            if let Some(s) = shared()
                && s.backend.borrow_mut().has_text_focus()
            {
                dispatch_input(&s, |b| b.ime_end());
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_SETFOCUS => {
            if let Some(s) = shared() {
                dispatch_input(&s, |b| b.window_focus_changed(true));
            }
            0
        }

        WM_KILLFOCUS => {
            if let Some(s) = shared() {
                dispatch_input(&s, |b| {
                    b.window_focus_changed(false);
                    b.on_focus_lost();
                });
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

        // The display mode / topology changed — HDR toggled, the SDR white level or
        // panel changed. Let the app re-fit its colour map (it owns colour policy via
        // the callback), then repaint all chrome so already-painted surfaces pick up
        // the new draw-time map. Viz surfaces repaint every frame, so only static
        // chrome needs the nudge.
        WM_DISPLAYCHANGE => {
            display_change::note_display_change(hwnd);
            if let Some(s) = shared() {
                s.backend.borrow_mut().mark_all_dirty_and_repaint();
            }
            0
        }

        // The window moved/resized: re-fit only when it landed on a different
        // monitor, then fall through to DefWindowProc (which synthesizes WM_SIZE /
        // WM_MOVE from WM_WINDOWPOSCHANGED).
        WM_WINDOWPOSCHANGED => {
            display_change::note_possible_monitor_change(hwnd);
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_SETTINGCHANGE => {
            // The OS "SDR content brightness" slider and auto-colour / HDR-mode
            // toggles broadcast a plain WM_SETTINGCHANGE, so re-fit the colour map.
            display_change::note_display_change(hwnd);
            // An "ImmersiveColorSet" change flips the system light/dark theme.
            // Routed through the effective-theme resolver, so an app-forced
            // Light/Dark override ignores the system flip.
            if is_immersive_color_set(lparam)
                && let Some(s) = shared()
            {
                apply_effective_theme(&s);
            }
            // A caret blink-rate change also broadcasts WM_SETTINGCHANGE:
            // restart the focused field's compositor blink with the new period
            // (a no-op when no text field is focused).
            if let Some(s) = shared() {
                s.backend.borrow_mut().refresh_caret_blink();
            }
            0
        }

        WM_SIZE => {
            // Maximize/restore flips the drawn max-button glyph.
            if caption::set_maximized(wparam == SIZE_MAXIMIZED as WPARAM) {
                repaint_caption();
            }
            // Notify the app of a visibility edge: minimizing hides the window
            // (SIZE_MINIMIZED), restoring/maximizing shows it. De-duplicated inside
            // `note_visibility`, so an ordinary resize (already-visible → visible) is a
            // no-op. Lets the app pause expensive off-screen work while minimized.
            let visible = wparam != SIZE_MINIMIZED as WPARAM;
            visibility::note_visibility(visible);
            if let Some(s) = shared() {
                // Same gate for the frame pacer: a minimized window must not
                // keep rasterizing canvas frames nobody can see.
                s.pacer.set_visible(visible);
                let pw = (lparam & 0xFFFF) as i32;
                let ph = ((lparam >> 16) & 0xFFFF) as i32;
                if pw > 0 && ph > 0 {
                    let dpi = effective_dpi(hwnd);
                    let scale = dpi as f32 / 96.0;
                    s.render_host.set_dpi(dpi);
                    s.render_host.set_inner_size(WindowSize {
                        width: pw as f64 / scale as f64,
                        height: ph as f64 / scale as f64,
                    });
                    s.backend.borrow_mut().resize(pw, ph, dpi);
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

/// Create the window at a desired CLIENT size in DIPs (DPI-scaled to pixels,
/// non-client area added, centered on the nearest monitor's work area), and
/// return its HWND, DPI, and actual client pixel size.
fn create_window(
    title: &str,
    client_dip: Option<(f64, f64)>,
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

    // Create at a provisional size; we resize to the exact client size below once
    // we know the window's DPI and non-client metrics.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            PCWSTR(CLASS.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1000,
            700,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            hinstance,
            core::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(windows_core::Error::empty());
    }

    let dpi = effective_dpi(hwnd);
    let scale = dpi as f64 / 96.0;

    unsafe {
        // Non-client delta (borders + caption) for this window at this DPI.
        let mut wr = RECT::default();
        let mut cr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        let _ = GetClientRect(hwnd, &mut cr);
        let nc_w = (wr.right - wr.left) - (cr.right - cr.left);
        let nc_h = (wr.bottom - wr.top) - (cr.bottom - cr.top);
        // The nearest monitor's work area (also the centering target).
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = core::mem::zeroed();
        mi.cbSize = size_of::<MONITORINFO>() as u32;
        let have_mi = GetMonitorInfoW(mon, &mut mi).as_bool();
        let work_w = if have_mi { mi.rcWork.right - mi.rcWork.left } else { 0 };
        let work_h = if have_mi { mi.rcWork.bottom - mi.rcWork.top } else { 0 };
        // Desired client size in physical pixels: the caller's explicit DIP size,
        // or a display-proportionate default — 80% of the work area, floored to a
        // usable minimum — so a 4K desktop doesn't open a laptop-sized window.
        let (cw, ch) = match client_dip {
            Some((w, h)) => ((w * scale).round() as i32, (h * scale).round() as i32),
            None => {
                let min_w = (1200.0 * scale) as i32;
                let min_h = (800.0 * scale) as i32;
                let avail_w = (work_w - nc_w).max(1);
                let avail_h = (work_h - nc_h).max(1);
                (
                    (work_w * 4 / 5).max(min_w).min(avail_w),
                    (work_h * 4 / 5).max(min_h).min(avail_h),
                )
            }
        };
        let win_w = cw + nc_w;
        let win_h = ch + nc_h;
        // Center on the work area.
        let (mut x, mut y) = (CW_USEDEFAULT, CW_USEDEFAULT);
        if have_mi {
            x = mi.rcWork.left + (work_w - win_w).max(0) / 2;
            y = mi.rcWork.top + (work_h - win_h).max(0) / 2;
        }
        let _ = SetWindowPos(hwnd, core::ptr::null_mut(), x, y, win_w, win_h, SWP_NOZORDER);
    }

    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let pw = (rc.right - rc.left).max(1);
    let ph = (rc.bottom - rc.top).max(1);
    Ok((hwnd, dpi, (pw, ph)))
}
