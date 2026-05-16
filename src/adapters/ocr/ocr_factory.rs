use std::sync::Arc;
use crate::core::ports::OcrEngine;
use crate::infrastructure::settings::{OcrEngineType, OcrMode, Settings};
use super::{
    windows_ocr::WindowsOcr,
    builtin_paddle_ocr::BuiltinPaddleOcr,
    manga109_yolo_ocr::OnnxMangaRecognizer,
};

/// Factory responsible for centralizing the creation and fallback strategies
/// of the various OCR recognition adapters.
pub struct OcrAdapterFactory;

impl OcrAdapterFactory {
    /// Determines the active OCR engine type based on current mode settings.
    pub fn get_active_engine_type(settings: &Settings) -> OcrEngineType {
        match settings.ocr_mode {
            OcrMode::Game => settings.game_ocr_engine,
            OcrMode::Manga => settings.manga_ocr_engine,
            OcrMode::Document => settings.document_ocr_engine,
        }
    }

    /// Creates an instance of the configured OCR engine.
    /// Automatically falls back to Windows OCR if initialization fails.
    pub fn create_engine(settings: &Settings) -> (Arc<dyn OcrEngine>, Option<String>) {
        let engine_type = Self::get_active_engine_type(settings);
        match engine_type {
            OcrEngineType::BuiltinPaddle => {
                match std::panic::catch_unwind(|| {
                    BuiltinPaddleOcr::new("models/ppocr".to_string())
                }) {
                    Ok(engine) => (Arc::new(engine), None),
                    Err(_) => {
                        let err_msg = "Built-in PaddleOCR init failed, falling back to Windows OCR".to_string();
                        tracing::error!("{err_msg}");
                        (Arc::new(WindowsOcr::new()), Some(err_msg))
                    }
                }
            }
            OcrEngineType::MangaOCR => {
                (Arc::new(OnnxMangaRecognizer::new("models/manga-ocr", settings.perf.gpu_backend)), None)
            }
            OcrEngineType::Windows => {
                (Arc::new(WindowsOcr::new()), None)
            }
        }
    }
}
