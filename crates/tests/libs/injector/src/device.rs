//! An owned synthetic pointer device, destroyed when its stream ends.
//!
//! A live synthetic device is one the system believes is attached: a v1 device emits a
//! continuous no-op legacy mouse move at the resting cursor position for as long as it
//! exists, which lands in the count a legacy-message test asserts on. `Drop` destroys the
//! handle, so no device outlives the stream that created it.

use crate::bindings::*;
use crate::late::Late;
use crate::{Error, Result};

/// An owned `HSYNTHETICPOINTERDEVICE`.
pub(crate) struct Device {
    handle: HSYNTHETICPOINTERDEVICE,
}

impl Device {
    /// Creates a device of `kind` with `max` contacts, `size_mm` millimetres across when a
    /// size is given, in `feedback` mode, gesture-only when `gesture_only` is set.
    ///
    /// A touchpad must state a physical size, which `CreateSyntheticPointerDevice` v1 has no
    /// parameter for. The size is converted to himetric — hundredths of a millimetre — which
    /// is the unit the parameter block and the injected sample's `ptHimetricLocation` share,
    /// so a size and a position cannot be stated in different units.
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
            // Null maps the device to the whole virtual desktop: mandatory for a touchpad,
            // and what lets the other device kinds reach a window on any display.
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
        // SAFETY: `handle` is destroyed only in `Drop`, so it is live for the life of this
        // value; `frame` is a slice, so it is a contiguous run of initialized records and
        // the count passed is its own length.
        unsafe { InjectSyntheticPointerInput(self.handle, frame.as_ptr(), frame.len() as u32) }
            .ok()
            .map_err(|e| Error::call("InjectSyntheticPointerInput", e))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: `Device::new` is the only producer of this handle and the value is not
        // `Copy`, so this is the one destroy for it.
        unsafe { DestroySyntheticPointerDevice(self.handle) }
    }
}
