use serde::{Deserialize, Serialize};

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
            preserve_formatting: false,
            preserve_line_breaks: false,
            preserve_punctuation: false,
            preserve_honorifics: false,
            preserve_emojis: false,
            contextual_translation: false,
            creativity: 0.2,
            profanity_filter: false,
            tone: TranslationTone::Auto,
            preset: TranslationStylePreset::Standard,
            custom_prompts: CustomPromptSettings::default(),
        }
    }
}
