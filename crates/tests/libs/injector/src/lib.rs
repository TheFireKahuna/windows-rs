//! Synthetic mouse, touch, pen and precision-touchpad streams, for driving the real input
//! stack against a real window.
//!
//! **Real device streams, not a fake pointer layer.** A fake would test the fake. Every
//! stream here goes through the system's own input path and arrives at the window under test
//! as `WM_POINTER*`, which is what makes an assertion about the pointer stack an assertion
//! about the thing that ships.
//!
//! # One injection object
//!
//! `Windows.UI.Input.Preview.Injection.InputInjector` is the platform's injection surface
//! and covers mouse, pen, touch and the keyboard. The precision touchpad is the one device
//! class it does not reach, and that one alone falls back to `CreateSyntheticPointerDevice2`
//! — a touchpad must be created with a physical size, which is also why the v1 entry point
//! cannot create one at all.
//!
//! Two things about that object are worth knowing before writing against it, because each
//! fails by succeeding:
//!
//! * **Touch and pen have an initialize/uninitialize pair, and it is not ceremony.** A pen
//!   sample injected without `InitializePenInjection` is accepted, returns success and goes
//!   nowhere. The pair is owned by the injector and refcounted per device class, so it
//!   cannot be skipped and cannot be released under a live stream.
//! * **`SetCursorPos` is not injection.** A cursor warp moves the pointer and lets the
//!   window manager notice; only an injected event goes through the input stack. The same
//!   probe reads "zero pointer messages, legacy only" under a warp — the right conclusion's
//!   exact opposite, from an instrument that was never measuring the stack.
//!
//! # Which device to assert on
//!
//! **Touch.** A touch contact carries its own screen position, so it is exact by
//! construction, and it arrives on a device of its own, so nothing a person does at the
//! machine can interfere with it. Mouse is the opposite on both counts — no injection API
//! gives `PT_MOUSE` a device of its own, so it shares the one system cursor, and its
//! absolute coordinate is a 16-bit normalization rather than a pixel. Mouse is here for
//! `PT_MOUSE` coverage; where a claim can be made on touch it is made on touch.
//!
//! # Exactness
//!
//! A drag-fidelity assertion is on the **integral** — a drive of known total travel must
//! produce a cumulative translation equal to it — because a dropped sample shows up as a
//! short total and as nothing else. That only works if the harness places its samples where
//! it said it would:
//!
//! * touch, pen and touchpad carry a location in the sample, so they are exact by
//!   construction;
//! * mouse's normalization is **inverted in closed form and then verified once**
//!   ([`Injector::mouse`]). A harness that instead corrects its aim against `GetCursorPos`
//!   adds travel of its own — up to three injected moves per logical one — and a drag's
//!   integral counts every one of them.
//!
//! A mouse path is additionally handed over as **one call carrying its own timing**, so
//! nothing can interleave in the middle of a drag.
//!
//! # Concurrency
//!
//! There is one input stack per session, so there is one stream at a time: every constructor
//! takes `&mut self` and a stream borrows the injector for as long as it lives. Injection
//! tests do not run concurrently, for the same reason.
//!
//! # What this crate deliberately does not do
//!
//! It never calls `EnableMouseInPointer`. That opt-in is process-wide, one-way, and must
//! happen before the first window — so it belongs to the application under test, and a
//! harness that made it would be deciding for the window whether the thing being tested is
//! switched on at all.
//!
//! It also names no message constant. What arrived is a question for the window, and a
//! harness that both wrote the input and owned the constants naming the result would be
//! checking itself.

// `dead_code` covers what arrives with a named type rather than on its own — the members of
// an enum family. Nothing whole is filtered unused.
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

/// What went wrong, in terms a failing test can act on.
#[derive(Debug)]
pub enum Error {
    /// Something a stream needs is absent from this process or this build of `user32`.
    ///
    /// Not a fallback point: the stream is unavailable and says which thing is missing. See
    /// [`Capability`].
    Unavailable {
        /// What did not resolve — an export, or a capability.
        export: &'static str,
    },
    /// A platform call failed.
    Call {
        /// The function that failed.
        what: &'static str,
        /// What the platform said about it.
        source: windows_core::Error,
    },
    /// A mouse sample did not land where it was aimed.
    ///
    /// Injection is exact or it is not used: a harness that misses by a pixel reports a bug
    /// in the stack under test that is its own. See [`Injector::mouse`].
    Calibration {
        /// The screen pixel the injector asked for.
        asked: (i32, i32),
        /// Where the cursor actually landed.
        landed: (i32, i32),
        /// The virtual desktop the normalization divided by, as `(left, top, width, height)`.
        /// Carried because the two ways this fails look identical without it: a rounding rule
        /// that is not the one assumed, and a process reading metrics that were virtualised
        /// because its thread is not DPI-aware.
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

// No `source`: `windows_core::Error` is not a `core::error::Error`, and the platform's
// account of the failure is already in this one's `Display` — which is where a failing test
// reads it.
impl core::error::Error for Error {}

/// What every fallible operation in this crate answers with.
pub type Result<T> = core::result::Result<T, Error>;

/// Runs a drive on a thread of its own, so the caller can pump the window while it happens.
///
/// **This is not a convenience, and skipping it costs fidelity.** A harness that injects on
/// the same thread its window pumps on delivers the whole drive into a queue nobody is
/// reading, and the platform coalesces what it finds there. Touch survives that — a coalesced
/// frame keeps its samples in the pointer's history — but **mouse does not**: a coalesced
/// mouse update reports `historyCount` of 1 and the samples between are simply gone. Measured
/// on 26200: a 40-sample zigzag injected without a running pump arrives as 5 messages
/// carrying 300 DIPs of a 557-DIP path, which reads exactly like a stack that dropped them.
///
/// So the drive gets a thread and the window keeps pumping, which is also what a real
/// application does. The returned handle carries the drive's own result; join it, and pump
/// until it is done.
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

/// The one input stack, and the space its streams aim at.
///
/// Open one stream at a time; the borrow is what says so.
pub struct Injector {
    space: Space,
    injection: injection::Injection,
    late: late::Late,
    pace: pace::Pace,
    /// The virtual-desktop mapping the mouse normalizes against, once it has been proven.
    /// `None` until the first [`mouse`](Injector::mouse) call, because proving it moves the
    /// cursor and a run that injects no mouse should not pay for that.
    desktop: Option<mouse::Desktop>,
    /// The contact count `InitializeTouchInjection` was last given. It is process-wide, so it
    /// is raised when a wider stream asks and never lowered.
    touch_max: u32,
}

impl Injector {
    /// Aims every stream at `hwnd`'s client area, in DIPs.
    ///
    /// The window's origin and scale are read **here**, not held from somewhere earlier: a
    /// stream that cached a scale across a display hop would place every contact on the wrong
    /// pixel grid, silently, for the rest of the run. [`retarget`](Self::retarget) is how a
    /// moved or rescaled window is restated.
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

    /// Where a point stated in DIPs lands.
    #[must_use]
    pub const fn space(&self) -> Space {
        self.space
    }

    /// Re-reads the target window's origin and scale.
    ///
    /// Wanted after a move or a DPI change, and after nothing else — a window that has not
    /// moved does not need it, and doing it per sample would be two syscalls per sample.
    pub fn retarget(&mut self, hwnd: *mut core::ffi::c_void) -> Result<()> {
        self.space = Space::for_window(hwnd)?;
        Ok(())
    }

    /// What this process and this build can actually inject.
    #[must_use]
    pub fn capability(&self) -> Capability {
        self.late.capability()
    }

    /// A mouse stream.
    ///
    /// The first call **verifies the desktop's absolute-coordinate mapping** by placing three
    /// probes and reading back where they landed, and fails with [`Error::Calibration`]
    /// rather than proceeding approximately. That costs three injected moves and restores the
    /// cursor afterwards, so open this stream before the window under test starts counting
    /// anything.
    pub fn mouse(&mut self) -> Result<MouseStream<'_>> {
        if self.desktop.is_none() {
            self.desktop = Some(mouse::Desktop::verified(self.injection.injector())?);
        }
        Ok(MouseStream::new(self))
    }

    /// A touch stream carrying up to `contacts` simultaneous contacts.
    ///
    /// Every injected frame carries the whole live contact set, which is the platform's rule
    /// rather than a convenience: a frame that omits a contact still down ends it. The stream
    /// assembles that frame, so moving one contact holds the others where they are instead of
    /// dropping them.
    pub fn touch(&mut self, contacts: u32) -> Result<TouchStream<'_>> {
        TouchStream::open(self, contacts)
    }

    /// A pen stream. One contact, with pressure, tilt, the barrel button and the inverted end
    /// — and a hover, which is the thing a pen has that touch does not.
    ///
    /// **Refused in an unpackaged process**, because a pen is a virtual device and creating
    /// one needs the `inputInjectionBrokered` capability ([`Capability`]). Refusing is the
    /// point: every call on that path returns success and delivers nothing, so a stream that
    /// ran would let a test pass on no input at all.
    pub fn pen(&mut self) -> Result<PenStream<'_>> {
        self.require_brokered()?;
        PenStream::open(self)
    }

    /// A precision-touchpad stream.
    ///
    /// Its coordinates are **fractions of the pad**, not window DIPs, because a touchpad is
    /// an indirect device: it has no screen position, and the recogniser that reads it
    /// reports in units relative to the device. See [`TouchpadStream`].
    ///
    /// **Refused in an unpackaged process**, for the same reason as [`pen`](Self::pen): its
    /// contacts come from a virtual device. [`TouchpadStream::action`] does *not* — a global
    /// gesture is a message to a tracked window rather than a device sample — which is why the
    /// two inertia signals are reachable where touchpad contacts are not.
    pub fn touchpad(&mut self) -> Result<TouchpadStream<'_>> {
        self.require_brokered()?;
        TouchpadStream::open(self)
    }

    /// Whether this process could create a virtual input device at all.
    fn require_brokered(&self) -> Result<()> {
        if identity::packaged() {
            Ok(())
        } else {
            Err(Error::Unavailable {
                export: "the inputInjectionBrokered capability, which needs package identity",
            })
        }
    }

    /// A key press and release.
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

    /// Waits, precisely, without injecting anything.
    ///
    /// A hold is a gap in a stream rather than an absence of one: a press held for 900 ms is
    /// what raises `Holding`, and the recogniser measures that against a clock this harness
    /// has to be honest with. `thread::sleep` is not — its resolution is the scheduler's
    /// tick, an order of magnitude coarser than the thresholds under test.
    pub fn wait(&mut self, duration: core::time::Duration) -> Result<&mut Self> {
        self.pace.sleep(duration)?;
        Ok(self)
    }
}
