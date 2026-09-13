//! Unicode clipboard transfer, with ownership released on every failing branch.
use core::ffi::c_void;
use windows_core::{BOOL, Error, Result};
use windows_window::Hwnd;
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
struct Open;
impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
pub fn read(hwnd: Hwnd) -> Result<Vec<u16>> {
    // SAFETY: handles are checked, and the slice is bounded by the allocation's size.
    unsafe {
        if !OpenClipboard(hwnd.raw()).as_bool() {
            return Err(Error::from_thread());
        }
        let _open = Open;
        let handle = GetClipboardData(13);
        if handle.is_null() {
            return Err(Error::from_thread());
        }
        let count = GlobalSize(handle) / 2;
        let pointer = GlobalLock(handle).cast::<u16>();
        if pointer.is_null() {
            return Err(Error::from_thread());
        }
        let raw = core::slice::from_raw_parts(pointer, count);
        let length = raw.iter().position(|&c| c == 0).unwrap_or(count);
        let text = raw[..length].to_vec();
        let _ = GlobalUnlock(handle);
        Ok(text)
    }
}
pub fn write(hwnd: Hwnd, text: &[u16]) -> Result<()> {
    // SAFETY: moveable clipboard storage is transferred only on successful SetClipboardData.
    unsafe {
        if !OpenClipboard(hwnd.raw()).as_bool() {
            return Err(Error::from_thread());
        }
        let _open = Open;
        let memory = GlobalAlloc(2, (text.len() + 1) * 2);
        if memory.is_null() {
            return Err(Error::from_thread());
        }
        let pointer = GlobalLock(memory).cast::<u16>();
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
