//! Keeps the overlay above other windows. Windows lets another topmost window slide over ours (and
//! some apps jump over it when they are activated), so the order is claimed again now and then.

#[cfg(windows)]
pub fn raise(hwnd: usize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    // SAFETY: a plain z-order request for one of our own windows; nothing is moved, resized or activated.
    unsafe {
        let _ = SetWindowPos(
            HWND(hwnd as *mut core::ffi::c_void),
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(not(windows))]
pub fn raise(_hwnd: usize) {}
