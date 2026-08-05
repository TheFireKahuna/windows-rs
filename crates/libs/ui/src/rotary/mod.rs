//! Reads rotary input — Surface Dial and compatible wheels — through `RadialController`.
//!
//! A desktop application reaches the device through
//! `IRadialControllerInterop::CreateForWindow` and
//! `IRadialControllerConfigurationInterop::GetForWindow`. Both are HWND-based, so neither a
//! `CoreWindow` nor the App SDK is involved.
//!
//! The dial is a delta source, so it lands on the gesture seam and drives the value path a
//! knob drag uses rather than the pointer seam. A control that declares no
//! [`RotaryDecl`](crate::gesture::RotaryDecl) is not a dial target.
//!
//! `ScreenContactStarted` carries a position, so a dial resting on screen picks its target
//! out of the same flat hit array a finger does.

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

/// Reports what the dial did.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Rotation {
    /// Turned. `degrees` is signed; clockwise is positive.
    Turned {
        degrees: f64,
        /// Where the dial is resting on screen, in client DIPs, if it is on screen at all.
        contact: Option<Point>,
    },
    /// Pressed or released.
    Button {
        pressed: bool,
        contact: Option<Point>,
    },
    /// Pressed and released together, with nothing in between.
    Clicked { contact: Option<Point> },
    /// The window acquired or lost control of the dial.
    Control { acquired: bool },
    /// An on-screen dial arrived, moved or left.
    Contact { at: Option<Point> },
}

/// Queues the controller's events until the service tick drains them.
#[derive(Clone)]
pub struct RotaryEvents {
    queue: Rc<RefCell<Vec<Rotation>>>,
    /// Asks for a tick as each event arrives rather than waiting on the display clock. The
    /// gate is the window's, shared with the doorbell, so a dial turned during a drag costs
    /// no second post.
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

    /// Appends every event raised since the last drain to `out`, leaving the queue empty.
    ///
    /// A call that cannot borrow the queue — one re-entering from an event callback — leaves
    /// both the queue and `out` untouched.
    pub fn drain(&self, out: &mut Vec<Rotation>) {
        if let Ok(mut queue) = self.queue.try_borrow_mut() {
            out.append(&mut queue);
        }
    }
}

/// Owns the window's radial controller and the events it raises.
///
/// One per window, held for the window's life. [`Rotary::tune`] restates resolution and
/// haptics per focused target, so the detents match that target's declared step.
pub struct Rotary {
    controller: RadialController,
    configuration: RadialControllerConfiguration,
    events: RotaryEvents,
    /// Held for their `Drop`, which revokes.
    _revokers: Vec<EventRevoker>,
    /// The window's DPI scale, which a screen-space contact position is divided by.
    scale: f32,
}

impl Rotary {
    /// Creates the controller and its configuration for `window`, and registers the handlers
    /// that fill [`RotaryEvents`].
    ///
    /// # Errors
    ///
    /// The window is closed, no dial is present, the interop factory refuses the window, or
    /// an event registration fails.
    pub fn new(window: &windows_window::Window, service: Rc<Service>) -> Result<Self> {
        let scale = window.scale().ok_or_else(closed)?;
        let hwnd = window.hwnd();
        let interop = windows_core::factory::<RadialController, IRadialControllerInterop>()?;
        // SAFETY: `hwnd` belongs to `window`, which is borrowed for the whole call, so the
        // handle cannot be destroyed under it; `RadialController` is the class
        // `CreateForWindow` returns, so the requested interface is one the object implements.
        let controller: RadialController = unsafe { interop.CreateForWindow(hwnd)? };
        let config_interop = windows_core::factory::<
            RadialControllerConfiguration,
            IRadialControllerConfigurationInterop,
        >()?;
        // SAFETY: the same borrowed `hwnd`; `RadialControllerConfiguration` is the class
        // `GetForWindow` returns.
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

        // `ButtonPressed` and `ButtonReleased` live on `IRadialController2` rather than the
        // class's default interface, so they are reached by cast.
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

    /// Returns the queue the pacer tick drains.
    #[must_use]
    pub fn events(&self) -> &RotaryEvents {
        &self.events
    }

    /// Restates the window's scale after a DPI change.
    pub const fn rescale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// Matches the dial's detents to the step `decl` declares.
    ///
    /// Sets the rotation resolution from `decl.resolution_degrees` and turns automatic haptic
    /// feedback on or off from `decl.haptics`, so each detent the user feels advances the
    /// target by one step. Applies to the whole controller until the next call.
    ///
    /// # Errors
    ///
    /// The controller rejects the resolution or the feedback setting.
    pub fn tune(&self, decl: &RotaryDecl) -> Result<()> {
        self.controller
            .SetRotationResolutionInDegrees(decl.resolution_degrees)?;
        self.controller.SetUseAutomaticHapticFeedback(decl.haptics)
    }

    /// Replaces every item in the dial menu with `items`.
    ///
    /// Each item is built from a font glyph and a label, so the menu draws from the same icon
    /// font the application already carries.
    ///
    /// # Errors
    ///
    /// The menu cannot be read, cleared, or an item cannot be created or appended.
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

    /// Restores the system's own dial menu items.
    ///
    /// # Errors
    ///
    /// The configuration object refuses the reset.
    pub fn reset_menu(&self) -> Result<()> {
        self.configuration.ResetToDefaultMenuItems()
    }
}

/// Describes one entry in the dial menu.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    /// The text the menu shows for the entry.
    pub label: &'static str,
    /// The glyph shown with the label.
    pub glyph: &'static str,
    /// The font family the glyph is taken from.
    pub font: &'static str,
}

/// Converts a dial's on-screen contact position into client DIPs.
///
/// The position arrives in screen coordinates and is divided by `scale` into the space the hit
/// array is built in, so a caller resolves it through `Scene::hit` exactly as it would a
/// finger. Returns `None` when there is no contact, the position cannot be read, or `scale` is
/// not positive.
fn contact_point(contact: Option<&RadialControllerScreenContact>, scale: f32) -> Option<Point> {
    let contact = contact?;
    let position = contact.Position().ok()?;
    (scale > 0.0).then(|| Point {
        x: position.x / scale,
        y: position.y / scale,
    })
}
