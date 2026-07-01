// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod core;
mod infrastructure;
mod user_interface;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("ktranslator=debug,debug"));

    tracing_subscriber::fmt().with_env_filter(filter).init();



    tracing::info!("KTranslator starting up...");
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::UI::HiDpi::*;
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
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
            .with_icon(std::sync::Arc::new(icon_data))
            .with_always_on_top()
            .with_resizable(false),
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "KTranslator",
        native_options,
        Box::new(|cc| Ok(Box::new(user_interface::App::new(cc)))),
    )
}
