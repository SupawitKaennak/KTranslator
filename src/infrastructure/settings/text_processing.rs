use super::enums::{ChineseConversionMode, ThaiSegmentationMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLayoutSettings {
    pub merge_x_gap: f32, // Default 0.8
    pub merge_y_gap: f32, // Default 0.6
    pub inline_x_gap: f32, // Default 0.35
}

impl Default for TextLayoutSettings {
    fn default() -> Self {
        Self {
            merge_x_gap: 0.8,
            merge_y_gap: 0.6,
            inline_x_gap: 0.35,
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
    pub enable_wordninja: bool,
    pub enable_ocr_merge: bool,
    pub enable_spell_correction: bool,

    pub jp_merge_vertical: bool,
    pub jp_kana_normalization: bool,
    pub jp_remove_furigana: bool,

    pub cn_conversion: ChineseConversionMode,

    pub th_segmentation: ThaiSegmentationMode,
    pub th_zero_width_cleanup: bool,

    pub ar_rtl_correction: bool,

    pub layout: TextLayoutSettings,
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
            enable_ocr_merge: true,
            enable_spell_correction: false,

            jp_merge_vertical: true,
            jp_kana_normalization: true,
            jp_remove_furigana: true,

            cn_conversion: ChineseConversionMode::None,

            th_segmentation: ThaiSegmentationMode::Standard,
            th_zero_width_cleanup: true,

            ar_rtl_correction: true,

            layout: TextLayoutSettings::default(),
        }
    }
}
