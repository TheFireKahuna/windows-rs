//! System caret preferences, refreshed only at startup or a settings-change message.
use std::sync::atomic::{AtomicU64, Ordering};
windows_core::link!("user32.dll" "system" fn GetCaretBlinkTime() -> u32);
windows_core::link!("user32.dll" "system" fn SystemParametersInfoW(action:u32,param:u32,value:*mut core::ffi::c_void,flags:u32)->i32);
static CARET: AtomicU64 = AtomicU64::new(0);
pub(crate) fn refresh() {
    let mut width = 1u32;
    // SPI_GETCARETWIDTH writes a DWORD. A missing optional width preference is named.
    if unsafe { SystemParametersInfoW(0x2006, 0, (&mut width as *mut u32).cast(), 0) } == 0 {
        eprintln!(
            "disabled capability SPI_GETCARETWIDTH: {}",
            windows_core::Error::from_thread()
        );
    }
    let blink = unsafe { GetCaretBlinkTime() };
    CARET.store(
        (u64::from(width.max(1)) << 32) | u64::from(blink),
        Ordering::Release,
    );
}
pub(crate) fn caret() -> (u32, u32) {
    let packed = CARET.load(Ordering::Acquire);
    ((packed >> 32) as u32, packed as u32)
}
