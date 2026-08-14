//! Put text on the Windows clipboard from the backend.
//!
//! The webview's `navigator.clipboard` needs the document to hold focus, and the
//! one moment the OTP most needs copying is the moment auto-input has just
//! pushed the game window to the front — so that write is refused exactly when
//! it matters. The Win32 clipboard has no such condition.

/// Copy `text` to the clipboard. Returns whether it landed.
#[cfg(target_os = "windows")]
pub fn set_text(text: &str) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    // windows-sys 0.61 links GlobalAlloc but not its counterpart.
    unsafe extern "system" {
        fn GlobalFree(hmem: HGLOBAL) -> HGLOBAL;
    }

    const CF_UNICODETEXT: u32 = 13;

    let wide: Vec<u16> = std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let bytes = std::mem::size_of_val(wide.as_slice());

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }

        // The block owns this allocation right up until SetClipboardData takes
        // it; after that, freeing it would pull the text out from under whoever
        // pastes next.
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if handle.is_null() {
            CloseClipboard();
            return false;
        }
        let dest = GlobalLock(handle);
        if dest.is_null() {
            GlobalFree(handle);
            CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), dest as *mut u16, wide.len());
        GlobalUnlock(handle);

        EmptyClipboard();
        let placed = !SetClipboardData(CF_UNICODETEXT, handle as HANDLE).is_null();
        if !placed {
            GlobalFree(handle);
        }
        CloseClipboard();
        placed
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_text(_text: &str) -> bool {
    false
}
