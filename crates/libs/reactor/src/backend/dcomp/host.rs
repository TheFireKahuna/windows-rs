//! `DCompHost`: the **front thread** of the render-thread split — the Win32 +
//! system-compositor host that owns the HWND, the blocking `GetMessageW` pump
//! (true idle — zero CPU at rest), the input state machine, the real
//! [`DCompBackend`] (retained tree + compositor), and the vsync [`FramePacer`].
//! The root [`Component`] runs on a separate **app thread**
//! ([`dispatch::spawn_app_thread`]): its reconciler drives a
//! [`RecordingBackend`] whose `Send` command buffer ships here per reconcile
//! ([`post_commit`]) and is replayed into the backend, and input's queued
//! intents ship the other way ([`run_intents`] →
//! [`dispatch::deliver_intents`]). The two threads share nothing but `Send`
//! data; input, UIA, caption and resize paths borrow the front backend
//! directly and never block on app logic.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once};

use super::dispatch::{self, AppQueue};
use super::pacer::{FramePacer, WM_APP_FRAME};
use super::record::{self, FrontBackend};
use super::uia;
use super::*;
use crate::style::{set_current_color_scheme, set_theme_applier, requested_theme};
use crate::system_bindings::*;
use crate::{ColorScheme, Component, ControlId, Element, RenderCx, RequestedTheme, WindowSize};
// Explicit imports, so they win over the `system_bindings::*` glob above: the
// compositor half of this file is the safe wrapper's `Color` and dispatcher
// queue, never the raw generated bindings.
use windows_composition::{Color, DispatcherQueueController};
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
/// True while a pointer capture (a drag/gesture) is held.
///
/// Readable from any thread so app-thread render code can tell a gesture is in
/// flight. Settled-state UI — a config read-out, a serialized view, anything the
/// user cannot meaningfully read mid-drag — should hold still while this is set
/// instead of re-deriving itself on every pointer move: each such re-derivation
/// records a real command, and a non-empty buffer commits, relayouts, repaints
/// and recomposites the window, which tears the off-thread live surfaces.
static POINTER_CAPTURE: AtomicBool = AtomicBool::new(false);

/// See `POINTER_CAPTURE`. `true` between a capturing press and its release.
pub fn pointer_capture_active() -> bool {
    POINTER_CAPTURE.load(Ordering::Relaxed)
}

pub(crate) fn on_ui_thread() -> bool {
    UI_THREAD_ID.load(Ordering::Relaxed) == unsafe { GetCurrentThreadId() }
}

/// Run `f` against the backend on the UI thread, then ship any app intents it
/// queued to the app thread. Returns `None` when there is no live host on
/// this thread, or when the backend is already borrowed — a re-entrant call,
/// e.g. a composition scoped batch whose `Completed` fires synchronously from
/// inside a backend mutation (the `try_` borrow refuses instead of panicking,
/// and callers defer through the pump). Must be called on the UI thread (the
/// UIA layer reaches it through [`marshal_to_ui`]). The tree it reads is
/// always whole: each app commit is replayed in full within one message
/// ([`post_commit`]), so between messages there is no half-applied state.
pub(crate) fn with_backend<R>(f: impl FnOnce(&mut DCompBackend) -> R) -> Option<R> {
    let s = shared()?;
    let (out, intents) = {
        let mut b = s.backend.try_borrow_mut().ok()?;
        let out = f(&mut b);
        // UIA actions (Invoke/Toggle/SetValue) fire the same intents input
        // does — front state flips synchronously above, the app closure runs
        // a hop later on its own thread.
        (out, b.take_intents())
    };
    run_intents(&s, intents);
    Some(out)
}

/// Route one input entry point into the front backend, then ship the intents
/// it queued to the app thread. Every wndproc arm that can make the backend
/// fire an event goes through here. Input touches nothing but the front
/// backend — immediate feedback (hover/press ink, drag echo, scroll) is
/// served from the retained tree in this borrow, and app logic runs a hop
/// later from the queue, so input latency no longer couples to app-thread
/// load. That decoupling is the point of the render-thread split.
fn dispatch_input<R>(s: &HostShared, f: impl FnOnce(&mut DCompBackend) -> R) -> R {
    let (out, intents) = {
        let mut b = s.backend.borrow_mut();
        let out = f(&mut b);
        (out, b.take_intents())
    };
    run_intents(s, intents);
    out
}

/// Ship queued intents to the app thread, where the recorder resolves them
/// against its app-side handler maps and runs the jobs
/// ([`dispatch::deliver_intents`]). Fire-and-forget: the front never waits on
/// app logic.
fn run_intents(s: &HostShared, intents: Vec<record::Intent>) {
    if intents.is_empty() {
        return;
    }
    s.app
        .post(Box::new(move || dispatch::deliver_intents(intents)));
}

/// The app→front commit edge: called from the app thread after each reconcile
/// with the recorded command buffer and the new root. The posted job replays
/// the buffer into the front backend, services the queued surface and
/// pointer-interest declarations, re-lays-out and paints — the front half of
/// what `post_render` used to run inline — then ships any intents the replay
/// raised back to the app thread (today replay fires no events; taken anyway
/// so one can never sit in the queue until the next input message).
pub(crate) fn post_commit(hwnd: isize, cmds: Vec<record::Cmd>, root_id: Option<ControlId>) {
    post_ui(hwnd, move || {
        let Some(s) = shared() else { return };
        apply_commit(&s, cmds, root_id);
    });
}

/// Replay one command buffer into the front backend, service the queued surface-
/// and pointer-interest declarations, re-lay-out and paint, then ship any intents
/// the replay raised back to the app thread (today replay fires no events; taken
/// anyway so one can never sit in the queue until the next input message). Used by
/// the eager [`post_commit`] path and by the on-release [`end_capture`] flush.
fn apply_commit(s: &HostShared, cmds: Vec<record::Cmd>, root_id: Option<ControlId>) {
    let intents = {
        let mut b = s.backend.borrow_mut();
        // Replay before anything reads the arena: `set_root` looks the new root up
        // and silently no-ops if it is absent, and layout walks the whole tree.
        record::replay(&mut *b, cmds);
        // Apply the frame's pointer-surface presence declarations into the front-side
        // interest map so a bit filled during this render is visible to the next input
        // message (one frame ahead, by design — the router routes on these bits, never
        // the closures).
        pointer::service_ops();
        b.set_root(root_id);
        b.relayout_and_paint();
        // After layout, so a client that reacts by re-reading the subtree finds
        // the geometry it will be told about rather than the previous frame's.
        b.flush_structure_changes();
        b.take_intents()
    };
    run_intents(s, intents);
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

/// Per-thread state the front WndProc reaches into.
pub(crate) struct HostShared {
    /// The front half of the record seam: the real backend, owned by the host
    /// — not by the reconciler. Input, UIA, caption and resize paths borrow it
    /// here directly; the app thread's reconciler reaches it only through the
    /// command buffers [`post_commit`] replays.
    pub(crate) backend: Rc<RefCell<DCompBackend>>,
    /// The app thread's run loop: intents, size/theme notifications and frame
    /// ticks post here. Fire-and-forget — the front never blocks on it.
    app: Arc<AppQueue>,
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
    /// The thread's dispatcher queue, which the system `Compositor` requires to
    /// exist for as long as it is driven.
    ///
    /// This used to be created and immediately dropped on the floor — the raw
    /// `CreateDispatcherQueueController` out-pointer was never wrapped, so the
    /// controller was leaked and the queue survived by accident. The wrapper
    /// hands back an owned value, so it has to be held somewhere; here, and
    /// DELIBERATELY as the last field, because fields drop in declaration order
    /// and the queue must outlive `backend` (which owns the compositor).
    ///
    /// `None` when the thread already had a controller — see
    /// [`ensure_dispatcher_queue`].
    #[allow(dead_code)]
    queue: Option<DispatcherQueueController>,
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
/// The window, input and compositor live on the creating (front) thread; the
/// component tree runs on a spawned app thread, so `root` must be `Send` —
/// it is *constructed* wherever the caller builds it but only ever *runs* on
/// the app thread.
pub struct DCompHost {
    hwnd: HWND,
    /// The app thread's queue (for the shutdown quit) and join handle, taken
    /// by [`run`](Self::run) when the pump exits.
    app: Arc<AppQueue>,
    app_join: RefCell<Option<std::thread::JoinHandle<()>>>,
}

impl DCompHost {
    /// Create the window, compositor and backend, spawn the app thread, mount
    /// `root` on it, and schedule the first frame. Call [`run`](Self::run) to
    /// enter the message loop.
    pub fn new(
        title: impl AsRef<str>,
        root: Box<dyn Component + Send>,
    ) -> windows_core::Result<Self> {
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
        root: Box<dyn Component + Send>,
    ) -> windows_core::Result<Self> {
        Self::new_impl(title, Some((client_w_dip, client_h_dip)), root)
    }

    fn new_impl(
        title: impl AsRef<str>,
        client_dip: Option<(f64, f64)>,
        root: Box<dyn Component + Send>,
    ) -> windows_core::Result<Self> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        // Record the UI thread so UIA provider calls can detect the in-thread
        // fast path versus needing to marshal (see `marshal_to_ui`).
        UI_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
        // Seed the visual-effects preferences before anything can animate, so
        // the very first enter transition already honours them. Seeding
        // transparency also installs its change subscription.
        crate::motion::refresh_reduced_motion();
        crate::motion::refresh_advanced_effects_enabled();
        crate::motion::refresh_auto_hide_scroll_bars();
        // The same, for the caret thickness: a field can be focused on the
        // first frame, and it should be the user's width from the start.
        editor::refresh_caret_width();
        // Held until the host tears down (parked in `HostShared` below); the
        // compositor created a few lines on needs it live on this thread.
        let queue = ensure_dispatcher_queue();

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

        // The two halves of the record seam, now on two threads: the host owns
        // the real backend outright, and the app thread runs the reconciler
        // against a recorder that holds no backend reference at all — its
        // calls become a `Send` command buffer each reconcile ships here via
        // [`post_commit`]. The spawn builds the `RenderHost` on the app thread
        // (it is `!Send`), installs the marshaller from there so async-state
        // writes and re-renders land app-side, and kicks the first render;
        // the resulting commit is replayed by the pump once [`run`] starts.
        let backend = Rc::new(RefCell::new(backend));
        // Text input. The store reflects whichever editor has focus by reading
        // this very backend, so TSF must be handed the same `Rc` the host keeps
        // — and may only ever be entered with no borrow on it (see `tsf::bridge`).
        tsf::bridge::activate(Rc::clone(&backend), hwnd as isize);
        let (app, app_join) = dispatch::spawn_app_thread(
            hwnd as isize,
            root,
            WindowSize {
                width: dip.0 as f64,
                height: dip.1 as f64,
            },
            dpi,
        );

        let pacer = FramePacer::new(hwnd as isize);
        let pump_wake = pacer.wake_handle();
        DCOMP.with(|c| {
            *c.borrow_mut() = Some(Rc::new(HostShared {
                backend,
                app: Arc::clone(&app),
                pacer,
                hwnd: hwnd as isize,
                applied_dark: Cell::new(Some(dark)),
                queue,
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
        set_theme_applier(Some(Arc::new(move |_| {
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
        crate::set_frame_pump_wake(Some(Arc::new(move || pump_wake.wake())));

        // Opt-in composition diagnostics (`REACTOR_CENSUS=<seconds>`,
        // `REACTOR_HEATMAP=<name>`). Starts nothing unless a variable is set, and
        // is deliberately started here — after the window exists — so the first
        // sample and the heat-map request can both reach it.
        census::start_from_env();

        // `SW_SHOWNOACTIVATE` still RAISES the window — it lands on top of its
        // monitor exactly as `SW_SHOW` does — it just does not take activation.
        // For a capture run that is the whole difference: a newly-launched
        // process is granted foreground rights, so the default show pulls focus
        // off whatever the machine was doing and can minimize an exclusive
        // full-screen app on another monitor.
        unsafe {
            let cmd = if show_without_activating() { SW_SHOWNOACTIVATE } else { SW_SHOW };
            let _ = ShowWindow(hwnd, cmd as i32);
        }
        Ok(Self {
            hwnd,
            app,
            app_join: RefCell::new(Some(app_join)),
        })
    }

    /// Run the blocking message loop until the window closes, then stop the
    /// app thread and tear down.
    pub fn run(&self) {
        let mut msg: MSG = unsafe { core::mem::zeroed() };
        unsafe {
            while GetMessageW(&mut msg, core::ptr::null_mut(), 0, 0).as_bool() {
                // Text input is offered every key *before* translation: a TIP
                // that claims a composition key must claim it before it becomes
                // a `WM_CHAR`. An eaten key is neither translated nor dispatched.
                if !tsf::bridge::filter_key(&msg) {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                // …and told what the message changed, once the WndProc's backend
                // borrow is long gone. One site: every path that can move focus
                // or edit text ends here (input, UIA, an app commit replay), so
                // none of them has to remember to announce itself.
                tsf::bridge::flush();
            }
        }
        // Quit and join the app thread before tearing down front state, so no
        // late job races the teardown. Its posts to this (now dead) window
        // fail harmlessly; jobs still queued for it are dropped unrun.
        self.app.post_quit();
        if let Some(join) = self.app_join.borrow_mut().take() {
            let _ = join.join();
        }
        size::set_delivery(None);
        crate::set_frame_pump_wake(None);
        set_theme_applier(None);
        DCOMP.with(|c| *c.borrow_mut() = None);
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Convenience: build the host from a render function (the same `Fn(&mut
    /// RenderCx) -> Element` shape `App::render` takes) and run the message
    /// loop. `Send` because the function runs on the app thread.
    pub fn render<F>(title: impl AsRef<str>, f: F) -> windows_core::Result<()>
    where
        F: Fn(&mut RenderCx) -> Element + Send + 'static,
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
        F: Fn(&mut RenderCx) -> Element + Send + 'static,
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

pub(crate) fn shared() -> Option<Rc<HostShared>> {
    DCOMP.with(|c| c.borrow().clone())
}

/// Static light/dark token table for the window backdrop, resolved to a
/// compositor `Color` (the system compositor encodes it into the FP16 scRGB
/// surface). Node colours are the GUI's responsibility (it re-emits theme-bound
/// `Prop`s); the backend only owns the window backdrop, which flips with the
/// system theme.
pub(crate) fn window_backdrop(dark: bool) -> Color {
    // A compositor `Color` (8-bit sRGB, opaque) for the backdrop brush, not a
    // reactor theming `Color` — plain RGB literals.
    if dark {
        Color::rgb(14, 14, 17)
    } else {
        Color::rgb(243, 243, 245)
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
    // Scheme first: it is process-global, so the app-side re-render posted
    // below is guaranteed to read the new value.
    set_current_color_scheme(scheme_for(dark));
    apply_frame_dark_mode(s.hwnd as HWND, dark);
    {
        let mut b = s.backend.borrow_mut();
        b.apply_theme(dark);
        b.mark_all_dirty_and_repaint();
    }
    // Every component re-renders on the next pass: value-color props are
    // recomputed only inside render functions, so memoised components with
    // unchanged props would otherwise keep the old palette. Component
    // invalidation and the re-render are app-thread work.
    s.app.post(Box::new(|| {
        if let Some(a) = dispatch::app_shared() {
            a.render_host
                .with_reconciler_mut(|r| r.invalidate_all_components());
            a.render_host.request_render();
        }
    }));
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
fn effective_dpi(hwnd: HWND) -> u32 {
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

/// Whether `REACTOR_NO_MOUSELEAVE` asks this process to stop requesting
/// `WM_MOUSELEAVE`. Read once — this is consulted on every pointer move.
///
/// For a capture tool driving the window with POSTED pointer messages: the
/// system decides a leave from the REAL cursor, which in that setup is never
/// over the window, so a synthesized hover is revoked within milliseconds and
/// every hover-gated visual — the hover ink, the press ink — is gone before a
/// screenshot can see it. Routing does not need this; only the chrome does.
///
/// Deliberately narrow: it withholds the leave REQUEST rather than ignoring the
/// message, so a genuine `WM_MOUSELEAVE` (from a real cursor, or one already in
/// flight) still clears hover exactly as it always did. What it cannot do is
/// invent a leave nobody asked for.
/// An explicit initial placement from `REACTOR_WINDOW_RECT`.
#[derive(Copy, Clone)]
struct Placement {
    /// Window top-left in PHYSICAL screen coordinates, which is the currency a
    /// multi-monitor position has to be stated in — a second monitor left of the
    /// primary starts at a negative x.
    pos: Option<(i32, i32)>,
    /// Client size in PHYSICAL pixels — the same currency as `pos`, and as the
    /// capture tools that set this. Deliberately not DIPs: one variable mixing
    /// the two units is a unit bug waiting to happen, and a caller that already
    /// knows the pixel size it wants should not have to undo a scale to say so.
    size: Option<(i32, i32)>,
}

/// Parse `REACTOR_WINDOW_RECT` — `x,y` for a position, or `x,y,w,h` to set the
/// client size with it. All four are PHYSICAL pixels. Read once; the window is
/// placed once.
///
/// This exists because placing the window AFTER it is shown is visible: the
/// default centering puts it on the monitor nearest the system's default
/// position, so a capture run targeting a second screen gets a frame or two of
/// window on the first one, then a jump and a resize. Stating the rect up front
/// means it is born where it belongs.
fn env_placement() -> Option<Placement> {
    static PLACEMENT: std::sync::OnceLock<Option<Placement>> = std::sync::OnceLock::new();
    *PLACEMENT.get_or_init(|| {
        let spec = std::env::var("REACTOR_WINDOW_RECT").ok()?;
        let n: Vec<&str> = spec.split(',').map(str::trim).collect();
        let num = |s: &str| s.parse::<f64>().ok();
        match n.as_slice() {
            [x, y] => Some(Placement {
                pos: Some((num(x)? as i32, num(y)? as i32)),
                size: None,
            }),
            [x, y, w, h] => Some(Placement {
                pos: Some((num(x)? as i32, num(y)? as i32)),
                size: Some((num(w)? as i32, num(h)? as i32)),
            }),
            _ => {
                eprintln!("reactor: REACTOR_WINDOW_RECT={spec:?} — expected x,y or x,y,w,h");
                None
            }
        }
    })
}

/// Whether `REACTOR_SHOW_NOACTIVATE` asks the first show to skip activation.
/// Read once — the window is shown once.
///
/// Visibility and z-order are unaffected: the window still comes up on top of
/// its monitor. Only the focus transfer is withheld, which is what lets a
/// capture run put a window on a second screen without interrupting whatever is
/// in front on the first.
fn show_without_activating() -> bool {
    static NO_ACTIVATE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *NO_ACTIVATE.get_or_init(|| {
        std::env::var("REACTOR_SHOW_NOACTIVATE").is_ok_and(|v| v != "0" && !v.is_empty())
    })
}

fn suppress_mouse_leave() -> bool {
    static SUPPRESS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SUPPRESS.get_or_init(|| {
        std::env::var("REACTOR_NO_MOUSELEAVE").is_ok_and(|v| v != "0" && !v.is_empty())
    })
}

fn track_leave(hwnd: HWND) {
    if suppress_mouse_leave() {
        return;
    }
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
        WM_NCCALCSIZE => {
            // Both forms of the message describe the same frame and must answer
            // the same way. `wparam != 0` carries `NCCALCSIZE_PARAMS`, whose
            // `rgrc[0]` is the proposed client rect; `wparam == 0` carries a bare
            // `RECT` serving the identical purpose. Answering only the first left
            // the window reporting DefWindowProc's frame — caption included — to
            // everything that asked the second way, and the very first
            // calculation a window receives, during `CreateWindowExW`, is that
            // one. Measuring the frame there then over-counted by the caption,
            // and every client size the app asked for came out that much too
            // tall.
            let rect = if wparam != 0 {
                let params = lparam as *mut NCCALCSIZE_PARAMS;
                if params.is_null() {
                    return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
                }
                unsafe { &raw mut (*params).rgrc[0] }
            } else {
                let r = lparam as *mut RECT;
                if r.is_null() {
                    return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
                }
                r
            };
            let top = unsafe { (*rect).top };
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
                (*rect).top = top + pad;
            }
            r
        }

        // ── Size constraints: the app's registered minimum / maximum ──
        //
        // The system pre-fills the payload with its own defaults (including the
        // multi-monitor maximized size and position), so the handler overwrites
        // only the axes the app actually constrained and leaves the rest alone.
        // Arrives during CreateWindowExW too, before anything is registered —
        // `apply` reports that and the message falls through untouched.
        WM_GETMINMAXINFO => {
            let mmi = lparam as *mut MINMAXINFO;
            if mmi.is_null() {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
            if constraints::apply(hwnd, dpi_scale(hwnd), unsafe { &mut *mmi }) {
                0
            } else {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
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
            // Each backend read is hoisted into its own `let` so the RefCell
            // borrow ends at that statement: a borrow left live in an
            // `if`/let-chain condition lasts through the whole body, and the
            // next read panics the front thread ("already borrowed").
            let caption = s.backend.borrow_mut().caption_rect();
            if let Some((cx, cy, cw, ch)) = caption
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
                let back_active = s.backend.borrow_mut().back_button_active();
                let back_rect = s.backend.borrow_mut().back_button_rect();
                if back_active
                    && let Some((bx, by, bw, bh)) = back_rect
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


        // The pacer worker's vsync tick: drive the canvas/viz frame-tick
        // subscribers, then park the pacer once none remain (true idle).
        WM_APP_FRAME => {
            // The subscribers are app closures in the app thread's registry,
            // so the tick is carried there rather than driven here. Coalesced:
            // while the last tick job hasn't run — the app thread is
            // mid-reconcile — further frames fold into it instead of queueing
            // a burst it would drain back-to-back.
            static TICK_PENDING: AtomicBool = AtomicBool::new(false);
            if let Some(s) = shared() {
                s.pacer.begin_tick();
                if !TICK_PENDING.swap(true, Ordering::AcqRel) {
                    s.app.post(Box::new(|| {
                        TICK_PENDING.store(false, Ordering::Release);
                        crate::drive_frame_ticks();
                    }));
                }
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
            // The borrow ends inside the closure — it must not be live while
            // `UiaReturnRawElementProvider` runs, which can call straight
            // back into providers that reach for the backend.
            let root = if lparam as i32 == UIA_ROOT_OBJECT_ID {
                shared().and_then(|s| s.backend.borrow_mut().uia_root())
            } else {
                None
            };
            if let Some(root) = root {
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
                    POINTER_CAPTURE.store(true, Ordering::Relaxed);
                }
            }
            0
        }

        WM_LBUTTONUP => {
            if let Some(s) = shared() {
                let (x, y) = dip_xy(hwnd, lparam);
                release_capture_self();
                POINTER_CAPTURE.store(false, Ordering::Relaxed);
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
            // Whoever took it, we no longer hold capture — clear the gate on both
            // the stolen and the self-release path, so settled-state UI can never
            // be left frozen by a gesture that ended without a button-up.
            POINTER_CAPTURE.store(false, Ordering::Relaxed);
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
                        b.on_wheel_h(x, y, delta);
                    } else {
                        b.on_wheel(x, y, delta);
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

        // No `WM_IME_*` arms: composition is owned end-to-end by the TSF text
        // store (`tsf::bridge`), which sees it through the store's own document
        // protocol and the composition sink. There is deliberately no IMM32
        // path — see `tsf::mod`.
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

        // The display's colour capability (HDR toggle, SDR white level, panel /
        // monitor change) is no longer chased through WM_DISPLAYCHANGE or a
        // WM_WINDOWPOSCHANGED monitor-diff: `display_change::attach` subscribes the
        // window's DisplayInformation to AdvancedColorInfoChanged, one WinRT signal
        // that covers all of them (and tracks the window's current monitor), and
        // re-fits + repaints from there. So both of those arms are gone; WM_SIZE /
        // WM_MOVE are still synthesised by DefWindowProc via the default arm.
        WM_SETTINGCHANGE => {
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
            // (a no-op when no text field is focused). The thickness slider is
            // re-read on the same message; both land through one repaint, and
            // the width is refreshed FIRST so that repaint already carries it.
            editor::refresh_caret_width();
            if let Some(s) = shared() {
                s.backend.borrow_mut().refresh_caret_blink();
            }
            // So does an Accessibility animation-effects flip. Only a real
            // change rebuilds implicit collections — this message arrives for
            // every unrelated setting in the system.
            if crate::motion::refresh_reduced_motion()
                && let Some(s) = shared()
            {
                s.backend.borrow_mut().refresh_motion();
            }
            // The scroll-bar preference has no change signal of its own, so
            // this message is the only thing that refreshes it; nothing rebuilds
            // on it yet, so the change-only answer is discarded. The
            // transparency preference is NOT refreshed here — it is the one that
            // does have a WinRT event, which maintains its cache directly.
            crate::motion::refresh_auto_hide_scroll_bars();
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
                    // Relayout the current tree at the new size immediately —
                    // the solve is front-side, so the window never shows a
                    // stale-sized frame during an interactive resize. The app
                    // thread is then told, so components reading
                    // `use_inner_size` / `use_dpi` re-render; their commit
                    // lands a hop later.
                    s.backend.borrow_mut().resize(pw, ph, dpi);
                    let (w, h) = (pw as f64 / scale as f64, ph as f64 / scale as f64);
                    s.app.post(Box::new(move || {
                        if let Some(a) = dispatch::app_shared() {
                            a.render_host.set_dpi(dpi);
                            a.render_host
                                .set_inner_size(WindowSize { width: w, height: h });
                        }
                    }));
                }
            }
            0
        }

        WM_DESTROY => {
            // Drop the DisplayInformation + its AdvancedColorInfoChanged
            // subscription while the HWND it hooked is still valid.
            display_change::detach();
            // Before the window goes: pop the TSF context and deactivate the
            // thread manager while the HWND the store reports is still valid.
            tsf::bridge::shutdown();
            unsafe {
                PostQuitMessage(0);
            }
            0
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Stand up the `DispatcherQueue` the system `Compositor` needs on the calling
/// thread, returning the controller to hold for the compositor's lifetime.
///
/// The wrapper's `create_on_current_thread` makes exactly the call this used to
/// make by hand — `DQTYPE_THREAD_CURRENT` with `DQTAT_COM_ASTA`, which also
/// serves TSF: activation, document-manager and context creation were all
/// measured working in that apartment.
///
/// A controller already existing on this thread fails, and that failure is
/// ignored exactly as before — the queue we need is then already present, so
/// `None` here is a success, not a fault.
fn ensure_dispatcher_queue() -> Option<DispatcherQueueController> {
    DispatcherQueueController::create_on_current_thread().ok()
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

    // Create at the target POSITION but a provisional size; the exact client
    // size is applied below, once the window's own DPI and non-client metrics
    // can be measured.
    //
    // The position has to be right here rather than later, and not for
    // appearance — nothing is visible until `ShowWindow`, far below. It decides
    // which MONITOR the window is born on, and therefore which DPI
    // `GetDpiForWindow` reports and which non-client metrics get measured. Born
    // on the primary and moved afterwards, a window bound for a differently
    // scaled second monitor computes its whole geometry at the wrong scale and
    // then takes a `WM_DPICHANGED` correction.
    //
    // The size stays provisional because the frame this window actually gets is
    // NOT the one `AdjustWindowRectExForDpi` would predict: `WM_NCCALCSIZE`
    // above restores the top edge so the client extends into the caption. That
    // contract belongs to the message handler, so the geometry is MEASURED from
    // the real window rather than derived from a second copy of it here.
    let placement = env_placement();
    let (create_x, create_y) = match placement.and_then(|p| p.pos) {
        Some((x, y)) => (x, y),
        None => (CW_USEDEFAULT, CW_USEDEFAULT),
    };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            PCWSTR(CLASS.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            create_x,
            create_y,
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
    // Publish the window before anything can mount, so a producer thread holding
    // a `LiveText` always has somewhere to post its wake. Until this runs a
    // publish still queues — it simply has no one to tell, and the next publish
    // carries the batch.
    live_text::set_front_hwnd(hwnd as isize);

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
        // An explicit placement states PHYSICAL pixels and so skips the scale;
        // the app's own `client_dip` is in DIPs and does not.
        let (cw, ch) = if let Some((w, h)) = placement.and_then(|p| p.size) { (w, h) }
        else if let Some((w, h)) = client_dip { ((w * scale).round() as i32, (h * scale).round() as i32) } else {
            let min_w = (1200.0 * scale) as i32;
            let min_h = (800.0 * scale) as i32;
            let avail_w = (work_w - nc_w).max(1);
            let avail_h = (work_h - nc_h).max(1);
            (
                (work_w * 4 / 5).max(min_w).min(avail_w),
                (work_h * 4 / 5).max(min_h).min(avail_h),
            )
        };
        let win_w = cw + nc_w;
        let win_h = ch + nc_h;
        // Center on the work area.
        let (mut x, mut y) = (CW_USEDEFAULT, CW_USEDEFAULT);
        if have_mi {
            x = mi.rcWork.left + (work_w - win_w).max(0) / 2;
            y = mi.rcWork.top + (work_h - win_h).max(0) / 2;
        }
        // An explicit placement already went to `CreateWindowExW`, so this
        // re-states the same origin and the call resizes in place — the window
        // never moves. The centering path is the one that genuinely needs it:
        // its origin depends on `win_w`/`win_h`, which are not known until the
        // frame has been measured.
        if let Some(p) = placement
            && let Some((px, py)) = p.pos
        {
            x = px;
            y = py;
        }
        // One call: `nc_w`/`nc_h` were measured from a window whose frame already
        // answers as it will for the rest of its life, because `WM_NCCALCSIZE`
        // handles both forms of the message (see the handler). A correction pass
        // used to live here to undo the caption the first calculation had
        // over-counted; making the frame self-consistent removed the need for it.
        let _ = SetWindowPos(hwnd, core::ptr::null_mut(), x, y, win_w, win_h, SWP_NOZORDER);
    }

    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let pw = (rc.right - rc.left).max(1);
    let ph = (rc.bottom - rc.top).max(1);

    // Subscribe the display-capability signal now the window exists and the UI
    // thread already has its DispatcherQueue (both `GetForWindow` requirements).
    // This also fires the initial fit synchronously — before the first paint —
    // so frame one is already mapped to the display, and there is no pre-window
    // fit to get wrong.
    display_change::attach(hwnd);

    Ok((hwnd, dpi, (pw, ph)))
}
