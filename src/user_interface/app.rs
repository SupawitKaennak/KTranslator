use std::sync::{mpsc, Arc};

use eframe::egui;
use parking_lot::Mutex;

use crate::{
    adapters::{
        capture::screenshots_capture::ScreenshotsCapture,
        translate::{
            create_translator,
        },
    },
    core::{
        coordinator::BackgroundCoordinator,
        model::AppModel,
        ports::{FrameSource, OcrEngine, Translator},
        worker::SlotRuntimeState,
    },
    infrastructure::{
        settings::{load_settings, save_settings, Settings},
        platform::{self, PlatformServices},
    },
    user_interface::{
        components::{
            settings_ui::show_settings_window,
            slot_ui::render_slot_item,
        },
        crop_overlay::{run_crop_viewport, CropOutcome, CropOverlayState},
        font_loader,
        i18n::get_i18n,
        overlay_renderer,
    },
};


// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    model: Arc<Mutex<AppModel>>,
    settings: Settings,
    show_settings: bool,
    /// true once when user opens Settings: try to fetch models from API
    settings_fetch_models_pending: bool,
    err_handler: crate::core::usecases::error_handler::ErrorHandler,
    settings_ctrl: crate::core::usecases::settings_controller::SettingsController,

    /// Fullscreen drag-to-select overlay (one at a time).
    crop_session: Option<Arc<Mutex<CropOverlayState>>>,
    crop_finish: Arc<Mutex<Option<CropOutcome>>>,

    capture: Arc<dyn FrameSource>,

    /// Platform-specific services (overlay transparency, process priority)
    platform: Arc<dyn PlatformServices>,

    /// Active OCR engine (selected based on settings)
    ocr_engine: Arc<dyn OcrEngine>,

    /// Text-only translator via selected provider (Gemini/Groq/Ollama)
    translator: Option<Arc<dyn Translator + Send + Sync>>,

    // Background processing
    coordinator: BackgroundCoordinator,
    slots_runtime: Vec<SlotRuntimeState>,

    /// Available displays for capturing (ID, Label)
    available_screens: Vec<(u32, String)>,

    /// Cache for (smart_hash, source_lang, target_lang) → (ocr_text, translated_text)
    translation_cache: Arc<Mutex<std::collections::HashMap<(u64, Option<String>, String), (String, String)>>>,

    /// Cache for OCR text hash → translated_text.
    /// Catches cases where the same text appears with different pixel content
    /// (e.g., cursor blink, slight background variation) without re-calling the API.
    text_translation_cache: Arc<Mutex<std::collections::HashMap<(u64, Option<String>, String), String>>>,

    /// Channel to signal error dismissal from the error viewport
    error_dismiss_tx: mpsc::Sender<()>,
    error_dismiss_rx: mpsc::Receiver<()>,

    /// Model download channels
    download_trigger_tx: std::sync::mpsc::Sender<()>,
    download_trigger_rx: std::sync::mpsc::Receiver<()>,
    download_progress_rx: tokio::sync::mpsc::Receiver<crate::infrastructure::asset_manager::DownloadProgress>,
    download_progress_tx: tokio::sync::mpsc::Sender<crate::infrastructure::asset_manager::DownloadProgress>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // ── Font setup: multi-script support ─────────────────────────────
        // egui's default fonts cover Latin, Cyrillic, and Greek.
        // We add the following fallbacks so every translated script renders:
        //   • Thai          → NotoSansThai (embedded, guaranteed)
        //   • CJK           → Microsoft YaHei / MS Gothic / Malgun Gothic (Windows system)
        //   • Arabic/Hebrew → Arial / Tahoma (Windows system)
        //   • Devanagari    → Nirmala UI / Mangal (Windows system)
        font_loader::setup_fonts(&cc.egui_ctx);

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
        let (ocr_engine, err_opt) = crate::adapters::ocr::ocr_factory::OcrAdapterFactory::create_engine(&settings);
        if let Some(err) = err_opt {
            err_handler.report_simple(err);
        }

        let (dt_tx, dt_rx) = std::sync::mpsc::channel();
        let (dp_tx, dp_rx) = tokio::sync::mpsc::channel(32);

        Self {
            model: Arc::new(Mutex::new(AppModel::new_default())),
            settings,
            show_settings: false,
            settings_fetch_models_pending: false,
            err_handler,
            settings_ctrl: crate::core::usecases::settings_controller::SettingsController::new(),
            crop_session: None,
            crop_finish: Arc::new(Mutex::new(None)),
            capture: Arc::new(ScreenshotsCapture::new()),
            platform: platform.clone(),
            ocr_engine,
            translator,
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
            translation_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            text_translation_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            error_dismiss_tx: err_tx,
            error_dismiss_rx: err_rx,
            download_trigger_tx: dt_tx,
            download_trigger_rx: dt_rx,
            download_progress_tx: dp_tx,
            download_progress_rx: dp_rx,
        }
    }

    fn ui_popups(&mut self, ctx: &egui::Context) {
        let snapshot = { self.model.lock().clone() };
        for (i, slot) in snapshot.slots.iter().enumerate() {
            if !slot.popup_open { continue; }
            overlay_renderer::render_popup_viewport(ctx, i, &self.model);
        }
    }

    fn ui_frames(&mut self, ctx: &egui::Context) {
        let snapshot = { self.model.lock().clone() };
        for (i, slot) in snapshot.slots.iter().enumerate() {
            if !slot.enabled { continue; }
            if self.slots_runtime.len() <= i {
                self.slots_runtime.push(SlotRuntimeState::new());
            }
            
            overlay_renderer::render_overlay_viewport(
                ctx,
                i,
                &self.model,
                &self.slots_runtime[i],
                &self.settings,
                &self.platform,
            );
        }
    }


    // -----------------------------------------------------------------------
    // Background processing: capture → compare → OCR+Translate (if changed)
    // -----------------------------------------------------------------------



    fn tick_background(&mut self, ctx: &egui::Context) {
        // 1. Process pending signals from popups/error window
        while let Ok(_) = self.error_dismiss_rx.try_recv() {
            self.err_handler.clear_all();
        }

        // 1b. Handle Download Trigger
        while let Ok(_) = self.download_trigger_rx.try_recv() {
            let tx = self.download_progress_tx.clone();
            tokio::spawn(async move {
                let _ = crate::infrastructure::asset_manager::download_models(tx).await;
            });
        }

        // 1c. Handle Download Progress
        while let Ok(prog) = self.download_progress_rx.try_recv() {
            let was_downloading = self.model.lock().download_progress.is_downloading;
            self.model.lock().download_progress = prog.clone();
            
            // If download just finished successfully, reload the engine
            if was_downloading && !prog.is_downloading && prog.error.is_none() {
                let factory_type = crate::adapters::ocr::ocr_factory::OcrAdapterFactory::get_active_engine_type(&self.settings);
                if factory_type == crate::infrastructure::settings::OcrEngineType::MangaOCR {
                    let (new_engine, err_opt) = crate::adapters::ocr::ocr_factory::OcrAdapterFactory::create_engine(&self.settings);
                    self.ocr_engine = new_engine;
                    if let Some(err) = err_opt {
                        self.err_handler.report_simple(err);
                    } else {
                        println!("Manga-OCR reloaded successfully after download.");
                    }
                }
            }
            
            ctx.request_repaint();
        }

        // 2. Delegate background logic to coordinator
        self.coordinator.process_results(
            &self.model,
            &mut self.slots_runtime,
            &self.err_handler,
            &self.translation_cache,
        );

        self.coordinator.tick(
            &self.model,
            &mut self.slots_runtime,
            &self.capture,
            &self.ocr_engine,
            &self.translator,
            &self.translation_cache,
            &self.text_translation_cache,
            &self.settings,
            ctx.clone(),
        );
    }

    fn ui_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let settings_arc = self.settings_ctrl.begin_edit(&self.settings);
        
        let resp = show_settings_window(
            ctx, 
            settings_arc.clone(), 
            &self.settings_ctrl,
            self.model.lock().download_progress.clone(),
            self.download_trigger_tx.clone(),
            &self.slots_runtime,
        );

        let updated = settings_arc.lock().clone();
        if updated != self.settings {
            let current_engine_type = crate::adapters::ocr::ocr_factory::OcrAdapterFactory::get_active_engine_type(&updated);
            let old_engine_type = crate::adapters::ocr::ocr_factory::OcrAdapterFactory::get_active_engine_type(&self.settings);
            let rebuild_ocr = current_engine_type != old_engine_type || updated.paddle_ocr_path != self.settings.paddle_ocr_path;
            
            let rebuild_trans = updated.provider != self.settings.provider 
                || updated.gemini_api_key != self.settings.gemini_api_key
                || updated.gemini_model != self.settings.gemini_model
                || updated.groq_api_key != self.settings.groq_api_key
                || updated.groq_model != self.settings.groq_model
                || updated.ollama_url != self.settings.ollama_url
                || updated.ollama_model != self.settings.ollama_model
                || updated.custom_openai_url != self.settings.custom_openai_url
                || updated.custom_openai_api_key != self.settings.custom_openai_api_key
                || updated.custom_openai_model != self.settings.custom_openai_model;

            self.settings = updated;
            if let Err(e) = save_settings(&self.settings) {
                self.err_handler.report_simple(format!("{e:#}"));
            } else {
                if rebuild_trans {
                    self.translator = create_translator(&self.settings);
                }
                
                if rebuild_ocr {
                    let (new_engine, err_opt) = crate::adapters::ocr::ocr_factory::OcrAdapterFactory::create_engine(&self.settings);
                    self.ocr_engine = new_engine;
                    if let Some(err) = err_opt {
                        self.err_handler.report_simple(err);
                    }
                }
            }
            
            // Request immediate repaint to show live update results on screen
            ctx.request_repaint();
        }

        if resp.close_clicked {
            self.show_settings = false;
            self.settings_ctrl.end_edit();
        }
    }

    fn ui_error_popup(&mut self, ctx: &egui::Context) {
        if !self.err_handler.has_errors() { return; }
        
        let viewport_id = egui::ViewportId::from_hash_of("error_popup");
        let tx = self.error_dismiss_tx.clone();
        let errors: Vec<String> = self.err_handler.get_all_errors().into_iter().map(|e| e.message).collect();
        
        ctx.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("KTranslator - Error Report")
                .with_inner_size([450.0, 220.0])
                .with_always_on_top()
                .with_decorations(true)
                .with_resizable(false),
            move |ctx, _| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.heading(
                            egui::RichText::new("[!] System Error")
                                .color(egui::Color32::from_rgb(255, 80, 80))
                                .strong()
                        );
                        ui.add_space(10.0);
                        
                        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                            for err in &errors {
                                ui.label(egui::RichText::new(err).size(14.0));
                                ui.add_space(4.0);
                            }
                        });
                        
                        ui.add_space(15.0);
                        if ui.button(egui::RichText::new(" Dismiss All Errors ").size(16.0)).clicked() {
                            let _ = tx.send(());
                        }
                    });
                });
            }
        );
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Essential for transparent viewports: 
        // Force the GPU background clear color to be fully transparent.
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let i18n = get_i18n(self.settings.ui_language);
        self.tick_background(ctx);
        self.ui_error_popup(ctx);

        if let Some(sess) = &self.crop_session {
            run_crop_viewport(ctx, sess.clone(), self.crop_finish.clone());
        }
        if let Some(out) = self.crop_finish.lock().take() {
            match out {
                CropOutcome::Done { slot, rect } => {
                    if let Some(s) = self.model.lock().slots.get_mut(slot) {
                        s.rect = Some(rect);
                    }
                }
                CropOutcome::Cancelled => {}
            }
            self.crop_session = None;
        }

        let mut required_height: f32 = 0.0;
        let mut required_width: f32 = 520.0;

        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::side_top_panel(ctx.style().as_ref()).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("KTranslator");
                    ui.add_space(12.0);
                    
                    let mut model = self.model.lock();
                    let running = &mut model.running;
                    
                    let (btn_text, btn_color) = if *running { 
                        (i18n.stop_capture, egui::Color32::from_rgb(200, 50, 50)) 
                    } else { 
                        (i18n.start_capture, egui::Color32::from_rgb(50, 150, 50)) 
                    };

                    let button = egui::Button::new(egui::RichText::new(btn_text).color(egui::Color32::WHITE).strong())
                        .fill(btn_color)
                        .min_size(egui::vec2(100.0, 24.0));
                        
                    if ui.add(button).clicked() {
                        *running = !*running;
                        if *running {
                            // Reset all timers to trigger immediately when starting manually
                            for slot in &mut model.slots {
                                slot.next_tick_at_ms = 0;
                            }
                            // Also clear errors when manually starting
                            self.err_handler.clear_all();
                        }
                    }
                    
                    ui.add_space(8.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let theme_icon = if self.settings.dark_mode { "🌙" } else { "🔆" };
                        if ui.button(theme_icon).on_hover_text("Toggle Dark/Light mode").clicked() {
                            self.settings.dark_mode = !self.settings.dark_mode;
                            let mut visuals = if self.settings.dark_mode { 
                                egui::Visuals::dark() 
                            } else { 
                                egui::Visuals::light() 
                            };
                            // Re-apply common rounding
                            visuals.window_corner_radius = 6.0.into();
                            visuals.widgets.noninteractive.corner_radius = 6.0.into();
                            visuals.widgets.inactive.corner_radius = 6.0.into();
                            visuals.widgets.hovered.corner_radius = 6.0.into();
                            visuals.widgets.active.corner_radius = 6.0.into();
                            visuals.widgets.open.corner_radius = 6.0.into();
                            ctx.set_visuals(visuals);
                            let _ = save_settings(&self.settings);
                        }

                        if ui.button("⚙").on_hover_text("Open Settings").clicked() {
                            self.show_settings = true;
                            self.settings_fetch_models_pending = true;
                            let _ = self.settings_ctrl.begin_edit(&self.settings);
                        }

                        if ui.button("🔄").on_hover_text("Clear Cache & Force Retranslate").clicked() {
                            for slot in &mut model.slots {
                                slot.last_ocr_text.clear();
                                slot.last_translation.clear();
                                slot.stable_hash = 0;
                                slot.next_tick_at_ms = 0;
                                slot.pending_text.clear();
                            }
                            for runtime in &mut self.slots_runtime {
                                runtime.last_hash = 0;
                                runtime.first_unstable_at = 0;
                            }
                            self.translation_cache.lock().clear();
                            self.text_translation_cache.lock().clear();
                            self.settings_ctrl.reset_models_cache();
                        }
                    });
                });
                required_height += ui.min_size().y;
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let slot_count = self.model.lock().slots.len();
            
            let mut remove_idx = None;
            let content_resp = ui.vertical(|ui| {
                for i in 0..slot_count {
                    let mut model = self.model.lock();
                    let resp = render_slot_item(ui, i, &mut model, &self.slots_runtime[i], &self.available_screens, self.settings.ui_language);
                    if resp.should_remove {
                        remove_idx = Some(i);
                    }
                    if resp.do_crop {
                        let display_id = model.slots[i].display_id;
                        drop(model); // Release lock before calling start
                        match CropOverlayState::start(i, display_id, ui.ctx()) {
                            Ok(st) => {
                                *self.crop_finish.lock() = None;
                                self.crop_session = Some(Arc::new(Mutex::new(st)));
                                self.err_handler.clear_all();
                            }
                            Err(e) => {
                                self.err_handler.report_simple(format!("{e:#}"));
                            }
                        }
                    }
                    ui.add_space(8.0);
                }

                ui.add_space(8.0);
                if ui.button(format!("➕ {}", i18n.clear_results.replace("Clear Results", "Add Region").replace("ล้างหน้าจอ", "เพิ่มพื้นที่"))).clicked() {
                    let mut model = self.model.lock();
                    model.add_slot();
                    self.slots_runtime.push(SlotRuntimeState::new());
                }

                ui.add_space(8.0);
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("💡 Tip: If games don't translate, use 'Borderless Windowed' mode.").small().weak());
                });
            });

            required_height += content_resp.response.rect.height();
            // Pin the width to prevent the feedback loop growth bug
            required_width = 560.0;
            // Add padding for the window bottom
            required_height += 40.0;

            if let Some(idx) = remove_idx {
                let mut model = self.model.lock();
                model.slots.remove(idx);
                if idx < self.slots_runtime.len() {
                    self.slots_runtime.remove(idx);
                }

                // Re-align Region IDs so they match array index
                for (i, slot) in model.slots.iter_mut().enumerate() {
                    slot.id.0 = i;
                }
            }
        });

        // Request resize if the current window height is different
        let current_size = ctx.screen_rect().size();
        if (current_size.y - required_height).abs() > 2.0 || (current_size.x - required_width).abs() > 2.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                required_width,
                required_height,
            )));
        }

        self.ui_settings(ctx);
        self.ui_popups(ctx);
        self.ui_frames(ctx);

        let mut min_wait_ms = u64::MAX;
        if self.model.lock().running {
            let now = BackgroundCoordinator::now_ms();
            let m = self.model.lock();
            for (i, slot) in m.slots.iter().enumerate() {
                if slot.enabled && slot.rect.is_some() && !self.slots_runtime[i].busy {
                    let wait = slot.next_tick_at_ms.saturating_sub(now);
                    min_wait_ms = min_wait_ms.min(wait);
                }
            }
        }

        if min_wait_ms != u64::MAX {
            ctx.request_repaint_after(std::time::Duration::from_millis(min_wait_ms.max(5)));
        }
    }
}
