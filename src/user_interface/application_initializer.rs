use eframe::egui;
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};

use crate::{
    adapters::{
        capture::screenshots_crate_adapter::ScreenshotsCapture, translate::create_translator,
    },
    core::{
        coordinator::BackgroundCoordinator,
        region_slot_state::{AppModel, SlotRuntimeState},
    },
    infrastructure::{
        platform::{self, PlatformServices},
        settings::load_settings,
    },
    user_interface::{
        application::App,
        application_services::{AppCaches, DownloadManager, PipelineServices},
        font_loader_setup,
    },
};

pub fn build_app(cc: &eframe::CreationContext<'_>) -> App {
    // ── Font setup: multi-script support ─────────────────────────────
    // egui's default fonts cover Latin, Cyrillic, and Greek.
    // We add the following fallbacks so every translated script renders:
    //   • Thai          → NotoSansThai (embedded, guaranteed)
    //   • CJK           → Microsoft YaHei / MS Gothic / Malgun Gothic (Windows system)
    //   • Arabic/Hebrew → Arial / Tahoma (Windows system)
    //   • Devanagari    → Nirmala UI / Mangal (Windows system)
    font_loader_setup::setup_fonts(&cc.egui_ctx);

    let settings = load_settings().unwrap_or_default();
    let platform: Arc<dyn PlatformServices> = Arc::from(platform::create_platform());
    platform.boost_process_priority();

    let translator = create_translator(&settings);

    let (err_tx, err_rx) = mpsc::channel();

    if settings.dark_mode {
        let mut visuals = egui::Visuals::dark();
        visuals.window_corner_radius = 6.0.into();
        visuals.widgets.noninteractive.corner_radius = 6.0.into();
        visuals.widgets.inactive.corner_radius = 6.0.into();
        visuals.widgets.hovered.corner_radius = 6.0.into();
        visuals.widgets.active.corner_radius = 6.0.into();
        visuals.widgets.open.corner_radius = 6.0.into();
        cc.egui_ctx.set_visuals(visuals);
    } else {
        let mut visuals = egui::Visuals::light();
        visuals.window_corner_radius = 6.0.into();
        visuals.widgets.noninteractive.corner_radius = 6.0.into();
        visuals.widgets.inactive.corner_radius = 6.0.into();
        visuals.widgets.hovered.corner_radius = 6.0.into();
        visuals.widgets.active.corner_radius = 6.0.into();
        visuals.widgets.open.corner_radius = 6.0.into();
        cc.egui_ctx.set_visuals(visuals);
    }

    let coordinator = BackgroundCoordinator::new();

    let err_handler = crate::core::usecases::error_handler::ErrorHandler::new();
    let (ocr_engine, _) =
        crate::adapters::ocr::ocr_adapter_factory::OcrAdapterFactory::create_engine(&settings);

    let (dt_tx, dt_rx) = std::sync::mpsc::channel();
    let (dp_tx, dp_rx) = tokio::sync::mpsc::unbounded_channel();

    let caches = AppCaches {
        translation: Arc::new(Mutex::new(indexmap::IndexMap::new())),
        text_translation: Arc::new(Mutex::new(indexmap::IndexMap::new())),
        last_cleanup_time: Arc::new(Mutex::new(crate::core::utils::now_ms())),
    };

    let downloads = DownloadManager {
        trigger_tx: dt_tx,
        trigger_rx: dt_rx,
        progress_tx: dp_tx,
        progress_rx: dp_rx,
    };

    let services = PipelineServices {
        capture: Arc::new(ScreenshotsCapture::new()),
        platform: platform.clone(),
        ocr_engine,
        translator,
    };

    App {
        model: Arc::new(Mutex::new(AppModel::new_default())),
        settings,
        show_settings: false,
        settings_fetch_models_pending: false,
        err_handler,
        settings_ctrl: crate::core::usecases::settings_controller::SettingsController::new(),
        region_session: None,
        region_finish: Arc::new(Mutex::new(None)),
        services,
        coordinator,
        slots_runtime: vec![SlotRuntimeState::new()],
        available_screens: screenshots::Screen::all()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let w = s.display_info.width;
                let h = s.display_info.height;
                let label = if s.display_info.is_primary {
                    format!("Primary {}x{} (Screen {})", w, h, s.display_info.id)
                } else {
                    format!("{}x{} (Screen {})", w, h, s.display_info.id)
                };
                (s.display_info.id, label)
            })
            .collect(),
        caches,
        error_dismiss_tx: err_tx,
        error_dismiss_rx: err_rx,
        downloads,
        settings_sync_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        settings_save_pending: false,
        last_settings_update_ms: 0,
    }
}
