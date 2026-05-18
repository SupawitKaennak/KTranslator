use eframe::egui;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::Mutex;
use crate::infrastructure::settings::{Settings, TranslationProvider, UiLanguage};
use crate::user_interface::i18n::get_i18n;

pub struct SettingsWindowResponse {
    pub close_clicked: bool,
}

#[derive(Clone)]
struct SlotDebugInfo {
    status: String,
    ocr_text: String,
    identical_frames: u32,
    ocr_lines_count: usize,
    trans_lines_count: usize,
    busy: bool,
    processing: bool,
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
    download_progress: crate::infrastructure::asset_manager::DownloadProgress,
    download_trigger_tx: std::sync::mpsc::Sender<crate::infrastructure::settings::OcrEngineType>,
    slots_runtime: &[crate::core::worker::SlotRuntimeState],
) -> SettingsWindowResponse {
    let close_flag = Arc::new(AtomicBool::new(false));
    
    let close_flag_inner = close_flag.clone();
    let settings_inner = settings_arc.clone();
    let ctrl_inner = ctrl.clone();
    
    // Extract the pristine captured frame from the first active slot that has one
    let sample_frame = slots_runtime.iter()
        .find_map(|slot| slot.last_frame.lock().clone());
        
    let debug_infos: Vec<SlotDebugInfo> = slots_runtime.iter().map(|slot| {
        SlotDebugInfo {
            status: slot.status.clone(),
            ocr_text: slot.last_stable_ocr_text.clone(),
            identical_frames: slot.identical_frames_count,
            ocr_lines_count: slot.persistent_ocr_lines.lock().len(),
            trans_lines_count: slot.persistent_trans_lines.lock().len(),
            busy: slot.busy,
            processing: slot.processing,
        }
    }).collect();
    
    let viewport_id = egui::ViewportId::from_hash_of("settings_viewport");

    let i18n = {
        let s = settings_inner.lock();
        get_i18n(s.ui_language)
    };

    ctx.show_viewport_immediate(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title(format!("KTranslator - {}", i18n.settings))
            .with_inner_size([720.0, 500.0])
            .with_resizable(true)
            .with_always_on_top(),
        move |ctx, _| {
            if ctx.input(|i| i.viewport().close_requested()) {
                close_flag_inner.store(true, Ordering::Relaxed);
            }

            let active_tab: SettingsTab = ctx.data(|d| d.get_temp(egui::Id::new("settings_active_tab")))
                .unwrap_or(SettingsTab::General);

            // ── Left Sidebar (Vertical Tabs) ──
            egui::SidePanel::left("settings_tabs_panel")
                .resizable(false)
                .exact_width(200.0)
                .frame(egui::Frame::side_top_panel(ctx.style().as_ref()).inner_margin(8.0))
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new(format!("{}", i18n.settings)).strong());
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    let tabs = [
                        (SettingsTab::General,        i18n.tab_general),
                        (SettingsTab::AiProvider,     i18n.tab_ai_provider),
                        (SettingsTab::TranslationBehavior, i18n.tab_translation_behavior),
                        (SettingsTab::Performance,    i18n.tab_performance),
                        (SettingsTab::Ocr,            i18n.tab_ocr),
                        (SettingsTab::TextProcessing, i18n.tab_text_processing),
                        (SettingsTab::ImageProcessing, i18n.tab_image_processing),
                        (SettingsTab::Overlay,        i18n.tab_overlay),
                        (SettingsTab::Debugging,      i18n.tab_debugging),
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
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("settings_active_tab"), tab));
                        }
                    }

                });

            // ── Right Content Panel ──
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut settings = settings_inner.lock();
                let i18n = get_i18n(settings.ui_language);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    match active_tab {
                        SettingsTab::General => render_tab_general(ui, &mut settings, i18n),
                        SettingsTab::AiProvider => render_tab_ai_provider(ui, ctx, &mut settings, i18n, &ctrl_inner),
                        SettingsTab::TranslationBehavior => render_tab_translation_behavior(ui, &mut settings, i18n),
                        SettingsTab::Performance => render_tab_performance(ui, &mut settings, i18n),
                        SettingsTab::Ocr => render_tab_ocr(ui, &mut settings, i18n, &download_progress, &download_trigger_tx),
                        SettingsTab::TextProcessing => render_tab_text_processing(ui, &mut settings, i18n),
                        SettingsTab::ImageProcessing => render_tab_image_processing(ui, ctx, &mut settings, i18n, sample_frame.as_ref()),
                        SettingsTab::Overlay => render_tab_overlay(ui, &mut settings, i18n),
                        SettingsTab::Debugging => render_tab_debugging(ui, &debug_infos, i18n),
                    }
                });
            });
        },
    );
    
    SettingsWindowResponse {
        close_clicked: close_flag.load(Ordering::Relaxed),
    }
}

// ─────────────────────────────────────────────
// Tab 1: General
// ─────────────────────────────────────────────
fn render_tab_general(ui: &mut egui::Ui, settings: &mut Settings, i18n: &crate::user_interface::i18n::I18n) {
    ui.heading(i18n.tab_general);
    ui.add_space(8.0);

    section_header(ui, &format!("{}", i18n.ui_language));
    egui::ComboBox::from_id_salt("ui_lang_combo")
        .width(200.0)
        .selected_text(match settings.ui_language {
            UiLanguage::System  => i18n.system_default,
            UiLanguage::Thai    => "ไทย",
            UiLanguage::English => "English",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut settings.ui_language, UiLanguage::System, i18n.system_default);
            ui.selectable_value(&mut settings.ui_language, UiLanguage::Thai, "ไทย");
            ui.selectable_value(&mut settings.ui_language, UiLanguage::English, "English");
        });


    ui.add_space(12.0);
    section_header(ui, i18n.capture_section);
    let mut allow = !settings.hide_from_capture;
    if ui.checkbox(&mut allow, i18n.allow_capture).changed() {
        settings.hide_from_capture = !allow;
    }
}

// ─────────────────────────────────────────────
// Tab 2: AI Provider
// ─────────────────────────────────────────────
fn render_tab_ai_provider(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
    ctrl: &crate::core::usecases::settings_controller::SettingsController,
) {
    ui.heading(i18n.tab_ai_provider);
    ui.add_space(8.0);

    section_header(ui, i18n.provider);
    ui.add_space(4.0);

    let providers = [
        (TranslationProvider::Gemini,       "Gemini"),
        (TranslationProvider::Groq,         "Groq"),
        (TranslationProvider::Ollama,       &format!("Ollama ({})", i18n.offline)),
        (TranslationProvider::CustomOpenAI, &format!("Custom ({})", i18n.compatible)),
        (TranslationProvider::Google,       &format!("Google Translate ({})", i18n.auto_detect)), 
    ];
    for (prov, label) in providers {
        ui.radio_value(&mut settings.provider, prov, label);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // Show config section for the selected provider
    match settings.provider {
        TranslationProvider::Gemini => {
            section_header(ui, &format!("Gemini {}", i18n.config_for));
            render_api_key_field(ui, i18n, &mut settings.gemini_api_key, &ctrl.gemini_models, &ctrl.gemini_fetching);
            try_fetch_gemini(ctx, settings, &ctrl.gemini_models, &ctrl.gemini_fetching);
            if !settings.gemini_api_key.trim().is_empty() {
                render_model_dropdown(ui, i18n, "gemini_mdl", &mut settings.gemini_model, &ctrl.gemini_models, &ctrl.gemini_fetching);
            }
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.get_api_key, "https://aistudio.google.com/app/apikey");
        }
        TranslationProvider::Groq => {
            section_header(ui, &format!("Groq {}", i18n.config_for));
            render_api_key_field(ui, i18n, &mut settings.groq_api_key, &ctrl.groq_models, &ctrl.groq_fetching);
            try_fetch_groq(ctx, settings, &ctrl.groq_models, &ctrl.groq_fetching);
            if !settings.groq_api_key.trim().is_empty() {
                render_model_dropdown(ui, i18n, "groq_mdl", &mut settings.groq_model, &ctrl.groq_models, &ctrl.groq_fetching);
            }
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.get_api_key, "https://console.groq.com/keys");
        }
        TranslationProvider::Ollama => {
            section_header(ui, &format!("Ollama {}", i18n.config_for));
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.server_url));
                let resp = ui.text_edit_singleline(&mut settings.ollama_url);
                if resp.lost_focus() && resp.changed() { ctrl.ollama_models.lock().clear(); }
            });
            try_fetch_ollama(ctx, settings, &ctrl.ollama_models, &ctrl.ollama_fetching);
            if !settings.ollama_url.trim().is_empty() {
                render_model_dropdown(ui, i18n, "ollama_mdl", &mut settings.ollama_model, &ctrl.ollama_models, &ctrl.ollama_fetching);
            }
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.browse_models, "https://ollama.com/library");
        }
        TranslationProvider::CustomOpenAI => {
            section_header(ui, i18n.prov_custom_endpoint);
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.base_url));
                let resp = ui.text_edit_singleline(&mut settings.custom_openai_url);
                if resp.lost_focus() && resp.changed() { ctrl.custom_openai_models.lock().clear(); }
            });
            ui.horizontal(|ui| {
                ui.label(i18n.api_key);
                let resp = ui.add(egui::TextEdit::singleline(&mut settings.custom_openai_api_key).password(true));
                if resp.lost_focus() && resp.changed() { ctrl.custom_openai_models.lock().clear(); }
            });
            ui.add_space(4.0);
            ui.label(format!("{}:", i18n.model_selection));
            ui.horizontal(|ui| {
                if ui.radio_value(&mut settings.custom_openai_use_list, false, i18n.manual_entry).changed() { ctrl.custom_openai_models.lock().clear(); }
                if ui.radio_value(&mut settings.custom_openai_use_list, true, i18n.fetch_list).changed() { ctrl.custom_openai_models.lock().clear(); }
            });
            if settings.custom_openai_use_list {
                try_fetch_custom(ctx, settings, &ctrl.custom_openai_models, &ctrl.custom_openai_fetching);
                render_model_dropdown(ui, i18n, "custom_mdl", &mut settings.custom_openai_model, &ctrl.custom_openai_models, &ctrl.custom_openai_fetching);
            } else {
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", i18n.model_name));
                    ui.text_edit_singleline(&mut settings.custom_openai_model);
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.prov_api_verification));
                ui.hyperlink_to("OpenRouter", "https://openrouter.ai/keys");
                ui.label("|");
                ui.hyperlink_to("Together AI", "https://api.together.xyz/settings/api-keys");
                ui.label("|");
                ui.hyperlink_to("OpenAI", "https://platform.openai.com/api-keys");
            });
        }
        TranslationProvider::Google => {
            section_header(ui, i18n.prov_google);
            ui.label(i18n.no_config_needed);
        }
    }
}

// ─────────────────────────────────────────────
// Tab 3: OCR Engine
// ─────────────────────────────────────────────
fn render_tab_ocr(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
    download_progress: &crate::infrastructure::asset_manager::DownloadProgress,
    download_trigger_tx: &std::sync::mpsc::Sender<crate::infrastructure::settings::OcrEngineType>,
) {
    ui.heading(i18n.tab_ocr);
    ui.add_space(8.0);

    section_header(ui, i18n.ocr_engine_mode_setup);
    ui.add_space(4.0);

    let modes = [
        (crate::infrastructure::settings::OcrMode::Game,     i18n.mode_game),
        (crate::infrastructure::settings::OcrMode::Manga,    i18n.mode_manga),
        (crate::infrastructure::settings::OcrMode::Document, i18n.mode_document),
    ];
    for (mode, label) in modes {
        ui.radio_value(&mut settings.ocr_mode, mode, label);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // Show engine config for the selected mode
    let (engine_ref, mode_name) = match settings.ocr_mode {
        crate::infrastructure::settings::OcrMode::Game     => (&mut settings.game_ocr_engine, i18n.mode_game),
        crate::infrastructure::settings::OcrMode::Manga    => (&mut settings.manga_ocr_engine, i18n.mode_manga),
        crate::infrastructure::settings::OcrMode::Document => (&mut settings.document_ocr_engine, i18n.mode_document),
    };

    section_header(ui, &format!("{} — {}", i18n.choose_ocr, mode_name));
    ui.add_space(4.0);
    ui.radio_value(engine_ref, crate::infrastructure::settings::OcrEngineType::Windows, i18n.ocr_windows_desc);
    ui.radio_value(engine_ref, crate::infrastructure::settings::OcrEngineType::BuiltinPaddle, i18n.ocr_builtin_paddle_desc);
    ui.radio_value(engine_ref, crate::infrastructure::settings::OcrEngineType::MangaOCR, i18n.ocr_manga_desc);

    // MangaOCR: download section
    if *engine_ref == crate::infrastructure::settings::OcrEngineType::MangaOCR {
        ui.add_space(8.0);
        if download_progress.is_downloading {
            ui.label(format!("{}: {}", i18n.downloading, download_progress.current_file));
            ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
        } else {
            if let Some(err) = &download_progress.error {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), format!("Error: {}", err));
            }
            let enc_p = "models/manga-ocr/encoder_model.onnx";
            let dec_p = "models/manga-ocr/decoder_model.onnx";
            let tok_p = "models/manga-ocr/tokenizer.json";
            let yolo_p = "models/manga-ocr/manga109_yolo_s.onnx";

            let models_exist = check_file_exists(enc_p) && check_file_exists(dec_p) && check_file_exists(tok_p) && check_file_exists(yolo_p);
            
            if !models_exist {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), i18n.models_not_found);
                if ui.button(i18n.download_install).clicked() { let _ = download_trigger_tx.send(crate::infrastructure::settings::OcrEngineType::MangaOCR); }
            } else {
                ui.colored_label(egui::Color32::from_rgb(100, 255, 100), i18n.models_installed);
                if ui.button(i18n.reinstall_update).clicked() { let _ = download_trigger_tx.send(crate::infrastructure::settings::OcrEngineType::MangaOCR); }
            }
        }
    }

    // Built-in PaddleOCR: configuration & downloader
    if *engine_ref == crate::infrastructure::settings::OcrEngineType::BuiltinPaddle {
        ui.add_space(8.0);
        
        // Configuration dropdowns
        ui.horizontal(|ui| {
            ui.label(i18n.ppocr_variant_label);
            egui::ComboBox::from_id_salt("ppocr_model_suite_combo")
                .selected_text(match settings.ppocr_model {
                    crate::infrastructure::settings::PpocrModelSuite::CnEnMobile => i18n.ppocr_suite_cnen_mobile,
                    crate::infrastructure::settings::PpocrModelSuite::CnEnServer => i18n.ppocr_suite_cnen_server,
                    crate::infrastructure::settings::PpocrModelSuite::JapaneseMobile => i18n.ppocr_suite_jp_mobile,
                    crate::infrastructure::settings::PpocrModelSuite::JapaneseServer => i18n.ppocr_suite_jp_server,
                    crate::infrastructure::settings::PpocrModelSuite::KoreanMobile => i18n.ppocr_suite_ko_mobile,
                    crate::infrastructure::settings::PpocrModelSuite::KoreanServer => i18n.ppocr_suite_ko_server,
                    crate::infrastructure::settings::PpocrModelSuite::ThaiMobile => i18n.ppocr_suite_th_mobile,
                    crate::infrastructure::settings::PpocrModelSuite::ThaiServer => i18n.ppocr_suite_th_server,
                    crate::infrastructure::settings::PpocrModelSuite::LatinMobile => i18n.ppocr_suite_latin_mobile,
                    crate::infrastructure::settings::PpocrModelSuite::LatinServer => i18n.ppocr_suite_latin_server,
                    crate::infrastructure::settings::PpocrModelSuite::CyrillicMobile => i18n.ppocr_suite_cyrillic_mobile,
                    crate::infrastructure::settings::PpocrModelSuite::CyrillicServer => i18n.ppocr_suite_cyrillic_server,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::CnEnMobile, i18n.ppocr_suite_cnen_mobile);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::CnEnServer, i18n.ppocr_suite_cnen_server);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::JapaneseMobile, i18n.ppocr_suite_jp_mobile);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::JapaneseServer, i18n.ppocr_suite_jp_server);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::KoreanMobile, i18n.ppocr_suite_ko_mobile);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::KoreanServer, i18n.ppocr_suite_ko_server);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::ThaiMobile, i18n.ppocr_suite_th_mobile);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::ThaiServer, i18n.ppocr_suite_th_server);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::LatinMobile, i18n.ppocr_suite_latin_mobile);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::LatinServer, i18n.ppocr_suite_latin_server);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::CyrillicMobile, i18n.ppocr_suite_cyrillic_mobile);
                    ui.selectable_value(&mut settings.ppocr_model, crate::infrastructure::settings::PpocrModelSuite::CyrillicServer, i18n.ppocr_suite_cyrillic_server);
                });
        });

        ui.add_space(6.0);

        if download_progress.is_downloading {
            ui.label(format!("{}: {}", i18n.downloading, download_progress.current_file));
            ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
        } else {
            if let Some(err) = &download_progress.error {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), format!("Error: {}", err));
            }
            
            // Check status dynamically based on current configuration suite folder
            let folder_name = settings.ppocr_model.folder_name();

            let base_p = format!("models/ppocr/{}", folder_name);
            let det_path = format!("{}/det.onnx", base_p);
            let rec_path = format!("{}/rec.onnx", base_p);
            let dict_path = format!("{}/dict.txt", base_p);

            let det_exists = check_file_exists(&det_path);
            let rec_exists = check_file_exists(&rec_path);
            let dict_exists = check_file_exists(&dict_path);

            if det_exists && rec_exists && dict_exists {
                ui.colored_label(egui::Color32::from_rgb(100, 255, 100), format!("✔ {} ({})", i18n.models_found, folder_name));
                if ui.button(i18n.reinstall_update).clicked() {
                    let _ = download_trigger_tx.send(crate::infrastructure::settings::OcrEngineType::BuiltinPaddle);
                }
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 100, 100), 
                    format!("⚠ {}: models/ppocr/{}", i18n.missing_models, folder_name)
                );
                ui.label(i18n.ppocr_download_hint);
                if ui.button(i18n.download_install).clicked() {
                    let _ = download_trigger_tx.send(crate::infrastructure::settings::OcrEngineType::BuiltinPaddle);
                }
            }
        }
    }
}

fn check_file_exists(rel_path: &str) -> bool {
    if std::path::Path::new(rel_path).exists() { return true; }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join(rel_path).exists() { return true; }
        }
    }
    false
}

// ─────────────────────────────────────────────
// Tab 4: Text Processing
// ─────────────────────────────────────────────
fn render_tab_text_processing(ui: &mut egui::Ui, settings: &mut Settings, i18n: &crate::user_interface::i18n::I18n) {
    ui.heading(i18n.tab_text_processing);
    ui.add_space(8.0);

    section_header(ui, i18n.txt_pre_trans);
    ui.add_space(4.0);
    ui.checkbox(&mut settings.smart_merge, i18n.smart_merge);
    ui.checkbox(&mut settings.txt_proc.enable_wordninja, i18n.txt_wordninja);
    
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    section_header(ui, i18n.txt_pre_trans);
    ui.label(egui::RichText::new(i18n.txt_proc_adv_desc).italics());
    ui.add_space(6.0);

    let tp = &mut settings.txt_proc;

    egui::Grid::new("txt_proc_grid")
        .num_columns(2)
        .spacing([20.0, 8.0])
        .show(ui, |ui| {
            ui.checkbox(&mut tp.remove_duplicates, i18n.clean_remove_dups);
            ui.checkbox(&mut tp.merge_broken_lines, i18n.clean_merge_broken);
            ui.end_row();

            ui.checkbox(&mut tp.merge_subtitle_fragments, i18n.clean_merge_fragments);
            ui.checkbox(&mut tp.remove_garbage, i18n.clean_remove_garbage);
            ui.end_row();

            ui.checkbox(&mut tp.recurring_suppression, i18n.clean_recurring);
            ui.checkbox(&mut tp.repeated_char_collapse, i18n.clean_repeat_char);
            ui.end_row();

            ui.checkbox(&mut tp.consonant_spam_filter, i18n.clean_consonant_spam);
            ui.checkbox(&mut tp.kana_spam_filter, i18n.clean_kana_spam);
            ui.end_row();

            ui.checkbox(&mut tp.punctuation_normalization, i18n.clean_punc_norm);
            ui.end_row();
        });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(format!("{}:", i18n.clean_min_len));
        ui.add(egui::Slider::new(&mut tp.min_text_length, 1..=10).suffix(" chars"));
    });
    
    if tp.remove_garbage {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(format!("{}:", i18n.clean_spec_ratio));
            ui.add(egui::Slider::new(&mut tp.special_char_ratio_limit, 0.1..=1.0));
        });
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Language-Specific Processing Section ──
    section_header(ui, i18n.txt_lang_spec);
    ui.label(egui::RichText::new("Advanced rules optimized for specific writing systems and linguistic nuances:").italics());
    ui.add_space(8.0);

    egui::Grid::new("lang_spec_grid").num_columns(2).spacing([20.0, 12.0]).show(ui, |ui| {
        // Japanese
        ui.label(egui::RichText::new(i18n.lang_japanese).strong());
        ui.vertical(|ui| {
            ui.checkbox(&mut tp.jp_merge_vertical, i18n.jp_merge_v);
            ui.checkbox(&mut tp.jp_kana_normalization, i18n.jp_kana_norm);
            ui.checkbox(&mut tp.jp_remove_furigana, i18n.jp_strip_furi);
            ui.checkbox(&mut tp.jp_preserve_honorifics, i18n.jp_honorifics);
        });
        ui.end_row();

        // Chinese
        ui.label(egui::RichText::new("Chinese:").strong());
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("cn_conv_sel")
                .selected_text(match tp.cn_conversion {
                    crate::infrastructure::settings::ChineseConversionMode::None => i18n.cn_no_conv,
                    crate::infrastructure::settings::ChineseConversionMode::SimplifiedToTraditional => i18n.cn_s2t,
                    crate::infrastructure::settings::ChineseConversionMode::TraditionalToSimplified => i18n.cn_t2s,
                })
                .show_ui(ui, |ui| {
                    use crate::infrastructure::settings::ChineseConversionMode::*;
                    ui.selectable_value(&mut tp.cn_conversion, None, i18n.cn_no_conv);
                    ui.selectable_value(&mut tp.cn_conversion, SimplifiedToTraditional, i18n.cn_s2t);
                    ui.selectable_value(&mut tp.cn_conversion, TraditionalToSimplified, i18n.cn_t2s);
                });
        });
        ui.end_row();

        // Thai
        ui.label(egui::RichText::new("Thai:").strong());
        ui.vertical(|ui| {
            egui::ComboBox::from_id_salt("th_seg_sel")
                .selected_text(match tp.th_segmentation {
                    crate::infrastructure::settings::ThaiSegmentationMode::Standard => i18n.th_std_split,
                    crate::infrastructure::settings::ThaiSegmentationMode::DictionaryAssisted => i18n.th_dict_break,
                    crate::infrastructure::settings::ThaiSegmentationMode::SyllableLevel => i18n.th_syllable,
                })
                .show_ui(ui, |ui| {
                    use crate::infrastructure::settings::ThaiSegmentationMode::*;
                    ui.selectable_value(&mut tp.th_segmentation, Standard, i18n.th_std_split);
                    ui.selectable_value(&mut tp.th_segmentation, DictionaryAssisted, i18n.th_dict_break);
                    ui.selectable_value(&mut tp.th_segmentation, SyllableLevel, i18n.th_syllable);
                });
            ui.add_space(4.0);
            ui.checkbox(&mut tp.th_zero_width_cleanup, i18n.th_zw_cleanup);
        });
        ui.end_row();

        // Arabic
        ui.label(egui::RichText::new(i18n.lang_arabic).strong());
        ui.checkbox(&mut tp.ar_rtl_correction, i18n.ar_rtl_fix);
        ui.end_row();
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    section_header(ui, i18n.txt_regex);
    ui.label(egui::RichText::new(i18n.regex_adv_desc).italics());
    ui.add_space(6.0);

    let mut remove_idx = None;

    for (idx, rule) in settings.regex_rules.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut rule.enabled, format!("#{idx}"));
                
                egui::ComboBox::from_id_salt(format!("type_{idx}"))
                    .selected_text(format!("{:?}", rule.rule_type))
                    .show_ui(ui, |ui| {
                        use crate::infrastructure::settings::RegexRuleType::*;
                        ui.selectable_value(&mut rule.rule_type, Ignore, "Ignore (Strip pattern)");
                        ui.selectable_value(&mut rule.rule_type, PreTranslation, "PreTranslation (Replace before AI)");
                        ui.selectable_value(&mut rule.rule_type, Protected, "Protected (Mask word from AI)");
                        ui.selectable_value(&mut rule.rule_type, Replace, "Replace (General cleanup)");
                        ui.selectable_value(&mut rule.rule_type, Split, "Split (Match -> Newline)");
                        ui.selectable_value(&mut rule.rule_type, PostTranslation, "PostTranslation (Repair output)");
                    });

                if ui.button("🗑").clicked() {
                    remove_idx = Some(idx);
                }
            });

            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.pattern));
                ui.add(egui::TextEdit::singleline(&mut rule.pattern).desired_width(140.0));

                let requires_replacement = match rule.rule_type {
                    crate::infrastructure::settings::RegexRuleType::Ignore 
                    | crate::infrastructure::settings::RegexRuleType::Split 
                    | crate::infrastructure::settings::RegexRuleType::Protected => false,
                    _ => true,
                };

                if requires_replacement {
                    ui.label(format!("{}:", i18n.replace));
                    ui.add(egui::TextEdit::singleline(&mut rule.replacement).desired_width(100.0));
                }
            });
        });
        ui.add_space(4.0);
    }

    if let Some(idx) = remove_idx {
        settings.regex_rules.remove(idx);
    }

    if ui.button(i18n.add_regex).clicked() {
        settings.regex_rules.push(crate::infrastructure::settings::RegexRule {
            enabled: true,
            pattern: String::new(),
            replacement: String::new(),
            rule_type: crate::infrastructure::settings::RegexRuleType::PreTranslation,
        });
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    section_header(ui, &format!("Custom Dictionary / {}", i18n.tab_text_processing.replace("Text Processing", "Glossary Engine").replace("การประมวลผลข้อความ", "พจนานุกรม")));
    ui.label(egui::RichText::new(i18n.gloss_adv_desc).italics());
    ui.add_space(6.0);

    let mut remove_gloss_idx = None;

    for (idx, entry) in settings.glossary_entries.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut entry.enabled, format!("#{idx}"));

                egui::ComboBox::from_id_salt(format!("gtype_{idx}"))
                    .selected_text(format!("{:?}", entry.entry_type))
                    .show_ui(ui, |ui| {
                        use crate::infrastructure::settings::GlossaryType::*;
                        ui.selectable_value(&mut entry.entry_type, CharacterName, i18n.gloss_char_name);
                        ui.selectable_value(&mut entry.entry_type, GameTerminology, i18n.gloss_game_term);
                        ui.selectable_value(&mut entry.entry_type, SlangJargon, i18n.gloss_slang);
                        ui.selectable_value(&mut entry.entry_type, ProtectedWord, i18n.gloss_protected);
                        ui.selectable_value(&mut entry.entry_type, PhraseOverride, i18n.gloss_phrase);
                        ui.selectable_value(&mut entry.entry_type, TranslationMemory, i18n.gloss_tm);
                    });

                ui.label(format!("{}:", i18n.prio));
                ui.add(egui::DragValue::new(&mut entry.priority).range(0..=100));

                if ui.button("🗑").clicked() {
                    remove_gloss_idx = Some(idx);
                }
            });

            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.source));
                ui.add(egui::TextEdit::singleline(&mut entry.source).desired_width(120.0));

                ui.label(format!("{}:", i18n.target));
                ui.add(egui::TextEdit::singleline(&mut entry.target).desired_width(120.0));
            });
        });
        ui.add_space(4.0);
    }

    if let Some(idx) = remove_gloss_idx {
        settings.glossary_entries.remove(idx);
    }

    if ui.button(i18n.add_glossary).clicked() {
        settings.glossary_entries.push(crate::infrastructure::settings::GlossaryEntry {
            enabled: true,
            source: String::new(),
            target: String::new(),
            entry_type: crate::infrastructure::settings::GlossaryType::GameTerminology,
            priority: 10,
        });
    }
}

// ─────────────────────────────────────────────
// Tab 5: Overlay
// ─────────────────────────────────────────────
fn render_tab_overlay(ui: &mut egui::Ui, settings: &mut Settings, i18n: &crate::user_interface::i18n::I18n) {
    ui.heading(i18n.tab_overlay);
    ui.add_space(8.0);

    section_header(ui, i18n.overlay_customization);
    ui.add_space(4.0);

    egui::Grid::new("overlay_grid")
        .num_columns(2)
        .spacing([20.0, 10.0])
        .show(ui, |ui| {
            ui.label(format!("{}:", i18n.bg_color));
            ui.horizontal(|ui| {
                let mut rgb = [
                    settings.overlay_bg_color[0],
                    settings.overlay_bg_color[1],
                    settings.overlay_bg_color[2],
                ];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    settings.overlay_bg_color[0] = rgb[0];
                    settings.overlay_bg_color[1] = rgb[1];
                    settings.overlay_bg_color[2] = rgb[2];
                }
                ui.add_space(8.0);
                ui.label(format!("{}:", i18n.opacity));
                ui.add(egui::Slider::new(&mut settings.overlay_bg_color[3], 0..=255));
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.text_color));
            ui.horizontal(|ui| {
                let mut rgb = [
                    settings.overlay_text_color[0],
                    settings.overlay_text_color[1],
                    settings.overlay_text_color[2],
                ];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    settings.overlay_text_color[0] = rgb[0];
                    settings.overlay_text_color[1] = rgb[1];
                    settings.overlay_text_color[2] = rgb[2];
                }
                ui.add_space(8.0);
                ui.label(format!("{}:", i18n.opacity));
                ui.add(egui::Slider::new(&mut settings.overlay_text_color[3], 0..=255));
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.font_size));
            ui.add(egui::Slider::new(&mut settings.overlay_font_size, 8.0..=48.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.padding));
            ui.add(egui::Slider::new(&mut settings.overlay_padding, 0.0..=20.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.corner_radius));
            ui.add(egui::Slider::new(&mut settings.overlay_corner_radius, 0.0..=20.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.text_align));
            ui.horizontal(|ui| {
                ui.radio_value(&mut settings.overlay_text_align, crate::infrastructure::settings::TextAlign::Left, i18n.align_left);
                ui.radio_value(&mut settings.overlay_text_align, crate::infrastructure::settings::TextAlign::Center, i18n.align_center);
                ui.radio_value(&mut settings.overlay_text_align, crate::infrastructure::settings::TextAlign::Right, i18n.align_right);
            });
            ui.end_row();
        });
}

// ─────────────────────────────────────────────
// Shared Helpers
// ─────────────────────────────────────────────
fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().size(14.0));
    ui.add_space(2.0);
}

fn render_api_key_field(
    ui: &mut egui::Ui,
    i18n: &crate::user_interface::i18n::I18n,
    key: &mut String,
    models: &Arc<Mutex<Vec<String>>>,
    _fetching: &Arc<Mutex<bool>>,
) {
    ui.horizontal(|ui| {
        ui.label(format!("{}:", i18n.api_key));
        let resp = ui.add(egui::TextEdit::singleline(key).password(true));
        if resp.lost_focus() && resp.changed() { models.lock().clear(); }
    });
}

fn render_model_dropdown(
    ui: &mut egui::Ui,
    i18n: &crate::user_interface::i18n::I18n,
    id: &str,
    selected: &mut String,
    models: &Arc<Mutex<Vec<String>>>,
    fetching: &Arc<Mutex<bool>>,
) {
    ui.horizontal(|ui| {
        ui.label(format!("{}:", i18n.model));
        let m = models.lock();
        if m.is_empty() {
            ui.label(egui::RichText::new("(Fetching models...)").italics());
        } else {
            egui::ComboBox::from_id_salt(id).width(250.0).selected_text(selected.as_str()).show_ui(ui, |ui| {
                for name in m.iter() { ui.selectable_value(selected, name.clone(), name); }
            });
        }
        if *fetching.lock() { ui.spinner(); }
    });
}

// ── Auto-fetch helpers ──
fn try_fetch_gemini(ctx: &egui::Context, settings: &Settings, models: &Arc<Mutex<Vec<String>>>, fetching: &Arc<Mutex<bool>>) {
    let should = { models.lock().is_empty() && !*fetching.lock() && !settings.gemini_api_key.trim().is_empty() };
    if !should { return; }
    let key = settings.gemini_api_key.clone();
    let m = models.clone(); let f = fetching.clone(); let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        if let Ok(list) = crate::adapters::translate::gemini::GeminiTranslator::list_models(&key) {
            *m.lock() = list.into_iter().map(|x| x.id).collect();
        }
        *f.lock() = false; c.request_repaint();
    });
}

fn try_fetch_groq(ctx: &egui::Context, settings: &Settings, models: &Arc<Mutex<Vec<String>>>, fetching: &Arc<Mutex<bool>>) {
    let should = { models.lock().is_empty() && !*fetching.lock() && !settings.groq_api_key.trim().is_empty() };
    if !should { return; }
    let key = settings.groq_api_key.clone();
    let m = models.clone(); let f = fetching.clone(); let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        if let Ok(list) = crate::adapters::translate::groq::GroqTranslator::list_models(&key) { *m.lock() = list; }
        *f.lock() = false; c.request_repaint();
    });
}

fn try_fetch_ollama(ctx: &egui::Context, settings: &Settings, models: &Arc<Mutex<Vec<String>>>, fetching: &Arc<Mutex<bool>>) {
    let should = { models.lock().is_empty() && !*fetching.lock() && !settings.ollama_url.trim().is_empty() };
    if !should { return; }
    let url = settings.ollama_url.clone();
    let m = models.clone(); let f = fetching.clone(); let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        if let Ok(list) = crate::adapters::translate::ollama::OllamaTranslator::list_models(&url) { *m.lock() = list; }
        *f.lock() = false; c.request_repaint();
    });
}

fn try_fetch_custom(ctx: &egui::Context, settings: &Settings, models: &Arc<Mutex<Vec<String>>>, fetching: &Arc<Mutex<bool>>) {
    let should = { settings.custom_openai_use_list && models.lock().is_empty() && !*fetching.lock() && !settings.custom_openai_url.trim().is_empty() };
    if !should { return; }
    let url = settings.custom_openai_url.clone();
    let key = settings.custom_openai_api_key.clone();
    let m = models.clone(); let f = fetching.clone(); let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        if let Ok(list) = crate::adapters::translate::openai::OpenAiTranslator::list_models(&url, &key) { *m.lock() = list; }
        *f.lock() = false; c.request_repaint();
    });
}

// ─────────────────────────────────────────────
// Tab 4b: Image Processing (Pre-OCR)
// ─────────────────────────────────────────────
fn render_tab_image_processing(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
    captured_frame: Option<&crate::core::ports::FrameRgba>,
) {
    ui.heading(i18n.tab_image_processing);
    ui.add_space(8.0);

    let img_proc = &mut settings.img_proc;

    // --- LIVE PREVIEW SECTION ---
    section_header(ui, "Live Preview Processed Image");
    ui.label("Real-time preview of filters applied before OCR engine extraction:");
    ui.add_space(4.0);

    // Let's handle vector allocation cleanly to satisfy strict compiler references:
    let dummy_buffer;
    let raw_pixels: &[u8];
    let w;
    let h;

    if let Some(frame) = captured_frame {
        raw_pixels = &frame.data;
        w = frame.width;
        h = frame.height;
        if frame.width > 0 {
            ui.label(egui::RichText::new(format!("Using live captured frame ({}x{})", w, h)).color(egui::Color32::LIGHT_GREEN));
        }
    } else {
        let fw = 400;
        let fh = 80;
        let mut sample = vec![240u8; (fw * fh * 4) as usize];
        for y in 20..60 {
            for x in 40..360 {
                if (x / 15) % 2 == 0 && (y / 5) % 2 == 0 {
                    let idx = ((y * fw + x) * 4) as usize;
                    sample[idx]   = 40; 
                    sample[idx+1] = 40; 
                    sample[idx+2] = 40; 
                    sample[idx+3] = 255;
                }
            }
        }
        dummy_buffer = sample;
        raw_pixels = &dummy_buffer;
        w = fw;
        h = fh;
        ui.label(egui::RichText::new("Using placeholder sample text (capture screen to view live frame)").color(egui::Color32::LIGHT_YELLOW));
    }
    ui.add_space(4.0);

    // Apply high-performance processing pipeline
    let (processed_data, pw, ph) = crate::core::usecases::image_processor::process_image_for_ocr(
        raw_pixels, w, h, img_proc
    );

    // Render Preview Texture on GUI
    let color_img = egui::ColorImage::from_rgba_unmultiplied(
        [pw as usize, ph as usize],
        &processed_data,
    );
    let handle = ctx.load_texture(
        "img_proc_preview",
        color_img,
        egui::TextureOptions::NEAREST, 
    );

    ui.add(egui::Image::new(&handle).max_width(ui.available_width().min(pw as f32)));
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // --- CONTROLS SECTION ---
    egui::Grid::new("img_proc_grid")
        .num_columns(2)
        .spacing([20.0, 10.0])
        .show(ui, |ui| {
            ui.label("Grayscale:");
            ui.checkbox(&mut img_proc.grayscale, "Convert to Monochrome");
            ui.end_row();

            ui.label("Invert Colors:");
            ui.checkbox(&mut img_proc.invert, "Negative Mapping (White on Black)");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_binarize));
            ui.horizontal(|ui| {
                ui.checkbox(&mut img_proc.binarize, "Enable");
                if img_proc.binarize {
                    ui.add_space(10.0);
                    ui.add(egui::Slider::new(&mut img_proc.binary_threshold, 0..=255).text("Level"));
                }
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.img_adaptive));
            ui.checkbox(&mut img_proc.adaptive_threshold, "Local Box-filter Mean (Best for gradients)");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_contrast));
            ui.add(egui::Slider::new(&mut img_proc.contrast, 0.0..=3.0));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_brightness));
            ui.add(egui::Slider::new(&mut img_proc.brightness, -255..=255));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_gamma));
            ui.add(egui::Slider::new(&mut img_proc.gamma, 0.1..=5.0));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_sharpen));
            ui.checkbox(&mut img_proc.sharpen, "3x3 Spatial Edge Boost");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_denoise));
            ui.checkbox(&mut img_proc.denoise, "Box Smoothing Filter");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_morphology));
            ui.horizontal(|ui| {
                ui.radio_value(&mut img_proc.morphology, crate::infrastructure::settings::MorphologyOp::None, "None");
                ui.radio_value(&mut img_proc.morphology, crate::infrastructure::settings::MorphologyOp::Dilation, "Dilation (Thick)");
                ui.radio_value(&mut img_proc.morphology, crate::infrastructure::settings::MorphologyOp::Erosion, "Erosion (Thin)");
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.img_resize));
            ui.add(egui::Slider::new(&mut img_proc.resize_scale, 0.5..=4.0).suffix("x"));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_antialias));
            ui.checkbox(&mut img_proc.anti_alias_removal, "Quantize Boundary Smoothing");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_deskew));
            ui.checkbox(&mut img_proc.deskew, "Auto Alignment Correction");
            ui.end_row();
        });
}

// ─────────────────────────────────────────────
// Tab: Translation Behavior
// ─────────────────────────────────────────────
fn render_tab_translation_behavior(ui: &mut egui::Ui, settings: &mut Settings, i18n: &crate::user_interface::i18n::I18n) {
    ui.heading(i18n.tab_translation_behavior);
    ui.add_space(8.0);
    
    let beh = &mut settings.trans_behavior;

    section_header(ui, i18n.beh_prompt_cust);
    ui.checkbox(&mut beh.custom_prompts.enabled, "Enable Custom AI Prompts Overrides");
    if beh.custom_prompts.enabled {
        ui.add_space(4.0);
        egui::CollapsingHeader::new("Edit Prompt Templates")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Placeholders: {source_lang}, {target_lang}, {text}, {count}, {numbered_lines}").small().color(egui::Color32::GRAY));
                ui.add_space(6.0);
                
                ui.label("System Prompt (Role & Guidelines):");
                ui.add(egui::TextEdit::multiline(&mut beh.custom_prompts.system_prompt).desired_rows(3).desired_width(f32::INFINITY));
                ui.add_space(6.0);
                
                ui.label("Single-line User Prompt Template:");
                ui.add(egui::TextEdit::multiline(&mut beh.custom_prompts.single_line_user_prompt).desired_rows(2).desired_width(f32::INFINITY));
                ui.add_space(6.0);
                
                ui.label("Multi-line Batch User Prompt Template:");
                ui.add(egui::TextEdit::multiline(&mut beh.custom_prompts.multi_line_user_prompt).desired_rows(2).desired_width(f32::INFINITY));
                ui.add_space(4.0);
                
                if ui.button("Reset to Default Prompts").clicked() {
                    beh.custom_prompts = crate::infrastructure::settings::CustomPromptSettings {
                        enabled: true,
                        ..Default::default()
                    };
                }
            });
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    section_header(ui, i18n.beh_preset_modes);
    ui.horizontal(|ui| {
        ui.radio_value(&mut beh.preset, crate::infrastructure::settings::TranslationStylePreset::Standard, "Standard");
        ui.radio_value(&mut beh.preset, crate::infrastructure::settings::TranslationStylePreset::JrpgMode, "JRPG Mode");
        ui.radio_value(&mut beh.preset, crate::infrastructure::settings::TranslationStylePreset::AnimeSubtitle, "Anime Subtitle");
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.radio_value(&mut beh.preset, crate::infrastructure::settings::TranslationStylePreset::VisualNovel, "Visual Novel");
        ui.radio_value(&mut beh.preset, crate::infrastructure::settings::TranslationStylePreset::StreamerMode, "Streamer Mode");
    });
    
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    section_header(ui, i18n.beh_sliders);
    egui::Grid::new("behavior_sliders_grid").num_columns(2).spacing([20.0, 10.0]).show(ui, |ui| {
        ui.label("Style Balance:");
        ui.add(egui::Slider::new(&mut beh.literal_natural_slider, 0.0..=1.0)
            .text("Literal ↔ Natural"));
        ui.end_row();
        
        ui.label("AI Creativity:");
        ui.add(egui::Slider::new(&mut beh.creativity, 0.0..=1.0)
            .text("Low (Strict) ↔ High"));
        ui.end_row();
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    section_header(ui, i18n.beh_tone_rules);
    ui.horizontal(|ui| {
        ui.label("Voice Tone:");
        egui::ComboBox::from_id_salt("tone_combobox")
            .selected_text(format!("{:?}", beh.tone))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut beh.tone, crate::infrastructure::settings::TranslationTone::Auto, "Auto");
                ui.selectable_value(&mut beh.tone, crate::infrastructure::settings::TranslationTone::Formal, "Formal / Polite");
                ui.selectable_value(&mut beh.tone, crate::infrastructure::settings::TranslationTone::Casual, "Casual / Lively");
                ui.selectable_value(&mut beh.tone, crate::infrastructure::settings::TranslationTone::Polite, "Standard Public Polite");
            });
    });

    ui.add_space(10.0);
    section_header(ui, i18n.beh_strict_pres);
    egui::Grid::new("preservations_grid").num_columns(2).spacing([15.0, 8.0]).show(ui, |ui| {
        ui.checkbox(&mut beh.preserve_formatting, "Preserve Formatting");
        ui.checkbox(&mut beh.preserve_line_breaks, "Preserve Line Breaks");
        ui.end_row();
        
        ui.checkbox(&mut beh.preserve_punctuation, "Preserve Punctuation");
        ui.checkbox(&mut beh.preserve_honorifics, "Preserve Honorifics (-san)");
        ui.end_row();
        
        ui.checkbox(&mut beh.preserve_emojis, "Preserve Emojis / Kaomojis");
        ui.checkbox(&mut beh.contextual_translation, "Contextual Adaptation");
        ui.end_row();
        
        ui.checkbox(&mut beh.profanity_filter, "Safe Profanity Filter");
        ui.end_row();
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Realtime Stability Section ──
    section_header(ui, i18n.beh_stability);
    ui.label(egui::RichText::new("Prevent screen flickering and stabilize typewriter subtitles in games.").small().color(egui::Color32::GRAY));
    ui.add_space(6.0);

    let real = &mut settings.realtime;
    egui::Grid::new("realtime_stability_grid").num_columns(2).spacing([20.0, 12.0]).show(ui, |ui| {
        ui.label("Debounce Delay (Frames):");
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut real.stability_threshold_frames, 1..=10).text("Frames"));
            ui.label(egui::RichText::new("Wait for scrolling text to stop").small().color(egui::Color32::GRAY));
        });
        ui.end_row();

        ui.label("Subtitle Persistence:");
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut real.subtitle_persistence_ms, 0..=10000).step_by(500.0).text("ms"));
            ui.label(egui::RichText::new("Hold text after dialogue disappears").small().color(egui::Color32::GRAY));
        });
        ui.end_row();

        ui.label("Context Memory:");
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut real.context_window_size, 0..=5).text("Segments"));
            ui.label(egui::RichText::new("Remember past chat history").small().color(egui::Color32::GRAY));
        });
        ui.end_row();

        ui.label("Translation Smoothing:");
        ui.checkbox(&mut real.fade_smoothing, "Apply visual state persistence");
        ui.end_row();
    });
}

fn render_tab_performance(ui: &mut egui::Ui, settings: &mut crate::infrastructure::settings::Settings, i18n: &crate::user_interface::i18n::I18n) {
    section_header(ui, i18n.tab_performance);
    ui.label(egui::RichText::new("Fine-tune thread execution, hardware acceleration, and cache footprints for maximal frame stability.").small().color(egui::Color32::GRAY));
    ui.add_space(12.0);

    // Enforce default locks immediately
    settings.perf.enforce_preset_locks();

    // ── Presets Selector ──
    ui.label(egui::RichText::new("Power & Speed Preset").strong());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        use crate::infrastructure::settings::PerformancePreset;
        let mut curr_preset = settings.perf.preset;
        
        let presets = [
            (PerformancePreset::Eco, "Eco", "Minimal CPU & VRAM usage"),
            (PerformancePreset::Balanced, "Balanced", "Optimal auto-tuned resources"),
            (PerformancePreset::Performance, "Performance", "High speed thread scheduling"),
            (PerformancePreset::Ultra, "Ultra", "Maximal cores & memory limits"),
            (PerformancePreset::Custom, "Custom", "Unlock manual fine-tuning overrides"),
        ];

        for (p, label, tooltip) in presets {
            if ui.selectable_value(&mut curr_preset, p, label).on_hover_text(tooltip).clicked() {
                settings.perf.apply_preset(p);
            }
        }
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Detailed Controls (Locked unless Custom) ──
    let is_custom = settings.perf.preset == crate::infrastructure::settings::PerformancePreset::Custom;
    let perf = &mut settings.perf;
    
    egui::Grid::new("performance_tuning_grid").num_columns(2).spacing([20.0, 12.0]).show(ui, |ui| {
        ui.label(format!("{}:", i18n.perf_threads));
        ui.horizontal(|ui| {
            ui.add_enabled(is_custom, egui::Slider::new(&mut perf.worker_threads, 1..=32).text("Threads"));
            ui.label(egui::RichText::new("Concurrent pipelines").small().color(egui::Color32::GRAY));
        });
        ui.end_row();

        ui.label(format!("{}:", i18n.perf_gpu));
        ui.add_enabled_ui(is_custom, |ui| {
            egui::ComboBox::from_id_salt("gpu_backend_sel")
                .selected_text(format!("{:?}", perf.gpu_backend))
                .show_ui(ui, |ui| {
                    use crate::infrastructure::settings::GpuBackend;
                    ui.selectable_value(&mut perf.gpu_backend, GpuBackend::Auto, "Auto-Detect");
                    ui.selectable_value(&mut perf.gpu_backend, GpuBackend::Cpu, "CPU fallback");
                    ui.selectable_value(&mut perf.gpu_backend, GpuBackend::Cuda, "Nvidia CUDA");
                    ui.selectable_value(&mut perf.gpu_backend, GpuBackend::DirectMl, "DirectML (Windows)");
                    ui.selectable_value(&mut perf.gpu_backend, GpuBackend::TensorRt, "Nvidia TensorRT");
                });
        });
        ui.end_row();

        ui.label(format!("{}:", i18n.perf_parallel));
        ui.add_enabled_ui(is_custom, |ui| {
            ui.checkbox(&mut perf.parallel_ocr, "Scan multi-regions concurrently");
        });
        ui.end_row();

        ui.label(format!("{}:", i18n.perf_batching));
        ui.add_enabled_ui(is_custom, |ui| {
            ui.checkbox(&mut perf.enable_batching, "Batch short strings into single API requests");
        });
        ui.end_row();

        ui.label(format!("{}:", i18n.perf_memory));
        ui.horizontal(|ui| {
            ui.add_enabled(is_custom, egui::Slider::new(&mut perf.memory_cleanup_interval_secs, 10..=3600).step_by(10.0).text("Seconds"));
        });
        ui.end_row();

        ui.label(format!("{}:", i18n.perf_cache));
        ui.horizontal(|ui| {
            ui.add_enabled(is_custom, egui::Slider::new(&mut perf.max_cache_entries, 500..=100000).step_by(500.0).text("Entries"));
        });
        ui.end_row();

        ui.label(format!("{}:", i18n.perf_vram));
        ui.horizontal(|ui| {
            ui.add_enabled(is_custom, egui::Slider::new(&mut perf.vram_limit_mb, 0..=24576).step_by(512.0).text("MB"));
            let tooltip_str = if perf.vram_limit_mb == 0 { "Unlimited" } else { "Hard cap" };
            ui.label(egui::RichText::new(tooltip_str).small().color(egui::Color32::GRAY));
        });
        ui.end_row();
    });
}

// ─────────────────────────────────────────────
// Tab: Debugging Panel
// ─────────────────────────────────────────────
fn render_tab_debugging(ui: &mut egui::Ui, debug_infos: &[SlotDebugInfo], i18n: &crate::user_interface::i18n::I18n) {
    ui.heading(i18n.tab_debugging);
    ui.add_space(8.0);
    
    section_header(ui, i18n.dbg_telemetry);
    ui.label(egui::RichText::new(i18n.dbg_desc).small().color(egui::Color32::GRAY));
    ui.add_space(10.0);

    if debug_infos.is_empty() {
        ui.label(egui::RichText::new(i18n.dbg_no_active).color(egui::Color32::DARK_GRAY));
        return;
    }

    for (idx, info) in debug_infos.iter().enumerate() {
        egui::CollapsingHeader::new(format!("{} #{} [{}]", i18n.region, idx + 1, info.status))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new(format!("debug_grid_{}", idx)).num_columns(2).spacing([20.0, 8.0]).show(ui, |ui| {
                    ui.label(format!("{}:", i18n.dbg_worker_state));
                    ui.horizontal(|ui| {
                        if info.busy {
                            ui.label(egui::RichText::new(i18n.dbg_capturing).color(egui::Color32::GOLD));
                        } else if info.processing {
                            ui.label(egui::RichText::new(i18n.dbg_waiting_ai).color(egui::Color32::LIGHT_BLUE));
                        } else {
                            ui.label(egui::RichText::new(i18n.idle).color(egui::Color32::GREEN));
                        }
                        ui.label(format!("({})", info.status));
                    });
                    ui.end_row();
 
                    ui.label(format!("{}:", i18n.dbg_debounce));
                    ui.label(format!("{} {}", info.identical_frames, i18n.dbg_frames_ident));
                    ui.end_row();

                    ui.label(format!("{}:", i18n.dbg_ocr_lines));
                    ui.label(format!("{} {}", info.ocr_lines_count, i18n.dbg_entries_mapped));
                    ui.end_row();

                    ui.label(format!("{}:", i18n.dbg_trans_lines));
                    ui.label(format!("{} {}", info.trans_lines_count, i18n.dbg_entries_mapped));
                    ui.end_row();

                    ui.label(format!("{}:", i18n.dbg_processed_ocr));
                    ui.end_row();
                });

                ui.add_space(4.0);
                egui::Frame::default()
                    .fill(ui.visuals().extreme_bg_color)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        let text_to_show = if info.ocr_text.is_empty() { "<Empty>" } else { &info.ocr_text };
                        ui.label(egui::RichText::new(text_to_show).monospace().size(12.0));
                    });
                
                ui.add_space(8.0);
            });
            ui.add_space(6.0);
    }
}
