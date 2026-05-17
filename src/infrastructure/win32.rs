#[cfg(target_os = "windows")]
use std::ptr;
#[cfg(target_os = "windows")]
use windows::core::HSTRING;
#[cfg(target_os = "windows")]
use windows::Data::Text::WordsSegmenter;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{COLORREF, HWND};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetLayeredWindowAttributes, LWA_COLORKEY, LWA_ALPHA,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, DeleteObject, SetWindowRgn, RGN_XOR,
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

/// Excludes or includes a window from screen capture (no transparency attributes).
pub fn set_hide_from_capture(hwnd_raw: isize, hide: bool) {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        if hide {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        } else {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
        }
    }
    let _ = hwnd_raw;
    let _ = hide;
}

/// Applies transparency color-key and capture exclusion to a window.
pub fn apply_overlay_attributes(hwnd_raw: isize, hide_from_capture: bool) {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        
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

/// Hollow window region: only the border ring receives mouse hits; center is click-through.
/// Borders can be specified individually for each side.
pub fn set_hollow_window_region(
    hwnd_raw: isize,
    width: i32,
    height: i32,
    top: i32,
    left: i32,
    right: i32,
    bottom: i32,
) {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let outer = CreateRectRgn(0, 0, width, height);
        // Create the "hole" in the middle
        let inner = CreateRectRgn(left, top, width - right, height - bottom);
        let frame = CreateRectRgn(0, 0, 0, 0);
        
        if outer.0.is_null() || inner.0.is_null() || frame.0.is_null() {
            if !outer.0.is_null() { let _ = DeleteObject(outer.into()); }
            if !inner.0.is_null() { let _ = DeleteObject(inner.into()); }
            return;
        }
        
        let _ = CombineRgn(Some(frame), Some(outer), Some(inner), RGN_XOR);
        let _ = SetWindowRgn(hwnd, Some(frame), true);
        let _ = DeleteObject(outer.into());
        let _ = DeleteObject(inner.into());
    }
    let _ = (hwnd_raw, width, height, top, left, right, bottom);
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

/// Uses Windows.Globalization.WordsSegmenter to break Thai text into words.
/// This ensures proper wrapping in the UI for Thai language which doesn't use spaces.
pub fn segment_thai(
    text: &str,
    mode: crate::infrastructure::settings::ThaiSegmentationMode,
) -> String {
    use crate::infrastructure::settings::ThaiSegmentationMode;

    if !text.chars().any(|c| (c as u32) >= 0x0E01 && (c as u32) <= 0x0E5B) {
        return text.to_string();
    }

    match mode {
        ThaiSegmentationMode::Standard => {
            if text.contains(' ') {
                return text.to_string();
            }
        }
        ThaiSegmentationMode::SyllableLevel => {
            return syllable_segment_thai(text);
        }
        ThaiSegmentationMode::DictionaryAssisted => {}
    }

    #[cfg(target_os = "windows")]
    {
        let segmenter = WordsSegmenter::CreateWithLanguage(&HSTRING::from("th-TH"));
        if let Ok(segmenter) = segmenter {
            let tokens = segmenter.GetTokens(&HSTRING::from(text));
            if let Ok(tokens) = tokens {
                let mut result = String::with_capacity(text.len() + 10);
                for token in tokens {
                    if let Ok(word_text) = token.Text() {
                        result.push_str(&word_text.to_string());
                        result.push(' ');
                    }
                }
                return result.trim().to_string();
            }
        }
    }
    text.to_string()
}

/// Syllable-level breaks using Thai combining-mark clusters (no dictionary).
fn syllable_segment_thai(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    let mut prev_base = false;
    for c in text.chars() {
        let u = c as u32;
        let is_mark = (0x0E30..=0x0E3A).contains(&u)
            || (0x0E40..=0x0E44).contains(&u)
            || (0x0E47..=0x0E4E).contains(&u);
        let is_base = (0x0E01..=0x0E2E).contains(&u);
        if is_base && prev_base {
            out.push('\u{200B}');
        }
        out.push(c);
        prev_base = is_base && !is_mark;
    }
    out
}
