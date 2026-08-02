//! Rotary input — `RadialController`.
//!
//! Surface Dial and compatible wheels reach a desktop application through
//! `IRadialControllerInterop::CreateForWindow` and
//! `IRadialControllerConfigurationInterop::GetForWindow`: HWND-based, no `CoreWindow`, no App
//! SDK. Unusually apt here, since this application's signature control *is* a knob.
//!
//! **The dial is a delta source**, so it lands on the gesture seam and drives the same value
//! path a knob drag does — not the pointer seam. A control that declares no
//! [`RotaryDecl`](crate::gesture::RotaryDecl) is simply not a dial target.
//!
//! The on-screen contact is the interesting case: `ScreenContactStarted` carries a position,
//! so **the flat hit array picks the target underneath it exactly as a finger would** — one
//! authority, extended rather than forked.

use crate::bindings::*;
use crate::gesture::RotaryDecl;
use crate::input::Service;
use std::cell::RefCell;
use std::rc::Rc;
use windows_core::{EventRevoker, Interface, Result};
use windows_scene::Point;

fn closed() -> windows_core::Error {
    windows_core::Error::new(
        windows_core::HRESULT(0x8007_0006u32 as i32),
        "the window is closed",
    )
}

/// What the dial did.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Rotation {
    /// Turned. `degrees` is signed; clockwise is positive.
    Turned {
        degrees: f64,
        /// Where the dial is resting on screen, in client DIPs, if it is on screen at all.
        contact: Option<Point>,
    },
    /// Pressed, released, or clicked — press and release together with nothing between.
    Button {
        pressed: bool,
        contact: Option<Point>,
    },
    Clicked {
        contact: Option<Point>,
    },
    /// The dial is now, or is no longer, ours to read.
    Control {
        acquired: bool,
    },
    /// An on-screen dial arrived, moved or left.
    Contact {
        at: Option<Point>,
    },
}

/// Where the controller's events land, drained by the service tick like everything else.
#[derive(Clone)]
pub struct RotaryEvents {
    queue: Rc<RefCell<Vec<Rotation>>>,
    /// **A detent is as discrete as a keystroke**, so it asks for the tick rather than
    /// waiting for the display. The gate is the window's, shared with the doorbell, so a
    /// dial turned during a drag costs no second post.
    service: Rc<Service>,
}

impl RotaryEvents {
    fn new(service: Rc<Service>) -> Self {
        Self {
            queue: Rc::new(RefCell::new(Vec::new())),
            service,
        }
    }

    fn push(&self, event: Rotation) {
        if let Ok(mut queue) = self.queue.try_borrow_mut() {
            queue.push(event);
        }
        self.service.now();
    }

    /// Takes everything raised since the last drain.
    pub fn drain(&self, out: &mut Vec<Rotation>) {
        if let Ok(mut queue) = self.queue.try_borrow_mut() {
            out.append(&mut queue);
        }
    }
}

/// The window's radial controller.
///
/// One per window, held for the window's life. Its resolution and haptics are restated per
/// focused target, because a detent that matches a knob's step is the difference between a
/// dial that feels like the control and one that feels like a scroll wheel.
pub struct Rotary {
    controller: RadialController,
    configuration: RadialControllerConfiguration,
    events: RotaryEvents,
    /// Held for their `Drop`, which revokes.
    _revokers: Vec<EventRevoker>,
    /// The scale the contact position is reported in.
    scale: f32,
}

impl Rotary {
    /// Creates the controller for `window`.
    ///
    /// # Errors
    ///
    /// The window is closed, no dial is present, or the interop factory refused it.
    pub fn new(window: &windows_window::Window, service: Rc<Service>) -> Result<Self> {
        let scale = window.scale().ok_or_else(closed)?;
        let hwnd = window.hwnd();
        let interop = windows_core::factory::<RadialController, IRadialControllerInterop>()?;
        // SAFETY: `hwnd` is live for the call, and the interface asked for is the one the
        // returned object implements.
        let controller: RadialController = unsafe { interop.CreateForWindow(hwnd)? };
        let config_interop = windows_core::factory::<
            RadialControllerConfiguration,
            IRadialControllerConfigurationInterop,
        >()?;
        // SAFETY: as above.
        let configuration: RadialControllerConfiguration =
            unsafe { config_interop.GetForWindow(hwnd)? };

        let events = RotaryEvents::new(service);
        let mut revokers = Vec::with_capacity(6);

        let sink = events.clone();
        revokers.push(controller.RotationChanged(move |_, args| {
            if let Some(args) = args.as_ref()
                && let Ok(degrees) = args.RotationDeltaInDegrees()
            {
                sink.push(Rotation::Turned {
                    degrees,
                    contact: contact_point(args.Contact().ok().as_ref(), scale),
                });
            }
        })?);

        let sink = events.clone();
        revokers.push(controller.ButtonClicked(move |_, args| {
            let contact = args.as_ref().and_then(|args| args.Contact().ok());
            sink.push(Rotation::Clicked {
                contact: contact_point(contact.as_ref(), scale),
            });
        })?);

        // The three button transitions arrived after the class did, so they live on its
        // second interface and are reached by cast.
        let buttons: IRadialController2 = controller.cast()?;
        let sink = events.clone();
        revokers.push(buttons.ButtonPressed(move |_, _| {
            sink.push(Rotation::Button {
                pressed: true,
                contact: None,
            });
        })?);

        let sink = events.clone();
        revokers.push(buttons.ButtonReleased(move |_, _| {
            sink.push(Rotation::Button {
                pressed: false,
                contact: None,
            });
        })?);

        let sink = events.clone();
        revokers.push(controller.ControlAcquired(move |_, _| {
            sink.push(Rotation::Control { acquired: true });
        })?);

        let sink = events.clone();
        revokers.push(controller.ControlLost(move |_, _| {
            sink.push(Rotation::Control { acquired: false });
        })?);

        let sink = events.clone();
        revokers.push(controller.ScreenContactStarted(move |_, args| {
            let contact = args.as_ref().and_then(|args| args.Contact().ok());
            sink.push(Rotation::Contact {
                at: contact_point(contact.as_ref(), scale),
            });
        })?);

        let sink = events.clone();
        revokers.push(controller.ScreenContactContinued(move |_, args| {
            let contact = args.as_ref().and_then(|args| args.Contact().ok());
            sink.push(Rotation::Contact {
                at: contact_point(contact.as_ref(), scale),
            });
        })?);

        let sink = events.clone();
        revokers.push(controller.ScreenContactEnded(move |_, _| {
            sink.push(Rotation::Contact { at: None });
        })?);

        Ok(Self {
            controller,
            configuration,
            events,
            _revokers: revokers,
            scale,
        })
    }

    /// The queue the pacer tick drains.
    #[must_use]
    pub fn events(&self) -> &RotaryEvents {
        &self.events
    }

    /// Restates the window's scale after a DPI change.
    pub const fn rescale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// Matches the dial's detents to a target's declared step.
    ///
    /// `UseAutomaticHapticFeedback` is what makes a detent something the user feels, and
    /// matching it to the value step is fidelity no other input device on this machine can
    /// produce.
    pub fn tune(&self, decl: &RotaryDecl) -> Result<()> {
        self.controller
            .SetRotationResolutionInDegrees(decl.resolution_degrees)?;
        self.controller.SetUseAutomaticHapticFeedback(decl.haptics)
    }

    /// Replaces the dial menu's system items with ours.
    ///
    /// The items are a glyph and a label each, from the application's own icon font: an icon
    /// stream would be a second asset path for something that already exists as a glyph.
    pub fn menu(&self, items: &[MenuItem]) -> Result<()> {
        let menu = self.controller.Menu()?;
        let list = menu.Items()?;
        list.Clear()?;
        for item in items {
            let created =
                RadialControllerMenuItem::CreateFromFontGlyph(item.label, item.glyph, item.font)?;
            list.Append(&created)?;
        }
        Ok(())
    }

    /// Restores the system's own menu items.
    pub fn reset_menu(&self) -> Result<()> {
        self.configuration.ResetToDefaultMenuItems()
    }
}

/// One entry in the dial menu.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: &'static str,
    pub glyph: &'static str,
    pub font: &'static str,
}

/// A dial's on-screen contact, in client DIPs.
///
/// The position arrives in **screen** coordinates, so it crosses the same way a finger does:
/// scaled into the space the hit array is built in. The caller resolves it through
/// `Scene::hit`, which is what makes an on-screen dial a first-class contact rather than a
/// second routing path.
fn contact_point(contact: Option<&RadialControllerScreenContact>, scale: f32) -> Option<Point> {
    let contact = contact?;
    let position = contact.Position().ok()?;
    (scale > 0.0).then(|| Point {
        x: position.x / scale,
        y: position.y / scale,
    })
}
