//! Auto-paste credentials into the MapleStory game window via Win32 APIs.
//!
//! Finds the MapleStory window by class name, brings it to the foreground,
//! clears the account/password fields, and types the credentials character
//! by character using `PostMessageW(WM_CHAR, ...)`.
//!
//! This module is Windows-only. On other platforms it compiles to a no-op
//! that always returns `false`.

/// Attempt to auto-paste credentials into the MapleStory game window.
///
/// Returns `true` if the game window was found and credentials were sent,
/// `false` if the window was not found (caller should fall back to clipboard).
///
/// # Safety
///
/// Calls Win32 APIs (`FindWindowW`, `SetForegroundWindow`, `PostMessageW`,
/// etc.) which are inherently unsafe but wrapped here behind a safe interface.
#[cfg(target_os = "windows")]
pub fn auto_paste_credentials(account_id: &str, otp: &str, is_hk: bool) -> bool {
    win32::do_auto_paste(account_id, otp, is_hk)
}

#[cfg(not(target_os = "windows"))]
pub fn auto_paste_credentials(_account_id: &str, _otp: &str, _is_hk: bool) -> bool {
    false
}

#[cfg(target_os = "windows")]
mod win32 {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetClientRect, GetForegroundWindow, GetWindowThreadProcessId, PostMessageW,
        SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    extern "system" {
        fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: i32) -> i32;
    }

    // Windows message constants
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_CHAR: u32 = 0x0102;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_LBUTTONUP: u32 = 0x0202;

    // Virtual key codes
    const VK_BACK: u32 = 0x08;
    const VK_TAB: u32 = 0x09;
    const VK_RETURN: u32 = 0x0D;
    const VK_ESCAPE: u32 = 0x1B;
    const VK_END: u32 = 0x23;

    /// Known MapleStory window class names.
    const CLASS_NAMES: &[&str] = &["MapleStoryClass", "MapleStoryClassTW"];

    /// Encode a Rust string to a null-terminated wide (UTF-16) string.
    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Build the `lParam` for a `WM_KEYDOWN` message.
    ///
    /// Layout: repeat count (bits 0-15) = 1, scan code (bits 16-23),
    /// extended flag (bit 24) = 0, context (bit 29) = 0,
    /// previous state (bit 30) = 0, transition (bit 31) = 0.
    fn make_key_lparam(vk: u32) -> LPARAM {
        let scan_code = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) };
        (1 | ((scan_code as LPARAM) << 16)) as LPARAM
    }

    /// Find the MapleStory window handle by trying known class names.
    fn find_maple_window() -> Option<HWND> {
        for class_name in CLASS_NAMES {
            let wide = to_wide(class_name);
            let hwnd = unsafe { FindWindowW(wide.as_ptr(), std::ptr::null()) };
            if !hwnd.is_null() {
                tracing::info!("found MapleStory window (class: {class_name})");
                return Some(hwnd);
            }
        }
        None
    }

    /// Send a single virtual key press via `PostMessageW(WM_KEYDOWN)`.
    fn send_key(hwnd: HWND, vk: u32) {
        let lparam = make_key_lparam(vk);
        unsafe {
            PostMessageW(hwnd, WM_KEYDOWN, vk as WPARAM, lparam);
        }
    }

    /// Send a single character via `PostMessageW(WM_CHAR)`.
    fn send_char(hwnd: HWND, ch: char) {
        unsafe {
            PostMessageW(hwnd, WM_CHAR, ch as WPARAM, 0);
        }
    }

    /// Send a mouse click at a specific position within the client area.
    fn send_click(hwnd: HWND, x: i32, y: i32) {
        let lparam = ((y as LPARAM) << 16) | (x as LPARAM & 0xFFFF);
        unsafe {
            PostMessageW(hwnd, WM_LBUTTONDOWN, 0, lparam);
        }
        sleep_ms(50);
        unsafe {
            PostMessageW(hwnd, WM_LBUTTONUP, 0, lparam);
        }
    }

    /// Clear a text field by pressing END then BACKSPACE `count` times.
    fn clear_field(hwnd: HWND, backspace_count: u32) {
        send_key(hwnd, VK_END);
        sleep_ms(10);
        for _ in 0..backspace_count {
            send_key(hwnd, VK_BACK);
        }
        sleep_ms(10);
    }

    /// Type a string character by character via `WM_CHAR`.
    fn type_string(hwnd: HWND, text: &str) {
        for ch in text.chars() {
            send_char(hwnd, ch);
            sleep_ms(5);
        }
    }

    /// Sleep for the given number of milliseconds (sync, NOT tokio).
    fn sleep_ms(ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }

    /// Main auto-paste implementation.
    pub fn do_auto_paste(account_id: &str, otp: &str, is_hk: bool) -> bool {
        let hwnd = match find_maple_window() {
            Some(h) => h,
            None => {
                tracing::info!("MapleStory window not found, skipping auto-paste");
                return false;
            }
        };

        // Bring the game window to the foreground reliably
        // Windows restricts SetForegroundWindow — use AttachThreadInput trick
        unsafe {
            let fg_hwnd = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut());
            let our_thread = GetCurrentThreadId();

            if fg_thread != our_thread {
                AttachThreadInput(our_thread, fg_thread, 1); // attach
            }

            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);

            if fg_thread != our_thread {
                AttachThreadInput(our_thread, fg_thread, 0); // detach
            }
        }
        sleep_ms(200);

        // For HK MapleStory: press ESC to close any dialog, then click the
        // account text box at approximately 50% width, 40% height.
        if is_hk {
            send_key(hwnd, VK_ESCAPE);
            sleep_ms(100);

            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let got_rect = unsafe { GetClientRect(hwnd, &mut rect) };
            if got_rect != 0 {
                let click_x = (rect.right - rect.left) / 2;
                let click_y = ((rect.bottom - rect.top) as f64 * 0.4) as i32;
                send_click(hwnd, click_x, click_y);
                sleep_ms(100);
            }
        }

        // Clear account field (END + 64× BACKSPACE) and type account ID
        clear_field(hwnd, 64);
        type_string(hwnd, account_id);
        sleep_ms(50);

        // TAB to password field
        send_key(hwnd, VK_TAB);
        sleep_ms(50);

        // Clear password field (END + 20× BACKSPACE) and type OTP
        clear_field(hwnd, 20);
        type_string(hwnd, otp);
        sleep_ms(50);

        // Press ENTER to submit
        send_key(hwnd, VK_RETURN);

        tracing::info!("auto-paste completed for account {account_id}");
        true
    }
}
