#[cfg(target_os = "windows")]
use std::ptr;
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicPtr, Ordering};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetLayeredWindowAttributes, LWA_COLORKEY, LWA_ALPHA,
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    CreateWindowExW, DefWindowProcW, RegisterClassW,
    WS_EX_LAYERED, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    WM_NCHITTEST, WM_DESTROY, WM_PAINT,
    HTCAPTION, CS_HREDRAW, CS_VREDRAW,
    WNDCLASSW, WINDOW_STYLE, WINDOW_EX_STYLE,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    GetDC, ReleaseDC,
    AC_SRC_OVER, AC_SRC_ALPHA,
    CreateCompatibleDC, DeleteDC, DeleteObject, SelectObject,
    CreateDIBSection, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    HDC,
};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::POINT;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::SIZE;
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawBLENDFUNCTION {
    BlendOp: u8,
    BlendFlags: u8,
    SourceConstantAlpha: u8,
    AlphaFormat: u8,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn UpdateLayeredWindow(
        hwnd: HWND,
        hdcDst: HDC,
        pptDst: *const POINT,
        psize: *const SIZE,
        hdcSrc: HDC,
        pptSrc: *const POINT,
        crKey: u32,
        pblend: *const RawBLENDFUNCTION,
        dwFlags: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
static OVERLAY_WINDOW_CLASS: AtomicPtr<u16> = AtomicPtr::new(ptr::null_mut());

/// Registers the window class for draggable overlay windows.
#[cfg(target_os = "windows")]
fn register_overlay_window_class() -> Result<(), anyhow::Error> {
    let class_name = "KTranslatorOverlayWindow\0".encode_utf16().collect::<Vec<u16>>();

    unsafe {
        let hinstance = GetModuleHandleW(PCWSTR::null())?;

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_window_proc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        if RegisterClassW(&wnd_class) == 0 {
            // Class might already be registered, check error
            let error = windows::Win32::Foundation::GetLastError();
            if error != windows::Win32::Foundation::WIN32_ERROR(141) { // ERROR_CLASS_ALREADY_EXISTS
                return Err(anyhow::anyhow!("Failed to register window class: {:?}", error));
            }
        }

        // Store the class name for later use
        let class_name_ptr = class_name.leak().as_ptr() as *mut u16;
        OVERLAY_WINDOW_CLASS.store(class_name_ptr, Ordering::Release);
    }

    Ok(())
}

/// Window procedure for draggable overlay windows.
#[cfg(target_os = "windows")]
unsafe extern "system" fn overlay_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            // Make the entire window draggable by returning HTCAPTION
            LRESULT(HTCAPTION as isize)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0); }
            LRESULT(0)
        }
        WM_PAINT => {
            // Paint will be handled by the rendering system
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;

/// Creates a draggable layered overlay window with transparency support.
#[cfg(target_os = "windows")]
pub fn create_draggable_overlay_window(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    title: &str,
) -> Result<HWND, anyhow::Error> {
    // Register window class if not already registered
    register_overlay_window_class()?;

    let title_w: Vec<u16> = format!("{}\0", title).encode_utf16().collect();
    let class_ptr = OVERLAY_WINDOW_CLASS.load(Ordering::Acquire);

    if class_ptr.is_null() {
        return Err(anyhow::anyhow!("Window class not registered"));
    }

    unsafe {
        let hinstance = GetModuleHandleW(PCWSTR::null())?;

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_TOPMOST.0 | WS_EX_TRANSPARENT.0),
            PCWSTR(class_ptr),
            PCWSTR(title_w.as_ptr()),
            WINDOW_STYLE(WS_POPUP.0),
            x,
            y,
            width,
            height,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;

        // Initially make the window fully transparent
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);

        Ok(hwnd)
    }
}

/// Updates the layered window with new content and transparency using UpdateLayeredWindow.
#[cfg(target_os = "windows")]
pub fn update_layered_window_content(
    hwnd: HWND,
    bitmap_data: &[u8],
    width: i32,
    height: i32,
    alpha: u8,
) -> Result<(), anyhow::Error> {
    unsafe {
        let hdc_screen = GetDC(None);
        if hdc_screen.is_invalid() {
            return Err(anyhow::anyhow!("Failed to get screen DC"));
        }

        // Create a compatible DC
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            ReleaseDC(None, hdc_screen);
            return Err(anyhow::anyhow!("Failed to create compatible DC"));
        }

        // Create DIB section for the bitmap
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // Negative for top-down DIB (egui uses top-left origin)
                biPlanes: 1,
                biBitCount: 32, // 32-bit RGBA
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };

        let mut ppv_bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbitmap = CreateDIBSection(
            Some(hdc_screen),
            &bmi,
            windows::Win32::Graphics::Gdi::DIB_USAGE(0), // DIB_RGB_COLORS
            &mut ppv_bits,
            None,
            0,
        )?;

        if hbitmap.is_invalid() {
            ReleaseDC(None, hdc_screen);
            DeleteDC(hdc_mem);
            return Err(anyhow::anyhow!("Failed to create DIB section"));
        }

        // Select the bitmap into the DC
        let hbitmap_old = SelectObject(hdc_mem, hbitmap.into());

        // Copy bitmap data to DIB section
        if !ppv_bits.is_null() {
            let dst_slice = std::slice::from_raw_parts_mut(
                ppv_bits as *mut u8,
                (width * height * 4) as usize,
            );

            // Convert RGBA to BGRA (Windows expects BGRA)
            for i in (0..bitmap_data.len()).step_by(4) {
                if i + 3 < bitmap_data.len() {
                    let r = bitmap_data[i];
                    let g = bitmap_data[i + 1];
                    let b = bitmap_data[i + 2];
                    let a = bitmap_data[i + 3];

                    dst_slice[i] = b;
                    dst_slice[i + 1] = g;
                    dst_slice[i + 2] = r;
                    dst_slice[i + 3] = a;
                }
            }
        }

        // Set up blend function for raw FFI
        let blend = RawBLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: alpha,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        let pt_src = POINT { x: 0, y: 0 };
        let size_window = SIZE { cx: width, cy: height };
        let pt_dst = POINT { x: 0, y: 0 };

        // Update the layered window using raw FFI
        let result = UpdateLayeredWindow(
            hwnd,
            hdc_screen,
            &pt_dst,
            &size_window,
            hdc_mem,
            &pt_src,
            0, // crKey
            &blend,
            2, // ULW_ALPHA
        );

        // Cleanup
        SelectObject(hdc_mem, hbitmap_old);
        DeleteObject(hbitmap.into());
        DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        if result != 0 {
            Ok(())
        } else {
            Err(anyhow::anyhow!("UpdateLayeredWindow failed"))
        }
    }
}

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

/// Boosts the current process priority to Above Normal to ensure 
/// background threads (OCR/Translation) get enough CPU cycles during gaming.
pub fn boost_process_priority() {
    #[cfg(target_os = "windows")]
    unsafe {
        let handle = GetCurrentProcess();
        let _ = SetPriorityClass(handle, ABOVE_NORMAL_PRIORITY_CLASS);
    }
}
