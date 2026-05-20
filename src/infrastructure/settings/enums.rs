use serde::{Deserialize, Serialize};

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
