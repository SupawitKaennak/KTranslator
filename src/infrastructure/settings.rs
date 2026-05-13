use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranslationProvider {
    Gemini,
    Groq,
    Ollama,
    CustomOpenAI,
    Google,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrEngineType {
    Windows,
    Paddle,
    MangaOCR,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrMode {
    Game,
    Manga,
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiLanguage {
    System,
    Thai,
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MorphologyOp {
    None,
    Dilation,
    Erosion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageProcessingSettings {
    pub grayscale: bool,
    pub invert: bool,
    pub contrast: f32,       // 0.0 - 3.0 (default 1.0)
    pub brightness: i32,     // -255 - 255 (default 0)
    pub gamma: f32,          // 0.1 - 5.0 (default 1.0)
    pub binarize: bool,
    pub binary_threshold: u8,// 0 - 255 (default 127)
    pub adaptive_threshold: bool,
    pub denoise: bool,
    pub sharpen: bool,
    pub morphology: MorphologyOp,
    pub resize_scale: f32,   // 0.5 - 4.0 (default 1.0)
    pub deskew: bool,
    pub anti_alias_removal: bool,
}

impl Default for ImageProcessingSettings {
    fn default() -> Self {
        Self {
            grayscale: false,
            invert: false,
            contrast: 1.0,
            brightness: 0,
            gamma: 1.0,
            binarize: false,
            binary_threshold: 127,
            adaptive_threshold: false,
            denoise: false,
            sharpen: false,
            morphology: MorphologyOp::None,
            resize_scale: 1.0,
            deskew: false,
            anti_alias_removal: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextProcessingSettings {
    pub remove_duplicates: bool,
    pub merge_broken_lines: bool,
    pub merge_subtitle_fragments: bool,
    pub remove_garbage: bool,
    pub recurring_suppression: bool,
    pub repeated_char_collapse: bool,
    pub min_text_length: usize,
    pub special_char_ratio_limit: f32, // 0.0 - 1.0
    pub consonant_spam_filter: bool,
    pub kana_spam_filter: bool,
    pub punctuation_normalization: bool,
}

impl Default for TextProcessingSettings {
    fn default() -> Self {
        Self {
            remove_duplicates: false, // Keep false by default to ensure 1-to-1 layout bounding box mapping
            merge_broken_lines: true,
            merge_subtitle_fragments: true,
            remove_garbage: true,
            recurring_suppression: true,
            repeated_char_collapse: true,
            min_text_length: 1,
            special_char_ratio_limit: 0.6,
            consonant_spam_filter: true,
            kana_spam_filter: true,
            punctuation_normalization: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RegexRuleType {
    PreTranslation,
    PostTranslation,
    Protected,
    Ignore,
    Replace,
    Split,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegexRule {
    pub enabled: bool,
    pub pattern: String,
    pub replacement: String,
    pub rule_type: RegexRuleType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GlossaryType {
    CharacterName,
    GameTerminology,
    ProtectedWord,
    PhraseOverride,
    SlangJargon,
    TranslationMemory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub enabled: bool,
    pub source: String,
    pub target: String,
    pub entry_type: GlossaryType,
    pub priority: i32, // Higher priority overrides lower ones
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: TranslationProvider,
    pub ocr_mode: OcrMode,
    pub game_ocr_engine: OcrEngineType,
    pub manga_ocr_engine: OcrEngineType,
    pub document_ocr_engine: OcrEngineType,
    pub ocr_engine: OcrEngineType, // Keep for backward compatibility or as fallback
    pub paddle_ocr_path: String,
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

    pub ui_language: UiLanguage,
    pub hide_from_capture: bool,
    
    pub img_proc: ImageProcessingSettings,
    pub txt_proc: TextProcessingSettings,
    pub regex_rules: Vec<RegexRule>,
    pub glossary_entries: Vec<GlossaryEntry>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: TranslationProvider::Gemini,
            ocr_mode: OcrMode::Game,
            game_ocr_engine: OcrEngineType::Windows,
            manga_ocr_engine: OcrEngineType::Paddle,
            document_ocr_engine: OcrEngineType::Windows,
            ocr_engine: OcrEngineType::Windows,
            paddle_ocr_path: String::new(),
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
            ui_language: UiLanguage::System,
            hide_from_capture: true,
            img_proc: ImageProcessingSettings::default(),
            txt_proc: TextProcessingSettings::default(),
            regex_rules: vec![],
            glossary_entries: vec![],
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

