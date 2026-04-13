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
pub fn auto_paste_credentials(account_id: &str, otp: &str, _is_hk: bool) -> bool {
    win32::do_auto_paste(account_id, otp)
}

#[cfg(not(target_os = "windows"))]
pub fn auto_paste_credentials(_account_id: &str, _otp: &str, _is_hk: bool) -> bool {
    false
}

#[cfg(target_os = "windows")]
mod win32 {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetClientRect, GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId,
        PostMessageW, SetCursorPos, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    extern "system" {
        fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: i32) -> i32;
    }

    // Windows message constants
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_CHAR: u32 = 0x0102;
    const WM_LBUTTONDOWN: u32 = 0x0201;

    // Virtual key codes
    const VK_BACK: u32 = 0x08;
    const VK_TAB: u32 = 0x09;
    const VK_RETURN: u32 = 0x0D;
    const VK_ESCAPE: u32 = 0x1B;
    const VK_END: u32 = 0x23;

    /// Known MapleStory window class names.
    const CLASS_NAMES: &[&str] = &[
        "MapleStoryClass",
        "MapleStoryClassTW",
        "MapleStoryClassHK",
        "StartUpDlgClass",
    ];

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

        // Fallback: try to find by window title containing "MapleStory"
        let title = to_wide("MapleStory");
        let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if !hwnd.is_null() {
            tracing::info!("found MapleStory window by title");
            return Some(hwnd);
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
    /// Auto-paste credentials into MapleStory window (HK + TW 統一版)
    pub fn do_auto_paste(account_id: &str, otp: &str) -> bool {
        let hwnd = match find_maple_window() {
            Some(h) => h,
            None => {
                tracing::warn!("MapleStory window not found, skipping auto-paste");
                return false;
            }
        };

        sleep_ms(100); // 給遊戲窗口完全載入

        // 可靠帶到最前面
        unsafe {
            let fg_hwnd = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut());
            let our_thread = GetCurrentThreadId();

            if fg_thread != our_thread {
                AttachThreadInput(our_thread, fg_thread, 1);
            }

            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);

            if fg_thread != our_thread {
                AttachThreadInput(our_thread, fg_thread, 0);
            }
        }
        sleep_ms(200);

        // 按 ESC + 點擊帳號欄
        send_key(hwnd, VK_ESCAPE);
        sleep_ms(180);

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;

            let click_x = w / 2;
            let click_y = (h as f64 * 0.40) as i32;

            let mut old_point = POINT { x: 0, y: 0 };
            unsafe { GetCursorPos(&mut old_point) };

            let mut screen_point = POINT { x: 0, y: 0 };
            unsafe { ClientToScreen(hwnd, &mut screen_point) };

            unsafe { SetCursorPos(screen_point.x + click_x, screen_point.y + click_y) };
            let pos = ((click_y as LPARAM) << 16) | (click_x as LPARAM & 0xFFFF);
            unsafe { PostMessageW(hwnd, WM_LBUTTONDOWN, 1, pos) };
            sleep_ms(280);

            unsafe { SetCursorPos(old_point.x, old_point.y) };
        }

        // ==================== 通用步驟 (HK + TW 都一樣) ====================
        clear_field(hwnd, 64); // 清空帳號欄
        type_string(hwnd, account_id);
        sleep_ms(100);

        send_key(hwnd, VK_TAB); // 切換到密碼欄
        sleep_ms(100);

        clear_field(hwnd, 20); // 清空密碼欄
        type_string(hwnd, otp);
        sleep_ms(100);

        send_key(hwnd, VK_RETURN); // 按登入

        tracing::info!("auto-paste completed for account {}", account_id);
        true
    }
}
