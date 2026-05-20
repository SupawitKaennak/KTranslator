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
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub groq_api_key: String,
    pub groq_model: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub custom_openai_url: String,
    pub custom_openai_api_key: String,
    pub custom_openai_model: String,
    pub custom_openai_use_list: bool,
    pub dark_mode: bool,
    pub smart_merge: bool,
    // Overlay Customization
    pub overlay_bg_color: [u8; 4],
    pub overlay_text_color: [u8; 4],
    pub overlay_font_size: f32,
    pub overlay_padding: f32,
    pub overlay_corner_radius: f32,
    pub overlay_text_align: TextAlign,

    pub use_yolo_bubble: bool,
    pub show_yolo_debug_borders: bool,

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
            dark_mode: true,
            smart_merge: false,
            overlay_bg_color: [0, 0, 0, 180], // Semi-transparent black
            overlay_text_color: [255, 255, 255, 255], // White
            overlay_font_size: 14.0,
            overlay_padding: 4.0,
            overlay_corner_radius: 4.0,
            overlay_text_align: TextAlign::Center,
            use_yolo_bubble: false,
            show_yolo_debug_borders: false,
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
    if !path.exists() {
        return Ok(Settings::default());
    }
    let bytes = fs::read(&path).with_context(|| format!("read settings at {}", path.display()))?;
    let s = serde_json::from_slice(&bytes).context("parse settings.json")?;
    Ok(s)
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create config dir {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(settings).context("serialize settings")?;
    fs::write(&path, bytes).with_context(|| format!("write settings at {}", path.display()))?;
    Ok(())
}
