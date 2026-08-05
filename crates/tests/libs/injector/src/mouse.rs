//! The mouse stream, and the only device here that shares its state with the user.
//!
//! Two properties come from the platform:
//!
//! * A mouse sample carries no pixel position. `InjectedInputMouseInfo` states an absolute
//!   coordinate normalized to a 16-bit grid over the virtual desktop, so on a wide desktop a
//!   whole pixel spans a fraction of one normalized step and the mapping back is not the
//!   identity. [`Desktop::normalize`] inverts it in closed form, aiming at the centre of the
//!   interval that lands on the wanted pixel, and [`Desktop::verified`] checks that at three
//!   points across the desktop before any test sample is placed. A miss is
//!   [`Error::Calibration`] rather than a correction: correcting aim against `GetCursorPos`
//!   injects up to three moves per logical one, and a drag's integral counts every one.
//! * There is one system cursor, and this crate is not its only source. No injection API
//!   produces a `PT_MOUSE` stream on a device of its own — pen, touch and touchpad each get
//!   one — so a hand on a real mouse adds travel to a drag whose total is under test. The
//!   exactness contract is therefore asserted on touch, which has its own device. Mouse
//!   covers `PT_MOUSE`, and its own integral is a diagnostic.
//!
//! A path is one sample per call, paced by a high-resolution timer. `InjectMouseInput` takes
//! a sequence whose samples each carry an offset in milliseconds, and that sequence is
//! coalesced. Measured on 26200 against a pumping window: a 40-sample zigzag handed over as
//! three batched calls arrives as 16 messages carrying 353 DIPs of a 557-DIP path, while the
//! same path injected one sample per call arrives whole.
//!
//! Two further limits on that sequence: it caps at sixteen samples per call — seventeen is
//! `E_INVALIDARG`, and the message names neither the parameter nor the limit — and the samples
//! it accepts are coalesced as above.
//!
//! An absolute move to the pixel the cursor is already on produces no input at all. A path
//! with a repeated point therefore arrives one sample short, which reads the same as a stack
//! that dropped one.

use windows_collections::IIterable;

use crate::bindings::*;
use crate::space::Point;
use crate::{Error, Injector, Rate, Result};

/// Holds the virtual-desktop extent an absolute mouse coordinate is normalized against,
/// verified to place a sample on the exact pixel asked for.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Desktop {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

impl Desktop {
    fn read() -> Self {
        // SAFETY: each call takes a metric index by value and returns a number; no pointer
        // crosses the boundary.
        unsafe {
            Self {
                left: GetSystemMetrics(SM_XVIRTUALSCREEN),
                top: GetSystemMetrics(SM_YVIRTUALSCREEN),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1),
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1),
            }
        }
    }

    /// Returns the normalized coordinate that lands on exactly the screen pixel `(x, y)`.
    ///
    /// The system maps a normalized value back by scaling it into the desktop's extent and
    /// truncating, so a range of normalized values lands on each pixel. Aiming at the centre
    /// of that range — `(p + ½) · 65536 / extent` — leaves the whole half-interval as margin,
    /// which on a 7680-pixel desktop is about four normalized steps either way. That margin
    /// is why one verification covers the desktop rather than one per pixel.
    const fn normalize(&self, x: i32, y: i32) -> (i32, i32) {
        (
            normalize_axis(x, self.left, self.width),
            normalize_axis(y, self.top, self.height),
        )
    }

    /// Reads the desktop and verifies its mapping, or returns [`Error::Calibration`].
    ///
    /// Places three probes — near the origin, at the centre, and near the far corner — where
    /// a rounding rule off by a step or by a scale shows. The cursor is restored afterwards,
    /// and the whole sequence runs before the caller's first stream.
    ///
    /// Each probe is retried rather than corrected. The cursor is shared, so a competing
    /// device moving it between the injection and the read produces a miss indistinguishable
    /// from a wrong mapping; a wrong mapping misses on every attempt and a competing device
    /// does not.
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
                // Put the cursor back before reporting, so a failing run does not leave the
                // pointer in a corner.
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

    /// Injects one absolute move, landing on the screen pixel `(x, y)`.
    fn place(&self, injector: &InputInjector, x: i32, y: i32) -> Result<()> {
        let (nx, ny) = self.normalize(x, y);
        inject(injector, vec![sample(MOVE, nx, ny, 0)?])
    }
}

/// The four options every absolute move carries.
///
/// `MoveNoCoalesce` keeps each injected move a distinct event, as a high-rate physical mouse
/// produces; without it the system coalesces them and shortens the integral a drag-fidelity
/// assertion is written on. `VirtualDesk` makes the coordinate span the whole virtual
/// desktop, so a secondary monitor is reachable.
const MOVE: InjectedInputMouseOptions = InjectedInputMouseOptions(
    InjectedInputMouseOptions::Move.0
        | InjectedInputMouseOptions::MoveNoCoalesce.0
        | InjectedInputMouseOptions::Absolute.0
        | InjectedInputMouseOptions::VirtualDesk.0,
);

/// Names a mouse button other than the one that starts and ends the contact.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Button {
    /// The button a context menu answers to. Pressed while nothing is down, it raises
    /// `RightTapped`.
    Secondary,
    /// The wheel button.
    Middle,
}

/// A mouse, aimed at the injector's space.
///
/// The primary button is absent from [`Button`] because it is the contact: a `WM_POINTERDOWN`
/// is a contact starting, and a second button pressed while the first is held arrives as an
/// update carrying a changed button set rather than as a second down. So [`down`](Self::down)
/// and [`up`](Self::up) start and end the contact, and [`press`](Self::press) /
/// [`release`](Self::release) change the buttons within one.
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
        // `Injector::mouse` sets this before constructing the stream, and nothing clears it.
        self.injector
            .desktop
            .expect("a mouse stream exists only after its desktop mapping was verified")
    }

    /// Moves to a point, landing on the exact pixel it maps to.
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
    /// The move is injected before the press, so the stack resolves the press against a
    /// position it has already seen the pointer reach.
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

    /// Presses and releases without moving between them, which is what raises `Tapped`.
    pub fn tap(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.down(at)?.up()
    }

    /// Walks a path, injecting one sample per call and waiting `rate` between them.
    ///
    /// One call per sample rather than one call per path, because `InjectMouseInput`
    /// coalesces a batched sequence. The wait is a high-resolution timer or the compositor
    /// clock, never `thread::sleep`, whose resolution is coarser than the rates under test.
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
    /// Without this, a drive that fails mid-drag leaves the button held for the rest of the
    /// session and every later test in the process runs with the mouse pressed.
    fn drop(&mut self) {
        if self.down
            && let Ok(one) = sample(InjectedInputMouseOptions::LeftUp, 0, 0, 0)
        {
            _ = inject(self.injector.injection.injector(), vec![one]);
        }
    }
}

/// Normalizes one axis for [`Desktop::normalize`].
const fn normalize_axis(value: i32, origin: i32, extent: i32) -> i32 {
    (((value - origin) as i64 * 2 + 1) * 32768 / extent as i64) as i32
}

/// Builds one mouse sample.
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
        // Zero, so the platform stamps the sample as it takes it. The offset is meaningful
        // only within a batched sequence, which this stream does not send.
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

/// Returns the cursor's current screen position.
fn cursor() -> Result<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: the destination is a stack local the call writes back through.
    unsafe { GetCursorPos(&mut point) }
        .ok()
        .map_err(|e| Error::call("GetCursorPos", e))?;
    Ok((point.x, point.y))
}

/// Returns the position the cursor reached, polling until it matches `target` or 250 ms pass.
///
/// Injection is asynchronous — the sample crosses the raw input thread — so reading the cursor
/// immediately after injecting reads the position from before the move. This loop injects
/// nothing, and runs only while verifying the mapping.
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
