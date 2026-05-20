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
    /// Legacy alias — old configs with "Paddle" will deserialize to BuiltinPaddle
    #[serde(alias = "Paddle")]
    BuiltinPaddle,
    MangaOCR,
    BubbleYOLO,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PpocrModelSuite {
    #[default]
    CnEnMobile,
    CnEnServer,
    JapaneseMobile,
    JapaneseServer,
    KoreanMobile,
    KoreanServer,
    ThaiMobile,
    ThaiServer,
    LatinMobile,
    LatinServer,
    CyrillicMobile,
    CyrillicServer,
}

impl PpocrModelSuite {
    pub fn folder_name(&self) -> &'static str {
        match self {
            PpocrModelSuite::CnEnMobile => "cn_en_mobile",
            PpocrModelSuite::CnEnServer => "cn_en_server",
            PpocrModelSuite::JapaneseMobile => "mobile_japanese",
            PpocrModelSuite::JapaneseServer => "server_japanese",
            PpocrModelSuite::KoreanMobile => "mobile_korean",
            PpocrModelSuite::KoreanServer => "server_korean",
            PpocrModelSuite::ThaiMobile => "mobile_thai",
            PpocrModelSuite::ThaiServer => "server_thai",
            PpocrModelSuite::LatinMobile => "mobile_latin",
            PpocrModelSuite::LatinServer => "server_latin",
            PpocrModelSuite::CyrillicMobile => "mobile_cyrillic",
            PpocrModelSuite::CyrillicServer => "server_cyrillic",
        }
    }
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
    pub contrast: f32,   // 0.0 - 3.0 (default 1.0)
    pub brightness: i32, // -255 - 255 (default 0)
    pub gamma: f32,      // 0.1 - 5.0 (default 1.0)
    pub binarize: bool,
    pub binary_threshold: u8, // 0 - 255 (default 127)
    pub adaptive_threshold: bool,
    pub denoise: bool,
    pub sharpen: bool,
    pub morphology: MorphologyOp,
    pub resize_scale: f32, // 0.5 - 4.0 (default 1.0)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChineseConversionMode {
    None,
    SimplifiedToTraditional,
    TraditionalToSimplified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThaiSegmentationMode {
    Standard,
    DictionaryAssisted,
    SyllableLevel,
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
    pub enable_wordninja: bool,

    // ── Language-Specific Processing ──
    pub jp_merge_vertical: bool,
    pub jp_kana_normalization: bool,
    pub jp_remove_furigana: bool,
    pub jp_preserve_honorifics: bool,

    pub cn_conversion: ChineseConversionMode,

    pub th_segmentation: ThaiSegmentationMode,
    pub th_zero_width_cleanup: bool,

    pub ar_rtl_correction: bool,
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
            enable_wordninja: false,

            jp_merge_vertical: true,
            jp_kana_normalization: true,
            jp_remove_furigana: true,
            jp_preserve_honorifics: false,

            cn_conversion: ChineseConversionMode::None,

            th_segmentation: ThaiSegmentationMode::Standard,
            th_zero_width_cleanup: true,

            ar_rtl_correction: true,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranslationTone {
    Auto,
    Formal,
    Casual,
    Polite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranslationStylePreset {
    Standard,
    JrpgMode,
    AnimeSubtitle,
    VisualNovel,
    StreamerMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationBehaviorSettings {
    pub literal_natural_slider: f32, // 0.0 (Literal) to 1.0 (Natural), default 0.5
    pub preserve_formatting: bool,
    pub preserve_line_breaks: bool,
    pub preserve_punctuation: bool,
    pub preserve_honorifics: bool,
    pub preserve_emojis: bool,
    pub contextual_translation: bool,
    pub creativity: f32, // 0.0 to 1.0, default 0.2
    pub profanity_filter: bool,
    pub tone: TranslationTone,
    pub preset: TranslationStylePreset,
    pub custom_prompts: CustomPromptSettings,
}

impl Default for TranslationBehaviorSettings {
    fn default() -> Self {
        Self {
            literal_natural_slider: 0.5,
            preserve_formatting: true,
            preserve_line_breaks: true,
            preserve_punctuation: true,
            preserve_honorifics: false,
            preserve_emojis: true,
            contextual_translation: true,
            creativity: 0.2,
            profanity_filter: false,
            tone: TranslationTone::Auto,
            preset: TranslationStylePreset::Standard,
            custom_prompts: CustomPromptSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RealtimeStabilitySettings {
    pub stability_threshold_frames: u32, // Wait N identical text frames before translating (typewriter debounce)
    pub subtitle_persistence_ms: u64,    // Keep text on screen for N ms after source disappears
    pub context_window_size: u32,        // N previous segment translations passed as context
    pub fade_smoothing: bool,            // Apply crossfade/smoothing animations
}

impl Default for RealtimeStabilitySettings {
    fn default() -> Self {
        Self {
            stability_threshold_frames: 1, // Default 1 (translates immediately on first full-text grab, or 2 for games)
            subtitle_persistence_ms: 2500, // Hold subtitles for 2.5 seconds
            context_window_size: 2,        // 2 prior segments
            fade_smoothing: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformancePreset {
    Eco,
    Balanced,
    Performance,
    Ultra,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuBackend {
    Auto,
    Cpu,
    Cuda,
    DirectMl,
    TensorRt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSettings {
    pub preset: PerformancePreset,
    pub worker_threads: usize,
    pub gpu_backend: GpuBackend,
    pub parallel_ocr: bool,
    pub enable_batching: bool,
    pub memory_cleanup_interval_secs: u64,
    pub max_cache_entries: usize,
    pub vram_limit_mb: u32, // 0 = unlimited
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            preset: PerformancePreset::Balanced,
            worker_threads: 4,
            gpu_backend: GpuBackend::Auto,
            parallel_ocr: true,
            enable_batching: true,
            memory_cleanup_interval_secs: 300, // 5 minutes
            max_cache_entries: 5000,
            vram_limit_mb: 0,
        }
    }
}

impl PerformanceSettings {
    pub fn apply_preset(&mut self, preset: PerformancePreset) {
        self.preset = preset;
        match preset {
            PerformancePreset::Eco => {
                self.worker_threads = 2;
                self.parallel_ocr = false;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 60;
                self.max_cache_entries = 1000;
                self.vram_limit_mb = 1024;
            }
            PerformancePreset::Balanced => {
                self.worker_threads = 4;
                self.parallel_ocr = true;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 300;
                self.max_cache_entries = 5000;
                self.vram_limit_mb = 0;
            }
            PerformancePreset::Performance => {
                self.worker_threads = 8;
                self.parallel_ocr = true;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 600;
                self.max_cache_entries = 20000;
                self.vram_limit_mb = 0;
            }
            PerformancePreset::Ultra => {
                self.worker_threads = 16;
                self.parallel_ocr = true;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 1200;
                self.max_cache_entries = 50000;
                self.vram_limit_mb = 0;
            }
            PerformancePreset::Custom => {
                // Keep values as-is to allow manual user fine-tuning
            }
        }
    }

    pub fn enforce_preset_locks(&mut self) {
        // Automatically restore locked preset values if preset is not Custom
        let current_preset = self.preset;
        if current_preset != PerformancePreset::Custom {
            self.apply_preset(current_preset);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomPromptSettings {
    pub enabled: bool,
    pub system_prompt: String,
    pub single_line_user_prompt: String,
    pub multi_line_user_prompt: String,
}

impl Default for CustomPromptSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            system_prompt: "You are a professional manga/game translator. Translate the text to {target_lang}. Maintain professional grammar, correct capitalization, and proper punctuation. Output ONLY the translated text, no explanations, no quotes.".to_string(),
            single_line_user_prompt: "Translate from {source_lang} to {target_lang}:\n\n{text}".to_string(),
            multi_line_user_prompt: "Translate these {count} segments from {source_lang} to {target_lang}:\n\n{numbered_lines}".to_string(),
        }
    }
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
