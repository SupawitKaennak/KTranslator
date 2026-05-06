mod core;
mod adapters;
mod infra;
mod ui;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("KTranslator starting up...");
    #[cfg(windows)]
    {
        use windows::Win32::UI::HiDpi::*;
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_transparent(true),
        ..Default::default()
    };
    eframe::run_native(
        "KTranslator",
        native_options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}
