//! Synthetic mouse, touch, pen and precision-touchpad streams, for driving the real input
//! stack against a real window.
//!
//! Every stream goes through the system's own input path and arrives at the target window as
//! `WM_POINTER*`. Nothing here fabricates a pointer message directly.
//!
//! # One injection object
//!
//! `Windows.UI.Input.Preview.Injection.InputInjector` covers mouse, pen, touch and the
//! keyboard. The precision touchpad is the one device class it does not reach, and that one
//! alone falls back to `CreateSyntheticPointerDevice2`: a touchpad must be created with a
//! physical size, and the v1 entry point has no parameter to state one.
//!
//! Two properties of that object fail by succeeding:
//!
//! * Touch and pen have an initialize/uninitialize pair. A pen sample injected without
//!   `InitializePenInjection` is accepted, returns success and delivers nothing. The injector
//!   owns the pair and refcounts it per device class, so it cannot be skipped and cannot be
//!   released under a live stream.
//! * `SetCursorPos` is not injection. A cursor warp moves the pointer without producing
//!   input, so a run driven by warps reports zero pointer messages and legacy messages only.
//!
//! # Which device to assert on
//!
//! A touch contact carries its own screen position, so it is exact by construction, and it
//! arrives on a device of its own, so no physical device at the machine interferes with it.
//! Mouse differs on both counts: no injection API gives `PT_MOUSE` a device of its own, so it
//! shares the one system cursor, and its absolute coordinate is a 16-bit normalization rather
//! than a pixel. Mouse covers `PT_MOUSE`; every claim that can be made on touch is made on
//! touch.
//!
//! # Exactness
//!
//! A drag-fidelity assertion compares path integrals: a drive of known total travel must
//! produce a cumulative translation equal to it, because a dropped sample shows up as a short
//! total and as nothing else. That holds only if the harness places its samples where it
//! stated:
//!
//! * touch, pen and touchpad carry a location in the sample, so they are exact by
//!   construction;
//! * mouse's normalization is inverted in closed form and verified once
//!   ([`Injector::mouse`]). Correcting aim against `GetCursorPos` instead would add travel of
//!   its own — up to three injected moves per logical one — and a drag's integral counts
//!   every one of them.
//!
//! # Concurrency
//!
//! There is one input stack per session, so there is one stream at a time: every constructor
//! takes `&mut self` and a stream borrows the injector for as long as it lives. Injection
//! tests do not run concurrently.
//!
//! # What this crate does not do
//!
//! It never calls `EnableMouseInPointer`. That opt-in is process-wide, one-way, and must
//! happen before the first window, so it belongs to the application under test.
//!
//! It names no message constant. The binding filter generates none, and the program reading
//! what arrived declares its own.

// `dead_code` covers enum members generated alongside the types this crate does use. No
// whole binding is unused.
#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms
)]
mod bindings;

mod device;
mod identity;
mod injection;
mod key;
mod late;
mod mouse;
mod pace;
mod pen;
mod space;
mod touch;
mod touchpad;

pub use key::Key;
pub use late::Capability;
pub use mouse::{Button, MouseStream};
pub use pace::Rate;
pub use pen::PenStream;
pub use space::{Point, Space, line, zigzag};
pub use touch::TouchStream;
pub use touchpad::{PAD_MM, TouchpadAction, TouchpadStream};

/// Reports why an injector operation failed.
#[derive(Debug)]
pub enum Error {
    /// Names an export or capability this process or this build of `user32` does not have.
    ///
    /// The stream is refused rather than degraded. [`Capability`] reports the same facts
    /// before a stream is opened.
    Unavailable {
        /// The export or capability that did not resolve.
        export: &'static str,
    },
    /// A platform call failed.
    Call {
        /// The function that failed.
        what: &'static str,
        /// The platform's error for that call.
        source: windows_core::Error,
    },
    /// A mouse sample did not land where it was aimed.
    ///
    /// Opening a mouse stream verifies the absolute-coordinate mapping and returns this
    /// instead of placing samples approximately. See [`Injector::mouse`].
    Calibration {
        /// The screen pixel the injector asked for.
        asked: (i32, i32),
        /// Where the cursor actually landed.
        landed: (i32, i32),
        /// The virtual desktop the normalization divided by, as `(left, top, width, height)`.
        /// It separates the two causes, which look identical without it: a rounding rule
        /// other than the one assumed, and a thread that is not DPI-aware reading metrics
        /// the system virtualised.
        desktop: (i32, i32, i32, i32),
    },
    /// A contact index outside the stream's declared maximum.
    Contact {
        /// The index asked for.
        index: u32,
        /// How many contacts the stream was opened with.
        max: u32,
    },
    /// An operation that needs a contact already down was called without one.
    NotDown,
}

impl Error {
    pub(crate) fn call(what: &'static str, source: windows_core::Error) -> Self {
        Self::Call { what, source }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unavailable { export } => write!(f, "unavailable here: {export}"),
            Self::Call { what, source } => write!(f, "{what} failed: {source}"),
            Self::Calibration {
                asked,
                landed,
                desktop,
            } => write!(
                f,
                "an absolute mouse sample aimed at {asked:?} landed at {landed:?} on a virtual \
                 desktop of {desktop:?}, so no sample can be placed exactly. If that desktop \
                 looks like the real one divided by its scale, the calling thread is not \
                 DPI-aware and every metric it reads has been virtualised"
            ),
            Self::Contact { index, max } => {
                write!(f, "contact {index} on a stream opened with {max}")
            }
            Self::NotDown => f.write_str("no contact is down"),
        }
    }
}

// No `source`: `windows_core::Error` does not implement `core::error::Error`, and its own
// message is already carried by this type's `Display`.
impl core::error::Error for Error {}

/// The result of every fallible operation in this crate, carrying [`Error`].
pub type Result<T> = core::result::Result<T, Error>;

/// Runs `drive` on a thread of its own, so the caller can pump the window while injection
/// happens.
///
/// Injecting on the thread that pumps the window delivers the whole drive into a queue that
/// is not being read, and the platform coalesces what it finds there. Touch survives — a
/// coalesced frame keeps its samples in the pointer's history — but a coalesced mouse update
/// reports `historyCount` of 1 and the samples between are gone. Measured on 26200: a
/// 40-sample zigzag injected without a running pump arrives as 5 messages carrying 300 DIPs
/// of a 557-DIP path.
///
/// The returned handle carries the drive's own result. Pump until it finishes, then join it.
///
/// ```no_run
/// # use injector::{Injector, Space, drive};
/// # fn go(space: Space) {
/// let driving = drive(space, |injector: &mut Injector| {
///     injector.mouse()?.tap((80.0, 50.0))?;
///     Ok(())
/// });
/// while !driving.is_finished() {
///     // pump the window here
/// }
/// driving.join().expect("the drive thread panicked").expect("the drive");
/// # }
/// ```
pub fn drive<F>(space: Space, drive: F) -> std::thread::JoinHandle<Result<()>>
where
    F: FnOnce(&mut Injector) -> Result<()> + Send + 'static,
{
    std::thread::spawn(move || {
        let mut injector = Injector::new(space)?;
        drive(&mut injector)
    })
}

/// Holds the session's one input stack and the space its streams aim at.
///
/// One stream is open at a time: every constructor borrows the injector mutably for as long
/// as the stream lives.
pub struct Injector {
    space: Space,
    injection: injection::Injection,
    late: late::Late,
    pace: pace::Pace,
    /// The virtual-desktop mapping the mouse normalizes against, once verified. `None` until
    /// the first [`mouse`](Injector::mouse) call: verifying it moves the cursor, so a run
    /// that injects no mouse never does it.
    desktop: Option<mouse::Desktop>,
    /// The contact count `InitializeTouchInjection` was last given. It is process-wide, so it
    /// is raised when a wider stream asks and never lowered.
    touch_max: u32,
}

impl Injector {
    /// Aims every stream at `hwnd`'s client area, in DIPs.
    ///
    /// Reads the window's origin and scale now and holds them for the injector's life. Until
    /// [`retarget`](Self::retarget) restates them, every sample is placed on the pixel grid
    /// read here, so a window that moved or changed DPI places its contacts on the old one.
    pub fn for_window(hwnd: *mut core::ffi::c_void) -> Result<Self> {
        Self::new(Space::for_window(hwnd)?)
    }

    /// Aims every stream at an explicit space.
    pub fn new(space: Space) -> Result<Self> {
        Ok(Self {
            space,
            injection: injection::Injection::open()?,
            late: late::Late::resolve(),
            pace: pace::Pace::default(),
            desktop: None,
            touch_max: 0,
        })
    }

    /// Returns the space a point stated in DIPs is placed in.
    #[must_use]
    pub const fn space(&self) -> Space {
        self.space
    }

    /// Re-reads the target window's origin and scale.
    ///
    /// Call after the window moves or changes DPI. Each call costs two syscalls, which is why
    /// the space is not re-read per sample.
    pub fn retarget(&mut self, hwnd: *mut core::ffi::c_void) -> Result<()> {
        self.space = Space::for_window(hwnd)?;
        Ok(())
    }

    /// Returns what this process and this build of `user32` can inject.
    #[must_use]
    pub fn capability(&self) -> Capability {
        self.late.capability()
    }

    /// Opens a mouse stream.
    ///
    /// The first call verifies the desktop's absolute-coordinate mapping by placing three
    /// probes and reading back where they landed, and returns [`Error::Calibration`] if one
    /// misses rather than proceeding approximately. It injects three moves and restores the
    /// cursor, so open this stream before the window under test starts counting messages.
    pub fn mouse(&mut self) -> Result<MouseStream<'_>> {
        if self.desktop.is_none() {
            self.desktop = Some(mouse::Desktop::verified(self.injection.injector())?);
        }
        Ok(MouseStream::new(self))
    }

    /// Opens a touch stream carrying up to `contacts` simultaneous contacts.
    ///
    /// Every injected frame carries the whole live contact set: a frame that omits a contact
    /// still down ends that contact. The stream assembles the frame, so moving one contact
    /// holds the others where they are.
    pub fn touch(&mut self, contacts: u32) -> Result<TouchStream<'_>> {
        TouchStream::open(self, contacts)
    }

    /// Opens a pen stream: one contact with pressure, tilt, the barrel button, the inverted
    /// end, and hover, which no other device here reports.
    ///
    /// Refused in an unpackaged process. A pen is a virtual device, and creating one needs
    /// the `inputInjectionBrokered` capability ([`Capability`]); without it every pen call
    /// returns success and delivers nothing.
    pub fn pen(&mut self) -> Result<PenStream<'_>> {
        self.require_brokered()?;
        PenStream::open(self)
    }

    /// Opens a precision-touchpad stream.
    ///
    /// Its coordinates are fractions of the pad rather than window DIPs: a touchpad is
    /// indirect, has no screen position, and the recogniser reading it reports in units
    /// relative to the device. See [`TouchpadStream`].
    ///
    /// Refused in an unpackaged process, because its contacts come from a virtual device, as
    /// [`pen`](Self::pen)'s do. [`TouchpadStream::action`] needs no such device — a global
    /// gesture is a message to a tracked window rather than a device sample — so the two
    /// inertia signals stay reachable where touchpad contacts are not.
    pub fn touchpad(&mut self) -> Result<TouchpadStream<'_>> {
        self.require_brokered()?;
        TouchpadStream::open(self)
    }

    /// Succeeds when this process has the package identity a virtual input device needs.
    fn require_brokered(&self) -> Result<()> {
        if identity::packaged() {
            Ok(())
        } else {
            Err(Error::Unavailable {
                export: "the inputInjectionBrokered capability, which needs package identity",
            })
        }
    }

    /// Presses a key and releases it.
    pub fn key(&mut self, key: Key) -> Result<&mut Self> {
        self.key_down(key)?.key_up(key)
    }

    /// Presses a key and leaves it down.
    pub fn key_down(&mut self, key: Key) -> Result<&mut Self> {
        key::send(self.injection.injector(), key, false)?;
        Ok(self)
    }

    /// Releases a key.
    pub fn key_up(&mut self, key: Key) -> Result<&mut Self> {
        key::send(self.injection.injector(), key, true)?;
        Ok(self)
    }

    /// Waits `duration` without injecting anything.
    ///
    /// A held press is a gap in the stream — 900 ms of it is what raises `Holding` — and the
    /// recogniser measures that gap against its own clock, so the wait runs on a
    /// high-resolution timer. `thread::sleep` resolves to the scheduler's tick, an order of
    /// magnitude coarser than the thresholds under test.
    pub fn wait(&mut self, duration: core::time::Duration) -> Result<&mut Self> {
        self.pace.sleep(duration)?;
        Ok(self)
    }
}
