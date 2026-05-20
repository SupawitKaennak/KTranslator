use crate::core::ports::OcrTextLine;

/// Results from background worker threads sent back to the main UI loop.
pub enum BgResult {
    /// Combined OCR + Translation completed successfully.
    Done {
        slot_idx: usize,
        language_version: u32,
        ocr_text: String,
        translated: String,
        frame_hash: u64,
        /// Per-line OCR bounding boxes for positional overlay rendering.
        ocr_lines: Vec<OcrTextLine>,
        /// Translation split by newline, matching ocr_lines length.
        trans_lines: Vec<String>,
        yolo_bubbles: Vec<OcrTextLine>,
    },
    /// The captured frame is identical to the previous one — skip API call.
    Unchanged { slot_idx: usize },
    /// The screen is changing. Update the stable hash tracker and skip API.
    HashChanged { slot_idx: usize, new_hash: u64 },
    /// The screen is stable but we are waiting for the debounce duration.
    WaitingDebounce { slot_idx: usize },
    /// The frame matches a cached translation.
    CacheHit {
        slot_idx: usize,
        language_version: u32,
        ocr_text: String,
        translated: String,
        frame_hash: u64,
        ocr_lines: Vec<OcrTextLine>,
        trans_lines: Vec<String>,
        yolo_bubbles: Vec<OcrTextLine>,
    },
    /// Direct status update for the UI spinner/label
    StatusUpdate { slot_idx: usize, status: String },
    /// An error occurred during OCR / Translation.
    Error {
        slot_idx: usize,
        language_version: u32,
        err: String,
    },
}
