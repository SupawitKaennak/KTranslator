use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranslationProvider {
    Gemini,
    Groq,
    Ollama,
    CustomOpenAI,
    Google,
    Claude,
    DeepSeek,
    DeepL,
    LmStudio,
    AzureOpenAI,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrEngineType {
    Windows,
    /// Legacy alias — old configs with "Paddle" will deserialize to BuiltinPaddle
    #[serde(alias = "Paddle")]
    BuiltinPaddle,
    MangaOCR,
    BubbleYOLO,
    CraftDetector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PpocrModelSuite {
    #[default]
    CnEnMobile,
    JapaneseMobile,
    KoreanMobile,
    ThaiMobile,
    LatinMobile,
    CyrillicMobile,
}

impl PpocrModelSuite {
    pub fn folder_name(&self) -> &'static str {
        match self {
            PpocrModelSuite::CnEnMobile => "cn_en_mobile",
            PpocrModelSuite::JapaneseMobile => "mobile_japanese",
            PpocrModelSuite::KoreanMobile => "mobile_korean",
            PpocrModelSuite::ThaiMobile => "mobile_thai",
            PpocrModelSuite::LatinMobile => "mobile_latin",
            PpocrModelSuite::CyrillicMobile => "mobile_cyrillic",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDetectorMode {
    /// No text detector — OCR runs on full frame or user-defined region
    None,
    /// YOLO Bubble Detector — optimized for manga/comic speech bubbles
    YoloBubble,
    /// CRAFT Text Detector — precise character-level text region detection
    CraftRegion,
    /// YOLO + Full Page OCR Hybrid — groups full page OCR results using YOLO bubbles
    YoloFullPageHybrid,
}
