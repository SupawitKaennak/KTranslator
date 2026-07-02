use serde::{Deserialize, Serialize};

use crate::core::{
    ports::OcrTextLine,
    types::{LanguageTag, Rect, RegionId},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionSlot {
    pub id: RegionId,
    pub display_id: u32,
    pub enabled: bool,
    pub show_frame: bool,
    pub rect: Option<Rect>,
    pub source_lang: Option<LanguageTag>, // None = auto
    pub target_lang: LanguageTag,
    #[serde(skip)]
    pub stable_hash: u64,
    #[serde(skip)]
    pub stable_since_ms: u64,
    pub refresh_ms: u64,
    pub last_ocr_text: String,
    pub last_translation: String,
    /// Per-line OCR results with bounding boxes (image-pixel coordinates).
    /// Used by the positional overlay to place translated text at the right position.
    #[serde(skip)]
    pub last_ocr_lines: Vec<OcrTextLine>,
    /// Translation split by newline, matched index-for-index with last_ocr_lines.
    #[serde(skip)]
    pub last_trans_lines: Vec<String>,
    #[serde(skip)]
    pub last_yolo_bubbles: Vec<OcrTextLine>,
    pub pending_text: String,
    pub next_tick_at_ms: u64,
    pub translate_backoff_ms: u64,
    pub translate_next_try_at_ms: u64,
    pub popup_open: bool,
    pub overlay_mode: bool,
    #[serde(skip)]
    pub language_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppModel {
    pub running: bool,
    pub slots: Vec<RegionSlot>,
    #[serde(skip)]
    pub download_progress: crate::core::types::DownloadProgress,
}

impl Default for RegionSlot {
    fn default() -> Self {
        Self {
            id: RegionId(0),
            display_id: 0,
            enabled: false,
            show_frame: false,
            rect: None,
            source_lang: Some(LanguageTag("en".to_string())),
            target_lang: LanguageTag("en".to_string()),
            stable_hash: 0,
            stable_since_ms: 0,
            refresh_ms: 5000,
            last_ocr_text: String::new(),
            last_translation: String::new(),
            last_ocr_lines: Vec::new(),
            last_trans_lines: Vec::new(),
            last_yolo_bubbles: Vec::new(),
            pending_text: String::new(),
            next_tick_at_ms: 0,
            translate_backoff_ms: 0,
            translate_next_try_at_ms: 0,
            popup_open: false,
            overlay_mode: false,
            language_version: 0,
        }
    }
}

impl AppModel {
    pub fn new_default() -> Self {
        Self {
            running: false,
            slots: vec![RegionSlot::default()],
            download_progress: crate::core::types::DownloadProgress::default(),
        }
    }
    pub fn add_slot(&mut self) -> usize {
        let new_idx = self.slots.len();
        self.slots.push(RegionSlot {
            id: RegionId(new_idx),
            ..RegionSlot::default()
        });
        new_idx
    }
}

// ---------------------------------------------------------------------------
// Runtime state for each translation slot
// ---------------------------------------------------------------------------

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::AtomicIsize;
use std::sync::Arc;

pub struct SlotRuntimeState {
    /// True if the slot has a background task running (capture or API)
    pub busy: bool,
    /// Timestamp (ms) when `busy` was last set to true — used for timeout recovery
    pub busy_since_ms: u64,
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
    /// ID of the active error shown in the UI for this slot
    pub active_error_id: Option<usize>,
    /// The visual rectangle of the frame, updated continuously during dragging
    pub visual_rect: Arc<Mutex<Option<Rect>>>,
}

impl SlotRuntimeState {
    pub fn new() -> Self {
        Self {
            busy: false,
            busy_since_ms: 0,
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
            active_error_id: None,
            visual_rect: Arc::new(Mutex::new(None)),
        }
    }
}
