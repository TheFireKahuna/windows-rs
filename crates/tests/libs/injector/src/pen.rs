//! The pen stream.
//!
//! A pen is the one device here that **hovers**: it reports in range without being in
//! contact, so pen users get the hover affordances touch users do not, and a stack that
//! derived hover presence from contact alone would light nothing for them. That is why
//! [`hover_to`](PenStream::hover_to) exists beside [`move_to`](PenStream::move_to) rather
//! than being the same call with a flag.
//!
//! It also carries what a pen has instead of a second mouse button — pressure, tilt, the
//! barrel button and the inverted (eraser) end — and those are state on the stream rather
//! than arguments to every sample, because that is how they behave: a pen tilted at 20°
//! stays tilted until it moves.
//!
//! **A pen stream has a lifecycle, and skipping it is the failure that looks like a broken
//! API.** `InitializePenInjection` must precede any pen sample; without it every call is
//! accepted, returns success and delivers nothing. That is owned by
//! [`Injection`](crate::injection::Injection) and released when the last pen stream drops.
//!
//! [`Phase`] enumerates the five frames an injected pointer is allowed to be, and there is
//! no way to state a sixth — the same closed set injected touch has.

use crate::bindings::*;
use crate::space::Point;
use crate::{Error, Injector, Rate, Result};

/// The five frames an injected pointer is allowed to be.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// In range, not touching. Hover starts or continues.
    Hover,
    /// The tip lands.
    Down,
    /// The tip moves while down.
    Drag,
    /// The tip lifts, staying in range.
    Lift,
    /// Out of range entirely.
    Leave,
}

impl Phase {
    const fn options(self) -> InjectedInputPointerOptions {
        InjectedInputPointerOptions(match self {
            Self::Hover => {
                InjectedInputPointerOptions::InRange.0 | InjectedInputPointerOptions::Update.0
            }
            Self::Down => {
                InjectedInputPointerOptions::InRange.0
                    | InjectedInputPointerOptions::InContact.0
                    | InjectedInputPointerOptions::PointerDown.0
            }
            Self::Drag => {
                InjectedInputPointerOptions::InRange.0
                    | InjectedInputPointerOptions::InContact.0
                    | InjectedInputPointerOptions::Update.0
            }
            Self::Lift => {
                InjectedInputPointerOptions::InRange.0 | InjectedInputPointerOptions::PointerUp.0
            }
            Self::Leave => InjectedInputPointerOptions::PointerUp.0,
        })
    }
}

/// A pen, aimed at the injector's space.
pub struct PenStream<'a> {
    injector: &'a mut Injector,
    in_range: bool,
    in_contact: bool,
    at: (i32, i32),
    pressure: f64,
    tilt: (i32, i32),
    barrel: bool,
    inverted: bool,
}

impl<'a> PenStream<'a> {
    pub(crate) fn open(injector: &'a mut Injector) -> Result<Self> {
        injector.injection.acquire_pen()?;
        Ok(Self {
            injector,
            in_range: false,
            in_contact: false,
            at: (0, 0),
            // Half scale, so a stream that never states a pressure still reports one a
            // pressure-sensitive control can act on.
            pressure: 0.5,
            tilt: (0, 0),
            barrel: false,
            inverted: false,
        })
    }

    /// How hard the tip is pressed, from 0 to 1.
    pub fn pressure(&mut self, pressure: f32) -> &mut Self {
        self.pressure = f64::from(pressure.clamp(0.0, 1.0));
        self
    }

    /// Tilt from vertical, in degrees, on each axis. Each is −90 to 90.
    pub fn tilt(&mut self, x: i32, y: i32) -> &mut Self {
        self.tilt = (x.clamp(-90, 90), y.clamp(-90, 90));
        self
    }

    /// Whether the barrel button is held. A pen's secondary button.
    pub fn barrel(&mut self, held: bool) -> &mut Self {
        self.barrel = held;
        self
    }

    /// Whether the pen is inverted — the eraser end.
    pub fn inverted(&mut self, inverted: bool) -> &mut Self {
        self.inverted = inverted;
        self
    }

    /// Brings the pen into range at a point, without touching down.
    pub fn hover_to(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.at = self.injector.space.to_px(at);
        self.in_range = true;
        self.in_contact = false;
        self.emit(Phase::Hover)
    }

    /// Touches down at a point.
    pub fn down(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.at = self.injector.space.to_px(at);
        // A pen that lands without ever having hovered still has to enter range first: the
        // state machine has no transition from nothing to contact.
        if !self.in_range {
            self.in_range = true;
            self.emit(Phase::Hover)?;
        }
        self.in_contact = true;
        self.emit(Phase::Down)
    }

    /// Moves, keeping whatever contact state the pen already has.
    pub fn move_to(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        if !self.in_range {
            return Err(Error::NotDown);
        }
        self.at = self.injector.space.to_px(at);
        self.emit(if self.in_contact {
            Phase::Drag
        } else {
            Phase::Hover
        })
    }

    /// Lifts the tip, staying in range — which is what a pen does, and what makes the
    /// following samples hover rather than nothing.
    pub fn up(&mut self) -> Result<&mut Self> {
        if !self.in_contact {
            return Err(Error::NotDown);
        }
        self.in_contact = false;
        self.emit(Phase::Lift)
    }

    /// Takes the pen out of range entirely.
    pub fn leave(&mut self) -> Result<&mut Self> {
        if !self.in_range {
            return Ok(self);
        }
        if self.in_contact {
            self.up()?;
        }
        self.in_range = false;
        self.emit(Phase::Leave)
    }

    /// Down and up at one point, without moving.
    pub fn tap(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.down(at)?.up()
    }

    /// Walks a path, one sample per point, at `rate`, keeping the current contact state.
    pub fn polyline(&mut self, points: &[Point], rate: Rate) -> Result<&mut Self> {
        for point in points {
            self.move_to(*point)?;
            self.injector.pace.wait(rate)?;
        }
        Ok(self)
    }

    /// Waits without injecting.
    pub fn hold(&mut self, duration: core::time::Duration) -> Result<&mut Self> {
        self.injector.pace.sleep(duration)?;
        Ok(self)
    }

    fn emit(&mut self, phase: Phase) -> Result<&mut Self> {
        let mut buttons = InjectedInputPenButtons::None.0;
        if self.barrel {
            buttons |= InjectedInputPenButtons::Barrel.0;
        }
        if self.inverted {
            // Inverted is the end that is presented; eraser is what that end does. A pen
            // turned over reports both, and a control reading only one of them either never
            // erases or erases while hovering.
            buttons |= InjectedInputPenButtons::Inverted.0 | InjectedInputPenButtons::Eraser.0;
        }

        let info =
            InjectedInputPenInfo::new().map_err(|e| Error::call("InjectedInputPenInfo::new", e))?;
        let fill = || -> windows_core::Result<()> {
            info.SetPointerInfo(InjectedInputPointerInfo {
                pointer_id: 0,
                pointer_options: phase.options(),
                pixel_location: InjectedInputPoint {
                    position_x: self.at.0,
                    position_y: self.at.1,
                },
                time_offset_in_milliseconds: 0,
                performance_count: 0,
            })?;
            info.SetPenButtons(InjectedInputPenButtons(buttons))?;
            info.SetPressure(self.pressure)?;
            info.SetTiltX(self.tilt.0)?;
            info.SetTiltY(self.tilt.1)?;
            info.SetPenParameters(InjectedInputPenParameters(
                InjectedInputPenParameters::Pressure.0
                    | InjectedInputPenParameters::TiltX.0
                    | InjectedInputPenParameters::TiltY.0,
            ))
        };
        fill().map_err(|e| Error::call("InjectedInputPenInfo", e))?;
        self.injector
            .injection
            .injector()
            .InjectPenInput(&info)
            .map_err(|e| Error::call("InjectPenInput", e))?;
        Ok(self)
    }
}

impl Drop for PenStream<'_> {
    /// Takes the pen out of range if the caller left it there, then gives the initialize back.
    fn drop(&mut self) {
        if self.in_range {
            if self.in_contact {
                self.in_contact = false;
                _ = self.emit(Phase::Lift);
            }
            self.in_range = false;
            _ = self.emit(Phase::Leave);
        }
        self.injector.injection.release_pen();
    }
}
