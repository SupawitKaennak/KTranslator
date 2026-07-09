use crate::core::types::{LanguageTag, Rect};

#[derive(Debug, Clone)]
pub struct FrameRgba {
    pub width: u32,
    pub height: u32,
    pub data: std::sync::Arc<Vec<u8>>, // RGBA8
}

/// One line of OCR-recognised text together with its bounding box in
/// image-pixel coordinates (origin = top-left of the captured frame).
/// Used by the positional overlay to render translated text at the same
/// position as the original source text.
#[derive(Debug, Clone, Default)]
pub struct OcrTextLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    #[allow(dead_code)] // kept for future text-wrapping / overflow detection
    pub w: f32,
    pub h: f32,
    pub bubble_idx: Option<usize>, // Track parent YOLO/CRAFT bubble to prevent background merging
}

/// A block of OCR text grouped together (e.g. a paragraph or speech bubble).
#[derive(Debug, Clone, Default)]
pub struct OcrTextBlock {
    pub lines: Vec<OcrTextLine>,
    pub source_text: String,
}

pub trait FrameSource: Send + Sync {
    fn capture_rect(&self, rect: Rect, display_id: u32) -> anyhow::Result<FrameRgba>;
}

#[allow(dead_code)] // trait contract; used by GeminiOcr and may be called directly in future
pub trait OcrEngine: Send + Sync {
    fn recognize(
        &self,
        frame: FrameRgba,
        lang_hint: Option<&LanguageTag>,
    ) -> anyhow::Result<String>;
    fn recognize_lines(
        &self,
        frame: FrameRgba,
        lang_hint: Option<&LanguageTag>,
    ) -> anyhow::Result<Vec<OcrTextLine>>;
}

pub trait Translator: Send + Sync {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
        context_hint: Option<&str>,
    ) -> anyhow::Result<String>;

    /// Optional: Post-process OCR text to fix character recognition errors based on language context.
    /// Default implementation simply returns the input text unmodified.
    fn correct_text(&self, text: &str, _lang_hint: Option<&LanguageTag>) -> anyhow::Result<String> {
        Ok(text.to_string())
    }

    /// Optional: Translate directly from an image frame (Vision mode)
    #[allow(dead_code)]
    fn translate_frame(
        &self,
        _frame: &FrameRgba,
        _source: Option<&LanguageTag>,
        _target: &LanguageTag,
    ) -> anyhow::Result<String> {
        Err(anyhow::anyhow!(
            "Vision translation not supported by this provider".to_string(),
        ))
    }
}
