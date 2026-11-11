//! Is a fullscreen app (a game, a fullscreen video) in the foreground? Used by "game mode" so the
//! overlay can step aside. Windows-only; other platforms always answer false.

#[cfg(windows)]
pub fn foreground_fullscreen() -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_STYLE, WS_CAPTION,
    };

    // SAFETY: plain Win32 queries on the current foreground window; every out-pointer is a live local.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        // Our own windows (the overlay, Settings) never count.
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
        if pid == GetCurrentProcessId() {
            return false;
        }
        // The desktop and the taskbar cover the whole screen too.
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class).max(0) as usize;
        let name = String::from_utf16_lossy(&class[..n.min(class.len())]);
        if matches!(
            name.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
        // A maximised normal window keeps its title bar; fullscreen games and F11 video do not.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if style & WS_CAPTION.0 == WS_CAPTION.0 {
            return false;
        }
        let mut r = RECT::default();
        if GetWindowRect(hwnd, &mut r).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let m = info.rcMonitor;
        r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }
}

#[cfg(not(windows))]
pub fn foreground_fullscreen() -> bool {
    false
}
