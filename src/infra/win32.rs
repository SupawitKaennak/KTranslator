#[cfg(target_os = "windows")]
use std::ptr;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::COLORREF;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetLayeredWindowAttributes, LWA_COLORKEY, LWA_ALPHA,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
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
pub fn apply_overlay_attributes(hwnd_raw: isize) {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::Foundation::HWND;
        let hwnd = HWND(hwnd_raw as *mut _);
        
        // Apply color key for transparency. 
        // We use LWA_COLORKEY | LWA_ALPHA and set alpha to 255 (opaque) 
        // because we want egui's own alpha handling to work, but the window 
        // itself MUST be considered 'there' by the OS to be captured.
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0x000000), 255, LWA_COLORKEY | LWA_ALPHA);
        
        // REMOVED: SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        // This was preventing Snipping Tool from seeing the overlay.
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
