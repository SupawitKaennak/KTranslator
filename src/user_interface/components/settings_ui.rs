use eframe::egui;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::Mutex;
use crate::infrastructure::settings::{Settings, TranslationProvider, UiLanguage};
use crate::user_interface::i18n::get_i18n;

pub struct SettingsWindowResponse {
    pub close_clicked: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    AiProvider,
    Ocr,
    TextProcessing,
    ImageProcessing,
    Overlay,
}

/// Renders the settings viewport with vertical tabs.
/// Returns a response indicating if save or close was requested.
pub fn show_settings_window(
    ctx: &egui::Context,
    settings_arc: Arc<Mutex<Settings>>,
    ctrl: &crate::core::usecases::settings_controller::SettingsController,
    download_progress: crate::infrastructure::asset_manager::DownloadProgress,
    download_trigger_tx: std::sync::mpsc::Sender<()>,
    slots_runtime: &[crate::core::worker::SlotRuntimeState],
) -> SettingsWindowResponse {
    let close_flag = Arc::new(AtomicBool::new(false));
    
    let close_flag_inner = close_flag.clone();
    let settings_inner = settings_arc.clone();
    let ctrl_inner = ctrl.clone();
    
    // Extract the pristine captured frame from the first active slot that has one
    let sample_frame = slots_runtime.iter()
        .find_map(|slot| slot.last_frame.lock().clone());
    
    let viewport_id = egui::ViewportId::from_hash_of("settings_viewport");

    ctx.show_viewport_immediate(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title("KTranslator - Settings")
            .with_inner_size([640.0, 500.0])
            .with_resizable(true)
            .with_always_on_top(),
        move |ctx, _| {
            if ctx.input(|i| i.viewport().close_requested()) {
                close_flag_inner.store(true, Ordering::Relaxed);
            }

            let active_tab: SettingsTab = ctx.data(|d| d.get_temp(egui::Id::new("settings_active_tab")))
                .unwrap_or(SettingsTab::General);

            let i18n = {
                let s = settings_inner.lock();
                get_i18n(s.ui_language)
            };

            // ── Left Sidebar (Vertical Tabs) ──
            egui::SidePanel::left("settings_tabs_panel")
                .resizable(false)
                .exact_width(150.0)
                .frame(egui::Frame::side_top_panel(ctx.style().as_ref()).inner_margin(8.0))
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new(format!("⚙ {}", i18n.settings)).strong());
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    let tabs = [
                        (SettingsTab::General,        i18n.tab_general),
                        (SettingsTab::AiProvider,     i18n.tab_ai_provider),
                        (SettingsTab::Ocr,            i18n.tab_ocr),
                        (SettingsTab::TextProcessing, i18n.tab_text_processing),
                        (SettingsTab::ImageProcessing, i18n.tab_image_processing),
                        (SettingsTab::Overlay,        i18n.tab_overlay),
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
                        SettingsTab::Ocr => render_tab_ocr(ui, &mut settings, i18n, &download_progress, &download_trigger_tx),
                        SettingsTab::TextProcessing => render_tab_text_processing(ui, &mut settings, i18n),
                        SettingsTab::ImageProcessing => render_tab_image_processing(ui, ctx, &mut settings, i18n, sample_frame.as_ref()),
                        SettingsTab::Overlay => render_tab_overlay(ui, &mut settings, i18n),
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

    section_header(ui, &format!("🌐 {}", i18n.ui_language));
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
    section_header(ui, "📸 Capture");
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
        (TranslationProvider::Ollama,       "Ollama (Offline)"),
        (TranslationProvider::CustomOpenAI, "Custom (OpenAI-Compatible)"),
        (TranslationProvider::Google,       "Google Translate (Free)"),
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
            section_header(ui, "⚙ Gemini Configuration");
            render_api_key_field(ui, i18n, &mut settings.gemini_api_key, &ctrl.gemini_models, &ctrl.gemini_fetching);
            try_fetch_gemini(ctx, settings, &ctrl.gemini_models, &ctrl.gemini_fetching);
            if !settings.gemini_api_key.trim().is_empty() {
                render_model_dropdown(ui, i18n, "gemini_mdl", &mut settings.gemini_model, &ctrl.gemini_models, &ctrl.gemini_fetching);
            }
            ui.add_space(4.0);
            ui.hyperlink_to("🔑 Get Gemini API Key", "https://aistudio.google.com/app/apikey");
        }
        TranslationProvider::Groq => {
            section_header(ui, "⚙ Groq Configuration");
            render_api_key_field(ui, i18n, &mut settings.groq_api_key, &ctrl.groq_models, &ctrl.groq_fetching);
            try_fetch_groq(ctx, settings, &ctrl.groq_models, &ctrl.groq_fetching);
            if !settings.groq_api_key.trim().is_empty() {
                render_model_dropdown(ui, i18n, "groq_mdl", &mut settings.groq_model, &ctrl.groq_models, &ctrl.groq_fetching);
            }
            ui.add_space(4.0);
            ui.hyperlink_to("🔑 Get Groq API Key", "https://console.groq.com/keys");
        }
        TranslationProvider::Ollama => {
            section_header(ui, "⚙ Ollama Configuration");
            ui.horizontal(|ui| {
                ui.label("Server URL:");
                let resp = ui.text_edit_singleline(&mut settings.ollama_url);
                if resp.lost_focus() && resp.changed() { ctrl.ollama_models.lock().clear(); }
            });
            try_fetch_ollama(ctx, settings, &ctrl.ollama_models, &ctrl.ollama_fetching);
            if !settings.ollama_url.trim().is_empty() {
                render_model_dropdown(ui, i18n, "ollama_mdl", &mut settings.ollama_model, &ctrl.ollama_models, &ctrl.ollama_fetching);
            }
            ui.add_space(4.0);
            ui.hyperlink_to("📦 Browse Ollama Models", "https://ollama.com/library");
        }
        TranslationProvider::CustomOpenAI => {
            section_header(ui, "⚙ Custom OpenAI-Compatible API");
            ui.horizontal(|ui| {
                ui.label("Base URL:");
                let resp = ui.text_edit_singleline(&mut settings.custom_openai_url);
                if resp.lost_focus() && resp.changed() { ctrl.custom_openai_models.lock().clear(); }
            });
            ui.horizontal(|ui| {
                ui.label(i18n.api_key);
                let resp = ui.add(egui::TextEdit::singleline(&mut settings.custom_openai_api_key).password(true));
                if resp.lost_focus() && resp.changed() { ctrl.custom_openai_models.lock().clear(); }
            });
            ui.add_space(4.0);
            ui.label("Model Selection:");
            ui.horizontal(|ui| {
                if ui.radio_value(&mut settings.custom_openai_use_list, false, "Manual Entry").changed() { ctrl.custom_openai_models.lock().clear(); }
                if ui.radio_value(&mut settings.custom_openai_use_list, true, "Fetch from List").changed() { ctrl.custom_openai_models.lock().clear(); }
            });
            if settings.custom_openai_use_list {
                try_fetch_custom(ctx, settings, &ctrl.custom_openai_models, &ctrl.custom_openai_fetching);
                render_model_dropdown(ui, i18n, "custom_mdl", &mut settings.custom_openai_model, &ctrl.custom_openai_models, &ctrl.custom_openai_fetching);
            } else {
                ui.horizontal(|ui| {
                    ui.label("Model Name:");
                    ui.text_edit_singleline(&mut settings.custom_openai_model);
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("API Keys:");
                ui.hyperlink_to("OpenRouter", "https://openrouter.ai/keys");
                ui.label("|");
                ui.hyperlink_to("Together AI", "https://api.together.xyz/settings/api-keys");
                ui.label("|");
                ui.hyperlink_to("OpenAI", "https://platform.openai.com/api-keys");
            });
        }
        TranslationProvider::Google => {
            section_header(ui, "ℹ Google Translate");
            ui.label("No configuration needed. Uses free Google Translate API.");
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
    download_trigger_tx: &std::sync::mpsc::Sender<()>,
) {
    ui.heading(i18n.tab_ocr);
    ui.add_space(8.0);

    section_header(ui, &format!("📋 {}", i18n.ocr));
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

    section_header(ui, &format!("⚙ {} — {}", i18n.choose_ocr, mode_name));
    ui.add_space(4.0);
    ui.radio_value(engine_ref, crate::infrastructure::settings::OcrEngineType::Windows, i18n.ocr_windows_desc);
    ui.radio_value(engine_ref, crate::infrastructure::settings::OcrEngineType::Paddle,  i18n.ocr_paddle_desc);
    ui.radio_value(engine_ref, crate::infrastructure::settings::OcrEngineType::MangaOCR, i18n.ocr_manga_desc);

    // MangaOCR: download section
    if *engine_ref == crate::infrastructure::settings::OcrEngineType::MangaOCR {
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
                if ui.button(i18n.download_install).clicked() { let _ = download_trigger_tx.send(()); }
            } else {
                ui.colored_label(egui::Color32::from_rgb(100, 255, 100), i18n.models_installed);
                if ui.button(i18n.reinstall_update).clicked() { let _ = download_trigger_tx.send(()); }
            }
        }
    }

    // PaddleOCR: path
    if *engine_ref == crate::infrastructure::settings::OcrEngineType::Paddle {
        ui.add_space(8.0);
        ui.label("PaddleOCR-json path:");
        ui.add(egui::TextEdit::singleline(&mut settings.paddle_ocr_path).hint_text("C:\\path\\to\\PaddleOCR-json.exe"));
    }
}

// ─────────────────────────────────────────────
// Tab 4: Text Processing
// ─────────────────────────────────────────────
fn render_tab_text_processing(ui: &mut egui::Ui, settings: &mut Settings, i18n: &crate::user_interface::i18n::I18n) {
    ui.heading(i18n.tab_text_processing);
    ui.add_space(8.0);

    section_header(ui, "📝 Sentence / Block Layout Alignment");
    ui.add_space(4.0);
    ui.checkbox(&mut settings.smart_merge, i18n.smart_merge);
    
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    section_header(ui, "🧹 Post-OCR Text Cleanup Filters");
    ui.label(egui::RichText::new("Advanced filters applied to scrub raw recognized text before entering Translation modules:").italics());
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

    section_header(ui, "🔤 Power User Regex Rule Engine");
    ui.label(egui::RichText::new("Advanced regular expression pipeline applied to OCR blocks or output translated text:").italics());
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
                ui.label("Pattern:");
                ui.add(egui::TextEdit::singleline(&mut rule.pattern).desired_width(140.0));

                let requires_replacement = match rule.rule_type {
                    crate::infrastructure::settings::RegexRuleType::Ignore 
                    | crate::infrastructure::settings::RegexRuleType::Split 
                    | crate::infrastructure::settings::RegexRuleType::Protected => false,
                    _ => true,
                };

                if requires_replacement {
                    ui.label("Replace:");
                    ui.add(egui::TextEdit::singleline(&mut rule.replacement).desired_width(100.0));
                }
            });
        });
        ui.add_space(4.0);
    }

    if let Some(idx) = remove_idx {
        settings.regex_rules.remove(idx);
    }

    if ui.button("➕ Add Regex Rule").clicked() {
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

    section_header(ui, "📖 Custom Dictionary / Glossary Engine");
    ui.label(egui::RichText::new("Enforce specific translations for characters, skills, items, slang, or memory overrides:").italics());
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
                        ui.selectable_value(&mut entry.entry_type, CharacterName, "Character Name");
                        ui.selectable_value(&mut entry.entry_type, GameTerminology, "Game Terminology");
                        ui.selectable_value(&mut entry.entry_type, SlangJargon, "Slang / Jargon");
                        ui.selectable_value(&mut entry.entry_type, ProtectedWord, "Protected Word (Masked)");
                        ui.selectable_value(&mut entry.entry_type, PhraseOverride, "Phrase Override (Pre-replace)");
                        ui.selectable_value(&mut entry.entry_type, TranslationMemory, "Translation Memory (100% Hit)");
                    });

                ui.label("Prio:");
                ui.add(egui::DragValue::new(&mut entry.priority).range(0..=100));

                if ui.button("🗑").clicked() {
                    remove_gloss_idx = Some(idx);
                }
            });

            ui.horizontal(|ui| {
                ui.label("Source:");
                ui.add(egui::TextEdit::singleline(&mut entry.source).desired_width(120.0));

                ui.label("Target:");
                ui.add(egui::TextEdit::singleline(&mut entry.target).desired_width(120.0));
            });
        });
        ui.add_space(4.0);
    }

    if let Some(idx) = remove_gloss_idx {
        settings.glossary_entries.remove(idx);
    }

    if ui.button("➕ Add Glossary Entry").clicked() {
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

    section_header(ui, &format!("🎨 {}", i18n.appearance));
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
    section_header(ui, "📺 Live Preview Processed Image");
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
            ui.label(egui::RichText::new(format!("📌 Using live captured frame ({}x{})", w, h)).color(egui::Color32::LIGHT_GREEN));
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
        ui.label(egui::RichText::new("📌 Using placeholder sample text (capture screen to view live frame)").color(egui::Color32::LIGHT_YELLOW));
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

            ui.label("Binarize Threshold:");
            ui.horizontal(|ui| {
                ui.checkbox(&mut img_proc.binarize, "Enable");
                if img_proc.binarize {
                    ui.add_space(10.0);
                    ui.add(egui::Slider::new(&mut img_proc.binary_threshold, 0..=255).text("Level"));
                }
            });
            ui.end_row();

            ui.label("Adaptive Threshold:");
            ui.checkbox(&mut img_proc.adaptive_threshold, "Local Box-filter Mean (Best for gradients)");
            ui.end_row();

            ui.label("Contrast Enhancement:");
            ui.add(egui::Slider::new(&mut img_proc.contrast, 0.0..=3.0));
            ui.end_row();

            ui.label("Brightness Adjustment:");
            ui.add(egui::Slider::new(&mut img_proc.brightness, -255..=255));
            ui.end_row();

            ui.label("Gamma Correction:");
            ui.add(egui::Slider::new(&mut img_proc.gamma, 0.1..=5.0));
            ui.end_row();

            ui.label("Sharpen Filter:");
            ui.checkbox(&mut img_proc.sharpen, "3x3 Spatial Edge Boost");
            ui.end_row();

            ui.label("Denoise:");
            ui.checkbox(&mut img_proc.denoise, "Box Smoothing Filter");
            ui.end_row();

            ui.label("Morphology Operation:");
            ui.horizontal(|ui| {
                ui.radio_value(&mut img_proc.morphology, crate::infrastructure::settings::MorphologyOp::None, "None");
                ui.radio_value(&mut img_proc.morphology, crate::infrastructure::settings::MorphologyOp::Dilation, "Dilation (Thick)");
                ui.radio_value(&mut img_proc.morphology, crate::infrastructure::settings::MorphologyOp::Erosion, "Erosion (Thin)");
            });
            ui.end_row();

            ui.label("Resize Scale:");
            ui.add(egui::Slider::new(&mut img_proc.resize_scale, 0.5..=4.0).suffix("x"));
            ui.end_row();

            ui.label("Anti-alias Removal:");
            ui.checkbox(&mut img_proc.anti_alias_removal, "Quantize Boundary Smoothing");
            ui.end_row();

            ui.label("Deskew Rotation:");
            ui.checkbox(&mut img_proc.deskew, "Auto Alignment Correction");
            ui.end_row();
        });
}
