use crate::core::ports::OcrTextLine;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicIsize;
use parking_lot::Mutex;

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
    Unchanged {
        slot_idx: usize,
    },
    /// The screen is changing. Update the stable hash tracker and skip API.
    HashChanged {
        slot_idx: usize,
        new_hash: u64,
    },
    /// The screen is stable but we are waiting for the debounce duration.
    WaitingDebounce {
        slot_idx: usize,
    },
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
    StatusUpdate {
        slot_idx: usize,
        status: String,
    },
    /// An error occurred during OCR / Translation.
    Error {
        slot_idx: usize,
        language_version: u32,
        err: String,
    },
}

// ---------------------------------------------------------------------------
// Runtime state for each translation slot
// ---------------------------------------------------------------------------

pub struct SlotRuntimeState {
    /// True if the slot has a background task running (capture or API)
    pub busy: bool,
    /// True if the slot is currently waiting for an AI response
    pub processing: bool,
    /// Human-readable status shown in the UI
    pub status: String,
    /// Hash of the last captured frame to detect changes
    pub last_hash: u64,
    /// Native HWND of the overlay window for Win32 transparency
    pub overlay_hwnd: Arc<AtomicIsize>,
    /// Native HWND of the live frame border window
    pub frame_live_hwnd: Arc<AtomicIsize>,
    /// Track language changes to invalidate caches
    pub last_langs: (Option<String>, String),
    pub last_ppocr_model: Option<crate::infrastructure::settings::PpocrModelSuite>,
    /// Time when the screen first became unstable. 
    /// Used to force a translation if it never settles (e.g. in games).
    pub first_unstable_at: u64,
    /// Whether the last capture attempt resulted in an instruction to hide the overlay
    pub last_capture_hide: Arc<Mutex<Option<bool>>>,
    /// Pristine copy of the last captured frame buffer stored locally in RAM for real-time Preview rendering
    pub last_frame: Arc<Mutex<Option<crate::core::ports::FrameRgba>>>,
    
    // ── Realtime Stability Trackers ──
    pub last_stable_ocr_text: String,
    pub identical_frames_count: u32,
    pub last_seen_text_at_ms: u64,
    pub persistent_translation: Arc<Mutex<Option<String>>>,
    pub persistent_ocr_lines: Arc<Mutex<Vec<OcrTextLine>>>,
    pub persistent_trans_lines: Arc<Mutex<Vec<String>>>,
    /// Number of consecutive errors for exponential backoff calculation
    pub error_streak: u32,
    /// Recent translated segments for low-token contextual translation
    pub recent_translations: VecDeque<String>,
    /// Overlay fade alpha (0.0–1.0) when fade smoothing is enabled
    pub overlay_fade_alpha: f32,
    pub overlay_fade_target: f32,
    pub last_overlay_fade_ms: u64,
}

impl SlotRuntimeState {
    pub fn new() -> Self {
        Self {
            busy: false,
            processing: false,
            status: "Idle".to_string(),
            last_hash: 0,
            overlay_hwnd: Arc::new(AtomicIsize::new(0)),
            frame_live_hwnd: Arc::new(AtomicIsize::new(0)),
            last_langs: (None, String::new()),
            last_ppocr_model: None,
            first_unstable_at: 0,
            last_capture_hide: Arc::new(Mutex::new(None)),
            last_frame: Arc::new(Mutex::new(None)),
            last_stable_ocr_text: String::new(),
            identical_frames_count: 0,
            last_seen_text_at_ms: 0,
            persistent_translation: Arc::new(Mutex::new(None)),
            persistent_ocr_lines: Arc::new(Mutex::new(Vec::new())),
            persistent_trans_lines: Arc::new(Mutex::new(Vec::new())),
            error_streak: 0,
            recent_translations: VecDeque::new(),
            overlay_fade_alpha: 1.0,
            overlay_fade_target: 1.0,
            last_overlay_fade_ms: 0,
        }
    }
}

/// Smart hash converts RGBA to thresholded grayscale before hashing.
/// This prevents minor lighting/background particle changes from triggering text translation.
/// Uses FNV-1a internally (see `crate::core::utils::fnv_hash_bytes` for the plain variant).
pub fn smart_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;
    let mut h: u64 = FNV_OFFSET;
    
    // Dynamic step: smaller regions need finer sampling to detect
    // single-character text changes; large regions can skip more.
    let pixel_count = data.len() / 4;
    let step: usize = if pixel_count < 50_000 { 8 } else { 32 };
    let mut i = 0;
    while i + 2 < data.len() {
        // Quantize each channel to 3 bits (8 levels) to ignore compression noise and dithering
        let r = data[i] >> 5;
        let g = data[i+1] >> 5;
        let b = data[i+2] >> 5;
        let combined = (r << 6) | (g << 3) | b;
        
        h ^= combined as u64;
        h = h.wrapping_mul(FNV_PRIME);
        
        i += step;
    }
    h
}
