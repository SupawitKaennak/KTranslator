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

    /// Returns (rec_url, dict_url) for this model suite.
    /// The detection model URL is shared across all suites and lives in PPOCR_MOBILE_MODELS[0].
    pub fn get_urls(&self) -> (&'static str, &'static str) {
        match self {
            PpocrModelSuite::CnEnMobile => (
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_rec.onnx",
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocrv5_dict.txt",
            ),
            PpocrModelSuite::JapaneseMobile => (
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/japan_pp-ocrv3_mobile_rec.onnx",
                "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/dict/japan_dict.txt",
            ),
            PpocrModelSuite::KoreanMobile => (
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/korean_pp-ocrv5_mobile_rec.onnx",
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocrv5_korean_dict.txt",
            ),
            PpocrModelSuite::ThaiMobile => (
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/th_pp-ocrv5_mobile_rec.onnx",
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocrv5_th_dict.txt",
            ),
            PpocrModelSuite::LatinMobile => (
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/latin_pp-ocrv5_mobile_rec.onnx",
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocrv5_latin_dict.txt",
            ),
            PpocrModelSuite::CyrillicMobile => (
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/cyrillic_pp-ocrv5_mobile_rec.onnx",
                "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocrv5_cyrillic_dict.txt",
            ),
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
