use std::sync::{mpsc, Arc};

use eframe::egui;
use parking_lot::Mutex;

use crate::{
    adapters::translate::create_translator,
    core::{
        background_result_dispatcher::ResultDispatcher, coordinator::BackgroundCoordinator,
        region_slot_state::AppModel, region_slot_state::SlotRuntimeState,
    },
    infrastructure::settings::{save_settings, Settings},
    user_interface::{
        components::{region_slot_panel::render_slot_item, settings_ui::show_settings_window},
        i18n::get_i18n,
        live_frame,
        region_selection_overlay::{run_region_viewport, RegionOutcome, RegionOverlayState},
        transparent_overlay_renderer,
    },
};

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub(crate) model: Arc<Mutex<AppModel>>,
    pub(crate) settings: Settings,
    pub(crate) show_settings: bool,
    /// true once when user opens Settings: try to fetch models from API
    pub(crate) settings_fetch_models_pending: bool,
    pub(crate) err_handler: crate::core::usecases::error_handler::ErrorHandler,
    pub(crate) settings_ctrl: crate::core::usecases::settings_controller::SettingsController,

    /// Fullscreen region pick / adjust overlay (one at a time).
    pub(crate) region_session: Option<Arc<Mutex<RegionOverlayState>>>,
    pub(crate) region_finish: Arc<Mutex<Option<RegionOutcome>>>,

    pub(crate) services: crate::user_interface::application_services::PipelineServices,

    // Background processing
    pub(crate) coordinator: BackgroundCoordinator,
    pub(crate) slots_runtime: Vec<SlotRuntimeState>,

    /// Available displays for capturing (ID, Label)
    pub(crate) available_screens: Vec<(u32, String)>,

    pub(crate) caches: crate::user_interface::application_services::AppCaches,

    /// Channel to signal error dismissal from the error viewport
    pub(crate) error_dismiss_tx: mpsc::Sender<()>,
    pub(crate) error_dismiss_rx: mpsc::Receiver<()>,

    pub(crate) downloads: crate::user_interface::application_services::DownloadManager,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::user_interface::application_initializer::build_app(cc)
    }

    fn ui_popups(&mut self, ctx: &egui::Context) {
        let snapshot = { self.model.lock().clone() };
        for (i, slot) in snapshot.slots.iter().enumerate() {
            if !slot.popup_open {
                continue;
            }
            transparent_overlay_renderer::render_popup_viewport(ctx, i, &self.model);
        }
    }

    fn ui_frames(&mut self, ctx: &egui::Context) {
        let snapshot = { self.model.lock().clone() };
        for (i, slot) in snapshot.slots.iter().enumerate() {
            if !slot.enabled {
                continue;
            }
            if self.slots_runtime.len() <= i {
                self.slots_runtime.push(SlotRuntimeState::new());
            }

            transparent_overlay_renderer::render_overlay_viewport(
                ctx,
                i,
                &self.model,
                &self.slots_runtime[i],
                &self.settings,
                &self.services.platform,
            );

            live_frame::render_live_frame_viewport(
                ctx,
                i,
                &self.model,
                &self.slots_runtime[i],
                &self.settings,
                &self.services.platform,
            );
        }
    }

    // -----------------------------------------------------------------------
    // Background processing: capture → compare → OCR+Translate (if changed)
    // -----------------------------------------------------------------------

    fn tick_background(&mut self, ctx: &egui::Context) -> bool {
        // 1. Process pending signals from popups/error window
        while self.error_dismiss_rx.try_recv().is_ok() {
            self.err_handler.clear_all();
        }

        // 1a. Periodic cache cleanup based on memory_cleanup_interval_secs
        let now = crate::core::utils::now_ms();
        let last_cleanup = *self.caches.last_cleanup_time.lock();
        let cleanup_interval_ms = self.settings.perf.memory_cleanup_interval_secs * 1000;
        if now.saturating_sub(last_cleanup) >= cleanup_interval_ms {
            self.enforce_cache_limits();
            *self.caches.last_cleanup_time.lock() = now;
            tracing::info!("Periodic cache cleanup completed");
        }

        // 1b. Handle Download Triggers
        while let Ok(engine_type) = self.downloads.trigger_rx.try_recv() {
            self.model.lock().download_progress.is_downloading = true; // Force UI awake
            let actual_tx = self.downloads.progress_tx.clone();
            let ctx_clone = ctx.clone();
            let model_clone = self.model.clone();

            // DROPPING THE CURRENT ENGINE TO RELEASE FILE LOCKS!
            // If the user clicks Reinstall, the current engine might be locking the .onnx files.
            // By replacing it with WindowsOcr, the old engine's Arc drops and ONNX Runtime releases the file handles.
            self.services.ocr_engine = std::sync::Arc::new(crate::adapters::ocr::windows_native_ocr_adapter::WindowsOcr::new());
            // Also idle the pipeline slots just in case they were holding any references
            for slot in &mut self.slots_runtime {
                slot.status = "Idle".to_string();
            }
            
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async move {
                    let (proxy_tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<crate::core::types::DownloadProgress>();
                    
                    // Relay task to wake up UI when progress updates
                    let proxy_handle = tokio::spawn(async move {
                        while let Some(msg) = proxy_rx.recv().await {
                            model_clone.lock().download_progress = msg.clone();
                            let _ = actual_tx.send(msg);
                            ctx_clone.request_repaint();
                            ctx_clone.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
                        }
                    });

                    match engine_type {
                        crate::infrastructure::settings::OcrEngineType::MangaOCR => {
                            let _ = crate::infrastructure::asset_download_manager::download_models(proxy_tx, true).await;
                        }
                        crate::infrastructure::settings::OcrEngineType::BuiltinPaddle => {
                            let _ = crate::infrastructure::asset_download_manager::download_ppocr_models(proxy_tx, true).await;
                        }
                        crate::infrastructure::settings::OcrEngineType::BubbleYOLO => {
                            let _ = crate::infrastructure::asset_download_manager::download_bubble_yolo_model(proxy_tx, true).await;
                        }
                        crate::infrastructure::settings::OcrEngineType::CraftDetector => {
                            let _ = crate::infrastructure::asset_download_manager::download_craft_model(proxy_tx, true).await;
                        }
                        _ => {}
                    }

                    // Wait for the proxy task to process all pending messages before dropping the runtime
                    let _ = proxy_handle.await;
                });
            });
        }

        // 1c. Handle Download Progress
        while let Ok(prog) = self.downloads.progress_rx.try_recv() {
            let was_downloading = self.model.lock().download_progress.is_downloading;
            self.model.lock().download_progress = prog.clone();
            ctx.request_repaint(); // Wake up UI to show progress
            ctx.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport")); // Wake up Settings window

            // If download just finished successfully, reload the engine
            if was_downloading && !prog.is_downloading && prog.error.is_none() {
                let factory_type =
                    crate::adapters::ocr::ocr_adapter_factory::OcrAdapterFactory::get_active_engine_type(
                        &self.settings,
                    );
                if factory_type == crate::infrastructure::settings::OcrEngineType::MangaOCR
                    || factory_type == crate::infrastructure::settings::OcrEngineType::BuiltinPaddle
                {
                    let (new_engine, err_opt) =
                        crate::adapters::ocr::ocr_adapter_factory::OcrAdapterFactory::create_engine(
                            &self.settings,
                        );
                    self.services.ocr_engine = new_engine;
                    if let Some(err) = err_opt {
                        self.err_handler.report_simple(err);
                    } else {
                        tracing::info!("{:?} reloaded successfully after download.", factory_type);
                    }
                }
            }

            ctx.request_repaint();
        }

        // 1. Delegate background logic to coordinator (Process Results FIRST)
        let processed_any = ResultDispatcher::process_results(
            &self.coordinator.bg_rx,
            &self.model,
            &mut self.slots_runtime,
            &self.err_handler,
            &self.caches.translation,
            &self.settings,
        );

        if processed_any {
            ctx.request_repaint(); // Wake up main window immediately to start fade animation
        }



        self.coordinator.tick(
            &self.model,
            &mut self.slots_runtime,
            &self.services.capture,
            &self.services.ocr_engine,
            &self.services.translator,
            &self.caches.translation,
            &self.caches.text_translation,
            &self.settings,
            &self.services.platform,
            ctx.clone(),
        );
        processed_any
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
            self.model.clone(),
            self.downloads.trigger_tx.clone(),
            &self.slots_runtime,
        );

        let updated = settings_arc.lock().clone();
        if updated != self.settings {
            let current_engine_type =
                crate::adapters::ocr::ocr_adapter_factory::OcrAdapterFactory::get_active_engine_type(
                    &updated,
                );
            let old_engine_type =
                crate::adapters::ocr::ocr_adapter_factory::OcrAdapterFactory::get_active_engine_type(
                    &self.settings,
                );
            let rebuild_ocr = current_engine_type != old_engine_type
                || updated.perf.gpu_backend != self.settings.perf.gpu_backend;

            let trans_behavior_changed = updated.trans_behavior != self.settings.trans_behavior;
            let realtime_changed = updated.realtime != self.settings.realtime;
            let txt_proc_changed = updated.txt_proc != self.settings.txt_proc;
            let rebuild_trans = updated.provider != self.settings.provider
                || updated.gemini_api_key != self.settings.gemini_api_key
                || updated.gemini_model != self.settings.gemini_model
                || updated.groq_api_key != self.settings.groq_api_key
                || updated.groq_model != self.settings.groq_model
                || updated.ollama_url != self.settings.ollama_url
                || updated.ollama_model != self.settings.ollama_model
                || updated.custom_openai_url != self.settings.custom_openai_url
                || updated.custom_openai_api_key != self.settings.custom_openai_api_key
                || updated.custom_openai_model != self.settings.custom_openai_model
                || trans_behavior_changed;

            self.settings = updated;
            if let Err(e) = save_settings(&self.settings) {
                self.err_handler.report_simple(format!("{e:#}"));
            } else {
                if rebuild_trans {
                    self.services.translator = create_translator(&self.settings);
                    if trans_behavior_changed {
                        self.caches.translation.lock().clear();
                        self.caches.text_translation.lock().clear();
                    }
                }
                if realtime_changed || txt_proc_changed {
                    for rt in &mut self.slots_runtime {
                        rt.recent_translations.clear();
                    }
                    self.caches.translation.lock().clear();
                    self.caches.text_translation.lock().clear();
                }

                if rebuild_ocr {
                    let (new_engine, err_opt) =
                        crate::adapters::ocr::ocr_adapter_factory::OcrAdapterFactory::create_engine(
                            &self.settings,
                        );
                    self.services.ocr_engine = new_engine;
                    if let Some(err) = err_opt {
                        self.err_handler.report_simple(err);
                    }
                }
            }

            // Request immediate repaint to show live update results on screen
            ctx.request_repaint();
        }

        let close_requested = ctx
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new("settings_close_requested")))
            .unwrap_or(false);

        if resp.close_clicked || close_requested {
            self.show_settings = false;
            self.settings_ctrl.end_edit();
            ctx.send_viewport_cmd_to(
                egui::ViewportId::from_hash_of("settings_viewport"),
                egui::ViewportCommand::Close,
            );
            ctx.request_repaint();
        }
    }

    fn ui_error_popup(&mut self, ctx: &egui::Context) {
        if !self.err_handler.has_errors() {
            return;
        }

        let viewport_id = egui::ViewportId::from_hash_of("error_popup");
        let tx = self.error_dismiss_tx.clone();
        let errors: Vec<String> = self
            .err_handler
            .get_all_errors()
            .into_iter()
            .map(|e| e.message)
            .collect();

        ctx.show_viewport_deferred(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("KTranslator - Error Report")
                .with_inner_size([450.0, 220.0])
                .with_always_on_top()
                .with_decorations(true)
                .with_resizable(false),
            move |ctx, _| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    let _ = tx.send(());
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.heading(
                            egui::RichText::new("[!] System Error")
                                .color(egui::Color32::from_rgb(255, 80, 80))
                                .strong(),
                        );
                        ui.add_space(10.0);

                        egui::ScrollArea::vertical()
                            .max_height(120.0)
                            .show(ui, |ui| {
                                for err in &errors {
                                    ui.label(egui::RichText::new(err).size(14.0));
                                    ui.add_space(4.0);
                                }
                            });

                        ui.add_space(15.0);
                        if ui
                            .button(egui::RichText::new(" Dismiss All Errors ").size(16.0))
                            .clicked()
                        {
                            let _ = tx.send(());
                        }
                    });
                });
            },
        );
    }

    /// Enforces cache size limits by removing oldest entries if cache exceeds max_cache_entries
    fn enforce_cache_limits(&self) {
        let max_entries = self.settings.perf.max_cache_entries;

        // Trim translation cache
        {
            let mut cache = self.caches.translation.lock();
            if cache.len() > max_entries {
                let to_remove = cache.len() - max_entries;
                for _ in 0..to_remove {
                    cache.shift_remove_index(0);
                }
                tracing::info!(
                    "Trimmed translation cache: removed {} oldest entries",
                    to_remove
                );
            }
        }

        // Trim text translation cache
        {
            let mut cache = self.caches.text_translation.lock();
            if cache.len() > max_entries {
                let to_remove = cache.len() - max_entries;
                for _ in 0..to_remove {
                    cache.shift_remove_index(0);
                }
                tracing::info!("Trimmed text cache: removed {} oldest entries", to_remove);
            }
        }
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
        let processed_any = self.tick_background(ctx);
        self.ui_error_popup(ctx);

        // Apply capture exclusion to main window and settings window if they are open
        let main_title = "KTranslator";
        if let Some(hwnd) = self.services.platform.find_window_by_title(main_title) {
            self.services.platform.set_window_capture_exclusion(hwnd, self.settings.hide_from_capture);
        }
        let settings_title = format!("KTranslator - {}", i18n.settings);
        if let Some(hwnd) = self.services.platform.find_window_by_title(&settings_title) {
            self.services.platform.set_window_capture_exclusion(hwnd, self.settings.hide_from_capture);
        }

        // Track if user interacted with UI to trigger a child sync (e.g. clicked Clear Cache)
        let ui_interacted = ctx.is_using_pointer() || ctx.wants_keyboard_input() || ctx.input(|i| i.pointer.any_click());
        // Settings window can also request a sync by setting this flag
        let force_sync = ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("force_sync_children"))).unwrap_or(false);
        let should_sync_children = processed_any || ui_interacted || force_sync;

        if let Some(sess) = &self.region_session {
            run_region_viewport(
                ctx,
                sess.clone(),
                self.region_finish.clone(),
                self.settings.ui_language,
            );
        }
        if let Some(out) = self.region_finish.lock().take() {
            match out {
                RegionOutcome::Done { slot, rect } => {
                    if let Some(s) = self.model.lock().slots.get_mut(slot) {
                        s.rect = Some(rect);
                    }
                }
                RegionOutcome::Cancelled => {}
            }
            self.region_session = None;
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

                    let button = egui::Button::new(
                        egui::RichText::new(btn_text)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
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
                        let theme_icon = if self.settings.dark_mode {
                            "🌙"
                        } else {
                            "🔆"
                        };
                        if ui
                            .button(theme_icon)
                            .on_hover_text(i18n.toggle_dark_mode)
                            .clicked()
                        {
                            self.settings.dark_mode = !self.settings.dark_mode;
                            if let Some(edit_arc) = &self.settings_ctrl.settings_edit {
                                edit_arc.lock().dark_mode = self.settings.dark_mode;
                            }
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
                            ctx.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("force_sync_children"), true));
                            let _ = save_settings(&self.settings);
                        }

                        if ui
                            .button("⚙")
                            .on_hover_text(i18n.open_settings_desc)
                            .clicked()
                        {
                            // Clear any leftover "poison pill" close requests from a previous session
                            ctx.data_mut(|d| {
                                d.remove_temp::<bool>(egui::Id::new("settings_close_requested"))
                            });
                            self.show_settings = true;
                            self.settings_fetch_models_pending = true;
                            let _ = self.settings_ctrl.begin_edit(&self.settings);
                        }

                        if ui
                            .button("🔄")
                            .on_hover_text(i18n.clear_cache_desc)
                            .clicked()
                        {
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
                                runtime.last_stable_ocr_text.clear();
                                runtime.persistent_translation.lock().take();
                                runtime.persistent_ocr_lines.lock().clear();
                                runtime.persistent_trans_lines.lock().clear();
                                runtime.recent_translations.clear();
                            }
                            self.caches.translation.lock().clear();
                            self.caches.text_translation.lock().clear();
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
                    let resp = render_slot_item(
                        ui,
                        i,
                        &mut model,
                        &self.slots_runtime[i],
                        &self.available_screens,
                        self.settings.ui_language,
                    );
                    if resp.should_remove {
                        remove_idx = Some(i);
                    }
                    if resp.do_crop {
                        let display_id = model.slots[i].display_id;
                        drop(model);
                        match RegionOverlayState::start(i, display_id, ui.ctx(), None) {
                            Ok(st) => {
                                *self.region_finish.lock() = None;
                                self.region_session = Some(Arc::new(Mutex::new(st)));
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
                if ui.button(format!("➕ {}", i18n.add_region)).clicked() {
                    let mut model = self.model.lock();
                    model.add_slot();
                    self.slots_runtime.push(SlotRuntimeState::new());
                }

                ui.add_space(8.0);
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(i18n.tip_borderless).small().weak());
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
        if (current_size.y - required_height).abs() > 2.0
            || (current_size.x - required_width).abs() > 2.0
        {
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
            let now = crate::core::utils::now_ms();
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

        // Keep polling while any slot has a background task running or if a download is active.
        // Without this, eframe may sleep and never call update() to collect results.
        let is_downloading = self.model.lock().download_progress.is_downloading;
        let any_slot_busy = self.slots_runtime.iter().any(|r| r.busy);
        if any_slot_busy || is_downloading {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        
        // Sync active child viewports only when their visual state might have changed.
        // This avoids spamming wgpu with 144+ repaint requests per second unnecessarily,
        // which was causing severe lock contention, stuttering, and eventual GPU deadlocks.
        if should_sync_children {
            let m = self.model.lock();
            let num_slots = m.slots.len();
            for i in 0..num_slots {
                let slot = &m.slots[i];
                if slot.enabled {
                    if slot.show_frame && slot.rect.is_some() {
                        ctx.request_repaint_of(egui::ViewportId::from_hash_of(format!("frame_live_{}", i)));
                    }
                    if slot.overlay_mode && slot.rect.is_some() {
                        ctx.request_repaint_of(egui::ViewportId::from_hash_of(format!("frame_overlay_{}", i)));
                    }
                }
                if slot.popup_open {
                    ctx.request_repaint_of(egui::ViewportId::from_hash_of(format!("popup_{}", i)));
                }
            }
        }
    }
}
