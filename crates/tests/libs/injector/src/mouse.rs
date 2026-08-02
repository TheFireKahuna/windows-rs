//! The mouse stream, and the one device here that shares something with the user.
//!
//! Two properties follow from the platform and neither is a choice this crate made:
//!
//! * **A mouse sample carries no position.** `InjectedInputMouseInfo` states an absolute
//!   coordinate normalized to a 16-bit grid over the virtual desktop, so on a wide desktop
//!   a whole pixel is a fraction of one normalized step and the mapping back is not the
//!   identity. [`Desktop::normalize`] inverts it in closed form — aiming at the *centre* of
//!   the interval that lands on the wanted pixel — and [`Desktop::verified`] proves that, at
//!   three points across the desktop, before a single test sample is placed. A miss is
//!   [`Error::Calibration`]: a failure, not a correction. A harness that instead corrects
//!   its aim against `GetCursorPos` injects up to three moves per logical one, and a drag's
//!   integral counts every one of them.
//! * **There is one system cursor and this is not its only source.** No injection API
//!   produces a `PT_MOUSE` stream on a device of its own — pen, touch and touchpad each get
//!   one, mouse does not — so a hand resting on a real mouse adds travel to a drag whose
//!   total is the thing under test. That is a property of the device class rather than of
//!   this harness, and the answer is that **the exactness contract is asserted on touch**,
//!   which has its own device and cannot be interfered with. Mouse is here for `PT_MOUSE`
//!   coverage, and its own integral is a diagnostic.
//!
//! **A path is one sample per call, paced.** `InjectMouseInput` takes a sequence and each
//! sample can carry its own offset in milliseconds, which reads like the right way to state a
//! rate — and it is not. Measured on 26200 against a pumping window: a 40-sample zigzag handed
//! over as three batched calls arrives as 16 messages carrying 353 DIPs of a 557-DIP path,
//! while the same path injected one sample per call, spaced by a high-resolution timer,
//! arrives whole. The batch is a convenience for throughput, not a fidelity mechanism, and
//! this stream wants fidelity.
//!
//! Two things about that sequence are worth recording even though nothing here uses it now,
//! because the next person to reach for it will meet both: **it caps at sixteen samples per
//! call** — seventeen is `E_INVALIDARG`, undocumented, and the message names neither the
//! parameter nor the limit — and the samples it does accept are coalesced as above.
//!
//! One consequence worth knowing before it confuses a run: **an absolute move to the pixel the
//! cursor is already on produces no input at all.** A path with a repeated point is a path one
//! sample short, and the harness cannot tell that apart from a stack that dropped it.

use windows_collections::IIterable;

use crate::bindings::*;
use crate::space::Point;
use crate::{Error, Injector, Rate, Result};

/// The virtual desktop an absolute mouse coordinate is normalized against, once the
/// normalization has been proven to place a sample exactly.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Desktop {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

impl Desktop {
    fn read() -> Self {
        // SAFETY: each takes an index and returns a metric.
        unsafe {
            Self {
                left: GetSystemMetrics(SM_XVIRTUALSCREEN),
                top: GetSystemMetrics(SM_YVIRTUALSCREEN),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1),
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1),
            }
        }
    }

    /// The normalized coordinate that lands on exactly this screen pixel.
    ///
    /// The system maps a normalized value back by scaling it into the desktop's extent and
    /// truncating, so a *range* of normalized values lands on each pixel. Aiming at the
    /// centre of that range — `(p + ½) · 65536 / extent` — leaves the whole half-interval as
    /// margin, which on a 7680-pixel desktop is about four normalized steps either way. That
    /// is what makes one verification enough rather than one per pixel.
    const fn normalize(&self, x: i32, y: i32) -> (i32, i32) {
        (
            normalize_axis(x, self.left, self.width),
            normalize_axis(y, self.top, self.height),
        )
    }

    /// Reads the desktop and proves its mapping, or refuses to place a sample.
    ///
    /// Three probes: near the origin, at the centre, and near the far corner — the places a
    /// rounding rule that is off by a step or by a scale would show. The cursor is restored
    /// afterwards, and all of it happens before the caller's first stream, so a window that
    /// has started counting has not seen any of it.
    ///
    /// **Each probe is attempted more than once, and that is not a correction.** The cursor
    /// is shared: a hand on a real mouse moves it between the injection and the read, and the
    /// miss that produces is indistinguishable from a mapping that is wrong. A retry
    /// separates them — a wrong mapping misses every time, a competing device does not — and
    /// it costs nothing a measurement could see, because calibration happens before the first
    /// measured sample.
    pub(crate) fn verified(injector: &InputInjector) -> Result<Self> {
        const ATTEMPTS: u32 = 4;
        let desktop = Self::read();
        let restore = cursor()?;
        let probes = [
            (desktop.left + 1, desktop.top + 1),
            (
                desktop.left + desktop.width / 2,
                desktop.top + desktop.height / 2,
            ),
            (
                desktop.left + desktop.width - 2,
                desktop.top + desktop.height - 2,
            ),
        ];
        for asked in probes {
            let mut landed = (0, 0);
            let mut held = false;
            for _ in 0..ATTEMPTS {
                desktop.place(injector, asked.0, asked.1)?;
                landed = settle(asked)?;
                held = landed == asked;
                if held {
                    break;
                }
            }
            if !held {
                // Put the cursor back before reporting, so a failing run does not also leave
                // the pointer in a corner.
                _ = desktop.place(injector, restore.0, restore.1);
                return Err(Error::Calibration {
                    asked,
                    landed,
                    desktop: (desktop.left, desktop.top, desktop.width, desktop.height),
                });
            }
        }
        desktop.place(injector, restore.0, restore.1)?;
        Ok(desktop)
    }

    /// One absolute move, to an exact screen pixel.
    fn place(&self, injector: &InputInjector, x: i32, y: i32) -> Result<()> {
        let (nx, ny) = self.normalize(x, y);
        inject(injector, vec![sample(MOVE, nx, ny, 0)?])
    }
}

/// Every absolute move carries these four.
///
/// `MoveNoCoalesce` is not optional: the system coalesces injected moves by default, which
/// shortens the integral a drag-fidelity assertion is written on — so a stack that dropped
/// nothing would fail against a harness that dropped for it. A real high-rate mouse produces
/// distinct events; so does this. `VirtualDesk` is what makes a secondary monitor reachable
/// rather than silently assuming the primary.
const MOVE: InjectedInputMouseOptions = InjectedInputMouseOptions(
    InjectedInputMouseOptions::Move.0
        | InjectedInputMouseOptions::MoveNoCoalesce.0
        | InjectedInputMouseOptions::Absolute.0
        | InjectedInputMouseOptions::VirtualDesk.0,
);

/// Which button, for the buttons that are not the contact itself.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Button {
    /// The one a context menu answers to. A press of this while nothing is down is what
    /// raises `RightTapped` on a mouse.
    Secondary,
    /// The wheel button.
    Middle,
}

/// A mouse, aimed at the injector's space.
///
/// The primary button is not on [`Button`] and that is the platform's model rather than an
/// omission: a `WM_POINTERDOWN` is *a contact starting*, and a second button pressed while
/// the first is held arrives as an update carrying a changed button set — never as a second
/// down. So [`down`](Self::down) and [`up`](Self::up) start and end the contact, and
/// [`press`](Self::press) / [`release`](Self::release) change the buttons within one.
pub struct MouseStream<'a> {
    injector: &'a mut Injector,
    down: bool,
    at: Option<Point>,
}

impl<'a> MouseStream<'a> {
    pub(crate) fn new(injector: &'a mut Injector) -> Self {
        Self {
            injector,
            down: false,
            at: None,
        }
    }

    fn desktop(&self) -> Desktop {
        // Set by `Injector::mouse` before this type is constructed, and never cleared.
        self.injector
            .desktop
            .expect("a mouse stream exists only after its desktop mapping was verified")
    }

    /// Moves to a point, exactly.
    pub fn move_to(&mut self, to: impl Into<Point>) -> Result<&mut Self> {
        let to = to.into();
        let (x, y) = self.injector.space.to_px(to);
        self.desktop()
            .place(self.injector.injection.injector(), x, y)?;
        self.at = Some(to);
        Ok(self)
    }

    /// Moves to a point and starts a contact there.
    ///
    /// The move is separate from the press on purpose: a press at a position the pointer has
    /// not been to is a press whose target the stack resolves from a sample it never saw
    /// arrive, which is not what a user does.
    pub fn down(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.move_to(at)?;
        self.button(InjectedInputMouseOptions::LeftDown)?;
        self.down = true;
        Ok(self)
    }

    /// Ends the contact.
    pub fn up(&mut self) -> Result<&mut Self> {
        self.button(InjectedInputMouseOptions::LeftUp)?;
        self.down = false;
        Ok(self)
    }

    /// Presses an additional button.
    pub fn press(&mut self, button: Button) -> Result<&mut Self> {
        self.button(match button {
            Button::Secondary => InjectedInputMouseOptions::RightDown,
            Button::Middle => InjectedInputMouseOptions::MiddleDown,
        })?;
        Ok(self)
    }

    /// Releases an additional button.
    pub fn release(&mut self, button: Button) -> Result<&mut Self> {
        self.button(match button {
            Button::Secondary => InjectedInputMouseOptions::RightUp,
            Button::Middle => InjectedInputMouseOptions::MiddleUp,
        })?;
        Ok(self)
    }

    /// A press and a release, without moving between them. What a `Tapped` is made of.
    pub fn tap(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.down(at)?.up()
    }

    /// Walks a path, one sample per call, waiting `rate` between them.
    ///
    /// One call per sample rather than one call per path, and that is measured rather than
    /// stylistic — see this module's documentation. The wait is a high-resolution timer or
    /// the compositor clock, never `thread::sleep`, whose resolution is coarser than the
    /// rates under test.
    pub fn polyline(&mut self, points: &[Point], rate: Rate) -> Result<&mut Self> {
        if points.is_empty() {
            return Ok(self);
        }
        for point in points {
            self.move_to(*point)?;
            self.injector.pace.wait(rate)?;
        }
        Ok(self)
    }

    /// Waits without injecting, so a press becomes a hold.
    pub fn hold(&mut self, duration: core::time::Duration) -> Result<&mut Self> {
        self.injector.pace.sleep(duration)?;
        Ok(self)
    }

    /// Vertical wheel, in notches. Positive is away from the user.
    pub fn wheel(&mut self, notches: f32) -> Result<&mut Self> {
        self.wheel_axis(InjectedInputMouseOptions::Wheel, notches)
    }

    /// Horizontal wheel, in notches. Positive is to the right.
    pub fn hwheel(&mut self, notches: f32) -> Result<&mut Self> {
        self.wheel_axis(InjectedInputMouseOptions::HWheel, notches)
    }

    /// Where the last sample was placed, in the injector's space.
    #[must_use]
    pub const fn at(&self) -> Option<Point> {
        self.at
    }

    fn button(&mut self, options: InjectedInputMouseOptions) -> Result<()> {
        inject(
            self.injector.injection.injector(),
            vec![sample(options, 0, 0, 0)?],
        )
    }

    fn wheel_axis(&mut self, axis: InjectedInputMouseOptions, notches: f32) -> Result<&mut Self> {
        let delta = (notches * WHEEL_DELTA as f32).round() as i32;
        inject(
            self.injector.injection.injector(),
            vec![sample(axis, 0, 0, delta as u32)?],
        )?;
        Ok(self)
    }
}

impl Drop for MouseStream<'_> {
    /// Ends a contact the caller left down.
    ///
    /// A test that fails mid-drag would otherwise leave the button held for the rest of the
    /// session, and every later test in the same process would run against a machine with the
    /// mouse pressed.
    fn drop(&mut self) {
        if self.down
            && let Ok(one) = sample(InjectedInputMouseOptions::LeftUp, 0, 0, 0)
        {
            _ = inject(self.injector.injection.injector(), vec![one]);
        }
    }
}

/// One axis of [`Desktop::normalize`].
const fn normalize_axis(value: i32, origin: i32, extent: i32) -> i32 {
    (((value - origin) as i64 * 2 + 1) * 32768 / extent as i64) as i32
}

/// One mouse sample.
fn sample(
    options: InjectedInputMouseOptions,
    dx: i32,
    dy: i32,
    data: u32,
) -> Result<Option<InjectedInputMouseInfo>> {
    let info =
        InjectedInputMouseInfo::new().map_err(|e| Error::call("InjectedInputMouseInfo::new", e))?;
    let fill = || -> windows_core::Result<()> {
        info.SetMouseOptions(options)?;
        info.SetDeltaX(dx)?;
        info.SetDeltaY(dy)?;
        info.SetMouseData(data)?;
        // Zero, so the platform stamps the sample as it takes it. The offset only means
        // something inside a batch, and a batch is not what this stream sends.
        info.SetTimeOffsetInMilliseconds(0)
    };
    fill().map_err(|e| Error::call("InjectedInputMouseInfo", e))?;
    Ok(Some(info))
}

fn inject(injector: &InputInjector, batch: Vec<Option<InjectedInputMouseInfo>>) -> Result<()> {
    if std::env::var("PROBE_MOUSE").is_ok() {
        let opts: Vec<u32> = batch
            .iter()
            .filter_map(|s| s.as_ref())
            .filter_map(|s| s.MouseOptions().ok())
            .map(|o| o.0)
            .collect();
        eprintln!(
            "  inject {} samples, options {:x?}",
            batch.len(),
            &opts[..opts.len().min(3)]
        );
    }
    injector
        .InjectMouseInput(&IIterable::<InjectedInputMouseInfo>::from(batch))
        .map_err(|e| Error::call("InjectMouseInput", e))
}

/// Where the cursor actually is.
fn cursor() -> Result<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: the destination is a stack local the call writes back through.
    unsafe { GetCursorPos(&mut point) }
        .ok()
        .map_err(|e| Error::call("GetCursorPos", e))?;
    Ok((point.x, point.y))
}

/// Reads the cursor until it reaches `target` or a deadline passes, and answers with where it
/// actually got to.
///
/// Injection is asynchronous — the sample crosses the raw input thread — so reading the cursor
/// in the same breath as injecting reads the position from before the move. This is a read
/// loop and not a wait: it adds no input, and it is used only while proving the mapping.
fn settle(target: (i32, i32)) -> Result<(i32, i32)> {
    let deadline = std::time::Instant::now() + core::time::Duration::from_millis(250);
    loop {
        let at = cursor()?;
        if at == target || std::time::Instant::now() >= deadline {
            return Ok(at);
        }
        std::thread::yield_now();
    }
}
