use crate::infrastructure::settings::{Settings, TranslationProvider};
use eframe::egui;
use parking_lot::Mutex;
use std::sync::Arc;

pub fn render_tab_ai_provider(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
    ctrl: &crate::core::usecases::settings_controller::SettingsController,
) {
    ui.heading(i18n.tab_ai_provider);
    ui.add_space(8.0);

    super::section_header(ui, i18n.provider);
    ui.add_space(4.0);

    let providers = [
        (TranslationProvider::Gemini, "Gemini"),
        (TranslationProvider::Groq, "Groq"),
        (
            TranslationProvider::Ollama,
            &format!("Ollama ({})", i18n.offline),
        ),
        (
            TranslationProvider::CustomOpenAI,
            &format!("Custom ({})", i18n.compatible),
        ),
        (
            TranslationProvider::Google,
            &format!("Google Translate ({})", i18n.auto_detect),
        ),
        (TranslationProvider::Claude, i18n.prov_claude),
        (TranslationProvider::DeepSeek, i18n.prov_deepseek),
        (TranslationProvider::DeepL, i18n.prov_deepl),
        (TranslationProvider::LmStudio, i18n.prov_lmstudio),
        (TranslationProvider::AzureOpenAI, i18n.prov_azure),
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
            super::section_header(ui, &format!("Gemini {}", i18n.config_for));
            render_api_key_field(
                ui,
                i18n,
                &mut settings.gemini_api_key,
                &ctrl.gemini_models,
                &ctrl.gemini_fetching,
            );
            try_fetch_gemini(ctx, settings, &ctrl.gemini_models, &ctrl.gemini_fetching);
            if !settings.gemini_api_key.trim().is_empty() {
                render_model_dropdown(
                    ui,
                    i18n,
                    "gemini_mdl",
                    &mut settings.gemini_model,
                    &ctrl.gemini_models,
                    &ctrl.gemini_fetching,
                );
            }
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.get_api_key, "https://aistudio.google.com/app/apikey");
        }
        TranslationProvider::Groq => {
            super::section_header(ui, &format!("Groq {}", i18n.config_for));
            render_api_key_field(
                ui,
                i18n,
                &mut settings.groq_api_key,
                &ctrl.groq_models,
                &ctrl.groq_fetching,
            );
            try_fetch_groq(ctx, settings, &ctrl.groq_models, &ctrl.groq_fetching);
            if !settings.groq_api_key.trim().is_empty() {
                render_model_dropdown(
                    ui,
                    i18n,
                    "groq_mdl",
                    &mut settings.groq_model,
                    &ctrl.groq_models,
                    &ctrl.groq_fetching,
                );
            }
            ui.add_space(4.0);
            egui::CollapsingHeader::new("Advanced (Custom Endpoint)").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Base URL:");
                    ui.text_edit_singleline(&mut settings.groq_base_url);
                });
            });
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.get_api_key, "https://console.groq.com/keys");
        }
        TranslationProvider::Ollama => {
            super::section_header(ui, &format!("Ollama {}", i18n.config_for));
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.server_url));
                let resp = ui.text_edit_singleline(&mut settings.ollama_url);
                if resp.lost_focus() && resp.changed() {
                    ctrl.ollama_models.lock().clear();
                }
            });
            try_fetch_ollama(ctx, settings, &ctrl.ollama_models, &ctrl.ollama_fetching);
            if !settings.ollama_url.trim().is_empty() {
                render_model_dropdown(
                    ui,
                    i18n,
                    "ollama_mdl",
                    &mut settings.ollama_model,
                    &ctrl.ollama_models,
                    &ctrl.ollama_fetching,
                );
            }
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.browse_models, "https://ollama.com/library");
        }
        TranslationProvider::CustomOpenAI => {
            super::section_header(ui, i18n.prov_custom_endpoint);
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.base_url));
                let resp = ui.text_edit_singleline(&mut settings.custom_openai_url);
                if resp.lost_focus() && resp.changed() {
                    ctrl.custom_openai_models.lock().clear();
                }
            });
            ui.horizontal(|ui| {
                ui.label(i18n.api_key);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut settings.custom_openai_api_key).password(true),
                );
                if resp.lost_focus() && resp.changed() {
                    ctrl.custom_openai_models.lock().clear();
                }
            });
            ui.add_space(4.0);
            ui.label(format!("{}:", i18n.model_selection));
            ui.horizontal(|ui| {
                if ui
                    .radio_value(
                        &mut settings.custom_openai_use_list,
                        false,
                        i18n.manual_entry,
                    )
                    .changed()
                {
                    ctrl.custom_openai_models.lock().clear();
                }
                if ui
                    .radio_value(&mut settings.custom_openai_use_list, true, i18n.fetch_list)
                    .changed()
                {
                    ctrl.custom_openai_models.lock().clear();
                }
            });
            if settings.custom_openai_use_list {
                try_fetch_custom(
                    ctx,
                    settings,
                    &ctrl.custom_openai_models,
                    &ctrl.custom_openai_fetching,
                );
                render_model_dropdown(
                    ui,
                    i18n,
                    "custom_mdl",
                    &mut settings.custom_openai_model,
                    &ctrl.custom_openai_models,
                    &ctrl.custom_openai_fetching,
                );
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
            });
        }
        TranslationProvider::Google => {
            super::section_header(ui, &format!("Google Translate {}", i18n.config_for));
            ui.label(i18n.google_api_desc);
        }
        TranslationProvider::Claude => {
            super::section_header(ui, &format!("{} {}", i18n.prov_claude, i18n.config_for));
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.api_key));
                ui.add(egui::TextEdit::singleline(&mut settings.claude_api_key).password(true));
            });
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.model_name));
                ui.text_edit_singleline(&mut settings.claude_model);
            });
            ui.add_space(4.0);
            egui::CollapsingHeader::new("Advanced (Custom Endpoint)").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Base URL:");
                    ui.text_edit_singleline(&mut settings.claude_base_url);
                });
            });
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.get_api_key, "https://console.anthropic.com/settings/keys");
        }
        TranslationProvider::DeepSeek => {
            super::section_header(ui, &format!("{} {}", i18n.prov_deepseek, i18n.config_for));
            render_api_key_field(
                ui,
                i18n,
                &mut settings.deepseek_api_key,
                &ctrl.deepseek_models,
                &ctrl.deepseek_fetching,
            );
            try_fetch_deepseek(ctx, settings, &ctrl.deepseek_models, &ctrl.deepseek_fetching);
            if !settings.deepseek_api_key.trim().is_empty() {
                render_model_dropdown(
                    ui,
                    i18n,
                    "deepseek_mdl",
                    &mut settings.deepseek_model,
                    &ctrl.deepseek_models,
                    &ctrl.deepseek_fetching,
                );
            }
            ui.add_space(4.0);
            egui::CollapsingHeader::new("Advanced (Custom Endpoint)").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Base URL:");
                    ui.text_edit_singleline(&mut settings.deepseek_base_url);
                });
            });
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.get_api_key, "https://platform.deepseek.com/api_keys");
        }
        TranslationProvider::DeepL => {
            super::section_header(ui, &format!("{} {}", i18n.prov_deepl, i18n.config_for));
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.api_key));
                ui.add(egui::TextEdit::singleline(&mut settings.deepl_api_key).password(true));
            });
            ui.add_space(4.0);
            egui::CollapsingHeader::new("Advanced (Custom Endpoint)").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Base URL:");
                    ui.text_edit_singleline(&mut settings.deepl_base_url);
                });
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new(i18n.deepl_note).color(ui.visuals().warn_fg_color));
            ui.add_space(4.0);
            ui.hyperlink_to(i18n.get_api_key, "https://www.deepl.com/pro-api");
        }
        TranslationProvider::LmStudio => {
            super::section_header(ui, &format!("{} {}", i18n.prov_lmstudio, i18n.config_for));
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.server_url));
                let resp = ui.text_edit_singleline(&mut settings.lm_studio_url);
                if resp.lost_focus() && resp.changed() {
                    ctrl.lm_studio_models.lock().clear();
                }
            });
            try_fetch_lm_studio(ctx, settings, &ctrl.lm_studio_models, &ctrl.lm_studio_fetching);
            if !settings.lm_studio_url.trim().is_empty() {
                render_model_dropdown(
                    ui,
                    i18n,
                    "lm_studio_mdl",
                    &mut settings.lm_studio_model,
                    &ctrl.lm_studio_models,
                    &ctrl.lm_studio_fetching,
                );
            }
        }
        TranslationProvider::AzureOpenAI => {
            super::section_header(ui, &format!("{} {}", i18n.prov_azure, i18n.config_for));
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.base_url));
                ui.text_edit_singleline(&mut settings.azure_openai_url);
            });
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.api_key));
                ui.add(egui::TextEdit::singleline(&mut settings.azure_openai_api_key).password(true));
            });
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.azure_deployment_name));
                ui.text_edit_singleline(&mut settings.azure_deployment_name);
            });
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.azure_api_version));
                ui.text_edit_singleline(&mut settings.azure_api_version);
            });
        }
    }
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
        if resp.lost_focus() && resp.changed() {
            models.lock().clear();
        }
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
            egui::ComboBox::from_id_salt(id)
                .width(250.0)
                .selected_text(selected.as_str())
                .show_ui(ui, |ui| {
                    for name in m.iter() {
                        ui.selectable_value(selected, name.clone(), name);
                    }
                });
        }
        if *fetching.lock() {
            ui.spinner();
        }
    });
}

fn try_fetch_gemini(
    ctx: &egui::Context,
    settings: &Settings,
    models: &Arc<Mutex<Vec<String>>>,
    fetching: &Arc<Mutex<bool>>,
) {
    let should = {
        models.lock().is_empty() && !*fetching.lock() && !settings.gemini_api_key.trim().is_empty()
    };
    if !should {
        return;
    }
    let key = settings.gemini_api_key.clone();
    let m = models.clone();
    let f = fetching.clone();
    let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        match crate::adapters::translate::gemini::GeminiTranslator::list_models(&key) {
            Ok(list) => {
                let items: Vec<String> = list.into_iter().map(|x| x.id).collect();
                *m.lock() = if items.is_empty() { vec!["(No models found)".to_string()] } else { items };
            }
            Err(e) => {
                *m.lock() = vec![format!("(Error: {})", e.to_string().lines().next().unwrap_or("Failed"))];
            }
        }
        *f.lock() = false;
        c.request_repaint();
        c.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
    });
}

fn try_fetch_groq(
    ctx: &egui::Context,
    settings: &Settings,
    models: &Arc<Mutex<Vec<String>>>,
    fetching: &Arc<Mutex<bool>>,
) {
    let should = {
        models.lock().is_empty() && !*fetching.lock() && !settings.groq_api_key.trim().is_empty()
    };
    if !should {
        return;
    }
    let key = settings.groq_api_key.clone();
    let m = models.clone();
    let f = fetching.clone();
    let base_url = settings.groq_base_url.clone();
    let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        match crate::adapters::translate::groq::GroqTranslator::list_models(&key, &base_url) {
            Ok(list) => {
                *m.lock() = if list.is_empty() { vec!["(No models found)".to_string()] } else { list };
            }
            Err(e) => {
                *m.lock() = vec![format!("(Error: {})", e.to_string().lines().next().unwrap_or("Failed"))];
            }
        }
        *f.lock() = false;
        c.request_repaint();
        c.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
    });
}

fn try_fetch_ollama(
    ctx: &egui::Context,
    settings: &Settings,
    models: &Arc<Mutex<Vec<String>>>,
    fetching: &Arc<Mutex<bool>>,
) {
    let should =
        { models.lock().is_empty() && !*fetching.lock() && !settings.ollama_url.trim().is_empty() };
    if !should {
        return;
    }
    let url = settings.ollama_url.clone();
    let m = models.clone();
    let f = fetching.clone();
    let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        match crate::adapters::translate::ollama::OllamaTranslator::list_models(&url) {
            Ok(list) => {
                *m.lock() = if list.is_empty() { vec!["(No models found)".to_string()] } else { list };
            }
            Err(e) => {
                *m.lock() = vec![format!("(Error: {})", e.to_string().lines().next().unwrap_or("Failed"))];
            }
        }
        *f.lock() = false;
        c.request_repaint();
        c.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
    });
}

fn try_fetch_custom(
    ctx: &egui::Context,
    settings: &Settings,
    models: &Arc<Mutex<Vec<String>>>,
    fetching: &Arc<Mutex<bool>>,
) {
    let should = {
        settings.custom_openai_use_list
            && models.lock().is_empty()
            && !*fetching.lock()
            && !settings.custom_openai_url.trim().is_empty()
    };
    if !should {
        return;
    }
    let url = settings.custom_openai_url.clone();
    let key = settings.custom_openai_api_key.clone();
    let m = models.clone();
    let f = fetching.clone();
    let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        match crate::adapters::translate::openai::OpenAiTranslator::list_models(&url, &key) {
            Ok(list) => {
                *m.lock() = if list.is_empty() { vec!["(No models found)".to_string()] } else { list };
            }
            Err(e) => {
                *m.lock() = vec![format!("(Error: {})", e.to_string().lines().next().unwrap_or("Failed"))];
            }
        }
        *f.lock() = false;
        c.request_repaint();
        c.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
    });
}

fn try_fetch_deepseek(
    ctx: &egui::Context,
    settings: &Settings,
    models: &Arc<Mutex<Vec<String>>>,
    fetching: &Arc<Mutex<bool>>,
) {
    let should = {
        models.lock().is_empty() && !*fetching.lock() && !settings.deepseek_api_key.trim().is_empty()
    };
    if !should {
        return;
    }
    let key = settings.deepseek_api_key.clone();
    let m = models.clone();
    let f = fetching.clone();
    let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        match crate::adapters::translate::openai::OpenAiTranslator::list_models("https://api.deepseek.com/v1", &key) {
            Ok(list) => {
                *m.lock() = if list.is_empty() { vec!["(No models found)".to_string()] } else { list };
            }
            Err(e) => {
                *m.lock() = vec![format!("(Error: {})", e.to_string().lines().next().unwrap_or("Failed"))];
            }
        }
        *f.lock() = false;
        c.request_repaint();
        c.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
    });
}

fn try_fetch_lm_studio(
    ctx: &egui::Context,
    settings: &Settings,
    models: &Arc<Mutex<Vec<String>>>,
    fetching: &Arc<Mutex<bool>>,
) {
    let should = {
        models.lock().is_empty() && !*fetching.lock() && !settings.lm_studio_url.trim().is_empty()
    };
    if !should {
        return;
    }
    let url = settings.lm_studio_url.clone();
    let m = models.clone();
    let f = fetching.clone();
    let c = ctx.clone();
    *f.lock() = true;
    std::thread::spawn(move || {
        match crate::adapters::translate::openai::OpenAiTranslator::list_models(&url, "") {
            Ok(list) => {
                *m.lock() = if list.is_empty() { vec!["(No models found)".to_string()] } else { list };
            }
            Err(e) => {
                *m.lock() = vec![format!("(Error: {})", e.to_string().lines().next().unwrap_or("Failed"))];
            }
        }
        *f.lock() = false;
        c.request_repaint();
        c.request_repaint_of(egui::ViewportId::from_hash_of("settings_viewport"));
    });
}
