#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod adapters;
mod infrastructure;
mod user_interface;

#[cfg(windows)]
fn redirect_stdout_to_nul() {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;
    
    if let Ok(file) = OpenOptions::new().write(true).open("NUL") {
        let handle = file.as_raw_handle();
        unsafe extern "system" {
            fn SetStdHandle(n_std_handle: u32, h_handle: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            let _ = SetStdHandle(4294967285, handle as *mut _); // STD_OUTPUT_HANDLE
            let _ = SetStdHandle(4294967284, handle as *mut _); // STD_ERROR_HANDLE
        }
        // Leak the file to keep the handle open
        std::mem::forget(file);
    }
}

#[tokio::main]
async fn main() -> eframe::Result<()> {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetConsoleWindow() -> *mut std::ffi::c_void;
        }
        let has_console = unsafe { !GetConsoleWindow().is_null() };
        if !has_console {
            redirect_stdout_to_nul();
        }
    }

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("ktranslator=info,info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    tracing::info!("KTranslator starting up...");
    #[cfg(windows)]
    {
        use windows::Win32::UI::HiDpi::*;
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        use windows::core::PCWSTR;
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            // Set AppUserModelID to ensure taskbar icon shows correctly
            let app_id: Vec<u16> = "KTranslator.V2.App".encode_utf16().chain(Some(0)).collect();
            let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR(app_id.as_ptr()));
        }
    }

    let icon = include_bytes!("../assets/icons/icon.png");
    let icon_image = image::load_from_memory(icon)
        .expect("Failed to load app icon from assets/icons/icon.png")
        .resize(256, 256, image::imageops::FilterType::Lanczos3) // Resize for better compatibility
        .to_rgba8();
    let (width, height) = icon_image.dimensions();
    let icon_data = egui::IconData {
        rgba: icon_image.into_raw(),
        width,
        height,
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_icon(std::sync::Arc::new(icon_data)),
        ..Default::default()
    };
    eframe::run_native(
        "KTranslator",
        native_options,
        Box::new(|cc| Ok(Box::new(user_interface::App::new(cc)))),
    )
}
