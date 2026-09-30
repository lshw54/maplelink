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
    use crate::services::login_locator::{locate_account_field, Frame, Point, Size};

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
    /// Pause after ESC so the dismissed notice is gone before the screenshot
    /// and the click.
    pub const ESCAPE_SETTLE_MS: u64 = 100;
    /// Pause after `clear_field` so the field has a frame or two to empty
    /// before characters start arriving.
    pub const CLEAR_SETTLE_MS: u64 = 60;

    /// The Win32 calls the paste sequence needs, behind a trait so the
    /// sequence can be exercised without a game window.
    pub trait PasteDriver {
        /// Press and release a virtual key.
        fn send_key(&mut self, vk: u32);
        /// Post a single character (`WM_CHAR`).
        fn send_char(&mut self, ch: char);
        /// Client-area size of the game window; `None` when unavailable.
        fn client_size(&mut self) -> Option<Size>;
        /// Screenshot the client area for the account-field locator. `None`
        /// means the capture is unavailable and the ratio fallback is used.
        fn capture_client(&mut self, size: Size) -> Option<Frame>;
        /// Left-click at `point` (client coordinates) and let it settle.
        fn click(&mut self, point: Point);
        fn sleep_ms(&mut self, ms: u64);
    }

    /// Where the account-field click point came from; logged so future
    /// reports can be diagnosed from the log.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ClickSource {
        Screenshot,
        RatioFallback,
    }

    impl ClickSource {
        pub fn as_str(self) -> &'static str {
            match self {
                ClickSource::Screenshot => "screenshot",
                ClickSource::RatioFallback => "ratio-fallback",
            }
        }
    }

    /// The legacy click point, `(0.5 × width, 0.4 × height)`, inherited from
    /// the WPF Beanfun. Only correct for the layout it was tuned on; kept as
    /// the fallback when the screenshot yields nothing.
    pub fn ratio_click_point(size: Size) -> Point {
        Point {
            x: size.width / 2,
            y: (size.height as f64 * 0.40) as i32,
        }
    }

    /// Pick the account-field click point: the located field when the frame
    /// shows the login panel, otherwise the ratio fallback.
    pub fn resolve_click_point(size: Size, frame: Option<&Frame>) -> (Point, ClickSource) {
        match frame.and_then(locate_account_field) {
            Some(point) => (point, ClickSource::Screenshot),
            None => (ratio_click_point(size), ClickSource::RatioFallback),
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
    ///
    /// The login panel's position and scale depend on the player's window /
    /// extended-UI settings, so a fixed fraction of the client area can miss
    /// the account box. A miss leaves focus wherever it was (after a logout:
    /// the password box) and the OTP ends up in the wrong field. The account
    /// box is therefore located from a screenshot
    /// ([`crate::services::login_locator`]); only if that fails is the ratio
    /// used.
    pub fn run_paste_sequence<D: PasteDriver>(driver: &mut D, account_id: &str, otp: &str) {
        // 按 ESC 關閉提示框 + 點擊帳號欄 (matches original T9 flow)
        driver.send_key(VK_ESCAPE);
        driver.sleep_ms(ESCAPE_SETTLE_MS);

        if let Some(size) = driver.client_size() {
            let frame = driver.capture_client(size);
            let (point, source) = resolve_click_point(size, frame.as_ref());
            tracing::info!(
                client_width = size.width,
                client_height = size.height,
                click_x = point.x,
                click_y = point.y,
                source = source.as_str(),
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

    use super::sequence::{run_paste_sequence, PasteDriver};
    use crate::services::login_locator::{capture_screen_region, Frame, Point, Size};

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

        /// `GetClientRect` and `ClientToScreen` from this per-monitor
        /// DPI-aware process give physical pixels, and so does the screen
        /// DC, so the frame lines up with the click coordinates.
        fn capture_client(&mut self, size: Size) -> Option<Frame> {
            let frame = capture_screen_region(client_origin(self.hwnd), size);
            if frame.is_none() {
                tracing::warn!("auto-paste: client-area capture failed, using ratio fallback");
            }
            frame
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
    use crate::services::login_locator::test_support::synth;
    use crate::services::login_locator::{ButtonRect, Frame, Point, Size};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Key(u32),
        Char(char),
        ClientSize,
        Capture(Size),
        Click(Point),
        Sleep(u64),
    }

    /// Records every call and hands back a planted frame (or `None`, which
    /// exercises the ratio fallback).
    struct RecordingDriver {
        calls: Vec<Call>,
        size: Size,
        frame: Option<Frame>,
    }

    impl RecordingDriver {
        fn new(size: Size, frame: Option<Frame>) -> Self {
            Self {
                calls: Vec::new(),
                size,
                frame,
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
        fn capture_client(&mut self, size: Size) -> Option<Frame> {
            self.calls.push(Call::Capture(size));
            self.frame.clone()
        }
        fn click(&mut self, point: Point) {
            self.calls.push(Call::Click(point));
        }
        fn sleep_ms(&mut self, ms: u64) {
            self.calls.push(Call::Sleep(ms));
        }
    }

    const SIZE: Size = Size {
        width: 800,
        height: 600,
    };

    /// A login panel with its 「登入」 button, laid out so the located point
    /// does not coincide with the ratio click.
    fn panel_frame() -> Frame {
        synth(
            SIZE,
            (90, 180, 60),
            Some(ButtonRect {
                left: 250,
                top: 150,
                width: 300,
                height: 250,
            }),
            Some(ButtonRect {
                left: 270,
                top: 300,
                width: 260,
                height: 45,
            }),
        )
    }

    /// Button centre x = 270 + 130 = 400; account row = 300 − 0.29 × 260 ≈ 225.
    const LOCATED: Point = Point { x: 400, y: 225 };

    #[test]
    fn ratio_click_point_matches_legacy_fraction() {
        assert_eq!(ratio_click_point(SIZE), Point { x: 400, y: 240 });
    }

    #[test]
    fn resolve_click_point_prefers_the_screenshot() {
        let frame = panel_frame();
        assert_eq!(
            resolve_click_point(SIZE, Some(&frame)),
            (LOCATED, ClickSource::Screenshot)
        );
        assert_ne!(
            LOCATED,
            ratio_click_point(SIZE),
            "test must not coincide with the ratio"
        );
    }

    #[test]
    fn resolve_click_point_falls_back_without_a_frame_or_a_panel() {
        let expected = (ratio_click_point(SIZE), ClickSource::RatioFallback);
        assert_eq!(resolve_click_point(SIZE, None), expected);
        let scenery_only = synth(SIZE, (90, 180, 60), None, None);
        assert_eq!(resolve_click_point(SIZE, Some(&scenery_only)), expected);
    }

    #[test]
    fn paste_sequence_clicks_the_located_account_field() {
        let mut driver = RecordingDriver::new(SIZE, Some(panel_frame()));
        run_paste_sequence(&mut driver, "acc", "otp");
        assert_eq!(driver.clicks(), vec![LOCATED]);
        assert!(driver.calls.contains(&Call::Capture(SIZE)));
    }

    #[test]
    fn paste_sequence_uses_the_ratio_when_capture_fails() {
        let mut driver = RecordingDriver::new(SIZE, None);
        run_paste_sequence(&mut driver, "acc", "otp");
        assert_eq!(driver.clicks(), vec![ratio_click_point(SIZE)]);
    }

    #[test]
    fn paste_sequence_order_is_unchanged() {
        let mut driver = RecordingDriver::new(SIZE, None);
        run_paste_sequence(&mut driver, "ab", "12");

        let mut expected = vec![
            Call::Key(VK_ESCAPE),
            Call::Sleep(ESCAPE_SETTLE_MS),
            Call::ClientSize,
            Call::Capture(SIZE),
            Call::Click(ratio_click_point(SIZE)),
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
