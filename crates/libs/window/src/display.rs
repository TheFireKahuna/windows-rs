use crate::bindings::*;
use core::cell::{Cell, RefCell};
use std::rc::Rc;
use windows_color::{DisplayCapability, Gamut};
use windows_core::{EventRevoker, Interface, Result};

/// The window's `DisplayInformation`, its one handler, and the last capability read.
///
/// WinRT rather than DXGI because `CurrentAdvancedColorKind` is the only three-way
/// discriminator: `DXGI_OUTPUT_DESC1`'s colour space carries two values for three modes, so
/// a wide-gamut panel reads there as plain SDR.
pub(crate) struct DisplayColor {
    info: IDisplayInformation5,
    /// Explicitly takeable: [`close`](Self::close) must revoke while the `HWND` is still
    /// valid, and a [`Subscription`] the application holds past that point keeps this alive.
    revoker: RefCell<Option<EventRevoker>>,
    current: Cell<DisplayCapability>,
    /// The application's callback and the identity of the subscription that installed it.
    slot: RefCell<Option<(u64, Rc<dyn Fn(DisplayCapability)>)>>,
    /// Monotonic, so a stale `Subscription` dropping after it was replaced cannot clear its
    /// successor's callback.
    next_id: Cell<u64>,
}

impl DisplayColor {
    /// Attaches to `hwnd`, reads the initial capability, and subscribes. `message` is posted
    /// to `hwnd` on every change.
    ///
    /// A desktop app has no `CoreWindow`, so this goes through the interop factory rather
    /// than `GetForCurrentView`. `GetForWindow` requires a `DispatcherQueue` on the calling
    /// thread, and the members live on the versioned `IDisplayInformation5`.
    pub(crate) fn new(hwnd: HWND, message: u32) -> Result<Self> {
        let interop: IDisplayInformationStaticsInterop =
            windows_core::factory::<DisplayInformation, IDisplayInformationStaticsInterop>()?;
        // SAFETY: `hwnd` is live for the call.
        let information: DisplayInformation = unsafe { interop.GetForWindow(hwnd)? };
        let info: IDisplayInformation5 = information.cast()?;

        // Read before subscribing. The reverse order can deliver a callback against a
        // capability nobody has read yet; a change arriving between the two only posts a
        // message the window services with a fresh read.
        //
        // An unrecognised kind falls back rather than failing closed as `refresh` does: there
        // is no previous capability to keep, and SDR is the mode whose transform is a no-op,
        // so it is the one wrong answer that cannot make a colour worse than untransformed.
        let initial = capability(&info)?.unwrap_or(DisplayCapability::Sdr);

        // The handler does nothing but post, so the re-read, the mapping and the
        // application's callback all run from the ordinary pump on a stack this crate owns
        // rather than inside a projection frame it may not re-enter.
        let target = Post(hwnd, message);
        let revoker = info.AdvancedColorInfoChanged(move |_, _| target.post())?;

        Ok(Self {
            info,
            revoker: RefCell::new(Some(revoker)),
            current: Cell::new(initial),
            slot: RefCell::new(None),
            next_id: Cell::new(0),
        })
    }

    /// The last capability read. No call crosses into WinRT.
    pub(crate) fn capability(&self) -> DisplayCapability {
        self.current.get()
    }

    /// Re-reads and notifies the application if it moved.
    ///
    /// Fails closed: an `AdvancedColorKind` this build does not recognise, or a read that
    /// errors because the monitor went away mid-hop, keeps the previous capability. A wrong
    /// guess is a screen at visibly the wrong brightness; a stale value is merely late.
    pub(crate) fn refresh(&self) {
        let Ok(Some(next)) = capability(&self.info) else {
            return;
        };
        if next == self.current.get() {
            return;
        }
        self.current.set(next);
        // Cloned out and the borrow released first: the callback may re-subscribe.
        let handler = self.slot.borrow().as_ref().map(|(_, f)| Rc::clone(f));
        if let Some(handler) = handler {
            handler(next);
        }
    }

    /// Installs the application's callback, replacing whatever was there.
    ///
    /// `AdvancedColorInfoChanged` removes any previously registered handler, so the system
    /// registration stays this type's alone and the application's is a slot behind it.
    pub(crate) fn subscribe(self: &Rc<Self>, f: Rc<dyn Fn(DisplayCapability)>) -> Subscription {
        let id = self.next_id.get() + 1;
        self.next_id.set(id);
        *self.slot.borrow_mut() = Some((id, f));
        Subscription {
            owner: Rc::clone(self),
            id,
        }
    }

    /// Revokes the system subscription and drops the application's callback.
    ///
    /// `GetForWindow` hooks the window's message loop, so this has to happen on `WM_DESTROY`
    /// while the handle is still valid.
    pub(crate) fn close(&self) {
        let revoker = self.revoker.borrow_mut().take();
        drop(revoker);
        let slot = self.slot.borrow_mut().take();
        drop(slot);
    }

    fn unsubscribe(&self, id: u64) {
        let mut slot = self.slot.borrow_mut();
        if slot.as_ref().is_some_and(|(installed, _)| *installed == id) {
            *slot = None;
        }
    }
}

/// A live registration for display-capability changes. Dropping it stops the callbacks.
///
/// It scopes the application's callback, not the system subscription — the window owns that
/// for its whole life and revokes it on `WM_DESTROY` whether or not this still exists.
pub struct Subscription {
    owner: Rc<DisplayColor>,
    id: u64,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.owner.unsubscribe(self.id);
    }
}

impl core::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Subscription")
            .field("id", &self.id)
            .finish()
    }
}

/// The window handle and the message, carried into the WinRT handler.
struct Post(HWND, u32);

impl Post {
    fn post(&self) {
        // SAFETY: posting is callable from any thread and resolves the handle itself. The
        // registration is revoked on `WM_DESTROY`, so the window is alive whenever this runs.
        unsafe {
            _ = PostMessageW(self.0, self.1, 0, 0);
        }
    }
}

/// Maps one `AdvancedColorInfo` snapshot onto a capability. `None` means a kind this build
/// does not know, which is the caller's signal to keep what it had.
fn capability(info: &IDisplayInformation5) -> Result<Option<DisplayCapability>> {
    let info = info.GetAdvancedColorInfo()?;
    let kind = info.CurrentAdvancedColorKind()?;

    if kind == AdvancedColorKind::StandardDynamicRange {
        // Nothing colour-manages, so there are no primaries worth reading: scRGB is
        // interpreted as sRGB and everything outside the unit range clips.
        return Ok(Some(DisplayCapability::Sdr));
    }

    let gamut = Gamut::from_primaries(
        xy(info.RedPrimary()?),
        xy(info.GreenPrimary()?),
        xy(info.BluePrimary()?),
        xy(info.WhitePoint()?),
    );

    if kind == AdvancedColorKind::WideColorGamut {
        return Ok(Some(DisplayCapability::WideGamut { gamut }));
    }

    if kind == AdvancedColorKind::HighDynamicRange {
        return Ok(Some(DisplayCapability::HighDynamicRange {
            gamut,
            // Read on this arm only: reference white is documented to apply to HDR displays.
            white_nits: info.SdrWhiteLevelInNits()?,
            // The small-area peak, which is the right ceiling while everything authored
            // above diffuse white is small-area. Full-screen extent would want
            // `MaxAverageFullFrameLuminanceInNits`.
            peak_nits: info.MaxLuminanceInNits()?,
        }));
    }

    Ok(None)
}

/// The primaries arrive as 32-bit CIE xy; the matrix inversion behind
/// `Gamut::from_primaries` is where the precision goes.
fn xy(p: Point) -> (f64, f64) {
    (p.x as f64, p.y as f64)
}
