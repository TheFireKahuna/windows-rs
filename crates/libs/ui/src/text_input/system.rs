//! System caret preferences and Unicode clipboard transfer.
//!
//! Preferences are read at startup and on a settings-change message; there is no timer.
//! Clipboard ownership is released on every failing branch.

use core::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use windows_core::{BOOL, Error, Result};
use windows_window::Hwnd;

windows_core::link!("user32.dll" "system" fn GetCaretBlinkTime() -> u32);
windows_core::link!("user32.dll" "system" fn SystemParametersInfoW(action: u32, param: u32, value: *mut c_void, flags: u32) -> i32);
windows_core::link!("user32.dll" "system" fn OpenClipboard(hwnd: *mut c_void) -> BOOL);
windows_core::link!("user32.dll" "system" fn CloseClipboard() -> BOOL);
windows_core::link!("user32.dll" "system" fn EmptyClipboard() -> BOOL);
windows_core::link!("user32.dll" "system" fn GetClipboardData(format: u32) -> *mut c_void);
windows_core::link!("user32.dll" "system" fn SetClipboardData(format: u32, data: *mut c_void) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn GlobalFree(memory: *mut c_void) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn GlobalLock(memory: *mut c_void) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn GlobalUnlock(memory: *mut c_void) -> BOOL);
windows_core::link!("kernel32.dll" "system" fn GlobalSize(memory: *mut c_void) -> usize);

/// The caret width in the high word and the blink half-period in the low one, at the
/// system's own defaults until the first refresh.
static CARET: AtomicU64 = AtomicU64::new((1 << 32) | 530);

/// Re-reads the system caret width and blink interval.
pub(crate) fn refresh() {
    let mut width = 1u32;
    // SPI_GETCARETWIDTH writes a DWORD. A missing optional width preference is named.
    if unsafe { SystemParametersInfoW(0x2006, 0, (&raw mut width).cast(), 0) } == 0 {
        eprintln!(
            "disabled capability SPI_GETCARETWIDTH: {}",
            Error::from_thread()
        );
    }
    let blink = unsafe { GetCaretBlinkTime() };
    CARET.store(
        (u64::from(width.max(1)) << 32) | u64::from(blink),
        Ordering::Release,
    );
}

/// Returns the caret width in pixels and the blink half-period in milliseconds.
pub(crate) fn caret() -> (u32, u32) {
    let packed = CARET.load(Ordering::Acquire);
    ((packed >> 32) as u32, packed as u32)
}

/// Holds clipboard ownership for one transfer and closes it on every path out.
struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

fn open(hwnd: Hwnd) -> Result<Open> {
    match unsafe { OpenClipboard(hwnd.raw()) }.as_bool() {
        true => Ok(Open),
        false => Err(Error::from_thread()),
    }
}

/// Reads the clipboard Unicode text, without its terminator.
pub(crate) fn read(hwnd: Hwnd) -> Result<Vec<u16>> {
    let _open = open(hwnd)?;
    // SAFETY: handles are checked, and the slice is bounded by the allocation's size.
    unsafe {
        let handle = GetClipboardData(13);
        let pointer = match handle.is_null() {
            true => handle,
            false => GlobalLock(handle),
        }
        .cast::<u16>();
        if pointer.is_null() {
            return Err(Error::from_thread());
        }
        let raw = core::slice::from_raw_parts(pointer, GlobalSize(handle) / 2);
        let text = raw[..raw.iter().position(|&c| c == 0).unwrap_or(raw.len())].to_vec();
        let _ = GlobalUnlock(handle);
        Ok(text)
    }
}

/// Replaces the clipboard with `text` as Unicode.
pub(crate) fn write(hwnd: Hwnd, text: &[u16]) -> Result<()> {
    let _open = open(hwnd)?;
    // SAFETY: moveable storage is transferred only on a successful SetClipboardData.
    unsafe {
        let memory = GlobalAlloc(2, (text.len() + 1) * 2);
        let pointer = match memory.is_null() {
            true => memory,
            false => GlobalLock(memory),
        }
        .cast::<u16>();
        if pointer.is_null() {
            GlobalFree(memory);
            return Err(Error::from_thread());
        }
        core::ptr::copy_nonoverlapping(text.as_ptr(), pointer, text.len());
        *pointer.add(text.len()) = 0;
        let _ = GlobalUnlock(memory);
        if !EmptyClipboard().as_bool() || SetClipboardData(13, memory).is_null() {
            GlobalFree(memory);
            return Err(Error::from_thread());
        }
        Ok(())
    }
}
