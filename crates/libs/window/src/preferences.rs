//! Reads system animation preferences.

use core::ffi::c_void;
use windows_core::{Error, Result};

windows_core::link!("user32.dll" "system" fn SystemParametersInfoW(action: u32, param: u32, value: *mut c_void, flags: u32) -> i32);

/// Returns whether Windows enables animations inside application windows.
///
/// # Errors
///
/// Returns the system error when `SPI_GETCLIENTAREAANIMATION` fails.
pub fn client_area_animations() -> Result<bool> {
    let mut enabled = 0i32;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL into the live stack destination.
    if unsafe { SystemParametersInfoW(0x1042, 0, (&raw mut enabled).cast(), 0) } == 0 {
        return Err(Error::from_thread());
    }
    Ok(enabled != 0)
}
