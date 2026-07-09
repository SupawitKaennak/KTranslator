pub mod enums;
pub mod image_processing;
pub mod performance;
pub mod rules;
pub mod text_processing;
pub mod translation;

pub use enums::*;
pub use image_processing::*;
pub use performance::*;
pub use rules::*;
pub use text_processing::*;
pub use translation::*;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: TranslationProvider,
    pub ocr_mode: OcrMode,
    pub game_ocr_engine: OcrEngineType,
    pub manga_ocr_engine: OcrEngineType,
    pub document_ocr_engine: OcrEngineType,
    pub ocr_engine: OcrEngineType, // Keep for backward compatibility or as fallback
    pub ppocr_model: PpocrModelSuite,
    #[serde(skip_serializing)]
    pub gemini_api_key: String,
    pub gemini_model: String,
    #[serde(skip_serializing)]
    pub groq_api_key: String,
    pub groq_model: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub custom_openai_url: String,
    #[serde(skip_serializing)]
    pub custom_openai_api_key: String,
    pub custom_openai_model: String,
    pub custom_openai_use_list: bool,
    #[serde(skip_serializing)]
    pub claude_api_key: String,
    pub claude_model: String,
    #[serde(skip_serializing)]
    pub deepseek_api_key: String,
    pub deepseek_model: String,
    #[serde(skip_serializing)]
    pub deepl_api_key: String,
    pub lm_studio_url: String,
    pub lm_studio_model: String,
    pub azure_openai_url: String,
    #[serde(skip_serializing)]
    pub azure_openai_api_key: String,
    pub azure_deployment_name: String,
    pub azure_api_version: String,
    pub dark_mode: bool,
    pub smart_merge: bool,
    pub enable_llm_ocr_correction: bool,
    // Overlay Customization
    pub overlay_bg_color: [u8; 4],
    pub overlay_text_color: [u8; 4],
    pub overlay_font_size: f32,
    pub overlay_padding: f32,
    pub overlay_corner_radius: f32,
    pub overlay_text_align: TextAlign,

    pub use_yolo_bubble: bool,
    pub show_yolo_debug_borders: bool,
    pub text_detector: TextDetectorMode,

    pub ui_language: UiLanguage,
    pub hide_from_capture: bool,

    pub img_proc: ImageProcessingSettings,
    pub txt_proc: TextProcessingSettings,
    pub regex_rules: Vec<RegexRule>,
    pub glossary_entries: Vec<GlossaryEntry>,
    pub trans_behavior: TranslationBehaviorSettings,
    pub realtime: RealtimeStabilitySettings,
    pub perf: PerformanceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: TranslationProvider::Gemini,
            ocr_mode: OcrMode::Game,
            game_ocr_engine: OcrEngineType::Windows,
            manga_ocr_engine: OcrEngineType::BuiltinPaddle,
            document_ocr_engine: OcrEngineType::Windows,
            ocr_engine: OcrEngineType::Windows,
            ppocr_model: PpocrModelSuite::CnEnMobile,
            gemini_api_key: String::new(),
            gemini_model: "gemini-2.0-flash".to_string(),
            groq_api_key: String::new(),
            groq_model: "llama-3.3-70b-versatile".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "llama3.2:1b".to_string(),
            custom_openai_url: "https://api.openai.com/v1".to_string(),
            custom_openai_api_key: String::new(),
            custom_openai_model: "gpt-4o-mini".to_string(),
            custom_openai_use_list: false,
            claude_api_key: String::new(),
            claude_model: "claude-3-5-sonnet-latest".to_string(),
            deepseek_api_key: String::new(),
            deepseek_model: "deepseek-chat".to_string(),
            deepl_api_key: String::new(),
            lm_studio_url: "http://localhost:1234/v1".to_string(),
            lm_studio_model: String::new(),
            azure_openai_url: String::new(),
            azure_openai_api_key: String::new(),
            azure_deployment_name: String::new(),
            azure_api_version: "2024-02-01".to_string(),
            dark_mode: true,
            smart_merge: false,
            enable_llm_ocr_correction: false,
            overlay_bg_color: [0, 0, 0, 180], // Semi-transparent black
            overlay_text_color: [255, 255, 255, 255], // White
            overlay_font_size: 14.0,
            overlay_padding: 4.0,
            overlay_corner_radius: 4.0,
            overlay_text_align: TextAlign::Center,
            use_yolo_bubble: false,
            show_yolo_debug_borders: false,
            text_detector: TextDetectorMode::None,
            ui_language: UiLanguage::System,
            hide_from_capture: true,
            img_proc: ImageProcessingSettings::default(),
            txt_proc: TextProcessingSettings::default(),
            regex_rules: vec![],
            glossary_entries: vec![],
            trans_behavior: TranslationBehaviorSettings::default(),
            realtime: RealtimeStabilitySettings::default(),
            perf: PerformanceSettings::default(),
        }
    }
}

fn settings_path() -> Result<PathBuf> {
    let proj = ProjectDirs::from("com", "ktranslator", "ktranslator")
        .context("ProjectDirs not available")?;
    Ok(proj.config_dir().join("settings.json"))
}

pub fn load_settings() -> Result<Settings> {
    let path = settings_path()?;
    let mut s = if !path.exists() {
        Settings::default()
    } else {
        let bytes = fs::read(&path).with_context(|| format!("read settings at {}", path.display()))?;
        serde_json::from_slice(&bytes).context("parse settings.json")?
    };
    
    load_or_migrate_secrets(&mut s);
    Ok(s)
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create config dir {}", parent.display()))?;
    }
    
    save_secrets(settings);
    
    let bytes = serde_json::to_vec_pretty(settings).context("serialize settings")?;
    fs::write(&path, bytes).with_context(|| format!("write settings at {}", path.display()))?;
    Ok(())
}

fn get_keyring_entry(key_name: &str) -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new("ktranslator", key_name)
}

fn load_or_migrate_secrets(settings: &mut Settings) {
    let keys = [
        ("gemini_api_key", &mut settings.gemini_api_key),
        ("groq_api_key", &mut settings.groq_api_key),
        ("custom_openai_api_key", &mut settings.custom_openai_api_key),
        ("claude_api_key", &mut settings.claude_api_key),
        ("deepseek_api_key", &mut settings.deepseek_api_key),
        ("deepl_api_key", &mut settings.deepl_api_key),
        ("azure_openai_api_key", &mut settings.azure_openai_api_key),
    ];

    for (name, setting_val) in keys {
        if let Ok(entry) = get_keyring_entry(name) {
            match entry.get_password() {
                Ok(pwd) => {
                    *setting_val = pwd;
                }
                Err(keyring::Error::NoEntry) => {
                    if !setting_val.is_empty() {
                        let _ = entry.set_password(setting_val.as_str());
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read {} from keyring: {}", name, e);
                }
            }
        }
    }
}

fn save_secrets(settings: &Settings) {
    let keys = [
        ("gemini_api_key", &settings.gemini_api_key),
        ("groq_api_key", &settings.groq_api_key),
        ("custom_openai_api_key", &settings.custom_openai_api_key),
        ("claude_api_key", &settings.claude_api_key),
        ("deepseek_api_key", &settings.deepseek_api_key),
        ("deepl_api_key", &settings.deepl_api_key),
        ("azure_openai_api_key", &settings.azure_openai_api_key),
    ];

    for (name, setting_val) in keys {
        if let Ok(entry) = get_keyring_entry(name) {
            if setting_val.is_empty() {
                let _ = entry.delete_credential();
            } else {
                let _ = entry.set_password(setting_val.as_str());
            }
        }
    }
}

