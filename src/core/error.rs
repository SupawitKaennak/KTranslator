use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[allow(dead_code)]
pub enum KError {
    #[error("Capture failed: {0}")]
    Capture(String),

    #[error("OCR engine failed: {0}")]
    Ocr(String),

    #[error("Translation failed: {0}")]
    Translation(String),

    #[error("Asset error: {0}")]
    Asset(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
