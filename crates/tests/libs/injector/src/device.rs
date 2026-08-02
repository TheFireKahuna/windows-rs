//! A synthetic pointer device, and the one thing that must happen when a stream ends.
//!
//! A device that outlives its stream is not merely untidy. A live synthetic device is a
//! device the system believes is attached, and the v1 one was observed emitting a
//! continuous no-op legacy mouse move at the resting cursor position for as long as it
//! existed — which lands in the count of the very thing a legacy-message test asserts.
//! So the handle is owned, and `Drop` is what destroys it.

use crate::bindings::*;
use crate::late::Late;
use crate::{Error, Result};

/// An owned `HSYNTHETICPOINTERDEVICE`.
pub(crate) struct Device {
    handle: HSYNTHETICPOINTERDEVICE,
}

impl Device {
    /// Creates a device of `kind` with `max` contacts, sized `width_mm` × `height_mm`.
    ///
    /// The physical size is not optional for a touchpad and is what v1 could not express,
    /// which is the whole reason this crate resolves v2 by hand. It is stated in himetric
    /// — hundredths of a millimetre — because that is the unit the parameter block and the
    /// injected sample's `ptHimetricLocation` both use, so a size and a position stated in
    /// the same unit cannot disagree.
    pub(crate) fn new(
        late: &Late,
        kind: POINTER_INPUT_TYPE,
        max: u32,
        feedback: POINTER_FEEDBACK_MODE,
        size_mm: Option<(f32, f32)>,
        gesture_only: bool,
    ) -> Result<Self> {
        let (width, height) = size_mm.map_or((0, 0), |(w, h)| {
            ((w * 100.0).round() as u32, (h * 100.0).round() as u32)
        });
        let mut options = SDCO_NONE;
        if size_mm.is_some() {
            options |= SDCO_PHYSICAL_SIZE;
        }
        if gesture_only {
            options |= SDCO_TOUCHPAD_GESTURE_ONLY;
        }
        let handle = late.create_device(&SYNTHETIC_DEVICE_CREATION_PARAMS {
            pointerType: kind,
            maxCount: max,
            feedbackMode: feedback,
            // Null maps the device to the virtual desktop, which is mandatory for a
            // touchpad and is what a harness that must reach a window on any display wants
            // for the other two.
            hMonitor: core::ptr::null_mut(),
            deviceWidth: width,
            deviceHeight: height,
            options,
        })?;
        Ok(Self { handle })
    }

    pub(crate) const fn handle(&self) -> HSYNTHETICPOINTERDEVICE {
        self.handle
    }

    /// Injects one frame: every contact the device currently has, in order.
    pub(crate) fn inject(&self, frame: &[POINTER_TYPE_INFO]) -> Result<()> {
        // SAFETY: `handle` is live for the life of this value, and the slice is a
        // contiguous run of fully initialized records of the declared length.
        unsafe { InjectSyntheticPointerInput(self.handle, frame.as_ptr(), frame.len() as u32) }
            .ok()
            .map_err(|e| Error::call("InjectSyntheticPointerInput", e))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: created by this type, destroyed exactly once.
        unsafe { DestroySyntheticPointerDevice(self.handle) }
    }
}
