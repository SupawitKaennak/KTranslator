use crate::core::{
    pipeline_execution_result::BgResult, region_slot_state::AppModel,
    region_slot_state::SlotRuntimeState, types::TranslationCache,
};
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};

/// Tick interval for animation frames (e.g. scrolling text).
const TICK_ANIMATION_MS: u64 = 50;
/// Tick interval for hash-change follow-up.
const TICK_HASH_FOLLOWUP_MS: u64 = 30;
/// Tick interval for debounce polling (~60fps).
const TICK_DEBOUNCE_POLL_MS: u64 = 16;
/// Rate limit exponential backoff base (seconds).
const RATE_LIMIT_BASE_SECS: u64 = 30;
/// Bad request retry delay (milliseconds).
const BAD_REQUEST_RETRY_MS: u64 = 10_000;
/// Server/network error retry delay (milliseconds).
const SERVER_ERROR_RETRY_MS: u64 = 5_000;
/// Default error retry delay (milliseconds).
const DEFAULT_ERROR_RETRY_MS: u64 = 3_000;

pub struct ResultDispatcher;

impl ResultDispatcher {
    pub fn process_results(
        bg_rx: &mpsc::Receiver<BgResult>,
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        err_handler: &crate::core::usecases::error_handler::ErrorHandler,
        translation_cache: &Arc<Mutex<TranslationCache>>,
        settings: &crate::infrastructure::settings::Settings,
    ) {
        while let Ok(result) = bg_rx.try_recv() {
            match result {
                BgResult::Done {
                    slot_idx,
                    language_version,
                    ocr_text,
                    translated,
                    frame_hash,
                    ocr_lines,
                    trans_lines,
                    yolo_bubbles,
                } => Self::handle_done(
                    model_arc,
                    slots_runtime,
                    err_handler,
                    translation_cache,
                    settings,
                    slot_idx,
                    language_version,
                    ocr_text,
                    translated,
                    frame_hash,
                    ocr_lines,
                    trans_lines,
                    yolo_bubbles,
                ),
                BgResult::Unchanged { slot_idx } => {
                    Self::handle_unchanged(model_arc, slots_runtime, slot_idx);
                }
                BgResult::HashChanged { slot_idx, new_hash } => {
                    Self::handle_hash_changed(model_arc, slots_runtime, slot_idx, new_hash);
                }
                BgResult::WaitingDebounce { slot_idx } => {
                    Self::handle_waiting_debounce(model_arc, slots_runtime, slot_idx);
                }
                BgResult::CacheHit {
                    slot_idx,
                    language_version,
                    ocr_text,
                    translated,
                    frame_hash,
                    ocr_lines,
                    trans_lines,
                    yolo_bubbles,
                } => Self::handle_cache_hit(
                    model_arc,
                    slots_runtime,
                    err_handler,
                    slot_idx,
                    language_version,
                    ocr_text,
                    translated,
                    frame_hash,
                    ocr_lines,
                    trans_lines,
                    yolo_bubbles,
                ),
                BgResult::StatusUpdate { slot_idx, status } => {
                    Self::handle_status_update(slots_runtime, slot_idx, status);
                }
                BgResult::Error {
                    slot_idx,
                    language_version,
                    err,
                } => Self::handle_error(
                    model_arc,
                    slots_runtime,
                    err_handler,
                    slot_idx,
                    language_version,
                    err,
                ),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_done(
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        err_handler: &crate::core::usecases::error_handler::ErrorHandler,
        translation_cache: &Arc<Mutex<TranslationCache>>,
        settings: &crate::infrastructure::settings::Settings,
        slot_idx: usize,
        language_version: u32,
        ocr_text: String,
        translated: String,
        frame_hash: u64,
        ocr_lines: Vec<crate::core::ports::OcrTextLine>,
        trans_lines: Vec<String>,
        yolo_bubbles: Vec<crate::core::ports::OcrTextLine>,
    ) {
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            let mut model = model_arc.lock();
            let slot = match model.slots.get_mut(slot_idx) {
                Some(s) => s,
                None => return,
            };

            if language_version != slot.language_version {
                runtime.busy = false;
                runtime.processing = false;
                runtime.first_unstable_at = 0;
                slot.next_tick_at_ms = 0;
                return;
            }

            runtime.busy = false;
            runtime.processing = false;
            runtime.first_unstable_at = 0;
            runtime.status = "Ready".to_string();
            runtime.last_hash = frame_hash;

            let now = crate::core::utils::now_ms();
            slot.next_tick_at_ms = now.saturating_add(slot.refresh_ms.max(500));

            let new_ocr = ocr_text.trim();
            let old_ocr = slot.last_ocr_text.trim();
            let realtime = &settings.realtime;

            if new_ocr.is_empty() {
                // ── Persistence Check ──
                if now < runtime.last_seen_text_at_ms + realtime.subtitle_persistence_ms {
                    let pers_trans = runtime.persistent_translation.lock().clone();
                    let pers_ocr = runtime.persistent_ocr_lines.lock().clone();
                    let pers_trans_lines = runtime.persistent_trans_lines.lock().clone();

                    if let Some(pt) = pers_trans {
                        slot.last_translation = pt;
                        slot.last_ocr_lines = pers_ocr;
                        slot.last_trans_lines = pers_trans_lines;
                    } else {
                        slot.last_ocr_text.clear();
                        slot.last_translation.clear();
                        slot.last_ocr_lines.clear();
                        slot.last_trans_lines.clear();
                        slot.last_yolo_bubbles.clear();
                    }
                } else {
                    slot.last_ocr_text.clear();
                    slot.last_translation.clear();
                    slot.last_ocr_lines.clear();
                    slot.last_trans_lines.clear();
                    slot.last_yolo_bubbles.clear();
                    *runtime.persistent_translation.lock() = None;
                    runtime.persistent_ocr_lines.lock().clear();
                    runtime.persistent_trans_lines.lock().clear();
                }
            } else {
                // ── Debounce Check (Typewriter effect) ──
                if new_ocr == runtime.last_stable_ocr_text.trim() {
                    runtime.identical_frames_count =
                        runtime.identical_frames_count.saturating_add(1);
                } else {
                    runtime.last_stable_ocr_text = new_ocr.to_string();
                    runtime.identical_frames_count = 1;
                }

                if runtime.identical_frames_count >= realtime.stability_threshold_frames {
                    runtime.last_seen_text_at_ms = now;

                    if new_ocr != old_ocr {
                        slot.last_ocr_text = ocr_text.clone();
                        slot.last_translation = translated.clone();
                        slot.last_ocr_lines = ocr_lines.clone();
                        slot.last_trans_lines = trans_lines.clone();
                        slot.last_yolo_bubbles = yolo_bubbles.clone();

                        if !translated.trim().is_empty() {
                            let cap = settings.realtime.context_window_size.max(1) as usize;
                            runtime
                                .recent_translations
                                .push_back(translated.trim().to_string());
                            while runtime.recent_translations.len() > cap {
                                runtime.recent_translations.pop_front();
                            }
                        }

                        if settings.realtime.fade_smoothing {
                            runtime.overlay_fade_target = 1.0;
                            runtime.overlay_fade_alpha = 0.35;
                            runtime.last_overlay_fade_ms = now;
                        }

                        if frame_hash != 0 {
                            let cache_key = (
                                frame_hash,
                                slot.source_lang.as_ref().map(|l| l.0.clone()),
                                slot.target_lang.0.clone(),
                            );
                            translation_cache
                                .lock()
                                .insert(cache_key, (ocr_text, translated.clone()));
                        }
                    } else if !translated.trim().is_empty() {
                        slot.last_trans_lines = trans_lines.clone();
                        slot.last_ocr_lines = ocr_lines.clone();
                        slot.last_yolo_bubbles = yolo_bubbles.clone();
                        slot.last_translation = translated.clone();
                    }

                    // Save to persistence store
                    if !slot.last_translation.trim().is_empty() {
                        *runtime.persistent_translation.lock() =
                            Some(slot.last_translation.clone());
                        *runtime.persistent_ocr_lines.lock() = slot.last_ocr_lines.clone();
                        *runtime.persistent_trans_lines.lock() = slot.last_trans_lines.clone();
                    }
                } else {
                    // Still debouncing typewriter animation
                    let pers_trans = runtime.persistent_translation.lock().clone();
                    let pers_ocr = runtime.persistent_ocr_lines.lock().clone();
                    let pers_trans_lines = runtime.persistent_trans_lines.lock().clone();

                    if let Some(pt) = pers_trans {
                        slot.last_translation = pt;
                        slot.last_ocr_lines = pers_ocr;
                        slot.last_trans_lines = pers_trans_lines;
                    }
                    slot.next_tick_at_ms = now.saturating_add(TICK_ANIMATION_MS);
                }
            }
        }
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            runtime.error_streak = 0;
            if let Some(err_id) = runtime.active_error_id.take() {
                err_handler.dismiss(err_id);
            }
        }
    }

    fn handle_unchanged(
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        slot_idx: usize,
    ) {
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            runtime.busy = false;
            runtime.status = "Ready".to_string();
            runtime.first_unstable_at = 0;
        }
        let now = crate::core::utils::now_ms();
        let mut model = model_arc.lock();
        if let Some(slot) = model.slots.get_mut(slot_idx) {
            slot.next_tick_at_ms = now.saturating_add(slot.refresh_ms.max(100));
        }
    }

    fn handle_hash_changed(
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        slot_idx: usize,
        new_hash: u64,
    ) {
        let mut model = model_arc.lock();
        let now = crate::core::utils::now_ms();
        if let Some(slot) = model.slots.get_mut(slot_idx) {
            slot.stable_hash = new_hash;
            slot.stable_since_ms = now;
            slot.next_tick_at_ms = now.saturating_add(TICK_HASH_FOLLOWUP_MS);
        }
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            runtime.busy = false;
            if runtime.first_unstable_at == 0 {
                runtime.first_unstable_at = now;
            }
        }
    }

    fn handle_waiting_debounce(
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        slot_idx: usize,
    ) {
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            runtime.busy = false;
        }
        let mut model = model_arc.lock();
        if let Some(slot) = model.slots.get_mut(slot_idx) {
            slot.next_tick_at_ms = crate::core::utils::now_ms() + TICK_DEBOUNCE_POLL_MS;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_cache_hit(
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        err_handler: &crate::core::usecases::error_handler::ErrorHandler,
        slot_idx: usize,
        language_version: u32,
        ocr_text: String,
        translated: String,
        frame_hash: u64,
        ocr_lines: Vec<crate::core::ports::OcrTextLine>,
        trans_lines: Vec<String>,
        yolo_bubbles: Vec<crate::core::ports::OcrTextLine>,
    ) {
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            let mut model = model_arc.lock();
            let slot = match model.slots.get_mut(slot_idx) {
                Some(s) => s,
                None => return,
            };

            if language_version != slot.language_version {
                runtime.busy = false;
                slot.next_tick_at_ms = 0;
                return;
            }

            runtime.busy = false;
            runtime.processing = false;
            runtime.status = "Ready (Cached)".to_string();
            runtime.error_streak = 0;
            if let Some(err_id) = runtime.active_error_id.take() {
                err_handler.dismiss(err_id);
            }
            runtime.first_unstable_at = 0;
            runtime.last_hash = frame_hash;

            slot.last_ocr_text = ocr_text;
            slot.last_translation = translated.clone();
            slot.last_ocr_lines = ocr_lines;
            slot.last_yolo_bubbles = yolo_bubbles;
            slot.last_trans_lines = trans_lines;

            slot.next_tick_at_ms = crate::core::utils::now_ms() + slot.refresh_ms.max(200);
        }
    }

    fn handle_status_update(
        slots_runtime: &mut [SlotRuntimeState],
        slot_idx: usize,
        status: String,
    ) {
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            runtime.status = status;
            if runtime.status.contains("Scanning") || runtime.status.contains("AI") {
                runtime.processing = true;
            }
        }
    }

    fn handle_error(
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        err_handler: &crate::core::usecases::error_handler::ErrorHandler,
        slot_idx: usize,
        language_version: u32,
        err: String,
    ) {
        if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
            let mut model = model_arc.lock();
            let slot = match model.slots.get_mut(slot_idx) {
                Some(s) => s,
                None => return,
            };

            runtime.busy = false;
            runtime.processing = false;
            runtime.status = "Error".to_string();
            runtime.error_streak = runtime.error_streak.saturating_add(1);

            let is_rate_limit =
                err.contains("quota") || err.contains("429") || err.contains("Too Many Requests");
            let is_bad_request =
                err.contains("400") || err.contains("parse") || err.contains("invalid");
            let is_server_err = err.contains("500")
                || err.contains("502")
                || err.contains("503")
                || err.contains("timeout");

            let (retry_delay_ms, friendly) = if is_rate_limit {
                let multiplier = 2u64.pow(runtime.error_streak.saturating_sub(1).min(5));
                let secs = RATE_LIMIT_BASE_SECS * multiplier;
                (
                    secs * 1000,
                    format!(
                        "Region {}: API rate limit hit — retrying in {secs}s",
                        slot_idx + 1
                    ),
                )
            } else if is_bad_request {
                (
                    BAD_REQUEST_RETRY_MS,
                    format!(
                        "Region {}: Data format error — retrying in {}s",
                        slot_idx + 1,
                        BAD_REQUEST_RETRY_MS / 1000
                    ),
                )
            } else if is_server_err {
                (
                    SERVER_ERROR_RETRY_MS,
                    format!(
                        "Region {}: Server/Network error — retrying in {}s",
                        slot_idx + 1,
                        SERVER_ERROR_RETRY_MS / 1000
                    ),
                )
            } else {
                let first_line = err.lines().next().unwrap_or(&err).trim().to_string();
                (
                    DEFAULT_ERROR_RETRY_MS,
                    format!("Region {}: {first_line}", slot_idx + 1),
                )
            };

            if let Some(old_err_id) = runtime.active_error_id.take() {
                err_handler.dismiss(old_err_id);
            }
            let error_id = err_handler.report_simple(friendly);
            runtime.active_error_id = Some(error_id);

            if language_version == slot.language_version {
                slot.next_tick_at_ms = crate::core::utils::now_ms() + retry_delay_ms;
            }
        }
    }
}
