use eframe::egui;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::Mutex;
use crate::infra::settings::{Settings, TranslationProvider, UiLanguage};
use crate::ui::i18n::get_i18n;

pub struct SettingsWindowResponse {
    pub save_clicked: bool,
    pub close_clicked: bool,
}

#[derive(serde::Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModelItem>,
}

#[derive(serde::Deserialize)]
struct OpenAiModelItem {
    id: String,
}

/// Renders the settings viewport. 
/// Returns a response indicating if save or close was requested.
pub fn show_settings_window(
    ctx: &egui::Context,
    settings_arc: Arc<Mutex<Settings>>,
    gemini_models: Arc<Mutex<Vec<String>>>,
    gemini_fetching: Arc<Mutex<bool>>,
    groq_models: Arc<Mutex<Vec<String>>>,
    groq_fetching: Arc<Mutex<bool>>,
    ollama_models: Arc<Mutex<Vec<String>>>,
    ollama_fetching: Arc<Mutex<bool>>,
    custom_models: Arc<Mutex<Vec<String>>>,
    custom_fetching: Arc<Mutex<bool>>,
    _custom_error: Arc<Mutex<Option<String>>>,
    download_progress: crate::infra::asset_manager::DownloadProgress,
    download_trigger_tx: std::sync::mpsc::Sender<()>,
) -> SettingsWindowResponse {
    let save_flag = Arc::new(AtomicBool::new(false));
    let close_flag = Arc::new(AtomicBool::new(false));
    
    let save_flag_inner = save_flag.clone();
    let close_flag_inner = close_flag.clone();
    let settings_inner = settings_arc.clone();
    
    let viewport_id = egui::ViewportId::from_hash_of("settings_viewport");

    ctx.show_viewport_immediate(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title("KTranslator - Settings")
            .with_inner_size([550.0, 600.0])
            .with_resizable(true)
            .with_always_on_top(),
        move |ctx, _| {
            if ctx.input(|i| i.viewport().close_requested()) {
                close_flag_inner.store(true, Ordering::Relaxed);
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                let mut settings = settings_inner.lock();
                let i18n = get_i18n(settings.ui_language);

                ui.heading(format!("⚙ {}", i18n.settings));
                ui.add_space(10.0);

                // --- 1. Provider Selection (Radio Buttons, 2 Columns) ---
                ui.label(egui::RichText::new(format!("{}:", i18n.provider)).strong());
                egui::Grid::new("provider_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut settings.provider, TranslationProvider::Gemini, "Gemini");
                            if settings.provider == TranslationProvider::Gemini {
                                if ui.button("⚙").on_hover_text("Configure Gemini").clicked() {
                                    ui.data_mut(|d| d.insert_temp(egui::Id::new("conf_gemini"), true));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut settings.provider, TranslationProvider::Groq, "Groq");
                            if settings.provider == TranslationProvider::Groq {
                                if ui.button("⚙").on_hover_text("Configure Groq").clicked() {
                                    ui.data_mut(|d| d.insert_temp(egui::Id::new("conf_groq"), true));
                                }
                            }
                        });
                        ui.end_row();

                        ui.horizontal(|ui| {
                            ui.radio_value(&mut settings.provider, TranslationProvider::Ollama, "Ollama (Offline)");
                            if settings.provider == TranslationProvider::Ollama {
                                if ui.button("⚙").on_hover_text("Configure Ollama").clicked() {
                                    ui.data_mut(|d| d.insert_temp(egui::Id::new("conf_ollama"), true));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut settings.provider, TranslationProvider::CustomOpenAI, "Custom (OpenAI)");
                            if settings.provider == TranslationProvider::CustomOpenAI {
                                if ui.button("⚙").on_hover_text("Configure Custom API").clicked() {
                                    ui.data_mut(|d| d.insert_temp(egui::Id::new("conf_custom"), true));
                                }
                            }
                        });
                        ui.end_row();

                        ui.horizontal(|ui| {
                            ui.radio_value(&mut settings.provider, TranslationProvider::Google, "Google Translate (Free)");
                        });
                        ui.end_row();
                    });
                
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // --- 2. OCR Mode Selection (User Friendly) ---
                ui.label(egui::RichText::new(format!("{}:", i18n.ocr)).strong());
                ui.add_space(4.0);
                
                let ocr_modes = [
                    (crate::infra::settings::OcrMode::Game, i18n.mode_game, "conf_ocr_game"),
                    (crate::infra::settings::OcrMode::Manga, i18n.mode_manga, "conf_ocr_manga"),
                    (crate::infra::settings::OcrMode::Document, i18n.mode_document, "conf_ocr_doc"),
                ];

                for (mode, label, conf_id) in ocr_modes {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut settings.ocr_mode, mode, label);
                        if settings.ocr_mode == mode {
                            if ui.button("⚙").on_hover_text("Select OCR Engine for this mode").clicked() {
                                ui.data_mut(|d| d.insert_temp(egui::Id::new(conf_id), true));
                            }
                        }
                    });
                    ui.add_space(2.0);
                }

                ui.add_space(12.0);
                ui.separator();
                ui.heading(format!("📺 {}", i18n.appearance));
                
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", i18n.ui_language));
                    egui::ComboBox::from_id_salt("ui_language_dropdown")
                        .selected_text(match settings.ui_language {
                            UiLanguage::System => i18n.system_default,
                            UiLanguage::Thai => "ไทย",
                            UiLanguage::English => "English",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut settings.ui_language, UiLanguage::System, i18n.system_default);
                            ui.selectable_value(&mut settings.ui_language, UiLanguage::Thai, "ไทย");
                            ui.selectable_value(&mut settings.ui_language, UiLanguage::English, "English");
                        });
                });
                
                ui.horizontal(|ui| {
                    ui.checkbox(&mut settings.smart_merge, i18n.smart_merge);
                });

                ui.horizontal(|ui| {
                    let mut allow = !settings.hide_from_capture;
                    if ui.checkbox(&mut allow, i18n.allow_capture).changed() {
                        settings.hide_from_capture = !allow;
                    }
                });

                ui.add_space(8.0);
                egui::Grid::new("overlay_settings_grid")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(format!("{}:", i18n.bg_color));
                        let mut bg_color = egui::Color32::from_rgba_unmultiplied(
                            settings.overlay_bg_color[0],
                            settings.overlay_bg_color[1],
                            settings.overlay_bg_color[2],
                            settings.overlay_bg_color[3],
                        );
                        if ui.color_edit_button_srgba(&mut bg_color).changed() {
                            settings.overlay_bg_color = bg_color.to_array();
                        }
                        ui.end_row();

                        ui.label(format!("{}:", i18n.text_color));
                        let mut text_color = egui::Color32::from_rgba_unmultiplied(
                            settings.overlay_text_color[0],
                            settings.overlay_text_color[1],
                            settings.overlay_text_color[2],
                            settings.overlay_text_color[3],
                        );
                        if ui.color_edit_button_srgba(&mut text_color).changed() {
                            settings.overlay_text_color = text_color.to_array();
                        }
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
                    });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(4.0);

                if ui.button(egui::RichText::new("💾 Save & Apply").size(16.0)).clicked() {
                    save_flag_inner.store(true, Ordering::Relaxed);
                }
            });

            // --- Technical Config Popups (Independent Viewports) ---
            
            // Gemini Config
            if ctx.data(|d| d.get_temp(egui::Id::new("conf_gemini")).unwrap_or(false)) {
                let settings_inner = settings_inner.clone();
                let gemini_models = gemini_models.clone();
                let gemini_fetching = gemini_fetching.clone();
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("viewport_gemini"),
                    egui::ViewportBuilder::default().with_title("Gemini Configuration").with_inner_size([400.0, 200.0]).with_always_on_top(),
                    move |ctx, _| {
                        if ctx.input(|i| i.viewport().close_requested()) { ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_gemini"), false)); }
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let mut settings = settings_inner.lock();
                            let i18n = get_i18n(settings.ui_language);
                            
                            let should_fetch = {
                                let models = gemini_models.lock();
                                let fetching = *gemini_fetching.lock();
                                models.is_empty() && !fetching && !settings.gemini_api_key.trim().is_empty()
                            };
                            if should_fetch {
                                let key = settings.gemini_api_key.clone();
                                let models_arc = gemini_models.clone();
                                let fetching_arc = gemini_fetching.clone();
                                let ctx_clone = ctx.clone();
                                *fetching_arc.lock() = true;
                                std::thread::spawn(move || {
                                    if let Ok(m_list) = crate::adapters::translate::gemini::GeminiTranslator::list_models(&key) {
                                        *models_arc.lock() = m_list.into_iter().map(|m| m.id).collect();
                                    }
                                    *fetching_arc.lock() = false;
                                    ctx_clone.request_repaint();
                                });
                            }

                            ui.horizontal(|ui| {
                                ui.label(i18n.api_key);
                                let resp = ui.add(egui::TextEdit::singleline(&mut settings.gemini_api_key).password(true));
                                if resp.lost_focus() && resp.changed() {
                                    gemini_models.lock().clear();
                                }
                            });
                            if !settings.gemini_api_key.trim().is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label(i18n.model);
                                    let models = gemini_models.lock();
                                    if models.is_empty() { ui.label(egui::RichText::new("(Fetching models...)").italics()); }
                                    else {
                                        egui::ComboBox::from_id_salt("gemini_model_dropdown").width(250.0).selected_text(settings.gemini_model.as_str()).show_ui(ui, |ui| {
                                            for m in models.iter() { ui.selectable_value(&mut settings.gemini_model, m.clone(), m); }
                                        });
                                    }
                                    if *gemini_fetching.lock() { ui.spinner(); }
                                });
                            }
                            ui.add_space(4.0);
                            ui.hyperlink_to("Get Gemini API Key", "https://aistudio.google.com/app/apikey");
                            ui.add_space(8.0);
                            if ui.button("Done").clicked() {
                                ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_gemini"), false));
                            }
                        });
                    }
                );
            }

            // Groq Config
            if ctx.data(|d| d.get_temp(egui::Id::new("conf_groq")).unwrap_or(false)) {
                let settings_inner = settings_inner.clone();
                let groq_models = groq_models.clone();
                let groq_fetching = groq_fetching.clone();
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("viewport_groq"),
                    egui::ViewportBuilder::default().with_title("Groq Configuration").with_inner_size([420.0, 220.0]).with_always_on_top(),
                    move |ctx, _| {
                        if ctx.input(|i| i.viewport().close_requested()) { ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_groq"), false)); }
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let mut settings = settings_inner.lock();
                            let i18n = get_i18n(settings.ui_language);
                            
                            let should_fetch = {
                                let models = groq_models.lock();
                                let fetching = *groq_fetching.lock();
                                models.is_empty() && !fetching && !settings.groq_api_key.trim().is_empty()
                            };
                            if should_fetch {
                                let key = settings.groq_api_key.clone();
                                let models_arc = groq_models.clone();
                                let fetching_arc = groq_fetching.clone();
                                let ctx_clone = ctx.clone();
                                *fetching_arc.lock() = true;
                                std::thread::spawn(move || {
                                    if let Ok(m_list) = crate::adapters::translate::groq::GroqTranslator::list_models(&key) {
                                        *models_arc.lock() = m_list;
                                    }
                                    *fetching_arc.lock() = false;
                                    ctx_clone.request_repaint();
                                });
                            }

                            ui.horizontal(|ui| {
                                ui.label(i18n.api_key);
                                let resp = ui.add(egui::TextEdit::singleline(&mut settings.groq_api_key).password(true));
                                if resp.lost_focus() && resp.changed() {
                                    groq_models.lock().clear();
                                }
                            });
                            if !settings.groq_api_key.trim().is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label(i18n.model);
                                    let models = groq_models.lock();
                                    if models.is_empty() { ui.label(egui::RichText::new("(Fetching models...)").italics()); }
                                    else {
                                        egui::ComboBox::from_id_salt("groq_model_dropdown").width(280.0).selected_text(settings.groq_model.as_str()).show_ui(ui, |ui| {
                                            for name in models.iter() { ui.selectable_value(&mut settings.groq_model, name.clone(), name); }
                                        });
                                    }
                                    if *groq_fetching.lock() { ui.spinner(); }
                                });
                            }
                            ui.hyperlink_to("Get Groq API Key", "https://console.groq.com/keys");
                            ui.add_space(8.0);
                            if ui.button("Done").clicked() {
                                ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_groq"), false));
                            }
                        });
                    }
                );
            }

            // Ollama Config
            if ctx.data(|d| d.get_temp(egui::Id::new("conf_ollama")).unwrap_or(false)) {
                let settings_inner = settings_inner.clone();
                let ollama_models = ollama_models.clone();
                let ollama_fetching = ollama_fetching.clone();
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("viewport_ollama"),
                    egui::ViewportBuilder::default().with_title("Ollama Configuration").with_inner_size([400.0, 200.0]).with_always_on_top(),
                    move |ctx, _| {
                        if ctx.input(|i| i.viewport().close_requested()) { ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_ollama"), false)); }
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let mut settings = settings_inner.lock();
                            
                            let should_fetch = {
                                let models = ollama_models.lock();
                                let fetching = *ollama_fetching.lock();
                                models.is_empty() && !fetching && !settings.ollama_url.trim().is_empty()
                            };
                            if should_fetch {
                                let url = settings.ollama_url.clone();
                                let models_arc = ollama_models.clone();
                                let fetching_arc = ollama_fetching.clone();
                                let ctx_clone = ctx.clone();
                                *fetching_arc.lock() = true;
                                std::thread::spawn(move || {
                                    if let Ok(m_list) = crate::adapters::translate::ollama::OllamaTranslator::list_models(&url) {
                                        *models_arc.lock() = m_list;
                                    }
                                    *fetching_arc.lock() = false;
                                    ctx_clone.request_repaint();
                                });
                            }

                            ui.label("Ollama (Local/Offline)");
                            ui.horizontal(|ui| {
                                ui.label("Server URL");
                                let resp = ui.text_edit_singleline(&mut settings.ollama_url);
                                if resp.lost_focus() && resp.changed() {
                                    ollama_models.lock().clear();
                                }
                            });
                            if !settings.ollama_url.trim().is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label("Model");
                                    let models = ollama_models.lock();
                                    if models.is_empty() { ui.label(egui::RichText::new("(Fetching models...)").italics()); }
                                    else {
                                        egui::ComboBox::from_id_salt("ollama_model_dropdown").width(250.0).selected_text(settings.ollama_model.as_str()).show_ui(ui, |ui| {
                                            for name in models.iter() { ui.selectable_value(&mut settings.ollama_model, name.clone(), name); }
                                        });
                                    }
                                    if *ollama_fetching.lock() { ui.spinner(); }
                                });
                            }
                            ui.add_space(4.0);
                            ui.hyperlink_to("Browse Ollama Models", "https://ollama.com/library");
                            ui.add_space(8.0);
                            if ui.button("Done").clicked() {
                                ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_ollama"), false));
                            }
                        });
                    }
                );
            }

            // Custom API Config
            if ctx.data(|d| d.get_temp(egui::Id::new("conf_custom")).unwrap_or(false)) {
                let settings_inner = settings_inner.clone();
                let custom_models = custom_models.clone();
                let custom_fetching = custom_fetching.clone();
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("viewport_custom"),
                    egui::ViewportBuilder::default().with_title("Custom API Configuration").with_inner_size([450.0, 250.0]).with_always_on_top(),
                    move |ctx, _| {
                        if ctx.input(|i| i.viewport().close_requested()) { ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_custom"), false)); }
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let mut settings = settings_inner.lock();
                            
                            let should_fetch = {
                                let models = custom_models.lock();
                                let fetching = *custom_fetching.lock();
                                settings.custom_openai_use_list && models.is_empty() && !fetching && !settings.custom_openai_url.trim().is_empty()
                            };
                            if should_fetch {
                                let url = settings.custom_openai_url.clone();
                                let key = settings.custom_openai_api_key.clone();
                                let models_arc = custom_models.clone();
                                let fetching_arc = custom_fetching.clone();
                                let ctx_clone = ctx.clone();
                                *fetching_arc.lock() = true;
                                std::thread::spawn(move || {
                                    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(10)).build();
                                    if let Ok(c) = client {
                                        let endpoint = format!("{}/models", url.trim_end_matches('/'));
                                        let mut req = c.get(&endpoint);
                                        if !key.trim().is_empty() { req = req.bearer_auth(key.trim()); }
                                        if let Ok(resp) = req.send() {
                                            if resp.status().is_success() {
                                                if let Ok(parsed) = serde_json::from_str::<OpenAiModelsResponse>(&resp.text().unwrap_or_default()) {
                                                    let mut m_list = parsed.data.into_iter().map(|i| i.id).collect::<Vec<_>>();
                                                    m_list.sort();
                                                    *models_arc.lock() = m_list;
                                                }
                                            }
                                        }
                                    }
                                    *fetching_arc.lock() = false;
                                    ctx_clone.request_repaint();
                                });
                            }

                            ui.horizontal(|ui| {
                                ui.label("Base URL");
                                let resp = ui.text_edit_singleline(&mut settings.custom_openai_url);
                                if resp.lost_focus() && resp.changed() { custom_models.lock().clear(); }
                            });
                            ui.horizontal(|ui| {
                                ui.label("API Key");
                                let resp = ui.add(egui::TextEdit::singleline(&mut settings.custom_openai_api_key).password(true));
                                if resp.lost_focus() && resp.changed() { custom_models.lock().clear(); }
                            });
                            ui.add_space(4.0);
                            ui.label("Model Selection Mode:");
                            ui.horizontal(|ui| {
                                if ui.radio_value(&mut settings.custom_openai_use_list, false, "Manual Entry").changed() { custom_models.lock().clear(); }
                                if ui.radio_value(&mut settings.custom_openai_use_list, true, "Fetch from List").changed() { custom_models.lock().clear(); }
                            });
                            if settings.custom_openai_use_list {
                                ui.horizontal(|ui| {
                                    ui.label("Model");
                                    let models = custom_models.lock();
                                    if models.is_empty() { ui.label(egui::RichText::new("(Fetching models...)").italics()); }
                                    else {
                                        egui::ComboBox::from_id_salt("custom_model_dropdown").width(250.0).selected_text(settings.custom_openai_model.as_str()).show_ui(ui, |ui| {
                                            for m in models.iter() { ui.selectable_value(&mut settings.custom_openai_model, m.clone(), m); }
                                        });
                                    }
                                    if *custom_fetching.lock() { ui.spinner(); }
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label("Model Name");
                                    ui.text_edit_singleline(&mut settings.custom_openai_model);
                                });
                            }
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Get API Keys:");
                                ui.hyperlink_to("OpenRouter", "https://openrouter.ai/keys");
                                ui.label("|");
                                ui.hyperlink_to("Together AI", "https://api.together.xyz/settings/api-keys");
                                ui.label("|");
                                ui.hyperlink_to("OpenAI", "https://platform.openai.com/api-keys");
                            });
                            ui.add_space(8.0);
                            if ui.button("Done").clicked() {
                                ctx.data_mut(|d| d.insert_temp(egui::Id::new("conf_custom"), false));
                            }
                        });
                    }
                );
            }

            // OCR Mode Settings
            for id in ["conf_ocr_game", "conf_ocr_manga", "conf_ocr_doc"] {
                if ctx.data(|d| d.get_temp(egui::Id::new(id)).unwrap_or(false)) {
                    let settings_inner = settings_inner.clone();
                    let download_progress = download_progress.clone();
                    let download_trigger_tx = download_trigger_tx.clone();
                    let title = match id { "conf_ocr_game" => "Game Mode OCR Settings", "conf_ocr_manga" => "Manga Mode OCR Settings", _ => "Document Mode OCR Settings" };
                    ctx.show_viewport_immediate(
                        egui::ViewportId::from_hash_of(id),
                        egui::ViewportBuilder::default().with_title(title).with_inner_size([400.0, 200.0]).with_always_on_top(),
                        move |ctx, _| {
                            if ctx.input(|i| i.viewport().close_requested()) { ctx.data_mut(|d| d.insert_temp(egui::Id::new(id), false)); }
                            egui::CentralPanel::default().show(ctx, |ui| {
                                let mut settings = settings_inner.lock();
                                let i18n = get_i18n(settings.ui_language);
                                let engine_ref = match id { "conf_ocr_game" => &mut settings.game_ocr_engine, "conf_ocr_manga" => &mut settings.manga_ocr_engine, _ => &mut settings.document_ocr_engine };
                                ui.label(i18n.choose_ocr);
                                ui.radio_value(engine_ref, crate::infra::settings::OcrEngineType::Windows, i18n.ocr_windows_desc);
                                ui.radio_value(engine_ref, crate::infra::settings::OcrEngineType::Paddle, i18n.ocr_paddle_desc);
                                ui.radio_value(engine_ref, crate::infra::settings::OcrEngineType::MangaOCR, i18n.ocr_manga_desc);
                                
                                if *engine_ref == crate::infra::settings::OcrEngineType::MangaOCR {
                                    ui.add_space(8.0);
                                    if download_progress.is_downloading {
                                        ui.label(format!("Downloading: {}", download_progress.current_file));
                                        ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
                                    } else {
                                        if let Some(err) = &download_progress.error {
                                            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), format!("Error: {}", err));
                                        }

                                        let models_exist = std::path::Path::new("models/manga-ocr/manga109_yolo_s.onnx").exists();
                                        if !models_exist {
                                            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), i18n.models_not_found);
                                            if ui.button(i18n.download_install).clicked() {
                                                let _ = download_trigger_tx.send(());
                                            }
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(100, 255, 100), i18n.models_installed);
                                            if ui.button(i18n.reinstall_update).clicked() {
                                                let _ = download_trigger_tx.send(());
                                            }
                                        }
                                    }
                                }

                                if *engine_ref == crate::infra::settings::OcrEngineType::Paddle {
                                    ui.add_space(8.0);
                                    ui.label("PaddleOCR-json path:");
                                    ui.add(egui::TextEdit::singleline(&mut settings.paddle_ocr_path).hint_text("C:\\path\\to\\PaddleOCR-json.exe"));
                                }
                                ui.add_space(8.0);
                                if ui.button("Done").clicked() { ctx.data_mut(|d| d.insert_temp(egui::Id::new(id), false)); }
                            });
                        }
                    );
                }
            }
        },
    );
    
    SettingsWindowResponse {
        save_clicked: save_flag.load(Ordering::Relaxed),
        close_clicked: close_flag.load(Ordering::Relaxed),
    }
}
