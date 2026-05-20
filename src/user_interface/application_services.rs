use crate::core::ports::{FrameSource, OcrEngine, Translator};
use crate::core::types::{DownloadProgress, TextTranslationCache, TranslationCache};
use crate::infrastructure::platform::PlatformServices;
use crate::infrastructure::settings::OcrEngineType;
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};

pub struct PipelineServices {
    pub capture: Arc<dyn FrameSource>,
    pub platform: Arc<dyn PlatformServices>,
    pub ocr_engine: Arc<dyn OcrEngine>,
    pub translator: Option<Arc<dyn Translator + Send + Sync>>,
}

pub struct AppCaches {
    pub translation: Arc<Mutex<TranslationCache>>,
    pub text_translation: Arc<Mutex<TextTranslationCache>>,
    pub last_cleanup_time: Arc<Mutex<u64>>,
}

pub struct DownloadManager {
    pub trigger_tx: mpsc::Sender<OcrEngineType>,
    pub trigger_rx: mpsc::Receiver<OcrEngineType>,
    pub progress_rx: tokio::sync::mpsc::Receiver<DownloadProgress>,
    pub progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
}
