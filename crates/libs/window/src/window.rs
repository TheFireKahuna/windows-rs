//! A top-level window, its window procedure, and the process-wide message loop.
//!
//! The procedure answers what belongs to a window's lifetime — visibility, DPI, the minimum
//! track size, the frame gate and the thread's scheduling class — around the application's
//! own message handler.

use crate::bindings::*;
use crate::caption::{BorderColor, Caption, CaptionHit, CaptionSpec, CaptionState};
use crate::display::{DisplayColor, Subscription};
use crate::dpi::Metrics;
use crate::feedback::FeedbackPolicy;
use crate::pace::Clock;
use crate::qos::{self, Speed};
use crate::visibility::{OcclusionStatus, Visibility, Watch};
use core::cell::{Cell, OnceCell, RefCell};
use core::mem::ManuallyDrop;
use core::sync::atomic::{AtomicU64, Ordering};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use windows_color::DisplayCapability;
use windows_core::*;

/// Receives the raw window handle, message code, and `wparam`/`lparam`. Return
/// `Some(result)` to handle the message, or `None` to fall through to default processing.
type MessageHandler = Box<dyn FnMut(*mut core::ffi::c_void, u32, usize, isize) -> Option<isize>>;

/// Receives the new client-area width and height in pixels.
type ResizeHandler = Box<dyn FnMut(i32, i32)>;

/// Receives the window's metrics after its DPI changed.
type ScaleHandler = Box<dyn FnMut(Metrics)>;

/// Holds everything the window procedure owns. Every field it mutates is behind interior
/// mutability, so a shared reference to this never aliases a `&mut`.
struct State {
    message: RefCell<Option<MessageHandler>>,
    resize: RefCell<Option<ResizeHandler>>,
    scale: RefCell<Option<ScaleHandler>>,
    /// Shared with the threads that draw for this window. The window procedure updates it
    /// directly, so delivery does not depend on the application forwarding a message.
    visibility: Arc<Visibility>,
    /// Held for its `Drop`, which unregisters. `None` when the registration failed.
    _occlusion: Option<OcclusionStatus>,
    /// Cleared on `WM_DESTROY`, while the handle is still valid.
    display: RefCell<Option<Rc<DisplayColor>>>,
    /// `Rc` so the caption outlives a hit test whose authority destroys the window while the
    /// caption is mid-call.
    caption: Option<Rc<Caption>>,
    /// Whether the precision-touchpad registration succeeded. `false` where `user32` has no
    /// `RegisterTouchpadCapableWindow`.
    touchpad: bool,
    /// The last scheduling class asked for, so a drag does not re-tag the thread on every
    /// `WM_WINDOWPOSCHANGED`.
    speed: Cell<Speed>,
    /// Resolved against the window's current DPI on each query rather than stored in pixels,
    /// so the floor is the same apparent size on every monitor.
    min_size_dips: Option<(f32, f32)>,
    quit_on_close: bool,
    /// The live pacer's clock, if there is one.
    frame_gate: RefCell<Option<Arc<Clock>>>,
}

/// Carries the system's occlusion-status notification. Numbered in the `WM_USER` range
/// because this crate registers the window class; `WM_APP` and above are the application's to
/// number.
const WM_USER_OCCLUSION: u32 = WM_USER as u32 + 0x45;

/// Carries a display-capability change out of its WinRT handler. Posting rather than calling
/// out from the handler runs the application's callback from the ordinary pump, in message
/// order, and not inside a projection frame it may not re-enter.
const WM_USER_COLOR: u32 = WM_USER as u32 + 0x46;

/// Supplies the identity stamped into a window's own extra bytes at creation. Monotonic for
/// the process, so no value is issued twice.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Locates that identity within the class's extra window bytes. The storage is kernel-side
/// and per-window: zeroed for a new window and freed with it, so no second window is handed
/// the same slot while a stale [`Window`] still holds its value.
const GWLP_WINDOW_ID: i32 = 0;

/// Names the system move/size operation a contact runs.
///
/// The eight sizing arms name the edge or corner that follows the contact, matching the codes
/// `WM_NCHITTEST` answers with; [`Self::Move`] moves the whole window instead.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MoveSize {
    Left,
    Right,
    Top,
    TopLeft,
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
    Move,
}

impl MoveSize {
    /// Returns the system's `MOVESIZE_OPERATION` value for this operation.
    ///
    /// The values are written out rather than derived from the discriminant: they belong to
    /// an ABI, so reordering the variants must not change them.
    const fn operation(self) -> i32 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Top => 3,
            Self::TopLeft => 4,
            Self::TopRight => 5,
            Self::Bottom => 6,
            Self::BottomLeft => 7,
            Self::BottomRight => 8,
            Self::Move => 9,
        }
    }
}

/// A top-level window.
///
/// Closing destroys the `HWND` while this value still holds its number, and handle values
/// are recycled, so every accessor checks that the handle still carries this window's
/// identity and answers `None` when it does not.
pub struct Window {
    hwnd: HWND,
    /// Never reissued, so a window carrying this value is this window. `IsWindow` answers for
    /// whatever now owns a recycled handle, and the state box's address is no better: the
    /// allocator hands a freed box to the next window.
    id: u64,
}

impl Window {
    /// Begins configuring a new window with the given title.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(title: &str) -> WindowBuilder {
        WindowBuilder {
            title: title.to_string(),
            size: Size::System,
            resizable: true,
            ex_style: 0,
            message: None,
            resize: None,
            scale: None,
            caption: None,
            feedback: FeedbackPolicy::SYSTEM,
            pointer_input: false,
            touchpad: false,
            hidden: false,
            quit_on_close: true,
            min_size_dips: None,
        }
    }

    /// Returns the raw window handle, for interop with other Windows APIs.
    ///
    /// The value is stale once the window is closed, as every handle value is. Ask
    /// [`is_open`](Self::is_open) first.
    #[must_use]
    pub fn hwnd(&self) -> *mut core::ffi::c_void {
        self.hwnd
    }

    /// Returns whether this handle still names this window rather than a recycled one.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state().is_some()
    }

    /// Shows a window built with [`WindowBuilder::hidden`], after its first content exists.
    ///
    /// `None` once the window is closed.
    pub fn show(&self) -> Option<()> {
        self.state()?;
        // SAFETY: the identity check above establishes that the handle is this window.
        unsafe {
            _ = ShowWindow(self.hwnd, SW_SHOWNORMAL);
        }
        Some(())
    }

    /// Returns whether anything this window draws can be seen.
    ///
    /// The value is `Send + Sync`, so a producer on another thread reads it directly. The
    /// window procedure maintains it, so an application that handles its own messages cannot
    /// starve it. Take [`watch`](Self::watch) to park on a change instead of reading.
    ///
    /// `None` once the window is closed.
    #[must_use]
    pub fn visibility(&self) -> Option<Arc<Visibility>> {
        Some(self.state()?.visibility.clone())
    }

    /// Returns a wake of the caller's own, added to [`visibility`](Self::visibility)'s list.
    ///
    /// One per consumer: a wake releases exactly one waiter, so a frame pacer and a present
    /// thread parked on the same window each take their own and a change reaches both.
    ///
    /// # Errors
    ///
    /// The window is closed, or the event could not be created.
    pub fn watch(&self) -> Result<Watch> {
        let Some(state) = self.state() else {
            return Err(Error::new(E_HANDLE, "the window is closed"));
        };
        state.visibility.watch()
    }

    /// Returns the client-area size in pixels as `(width, height)`. `None` once the window is
    /// closed.
    #[must_use]
    pub fn client_size(&self) -> Option<(i32, i32)> {
        self.state()?;
        let mut rect = RECT::default();
        // SAFETY: the identity check above establishes that the handle is this window.
        unsafe { GetClientRect(self.hwnd, &mut rect) }
            .as_bool()
            .then(|| (rect.right - rect.left, rect.bottom - rect.top))
    }

    /// Returns the window's DPI and the frame metrics derived from it. `None` once the window
    /// is closed.
    #[must_use]
    pub fn metrics(&self) -> Option<Metrics> {
        self.state()?;
        Some(Metrics::for_window(self.hwnd))
    }

    /// Returns `dpi / 96`, the DIP-to-physical-pixel factor. `None` once the window is closed.
    #[must_use]
    pub fn scale(&self) -> Option<f32> {
        Some(self.metrics()?.scale)
    }

    /// Returns what this window's current display can present.
    ///
    /// The answer is cached and re-read only when the system reports the display changed; one
    /// notification covers an HDR toggle, an SDR-white-level change and a monitor hop alike.
    ///
    /// `None` once the window is closed, rather than `Sdr`, which a caller would act on.
    #[must_use]
    pub fn color_capability(&self) -> Option<DisplayCapability> {
        Some(self.state()?.display.borrow().as_ref()?.capability())
    }

    /// Installs a callback run on this window's own thread whenever the display colour
    /// capability changes.
    ///
    /// There is one callback, and installing a second replaces the first: the system
    /// registration behind it permits exactly one handler. Dropping the returned
    /// [`Subscription`] clears the callback; the registration lives until the window is
    /// destroyed.
    ///
    /// `None` once the window is closed. There is no other failure: a window whose display
    /// cannot be interrogated is never created.
    #[must_use = "dropping the Subscription clears the callback again"]
    pub fn on_color_capability_changed(
        &self,
        f: impl Fn(DisplayCapability) + 'static,
    ) -> Option<Subscription> {
        let display = Rc::clone(self.state()?.display.borrow().as_ref()?);
        Some(display.subscribe(Rc::new(f)))
    }

    /// Installs the caption's hit authority.
    ///
    /// The authority is called with a client-space point in DIPs whenever the system asks
    /// what is under the pointer in the caption band. Until one is installed the band drags
    /// the window.
    ///
    /// `None` once the window is closed, and on a window that did not ask for a caption of
    /// its own, where nothing would ever consult the authority.
    #[must_use = "None means the authority was not installed and the band will only drag"]
    pub fn on_caption_hit(&self, f: impl FnMut(f32, f32) -> CaptionHit + 'static) -> Option<()> {
        self.caption()?.set_hit_authority(Box::new(f));
        Some(())
    }

    /// Installs the sink called when the hover or pressed state of a caption button changes.
    ///
    /// Those changes arrive as non-client pointer messages the application never sees: once
    /// the hit test claims a button, the system routes that contact to the frame.
    ///
    /// `None` as for [`on_caption_hit`](Self::on_caption_hit).
    #[must_use = "None means the sink was not installed and no button state will arrive"]
    pub fn on_caption_state(&self, f: impl FnMut(CaptionState) + 'static) -> Option<()> {
        self.caption()?.set_state_sink(Box::new(f));
        Some(())
    }

    /// Returns the caption band height in DIPs, for a window that has one.
    ///
    /// A [`CaptionSpec`] that named no height resolves to the system's caption height at this
    /// window's current DPI.
    #[must_use]
    pub fn caption_height_dips(&self) -> Option<f32> {
        let caption = self.caption()?;
        let metrics = self.metrics()?;
        Some(
            caption
                .spec()
                .height
                .unwrap_or_else(|| metrics.dips(metrics.caption)),
        )
    }

    /// Sets the one-pixel frame colour DWM draws around the window.
    ///
    /// The colour is re-applied on activation and on a theme change, neither of which
    /// preserves it. `None` on a window that does not draw its own caption.
    #[must_use = "None means the colour was not applied and the frame keeps the system's"]
    pub fn set_border_color(&self, color: BorderColor) -> Option<()> {
        self.caption()?.set_border(self.hwnd, color);
        Some(())
    }

    /// Enters the system's move/size loop for a press the client area received.
    ///
    /// A caption band needs none of this: it answers `WM_NCHITTEST` with `HTCAPTION` or one
    /// of the eight resize codes and the system enters the loop itself. A client-area press
    /// carries no hit code, which is the only argument `DefWindowProc` takes, so this is the
    /// only route to the loop for content that has decided the gesture moves or sizes the
    /// window rather than acting on it.
    ///
    /// `at` is the down event's own position in **screen** coordinates. The loop drags from
    /// that point, so passing the pointer's current position instead of the one the contact
    /// started at offsets the window by the difference between them.
    ///
    /// **Blocks until the contact is released**, pumping the drag's messages on this thread:
    /// this call is the sizing modal loop. The system owns everything inside it — snapping,
    /// Aero shake, the live frame, and the escape that cancels the drag.
    ///
    /// Keyboard move/size is not reachable here: `Alt`+`Space` arrives as `WM_SYSCOMMAND` and
    /// is `DefWindowProc`'s.
    ///
    /// `false` if the window is closed, or if `user32` has no `EnterMoveSizeLoop` — a Windows
    /// 11 addition that ships with no header and no import library, so it is resolved by name
    /// and there is nothing to link against even where it exists.
    #[must_use = "false means no loop ran and the window did not move"]
    pub fn begin_move_size(&self, at: (i32, i32), operation: MoveSize) -> bool {
        self.state().is_some() && enter_move_size_loop(self.hwnd, at, operation)
    }

    /// Returns this window's caption, for an open window that asked for one.
    fn caption(&self) -> Option<&Caption> {
        self.state()?.caption.as_deref()
    }

    /// Returns whether this window is registered for real precision-touchpad contacts.
    ///
    /// `false` when it was not asked for, and when `user32` has no
    /// `RegisterTouchpadCapableWindow`. An unregistered window receives synthesized wheel
    /// messages where a registered one receives two-finger pointer contacts.
    #[must_use]
    pub fn is_touchpad_capable(&self) -> bool {
        self.state().is_some_and(|state| state.touchpad)
    }

    /// Installs the pacer's clock. `false` if the window is closed or already has one.
    pub(crate) fn claim_frame_gate(&self, gate: Arc<Clock>) -> bool {
        let Some(state) = self.state() else {
            return false;
        };
        let mut slot = state.frame_gate.borrow_mut();
        if slot.is_some() {
            return false;
        }
        *slot = Some(gate);
        true
    }

    /// Clears the pacer's clock, leaving the slot free for another pacer.
    pub(crate) fn release_frame_gate(&self) {
        if let Some(state) = self.state() {
            *state.frame_gate.borrow_mut() = None;
        }
    }

    /// Returns this window's state, or `None` once the window has been closed.
    ///
    /// The check is identity, not liveness: `IsWindow` on a recycled handle value answers for
    /// whatever now owns it. The identity is read before the state box is touched, so a stale
    /// [`Window`] never dereferences a box it has no claim to.
    fn state(&self) -> Option<&State> {
        // SAFETY: reading a window word of a window that is gone, or of one belonging to a
        // class with no extra bytes, answers zero — and no identity is ever zero. A match
        // therefore names this window, whose box is freed on `WM_NCDESTROY` only after the
        // identity is cleared, on this same thread.
        unsafe {
            if GetWindowLongPtrW(self.hwnd, GWLP_WINDOW_ID) as u64 != self.id {
                return None;
            }
            let state = GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *const State;
            (!state.is_null()).then(|| &*state)
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // Destroys only a handle that still carries this window's state: a closed window
        // leaves a stale value here, and destroying whatever later answers to it would close
        // a window this one never owned.
        if self.state().is_some() {
            // SAFETY: the identity check above establishes that the handle is this window.
            unsafe {
                _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

impl core::fmt::Debug for Window {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Window")
            .field("hwnd", &self.hwnd)
            .field("open", &self.is_open())
            .finish()
    }
}

/// Names the initial size in the space the caller stated it in. One value rather than a pair
/// of numbers and a flag saying how to read them, so the two spaces cannot be mixed.
#[derive(Copy, Clone, Debug)]
enum Size {
    /// Whatever the system picks.
    System,
    Pixels(i32, i32),
    Dips(f32, f32),
}

/// Builder for a [`Window`].
pub struct WindowBuilder {
    title: String,
    size: Size,
    resizable: bool,
    ex_style: u32,
    message: Option<MessageHandler>,
    resize: Option<ResizeHandler>,
    scale: Option<ScaleHandler>,
    caption: Option<CaptionSpec>,
    feedback: FeedbackPolicy,
    pointer_input: bool,
    touchpad: bool,
    hidden: bool,
    quit_on_close: bool,
    min_size_dips: Option<(f32, f32)>,
}

impl WindowBuilder {
    /// Sets the initial window size in physical pixels.
    #[must_use]
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.size = Size::Pixels(width, height);
        self
    }

    /// Sets the initial window size in DIPs, resolved against the DPI the window lands on.
    ///
    /// `CreateWindowExW` takes physical pixels, and a per-monitor-v2-aware process does not
    /// know its window's DPI until the window exists, so the size is applied after creation
    /// and before the window is shown.
    #[must_use]
    pub fn size_dips(mut self, width: f32, height: f32) -> Self {
        self.size = Size::Dips(width, height);
        self
    }

    /// Whether the user can resize and maximize the window. Resizable by default.
    #[must_use]
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Creates the window with no redirection surface.
    ///
    /// Required for a composition-hosted window: its content is whatever the compositor is
    /// given, and nothing is drawn into a bitmap the window owns. The window keeps DWM's
    /// frame, shadow and rounded corners.
    #[must_use]
    pub fn no_redirection_bitmap(mut self) -> Self {
        self.ex_style |= WS_EX_NOREDIRECTIONBITMAP as u32;
        self
    }

    /// Delivers mouse input as `WM_POINTER*` rather than as legacy mouse messages.
    ///
    /// The opt-in is **process-wide and one-way**, and the system rejects it once the process
    /// owns a window, so it is made as part of creating the first one rather than by a free
    /// function anyone can call late.
    ///
    /// It does not suppress the legacy stream: `DefWindowProc` synthesizes that from whatever
    /// pointer message is left unhandled. Implied by
    /// [`custom_caption`](Self::custom_caption).
    #[must_use]
    pub fn pointer_input(mut self) -> Self {
        self.pointer_input = true;
        self
    }

    /// Registers for real precision-touchpad contacts instead of synthesized wheel input, so
    /// two-finger pans and zooms arrive as `PT_TOUCHPAD` pointer input.
    /// [`Window::is_touchpad_capable`] reports whether the registration took.
    #[must_use]
    pub fn touchpad_capable(mut self) -> Self {
        self.touchpad = true;
        self
    }

    /// Chooses which system-drawn touch and pen visuals this window keeps.
    #[must_use]
    pub fn feedback(mut self, policy: FeedbackPolicy) -> Self {
        self.feedback = policy;
        self
    }

    /// Removes the system caption and lets the application draw the title bar.
    ///
    /// The client area then covers the whole window, and the caption behaviours are kept,
    /// because the contacts that drive them are forwarded rather than consumed. Install
    /// [`Window::on_caption_hit`] once the application's hit structure exists.
    ///
    /// **Implies [`pointer_input`](Self::pointer_input).** The caption reads `WM_NCPOINTER*`,
    /// which a mouse produces only once the process has opted in.
    #[must_use]
    pub fn custom_caption(mut self, spec: CaptionSpec) -> Self {
        self.caption = Some(spec);
        self
    }

    /// Sets the smallest size the user may drag the window to, in DIPs.
    ///
    /// Answered from `WM_GETMINMAXINFO` at the window's current DPI, so the constraint is the
    /// same apparent size on every monitor. Without one the system's own minimum applies,
    /// which is a caption's width and nothing more.
    #[must_use]
    pub fn min_size_dips(mut self, width: f32, height: f32) -> Self {
        self.min_size_dips = Some((width, height));
        self
    }

    /// Creates the window without showing it. [`Window::show`] puts it on screen.
    ///
    /// A composition-hosted window has no redirection surface and so nothing to draw until
    /// the compositor's first commit; showing it before then shows an empty frame.
    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Sets whether destroying this window posts a quit message, ending [`run`]. On by
    /// default, which is what a single-window application wants.
    #[must_use]
    pub fn quit_on_close(mut self, quit: bool) -> Self {
        self.quit_on_close = quit;
        self
    }

    /// Sets a handler called for every window message. Return `Some(result)` to handle the
    /// message, or `None` to fall through to default processing.
    ///
    /// The window procedure runs three tiers in order:
    ///
    /// * **Before the handler, and not suppressable** — whether anything drawn can be seen,
    ///   the thread's scheduling class, the minimum track size and the frame gate. A handler
    ///   returning `Some` cannot stop a producer on another thread being told the window went
    ///   away.
    /// * **The handler.**
    /// * **After the handler, and only where it returned `None`** — resize, DPI change, and
    ///   the custom caption. Answering `WM_NCCALCSIZE` or `WM_NCHITTEST` here replaces the
    ///   caption rather than adding to it.
    ///
    /// This crate registers the window class, so `WM_USER..WM_APP` is its range to number and
    /// three of those values are in use, [`WM_FRAME`](crate::WM_FRAME) among them. An
    /// application numbering messages of its own starts at `WM_APP` (`0x8000`).
    #[must_use]
    pub fn on_message<F>(mut self, handler: F) -> Self
    where
        F: FnMut(*mut core::ffi::c_void, u32, usize, isize) -> Option<isize> + 'static,
    {
        self.message = Some(Box::new(handler));
        self
    }

    /// Adds `handler` behind whatever [`on_message`](Self::on_message) already installed,
    /// rather than in place of it.
    ///
    /// The installed handler answers first and `handler` runs only where it returned `None`,
    /// which is the layering `on_message` already documents between itself and this crate's
    /// own processing. What it is for is a layer that must see every message — a frame tick,
    /// a doorbell — attaching to a builder it did not configure, without discarding the
    /// caller's handler and without the caller knowing to check.
    #[must_use]
    pub fn chain_message<F>(mut self, mut handler: F) -> Self
    where
        F: FnMut(*mut core::ffi::c_void, u32, usize, isize) -> Option<isize> + 'static,
    {
        let Some(mut installed) = self.message.take() else {
            return self.on_message(handler);
        };
        self.message = Some(Box::new(move |hwnd, message, wparam, lparam| {
            installed(hwnd, message, wparam, lparam)
                .or_else(|| handler(hwnd, message, wparam, lparam))
        }));
        self
    }

    /// Sets a handler called when the client area is resized, with the new width and height
    /// in pixels. It replaces default processing of `WM_SIZE`.
    #[must_use]
    pub fn on_resize<F>(mut self, handler: F) -> Self
    where
        F: FnMut(i32, i32) + 'static,
    {
        self.resize = Some(Box::new(handler));
        self
    }

    /// Sets a handler called after the window's DPI changed, with its new metrics.
    ///
    /// The window has already been moved and resized to the rect the system suggested, so the
    /// handler is responsible for the content's scale and nothing else.
    #[must_use]
    pub fn on_scale_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(Metrics) + 'static,
    {
        self.scale = Some(Box::new(handler));
        self
    }

    /// Creates the window.
    ///
    /// Creating the first window in the process makes it per-monitor-DPI-aware, which is
    /// process-wide, one-way, and possible only before any window exists.
    ///
    /// # Errors
    ///
    /// The pointer opt-in was refused, the window class could not be registered, the thread
    /// could not be given a dispatcher queue, the window could not be created, or its display
    /// could not be interrogated.
    pub fn create(self) -> Result<Window> {
        // Before the class, and therefore before any window: process-wide, one-way, and
        // rejected once this process owns a window.
        if self.pointer_input || self.caption.is_some() {
            enable_pointer_input()?;
        }
        register_class()?;
        // Before the window: `DisplayInformation::GetForWindow` requires a dispatcher queue
        // on the calling thread, and so does a system compositor.
        ensure_dispatcher_queue()?;

        let mut style = WS_OVERLAPPEDWINDOW as u32;
        if !self.resizable {
            style &= !((WS_THICKFRAME | WS_MAXIMIZEBOX) as u32);
        }

        let mut title: Vec<u16> = self.title.encode_utf16().collect();
        title.push(0);

        // A DIP size cannot be resolved yet: this process is per-monitor-v2-aware, so the
        // window's DPI is whichever monitor it lands on. Applied after creation.
        let (width, height) = match self.size {
            Size::Pixels(width, height) => (width, height),
            Size::System | Size::Dips(..) => (CW_USEDEFAULT, CW_USEDEFAULT),
        };

        // SAFETY: the class is registered, the title is a live null-terminated buffer, and
        // every handle argument is null because this is a top-level window with no menu.
        let hwnd = unsafe {
            CreateWindowExW(
                self.ex_style,
                class_name(),
                PCWSTR(title.as_ptr()),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width,
                height,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err(Error::from_thread());
        }

        // Every fallible step runs before anything is installed: a failure past this point
        // would leave a live window that no `Window` owns and nothing destroys.
        //
        // A display that cannot be interrogated fails creation rather than degrading, because
        // a window that cannot name its display renders every colour for the wrong one.
        let display = match DisplayColor::new(hwnd, WM_USER_COLOR) {
            Ok(display) => Rc::new(display),
            Err(error) => {
                // SAFETY: `hwnd` is live and owned here.
                unsafe {
                    _ = DestroyWindow(hwnd);
                }
                return Err(error);
            }
        };

        let state = Box::new(State {
            message: RefCell::new(self.message),
            resize: RefCell::new(self.resize),
            scale: RefCell::new(self.scale),
            visibility: Arc::new(Visibility::new()),
            // Registered before the window is shown, so no status change after it appears is
            // missed.
            _occlusion: OcclusionStatus::register(hwnd, WM_USER_OCCLUSION),
            display: RefCell::new(Some(display)),
            caption: self.caption.map(|spec| Rc::new(Caption::new(spec))),
            touchpad: self.touchpad && register_touchpad_capable(hwnd),
            speed: Cell::new(Speed::Managed),
            min_size_dips: self.min_size_dips,
            quit_on_close: self.quit_on_close,
            frame_gate: RefCell::new(None),
        });
        let (caption, visibility) = (state.caption.clone(), Arc::clone(&state.visibility));
        let installed = Box::into_raw(state);
        // relaxed: the counter synchronises nothing, and uniqueness across threads is the
        // whole requirement, which the read-modify-write itself carries.
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        // SAFETY: `hwnd` is live, its class reserves the identity word, and the window
        // procedure takes ownership of the box from here until it frees it on `WM_NCDESTROY`.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, installed as _);
            SetWindowLongPtrW(hwnd, GWLP_WINDOW_ID, id as isize);
        }

        self.feedback.apply(hwnd);

        if let Size::Dips(width, height) = self.size {
            resize_to_dips(hwnd, width, height);
        }

        // After the state is installed, because this is what re-sends `WM_NCCALCSIZE` with
        // the handler in place.
        if let Some(caption) = caption {
            caption.apply(hwnd);
        }

        if !self.hidden {
            // SAFETY: `hwnd` is live.
            unsafe {
                _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            }
        }
        // Evaluated here so a window handed straight to a producer carries a real answer
        // rather than waiting for the first message to arrive.
        visibility.evaluate(hwnd);
        Ok(Window { hwnd, id })
    }
}

/// Runs a blocking, event-driven message loop until a quit message is posted.
pub fn run() {
    let mut message = MSG::default();
    loop {
        // SAFETY: no window filter, and the destination is a stack local.
        let more = unsafe { GetMessageW(&mut message, core::ptr::null_mut(), 0, 0) };
        // Zero is `WM_QUIT`; `-1` is a failed call, which leaves `message` unfilled.
        if more.0 <= 0 {
            return;
        }
        // SAFETY: `message` was just filled in by the call above.
        unsafe {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Posts a quit message, ending [`run`].
pub fn quit() {
    // SAFETY: takes no handle and writes through no pointer.
    unsafe { PostQuitMessage(0) };
}

/// Dispatches every currently-pending message without blocking.
///
/// `false` if a quit message was received, so a caller pumping while it waits on an external
/// condition knows to stop. Sticky: once it answers `false` it keeps answering `false`.
pub fn pump() -> bool {
    let mut message = MSG::default();
    loop {
        // SAFETY: no window filter, and the destination is a stack local.
        if !unsafe { PeekMessageW(&mut message, core::ptr::null_mut(), 0, 0, PM_REMOVE as u32) }
            .as_bool()
        {
            return true;
        }
        if message.message == WM_QUIT as u32 {
            // Reposts the quit that peeking removed, so a caller that checks the answer on
            // only some iterations does not lose it.
            // SAFETY: takes no handle and writes through no pointer.
            unsafe { PostQuitMessage(message.wParam as i32) };
            return false;
        }
        // SAFETY: `message` was just filled in by the call above.
        unsafe {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn class_name() -> PCWSTR {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    let name = NAME.get_or_init(|| "windows-window.Window\0".encode_utf16().collect());
    PCWSTR(name.as_ptr())
}

/// Registers the window class once per process.
///
/// The outcome is cached, success or failure alike: a class name this process could not take
/// stays taken, so a retry would only re-report the failure against a stale thread error.
///
/// # Errors
///
/// The class name is already registered to a window procedure that is not this one, which
/// would read this crate's state box through another crate's layout.
fn register_class() -> Result<()> {
    static ATOM: OnceLock<core::result::Result<ATOM, HRESULT>> = OnceLock::new();
    ATOM.get_or_init(|| {
        // The class carries no background brush: anything it painted would be a frame of the
        // wrong colour between the window appearing and the compositor's first commit.
        // `WM_ERASEBKGND` is answered rather than deferred for the same reason.
        // SAFETY: the descriptor is a stack local, and its class name outlives the process.
        let atom = unsafe {
            _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let wc = WNDCLASSW {
                style: (CS_HREDRAW | CS_VREDRAW) as u32,
                lpfnWndProc: Some(wndproc),
                hCursor: LoadCursorW(core::ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name(),
                // Kernel-side storage for each window's identity. It is freed with the window
                // rather than back to an allocator, so no second window is handed the same
                // slot while a stale `Window` still holds its value.
                cbWndExtra: size_of::<u64>() as i32,
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
    .map_err(|code| Error::new(code, "the window class could not be registered"))
}

/// Turns mouse input into pointer input for the whole process.
///
/// The state is queried first: the call is rejected once the process owns a window, and that
/// rejection is indistinguishable from a real failure.
fn enable_pointer_input() -> Result<()> {
    // SAFETY: neither call takes a handle or writes through a pointer.
    unsafe {
        if IsMouseInPointerEnabled().as_bool() {
            return Ok(());
        }
        EnableMouseInPointer(true.into()).ok()
    }
}

thread_local! {
    /// Holds the controller this crate minted for this thread, or `None` where the thread
    /// already had a queue. Kept for the **thread's** life rather than a window's: with two
    /// windows on one thread, the first must not take the queue from the second when it
    /// closes.
    ///
    /// The reference is never released. A thread-local's destructor runs at process exit from
    /// inside `LdrShutdownProcess`, and CoreMessaging fail-fasts on any call arriving after
    /// shutdown began, raising `0xE0464645` through `RaiseFailFastException` past any `catch`
    /// and past this process's own exit code, so releasing the last reference there ends the
    /// process by crashing it. Releasing it would not be a shutdown either: the queue's is
    /// `ShutdownQueueAsync`, which completes only while the thread keeps pumping, and a
    /// destructor cannot pump. An application that needs the queue drained owns a controller
    /// itself and shuts it down while its loop is still running.
    static QUEUE: OnceCell<Option<ManuallyDrop<DispatcherQueueController>>> =
        const { OnceCell::new() };
}

// Drop glue is what registers the thread-local destructor, so the type must have none.
const _: () = assert!(
    !core::mem::needs_drop::<Option<ManuallyDrop<DispatcherQueueController>>>(),
    "a droppable QUEUE registers a thread-local destructor that fail-fasts at process exit"
);

/// Ensures the calling thread has a dispatcher queue, minting one only if it has none.
///
/// A second controller on one thread fails, and the queue a `Compositor` or a
/// `DisplayInformation` finds is whichever the thread already has.
fn ensure_dispatcher_queue() -> Result<()> {
    QUEUE.with(|queue| {
        if queue.get().is_some() {
            return Ok(());
        }
        let minted = if DispatcherQueue::GetForCurrentThread().is_ok() {
            None
        } else {
            let options = DispatcherQueueOptions {
                dwSize: size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                // The queue joins this thread's existing apartment, which the application has
                // already chosen by the time it has a window to create.
                apartmentType: DQTAT_COM_NONE,
            };
            // SAFETY: the options are a stack local of the stated size and so is the
            // out-parameter; ownership of the controller transfers on success.
            Some(ManuallyDrop::new(unsafe {
                let mut controller = core::ptr::null_mut();
                CreateDispatcherQueueController(options, &mut controller).ok()?;
                DispatcherQueueController::from_raw(controller)
            }))
        };
        _ = queue.set(minted);
        Ok(())
    })
}

/// Registers `hwnd` for real precision-touchpad contacts. `false` if it could not.
///
/// The export is resolved by name rather than imported: it is documented against a
/// pre-release SDK, and a static import the running `user32` does not carry fails the
/// **process load**, so every process linking this crate would refuse to start on a machine
/// that has not taken the servicing update.
fn register_touchpad_capable(hwnd: HWND) -> bool {
    type Register = unsafe extern "system" fn(HWND, BOOL) -> BOOL;
    // SAFETY: `user32` is loaded — this crate has already registered the class and created
    // the window through it — so the handle is live and needs no free. The signature
    // transmuted onto the address is the documented one, and the call is made only if the
    // export exists.
    unsafe {
        let user32 = GetModuleHandleW(w!("user32.dll"));
        if user32.is_null() {
            return false;
        }
        let Some(address) = GetProcAddress(user32, s!("RegisterTouchpadCapableWindow")) else {
            return false;
        };
        let register: Register = core::mem::transmute(address);
        register(hwnd, true.into()).as_bool()
    }
}

/// Runs the system's move/size loop for `hwnd`, returning when the contact lifts. `false` if
/// `user32` does not export it.
///
/// Resolved by name for [`register_touchpad_capable`]'s reason and one further one: this
/// export has no header and no import library, so a static import is not expressible at all.
///
/// Resolved **per call**, with no cache: the call it guards then blocks for the whole drag,
/// which is several orders of magnitude longer than two handle lookups.
fn enter_move_size_loop(hwnd: HWND, at: (i32, i32), operation: MoveSize) -> bool {
    let Some(enter) = move_size_loop() else {
        return false;
    };
    let at = POINT { x: at.0, y: at.1 };
    // SAFETY: the address is the export's, the signature transmuted onto it is the documented
    // one, and `hwnd` is a live top-level window of this process — which is what the API
    // requires of it, and what `&Window` having answered `state()` establishes.
    unsafe { enter(hwnd, at, operation.operation()).as_bool() }
}

type MoveSizeLoop = unsafe extern "system" fn(HWND, POINT, i32) -> BOOL;

/// Resolves the export, or `None` on a build that does not carry it.
///
/// Split from [`enter_move_size_loop`] so the resolution can be asserted on its own: the loop
/// does not return until a contact is released, so no test can enter it.
fn move_size_loop() -> Option<MoveSizeLoop> {
    // SAFETY: `user32` is loaded — this crate registered the class and created the window
    // through it — so the handle is live and needs no free. The transmute is of an export's
    // address onto its documented signature.
    unsafe {
        let user32 = GetModuleHandleW(w!("user32.dll"));
        if user32.is_null() {
            return None;
        }
        GetProcAddress(user32, s!("EnterMoveSizeLoop")).map(|address| core::mem::transmute(address))
    }
}

/// Resizes a freshly created window from DIPs, once its real DPI is knowable.
fn resize_to_dips(hwnd: HWND, width: f32, height: f32) {
    let metrics = Metrics::for_window(hwnd);
    // SAFETY: `hwnd` is live; position is ignored under `SWP_NOMOVE`.
    unsafe {
        _ = SetWindowPos(
            hwnd,
            core::ptr::null_mut(),
            0,
            0,
            metrics.px(width),
            metrics.px(height),
            (SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) as u32,
        );
    }
}

/// Moves and resizes the window to the rect the system suggested for its new DPI.
///
/// The suggested rect keeps the window at the same apparent size and place across the scale
/// change; a rect computed here instead walks the window across the screen at every monitor
/// boundary.
///
/// # Safety
///
/// `lparam` must be the one `WM_DPICHANGED` carried.
unsafe fn apply_dpi_change(hwnd: HWND, lparam: LPARAM) {
    let suggested = lparam as *const RECT;
    if suggested.is_null() {
        return;
    }
    // SAFETY: the caller guarantees this is the message's own rect, valid for its duration.
    let rect = unsafe { *suggested };
    // SAFETY: `hwnd` is live.
    unsafe {
        _ = SetWindowPos(
            hwnd,
            core::ptr::null_mut(),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            (SWP_NOZORDER | SWP_NOACTIVATE) as u32,
        );
    }
}

/// Runs `f` against the state installed on `hwnd`, or answers `None` if there is none.
///
/// Re-derived on every use rather than held across one: a handler can destroy the window, and
/// `WM_NCDESTROY` frees the box before returning.
///
/// # Safety
///
/// `hwnd` must be a window of this crate's class.
unsafe fn with_state<R>(hwnd: HWND, f: impl FnOnce(&State) -> R) -> Option<R> {
    // SAFETY: the caller guarantees the class, so any state on it is a `State` this crate
    // installed, and the window procedure runs on the thread that owns it.
    unsafe {
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        (!state.is_null()).then(|| f(&*state))
    }
}

/// Runs `call` with the handler taken out of its slot.
///
/// Detached across the call so a handler that re-enters the window procedure sees an empty
/// slot and falls through to default processing rather than aliasing a handler that is
/// already running. It is put back only if the window still exists and nothing else claimed
/// the slot meanwhile; otherwise it drops here.
///
/// Handlers are invoked without `catch_unwind`: a panic unwinds to the `extern "system"`
/// boundary and aborts rather than crossing into the OS frames that called the procedure.
///
/// # Safety
///
/// `hwnd` must be a window of this crate's class, as for [`with_state`].
unsafe fn detached<H, R>(
    hwnd: HWND,
    slot: impl Fn(&State) -> &RefCell<Option<H>>,
    call: impl FnOnce(&mut H) -> R,
) -> Option<R> {
    // SAFETY: the caller guarantees the class.
    let mut handler = unsafe { with_state(hwnd, |state| slot(state).borrow_mut().take()) }??;
    let result = call(&mut handler);
    // SAFETY: as above.
    unsafe {
        with_state(hwnd, move |state| {
            let mut slot = slot(state).borrow_mut();
            if slot.is_none() {
                *slot = Some(handler);
            }
        })
    };
    Some(result)
}

/// Answers every message delivered to a window of this crate's class.
///
/// # Safety
///
/// The caller must be the system's message dispatch: `hwnd` must be a window of this crate's
/// class, and `wparam` and `lparam` must be the values delivered with `message`.
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // These arms run before the application's handler and are not suppressable: a handler
    // returning `Some` cannot swallow any of them.
    let edges = |state: &State| {
        match message {
            m if m == WM_SIZE as u32 || m == WM_WINDOWPOSCHANGED as u32 => {
                state.visibility.evaluate(hwnd);
                // The window's thread carries the pump, the input and the retained tree, so
                // it runs at full speed while any of that is observable. The re-tag is gated
                // on the scheduling class changing, not on a visibility edge: a window
                // created visible and never hidden sees no such edge.
                let speed = if state.visibility.is_hidden() {
                    Speed::Managed
                } else {
                    Speed::Full
                };
                if state.speed.replace(speed) != speed {
                    qos::set(speed);
                }
            }
            // The minimum track size runs ahead of the application's handler, so no handler
            // can drop it and leave the window draggable below what its layout can solve.
            m if m == WM_GETMINMAXINFO as u32 => {
                let info = lparam as *mut MINMAXINFO;
                if let Some((width, height)) = state.min_size_dips
                    && !info.is_null()
                {
                    let metrics = Metrics::for_window(hwnd);
                    // SAFETY: the system passes a writable `MINMAXINFO` valid for the
                    // duration of this message.
                    unsafe {
                        (*info).ptMinTrackSize = POINT {
                            x: metrics.px(width),
                            y: metrics.px(height),
                        };
                    }
                }
            }
            WM_USER_OCCLUSION => state.visibility.poke(),
            // Ahead of the application's frame work, so a frame completing during it is
            // posted rather than swallowed.
            crate::pace::WM_FRAME => {
                if let Some(gate) = state.frame_gate.borrow().as_ref() {
                    gate.begin_frame();
                }
            }
            _ => {}
        }
        state.caption.clone()
    };
    // SAFETY: this is the procedure of this crate's own class.
    let caption = unsafe { with_state(hwnd, edges) }.flatten();

    // One notification covers an HDR toggle, an SDR-white-level change and a monitor hop
    // alike. The display is cloned out first: the application's callback runs from here and
    // may destroy the window.
    if message == WM_USER_COLOR {
        // SAFETY: as above.
        let display = unsafe { with_state(hwnd, |state| state.display.borrow().clone()) }.flatten();
        if let Some(display) = display {
            display.refresh();
        }
    }

    // SAFETY: as above.
    let mut handled =
        unsafe { detached(hwnd, |s| &s.message, |h| h(hwnd, message, wparam, lparam)) }.flatten();

    if handled.is_none() && message == WM_SIZE as u32 {
        let width = (lparam & 0xffff) as i32;
        let height = ((lparam >> 16) & 0xffff) as i32;
        // SAFETY: as above.
        handled = unsafe {
            detached(
                hwnd,
                |s| &s.resize,
                |h| {
                    h(width, height);
                    0
                },
            )
        };
    }

    if handled.is_none() && message == WM_DPICHANGED as u32 {
        // Applied before the scale handler, which is then responsible for the content's
        // scale and nothing else.
        // SAFETY: this is the message's own `lparam`.
        unsafe { apply_dpi_change(hwnd, lparam) };
        // SAFETY: as above.
        unsafe { detached(hwnd, |s| &s.scale, |h| h(Metrics::for_window(hwnd))) };
        handled = Some(0);
    }

    if handled.is_none()
        && let Some(caption) = caption
    {
        handled = caption.message(hwnd, message, wparam, lparam);
    }

    let mut quit_on_close = false;
    if message == WM_DESTROY as u32 {
        // SAFETY: as above.
        let teardown = unsafe {
            with_state(hwnd, |state| {
                // Nothing this window draws can be seen again, so the hidden state is
                // published and every thread parked on it wakes: a pacer outliving a closed
                // window would otherwise keep waiting on the compositor clock and posting
                // into a handle this crate no longer owns.
                state.visibility.publish(true);
                // A full-speed request would otherwise stay on the thread for the rest of its
                // life.
                if state.speed.replace(Speed::Managed) != Speed::Managed {
                    qos::set(Speed::Managed);
                }
                (state.display.borrow_mut().take(), state.quit_on_close)
            })
        };
        if let Some((display, quit)) = teardown {
            quit_on_close = quit;
            // Closed while the handle is still valid: `GetForWindow` hooks the window's
            // message loop, and a subscription revoked after the window is gone is revoked
            // against a hook that no longer has one.
            if let Some(display) = display {
                display.close();
            }
        }
    }

    if message == WM_NCDESTROY as u32 {
        // SAFETY: as above. The identity is cleared before the box is freed, and both before
        // this returns to the system, which is what makes a matching identity proof of a live
        // box everywhere else.
        unsafe {
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
            if !state.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_WINDOW_ID, 0);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(state));
            }
        }
    }

    if let Some(result) = handled {
        return result;
    }

    match message as i32 {
        WM_ERASEBKGND => 1,
        WM_DESTROY => {
            if quit_on_close {
                // SAFETY: takes no handle and writes through no pointer.
                unsafe { PostQuitMessage(0) };
            }
            0
        }
        // SAFETY: the arguments are the ones just received.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts each operation against the value the API documents, not against the order the
    /// variants are written in.
    #[test]
    fn move_size_operations_are_the_systems() {
        for (operation, code) in [
            (MoveSize::Left, 1),
            (MoveSize::Right, 2),
            (MoveSize::Top, 3),
            (MoveSize::TopLeft, 4),
            (MoveSize::TopRight, 5),
            (MoveSize::Bottom, 6),
            (MoveSize::BottomLeft, 7),
            (MoveSize::BottomRight, 8),
            (MoveSize::Move, 9),
        ] {
            assert_eq!(operation.operation(), code, "{operation:?}");
        }
    }

    /// Asserts that this platform's `user32` exports `EnterMoveSizeLoop`. Without it
    /// `begin_move_size` answers `false` and the window does not drag.
    ///
    /// The two assertions are separate because they fail for unrelated reasons: a missing
    /// module means this process never loaded `user32`, which a test harness that has created
    /// no window may not have done, and says nothing about the export.
    #[test]
    fn the_move_size_loop_resolves_on_this_floor() {
        // Loads `user32`, which nothing in this test has needed yet and which
        // `GetModuleHandleW` finds rather than loads. The argument is not a window and the
        // answer is discarded; the load is what this call is for.
        // SAFETY: `GetDpiForWindow` is documented against any handle and answers zero for one
        // that is not a window.
        _ = unsafe { GetDpiForWindow(core::ptr::null_mut()) };
        // SAFETY: reading the handle of a module this process has loaded.
        let user32 = unsafe { GetModuleHandleW(w!("user32.dll")) };
        assert!(!user32.is_null(), "this process has no user32 loaded");
        assert!(
            move_size_loop().is_some(),
            "user32 has no EnterMoveSizeLoop: begin_move_size is inert on this build"
        );
    }
}
