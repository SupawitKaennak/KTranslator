#[cfg(target_os = "windows")]
use std::ptr;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{COLORREF, HWND};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::SetWindowRgn;
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetLayeredWindowAttributes, LWA_ALPHA, LWA_COLORKEY,
    GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_LAYERED,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

/// Finds a window by its title.
pub fn find_window(window_title: &str) -> Option<isize> {
    #[cfg(target_os = "windows")]
    unsafe {
        let title_w: Vec<u16> = format!("{}\0", window_title).encode_utf16().collect();
        if let Ok(hwnd) = FindWindowW(
            windows::core::PCWSTR(ptr::null()),
            windows::core::PCWSTR(title_w.as_ptr()),
        ) {
            let raw = hwnd.0 as isize;
            if raw != 0 {
                return Some(raw);
            }
        }
    }
    let _ = window_title;
    None
}




/// Applies transparency color-key and capture exclusion to a window.
pub fn apply_overlay_attributes(hwnd_raw: isize, hide_from_capture: bool) {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if (ex_style & WS_EX_LAYERED.0 as i32) == 0 {
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as i32);
        }

        // Apply color key for transparency (Black 0x000000 is our key)
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_COLORKEY | LWA_ALPHA);

        // Exclude from capture if requested to prevent OCR feedback loops
        if hide_from_capture {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        } else {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
        }
    }
    let _ = hwnd_raw;
    let _ = hide_from_capture;
}

/// Excludes the window from screen capture without making it color-keyed transparent.
pub fn set_window_capture_exclusion(hwnd_raw: isize, hide_from_capture: bool) {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        if hide_from_capture {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        } else {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
        }
    }
    let _ = hwnd_raw;
    let _ = hide_from_capture;
}

/// Sets the global window alpha transparency, preserving the color key.
pub fn set_window_alpha(hwnd_raw: isize, alpha: u8) {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if (ex_style & WS_EX_LAYERED.0 as i32) == 0 {
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as i32);
        }
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_COLORKEY | LWA_ALPHA);
    }
    let _ = hwnd_raw;
    let _ = alpha;
}

/// Clears the custom window region, restoring the window to solid (no holes).
pub fn clear_window_region(hwnd_raw: isize) {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let _ = SetWindowRgn(hwnd, None, true);
    }
    let _ = hwnd_raw;
}

/// Boosts the current process priority to Above Normal to ensure
/// background threads (OCR/Translation) get enough CPU cycles during gaming.
pub fn boost_process_priority() {
    #[cfg(target_os = "windows")]
    unsafe {
        let handle = GetCurrentProcess();
        let _ = SetPriorityClass(handle, ABOVE_NORMAL_PRIORITY_CLASS);
    }
}
