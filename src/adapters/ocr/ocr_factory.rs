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
        let (base_engine, err) = match engine_type {
            OcrEngineType::BuiltinPaddle => {
                match std::panic::catch_unwind(|| {
                    BuiltinPaddleOcr::new("models/ppocr".to_string())
                }) {
                    Ok(engine) => (Arc::new(engine) as Arc<dyn OcrEngine>, None),
                    Err(_) => {
                        let err_msg = "Built-in PaddleOCR init failed, falling back to Windows OCR".to_string();
                        tracing::error!("{err_msg}");
                        (Arc::new(WindowsOcr::new()) as Arc<dyn OcrEngine>, Some(err_msg))
                    }
                }
            }
            OcrEngineType::MangaOCR => {
                (Arc::new(OnnxMangaRecognizer::new("models/manga-ocr", settings.perf.gpu_backend)) as Arc<dyn OcrEngine>, None)
            }
            OcrEngineType::Windows => {
                (Arc::new(WindowsOcr::new()) as Arc<dyn OcrEngine>, None)
            }
        };

        if settings.ocr_use_yolo && engine_type != OcrEngineType::MangaOCR {
            let yolo_model_path = "models/manga-ocr/manga109_yolo_s.onnx".to_string();
            let wrapped = Arc::new(super::yolo_layout_wrapper::YoloLayoutOcrWrapper::new(
                base_engine,
                yolo_model_path,
                settings.perf.gpu_backend,
            ));
            (wrapped, err)
        } else {
            (base_engine, err)
        }
    }
}
