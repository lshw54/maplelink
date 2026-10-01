//! Auto-paste credentials into the MapleStory game window via Win32 APIs.
//!
//! Finds the MapleStory window by class name, brings it to the foreground,
//! clicks the account field, clears the account/password fields, and types
//! the credentials character by character using `PostMessageW(WM_CHAR, ...)`.
//!
//! The paste sequence itself is written against the [`PasteDriver`] trait so
//! it can be unit-tested with a recording driver; the Win32 driver lives in
//! the `win32` module. On other platforms the public entry point compiles to
//! a no-op that always returns `false`.

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

#[cfg(any(target_os = "windows", test))]
mod sequence {
    /// A point in client coordinates, physical pixels.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    /// A size in physical pixels.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Size {
        pub width: i32,
        pub height: i32,
    }

    // Virtual key codes
    pub const VK_BACK: u32 = 0x08;
    pub const VK_TAB: u32 = 0x09;
    pub const VK_RETURN: u32 = 0x0D;
    pub const VK_ESCAPE: u32 = 0x1B;
    pub const VK_END: u32 = 0x23;

    /// How long to wait after TAB before posting anything else.
    ///
    /// `clear_field` already explains why posted input needs a pause: a game
    /// that samples input per frame does not consume the queue in one go. TAB
    /// is worse, because what it changes is focus, and that lands at the end
    /// of the frame that read it — anything posted in the same batch still
    /// goes to the field we were trying to leave.
    ///
    /// Without this pause the backspaces meant for the password field eat the
    /// account instead, and the OTP is appended to whatever survived: both
    /// values in one box. That is exactly how this was reported, and it
    /// stayed reproducible across releases for the people it happened to,
    /// because nothing about it depends on the version — only on how fast the
    /// machine gets round to the message queue.
    pub const FOCUS_SETTLE_MS: u64 = 100;
    /// Pause after ESC so the dismissed notice is gone before the click.
    pub const ESCAPE_SETTLE_MS: u64 = 100;
    /// Pause after `clear_field` so the field has a frame or two to empty
    /// before characters start arriving.
    pub const CLEAR_SETTLE_MS: u64 = 60;

    /// Where the account box sits on the login screen, as fractions of the
    /// login screen's size (WPF `MainWindow.xaml.cs` L2206-2207).
    const CLICK_X_RATIO: f64 = 0.5;
    const CLICK_Y_RATIO: f64 = 0.4;

    /// The resolutions the client offers for its window. The login screen is
    /// drawn at exactly one of these, anchored top-left; see
    /// [`login_screen_size`].
    const LOGIN_SCREEN_PRESETS: &[Size] = &[
        Size {
            width: 3840,
            height: 2160,
        },
        Size {
            width: 2732,
            height: 1536,
        },
        Size {
            width: 2560,
            height: 1440,
        },
        Size {
            width: 1920,
            height: 1080,
        },
        Size {
            width: 1600,
            height: 900,
        },
        Size {
            width: 1366,
            height: 768,
        },
        Size {
            width: 1280,
            height: 720,
        },
        Size {
            width: 1024,
            height: 768,
        },
        Size {
            width: 800,
            height: 600,
        },
    ];

    /// The Win32 calls the paste sequence needs, behind a trait so the
    /// sequence can be exercised without a game window.
    pub trait PasteDriver {
        /// Press and release a virtual key.
        fn send_key(&mut self, vk: u32);
        /// Post a single character (`WM_CHAR`).
        fn send_char(&mut self, ch: char);
        /// Client-area size of the game window; `None` when unavailable.
        fn client_size(&mut self) -> Option<Size>;
        /// Left-click at `point` (client coordinates) and let it settle.
        fn click(&mut self, point: Point);
        fn sleep_ms(&mut self, ms: u64);
    }

    /// Size of the login screen inside a client of `size`.
    ///
    /// With the game's 擴展UI模式 (extended UI mode) the client can grow past
    /// the chosen resolution; the extra space goes to the right and the
    /// bottom while the login screen stays at the chosen resolution,
    /// anchored top-left. A reporter's 1920 × 1080 client had become
    /// 1920 × 1238, so the account box sat at `0.4 × 1080 = 432` while the
    /// ratio on the raw height gave `0.4 × 1238 = 495`, 63 px too low. A
    /// `WM_LBUTTONDOWN` that misses does not move focus; after a logout focus
    /// is on the password box, so the account went there, TAB moved to the
    /// account box, and the OTP was typed into it.
    ///
    /// The client only offers the resolutions in [`LOGIN_SCREEN_PRESETS`], so:
    ///
    /// - A client that *is* one of them is unstretched and the login screen
    ///   fills it — the original behaviour.
    /// - Any other size means extended-UI mode stretched the client, and the
    ///   login screen is the largest preset that fits inside it (by area).
    ///   Aspect ratio is deliberately not used: a diagonal drag can land on
    ///   16:9 again (2200 × 1238 is 1.777) and still be a stretched
    ///   1920 × 1080.
    /// - A client smaller than every preset is returned as-is.
    ///
    /// Known blind spot: a client stretched until a *larger* preset fits
    /// exactly inside it (a 1920 × 1080 player dragging to ≥ 2560 × 1440) is
    /// attributed to that larger preset. Nothing in the client size
    /// distinguishes the two cases.
    pub fn login_screen_size(size: Size) -> Size {
        if size.width <= 0 || size.height <= 0 {
            return size;
        }
        if LOGIN_SCREEN_PRESETS.contains(&size) {
            return size;
        }
        LOGIN_SCREEN_PRESETS
            .iter()
            .copied()
            .filter(|preset| preset.width <= size.width && preset.height <= size.height)
            .max_by_key(|preset| i64::from(preset.width) * i64::from(preset.height))
            .unwrap_or(size)
    }

    /// Where to click for the account field: the WPF ratios applied to the
    /// login screen's size, not the raw client size.
    pub fn compute_click_point(size: Size) -> Point {
        let screen = login_screen_size(size);
        Point {
            x: (screen.width as f64 * CLICK_X_RATIO) as i32,
            y: (screen.height as f64 * CLICK_Y_RATIO) as i32,
        }
    }

    /// Clear a text field by pressing END then BACKSPACE `count` times.
    /// Matches original C# — no sleep between keystrokes (PostMessage is
    /// async). Posted messages queue in order, but a game that samples input
    /// per frame doesn't consume them in one go, so give the field a frame or
    /// two to empty before characters start arriving.
    fn clear_field<D: PasteDriver>(driver: &mut D, backspace_count: u32) {
        driver.send_key(VK_END);
        for _ in 0..backspace_count {
            driver.send_key(VK_BACK);
        }
        driver.sleep_ms(CLEAR_SETTLE_MS);
    }

    /// Type a string character by character via `WM_CHAR`. Matches original
    /// C# `PostString` — no per-character delay since `PostMessageW` is
    /// non-blocking and the messages queue up correctly.
    fn type_string<D: PasteDriver>(driver: &mut D, text: &str) {
        for ch in text.chars() {
            driver.send_char(ch);
        }
    }

    /// The paste sequence, after the game window is already in the
    /// foreground (HK + TW 統一版).
    ///
    /// Timing:
    /// - ESC → 100ms
    /// - Click account field → 200ms (inside the driver's `click`)
    /// - TAB to the password field → 100ms (see FOCUS_SETTLE_MS)
    /// - Characters carry no per-character delay: PostMessage queues them and
    ///   the game reads them in order.
    pub fn run_paste_sequence<D: PasteDriver>(driver: &mut D, account_id: &str, otp: &str) {
        // 按 ESC 關閉提示框 + 點擊帳號欄 (matches original T9 flow)
        driver.send_key(VK_ESCAPE);
        driver.sleep_ms(ESCAPE_SETTLE_MS);

        if let Some(size) = driver.client_size() {
            let screen = login_screen_size(size);
            let point = compute_click_point(size);
            tracing::info!(
                client_width = size.width,
                client_height = size.height,
                login_width = screen.width,
                login_height = screen.height,
                click_x = point.x,
                click_y = point.y,
                "auto-paste: clicking account field"
            );
            driver.click(point);
        }

        // ==================== 通用步驟 (HK + TW 都一樣) ====================
        // The original C# fires these with no inter-step delays, and ordering
        // alone is enough for anything that only writes characters:
        // PostMessage queues them and the game reads them in order. Focus is
        // the exception — see FOCUS_SETTLE_MS.
        clear_field(driver, 64); // 清空帳號欄
        type_string(driver, account_id); // 輸入帳號
        driver.send_key(VK_TAB); // 切換到密碼欄
        driver.sleep_ms(FOCUS_SETTLE_MS); // 等焦點真的移過去，見 FOCUS_SETTLE_MS
        clear_field(driver, 20); // 清空密碼欄
        type_string(driver, otp); // 輸入密碼
        driver.send_key(VK_RETURN); // 按登入
    }
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

    use super::sequence::{run_paste_sequence, PasteDriver, Point, Size};

    extern "system" {
        fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: i32) -> i32;
    }

    // Windows message constants
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_KEYUP: u32 = 0x0101;
    const WM_CHAR: u32 = 0x0102;
    const WM_LBUTTONDOWN: u32 = 0x0201;

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

    /// Screen coordinates of the client area's top-left corner.
    fn client_origin(hwnd: HWND) -> Point {
        let mut screen_point = POINT { x: 0, y: 0 };
        unsafe { ClientToScreen(hwnd, &mut screen_point) };
        Point {
            x: screen_point.x,
            y: screen_point.y,
        }
    }

    /// Drives the real game window through `PostMessageW` and friends.
    struct Win32Driver {
        hwnd: HWND,
    }

    impl PasteDriver for Win32Driver {
        /// Press and release a virtual key.
        ///
        /// The release half used to be left out. A game that reads key state
        /// rather than only messages then sees the key as still held — for
        /// the 64 backspaces that clear a field, that means the deletion
        /// carries on into the characters posted next, and the account lands
        /// half-typed.
        fn send_key(&mut self, vk: u32) {
            let scan_code = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) };
            let down = (1 | ((scan_code as LPARAM) << 16)) as LPARAM;
            // Bits 30 and 31 mark the previous-down state and the up transition.
            let up = down | (1 << 30) | (1 << 31);
            unsafe {
                PostMessageW(self.hwnd, WM_KEYDOWN, vk as WPARAM, down);
                PostMessageW(self.hwnd, WM_KEYUP, vk as WPARAM, up);
            }
        }

        fn send_char(&mut self, ch: char) {
            unsafe {
                PostMessageW(self.hwnd, WM_CHAR, ch as WPARAM, 0);
            }
        }

        fn client_size(&mut self) -> Option<Size> {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetClientRect(self.hwnd, &mut rect) } == 0 {
                return None;
            }
            Some(Size {
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            })
        }

        /// Move the cursor over the point (the game reads the cursor
        /// position, not only the message), post `WM_LBUTTONDOWN`, let the
        /// game process it, then put the cursor back.
        fn click(&mut self, point: Point) {
            let mut old_point = POINT { x: 0, y: 0 };
            unsafe { GetCursorPos(&mut old_point) };

            let origin = client_origin(self.hwnd);
            unsafe { SetCursorPos(origin.x + point.x, origin.y + point.y) };
            let pos = ((point.y as LPARAM) << 16) | (point.x as LPARAM & 0xFFFF);
            unsafe { PostMessageW(self.hwnd, WM_LBUTTONDOWN, 1, pos) };
            self.sleep_ms(200);

            unsafe { SetCursorPos(old_point.x, old_point.y) };
        }

        /// Sleep for the given number of milliseconds (sync, NOT tokio).
        fn sleep_ms(&mut self, ms: u64) {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }

    /// Main auto-paste implementation: find the window, bring it to the
    /// foreground (SetForegroundWindow + 100ms), then run the paste sequence.
    pub fn do_auto_paste(account_id: &str, otp: &str) -> bool {
        let hwnd = match find_maple_window() {
            Some(h) => h,
            None => {
                tracing::warn!("MapleStory window not found, skipping auto-paste");
                return false;
            }
        };

        // 可靠帶到最前面 (matches original: SetForegroundWindow + 100ms)
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
        std::thread::sleep(std::time::Duration::from_millis(100));

        let mut driver = Win32Driver { hwnd };
        run_paste_sequence(&mut driver, account_id, otp);

        tracing::info!("auto-paste completed for account {}", account_id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::sequence::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Key(u32),
        Char(char),
        ClientSize,
        Click(Point),
        Sleep(u64),
    }

    /// Records every call and reports a fixed client size.
    struct RecordingDriver {
        calls: Vec<Call>,
        size: Size,
    }

    impl RecordingDriver {
        fn new(size: Size) -> Self {
            Self {
                calls: Vec::new(),
                size,
            }
        }

        fn clicks(&self) -> Vec<Point> {
            self.calls
                .iter()
                .filter_map(|c| match c {
                    Call::Click(p) => Some(*p),
                    _ => None,
                })
                .collect()
        }
    }

    impl PasteDriver for RecordingDriver {
        fn send_key(&mut self, vk: u32) {
            self.calls.push(Call::Key(vk));
        }
        fn send_char(&mut self, ch: char) {
            self.calls.push(Call::Char(ch));
        }
        fn client_size(&mut self) -> Option<Size> {
            self.calls.push(Call::ClientSize);
            Some(self.size)
        }
        fn click(&mut self, point: Point) {
            self.calls.push(Call::Click(point));
        }
        fn sleep_ms(&mut self, ms: u64) {
            self.calls.push(Call::Sleep(ms));
        }
    }

    fn size(width: i32, height: i32) -> Size {
        Size { width, height }
    }

    #[test]
    fn every_preset_keeps_the_original_ratio_point() {
        for (w, h) in [
            (3840, 2160),
            (2732, 1536),
            (2560, 1440),
            (1920, 1080),
            (1600, 900),
            (1366, 768),
            (1280, 720),
            (1024, 768),
            (800, 600),
        ] {
            let s = size(w, h);
            assert_eq!(login_screen_size(s), s);
            assert_eq!(
                compute_click_point(s),
                Point {
                    x: (w as f64 * 0.5) as i32,
                    y: (h as f64 * 0.4) as i32
                },
                "{w}x{h}"
            );
        }
    }

    #[test]
    fn stretched_1920x1080_clients_click_the_1920x1080_login_screen() {
        // The reporter's extended-UI client, plus sideways and diagonal
        // drags — including one that lands back on 16:9.
        for (w, h) in [(1920, 1238), (2200, 1080), (2200, 1238), (2400, 1300)] {
            let s = size(w, h);
            assert_eq!(login_screen_size(s), size(1920, 1080), "{w}x{h}");
            assert_eq!(compute_click_point(s), Point { x: 960, y: 432 }, "{w}x{h}");
        }
    }

    #[test]
    fn a_stretched_1366x768_client_clicks_the_1366x768_login_screen() {
        assert_eq!(login_screen_size(size(1400, 926)), size(1366, 768));
        assert_eq!(
            compute_click_point(size(1400, 926)),
            Point { x: 683, y: 307 }
        );
    }

    #[test]
    fn clients_smaller_than_every_preset_are_used_as_they_are() {
        assert_eq!(login_screen_size(size(700, 500)), size(700, 500));
        assert_eq!(
            compute_click_point(size(700, 500)),
            Point { x: 350, y: 200 }
        );
        assert_eq!(login_screen_size(size(0, 0)), size(0, 0));
        assert_eq!(compute_click_point(size(0, 0)), Point { x: 0, y: 0 });
    }

    #[test]
    fn paste_sequence_clicks_the_login_screen_point_on_a_stretched_client() {
        let mut driver = RecordingDriver::new(size(1920, 1238));
        run_paste_sequence(&mut driver, "acc", "otp");
        assert_eq!(driver.clicks(), vec![Point { x: 960, y: 432 }]);
    }

    #[test]
    fn paste_sequence_order_is_unchanged() {
        let s = size(800, 600);
        let mut driver = RecordingDriver::new(s);
        run_paste_sequence(&mut driver, "ab", "12");

        let mut expected = vec![
            Call::Key(VK_ESCAPE),
            Call::Sleep(ESCAPE_SETTLE_MS),
            Call::ClientSize,
            Call::Click(compute_click_point(s)),
            Call::Key(VK_END),
        ];
        expected.extend(std::iter::repeat_n(Call::Key(VK_BACK), 64));
        expected.extend([
            Call::Sleep(CLEAR_SETTLE_MS),
            Call::Char('a'),
            Call::Char('b'),
            Call::Key(VK_TAB),
            Call::Sleep(FOCUS_SETTLE_MS),
            Call::Key(VK_END),
        ]);
        expected.extend(std::iter::repeat_n(Call::Key(VK_BACK), 20));
        expected.extend([
            Call::Sleep(CLEAR_SETTLE_MS),
            Call::Char('1'),
            Call::Char('2'),
            Call::Key(VK_RETURN),
        ]);
        assert_eq!(driver.calls, expected);
    }
}
