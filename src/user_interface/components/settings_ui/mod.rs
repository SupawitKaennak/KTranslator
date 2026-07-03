use crate::infrastructure::settings::Settings;
use crate::user_interface::i18n::get_i18n;
use eframe::egui;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod debugging_tab;
mod general_tab;
mod image_processing_tab;
mod ocr_engine_tab;
mod overlay_setting_tab;
mod performance_tuning_tab;
mod text_processing_tab;
mod translation_behavior_tab;
mod translation_provider_tab;

use debugging_tab::render_tab_debugging;
use general_tab::render_tab_general;
use image_processing_tab::render_tab_image_processing;
use ocr_engine_tab::render_tab_ocr;
use overlay_setting_tab::render_tab_overlay;
use performance_tuning_tab::render_tab_performance;
use text_processing_tab::render_tab_text_processing;
use translation_behavior_tab::render_tab_translation_behavior;
use translation_provider_tab::render_tab_ai_provider;

pub struct SettingsWindowResponse {
    pub close_clicked: bool,
}

#[derive(Clone)]
pub struct SlotDebugInfo {
    pub status: String,
    pub ocr_text: String,
    pub identical_frames: u32,
    pub ocr_lines_count: usize,
    pub trans_lines_count: usize,
    pub busy: bool,
    pub processing: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    AiProvider,
    TranslationBehavior,
    Performance,
    Ocr,
    TextProcessing,
    ImageProcessing,
    Overlay,
    Debugging,
}

/// Renders the settings viewport with vertical tabs.
/// Returns a response indicating if save or close was requested.
pub fn show_settings_window(
    ctx: &egui::Context,
    settings_arc: Arc<Mutex<Settings>>,
    ctrl: &crate::core::usecases::settings_controller::SettingsController,
    download_progress: crate::core::types::DownloadProgress,
    download_trigger_tx: std::sync::mpsc::Sender<crate::infrastructure::settings::OcrEngineType>,
    slots_runtime: &[crate::core::region_slot_state::SlotRuntimeState],
) -> SettingsWindowResponse {
    let close_flag = Arc::new(AtomicBool::new(false));

    let close_flag_inner = close_flag.clone();
    let settings_inner = settings_arc.clone();
    let ctrl_inner = ctrl.clone();

    // Extract the pristine captured frame from the first active slot that has one
    let sample_frame = slots_runtime
        .iter()
        .find_map(|slot| slot.last_frame.lock().clone());

    let debug_infos: Vec<SlotDebugInfo> = slots_runtime
        .iter()
        .map(|slot| SlotDebugInfo {
            status: slot.status.clone(),
            ocr_text: slot.last_stable_ocr_text.clone(),
            identical_frames: slot.identical_frames_count,
            ocr_lines_count: slot.persistent_ocr_lines.lock().len(),
            trans_lines_count: slot.persistent_trans_lines.lock().len(),
            busy: slot.busy,
            processing: slot.processing,
        })
        .collect();

    let viewport_id = egui::ViewportId::from_hash_of("settings_viewport");

    let i18n = {
        let s = settings_inner.lock();
        get_i18n(s.ui_language)
    };

    ctx.show_viewport_deferred(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title(format!("KTranslator - {}", i18n.settings))
            .with_inner_size([720.0, 500.0])
            .with_resizable(true)
            .with_always_on_top(),
        move |ctx, _| {
            if download_progress.is_downloading {
                ctx.request_repaint_after(std::time::Duration::from_millis(80));
            }
            if ctx.input(|i| i.viewport().close_requested()) {
                close_flag_inner.store(true, Ordering::Relaxed);
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("settings_close_requested"), true));
                ctx.request_repaint();
            }

            let active_tab: SettingsTab = ctx
                .data(|d| d.get_temp(egui::Id::new("settings_active_tab")))
                .unwrap_or(SettingsTab::General);

            // ── Left Sidebar (Vertical Tabs) ──
            egui::SidePanel::left("settings_tabs_panel")
                .resizable(false)
                .exact_width(200.0)
                .frame(egui::Frame::side_top_panel(ctx.style().as_ref()).inner_margin(8.0))
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new(i18n.settings.to_string()).strong());
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    let tabs = [
                        (SettingsTab::General, i18n.tab_general),
                        (SettingsTab::AiProvider, i18n.tab_ai_provider),
                        (
                            SettingsTab::TranslationBehavior,
                            i18n.tab_translation_behavior,
                        ),
                        (SettingsTab::Performance, i18n.tab_performance),
                        (SettingsTab::Ocr, i18n.tab_ocr),
                        (SettingsTab::TextProcessing, i18n.tab_text_processing),
                        (SettingsTab::ImageProcessing, i18n.tab_image_processing),
                        (SettingsTab::Overlay, i18n.tab_overlay),
                        (SettingsTab::Debugging, i18n.tab_debugging),
                    ];

                    for (tab, label) in tabs {
                        let selected = active_tab == tab;
                        let text = egui::RichText::new(label).size(14.0);
                        let text = if selected { text.strong() } else { text };
                        let btn = ui.add_sized(
                            [ui.available_width(), 32.0],
                            egui::SelectableLabel::new(selected, text),
                        );
                        if btn.clicked() {
                            ctx.data_mut(|d| {
                                d.insert_temp(egui::Id::new("settings_active_tab"), tab)
                            });
                        }
                    }
                });

            // ── Right Content Panel ──
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut settings = settings_inner.lock();
                let initial_settings = settings.clone();
                let i18n = get_i18n(settings.ui_language);

                egui::ScrollArea::vertical().show(ui, |ui| match active_tab {
                    SettingsTab::General => render_tab_general(ui, &mut settings, i18n),
                    SettingsTab::AiProvider => {
                        render_tab_ai_provider(ui, ctx, &mut settings, i18n, &ctrl_inner)
                    }
                    SettingsTab::TranslationBehavior => {
                        render_tab_translation_behavior(ui, &mut settings, i18n)
                    }
                    SettingsTab::Performance => render_tab_performance(ui, &mut settings, i18n),
                    SettingsTab::Ocr => render_tab_ocr(
                        ui,
                        &mut settings,
                        i18n,
                        &download_progress,
                        &download_trigger_tx,
                    ),
                    SettingsTab::TextProcessing => {
                        render_tab_text_processing(ui, &mut settings, i18n)
                    }
                    SettingsTab::ImageProcessing => render_tab_image_processing(
                        ui,
                        ctx,
                        &mut settings,
                        i18n,
                        sample_frame.as_ref(),
                    ),
                    SettingsTab::Overlay => render_tab_overlay(
                        ui,
                        &mut settings,
                        i18n,
                        &download_progress,
                        &download_trigger_tx,
                    ),
                    SettingsTab::Debugging => render_tab_debugging(ui, &debug_infos, i18n),
                });

                // If any setting was actually modified, notify the main window to sync 
                // the child viewports (like overlay) so settings take effect in real-time.
                if *settings != initial_settings {
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("force_sync_children"), true));
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }

                // Ensure dynamic tabs like Image Processing (live preview), Debugging, 
                // and Download Progress update in real-time even when mouse is idle
                ctx.request_repaint_after(std::time::Duration::from_millis(150));
            });
        },
    );

    SettingsWindowResponse {
        close_clicked: close_flag.load(Ordering::Relaxed),
    }
}

pub fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().size(14.0));
    ui.add_space(2.0);
}

pub fn check_file_exists(rel_path: &str) -> bool {
    if std::path::Path::new(rel_path).exists() {
        return true;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join(rel_path).exists() {
                return true;
            }
        }
    }
    false
}
