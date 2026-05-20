use crate::core::{
    model::AppModel,
    ports::{FrameSource, OcrEngine, Translator},
    usecases::pipeline::TranslationPipeline,
    worker::{BgResult, SlotRuntimeState},
};
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};

type TranslationCache = indexmap::IndexMap<(u64, Option<String>, String), (String, String)>;
type TextTranslationCache = indexmap::IndexMap<(u64, Option<String>, String), String>;

pub struct BackgroundCoordinator {
    pub bg_tx: mpsc::Sender<BgResult>,
    pub bg_rx: mpsc::Receiver<BgResult>,
    pool: Mutex<threadpool::ThreadPool>,
    yolo_bubble:
        Arc<Mutex<Option<Arc<crate::adapters::ocr::yolo_bubble_detector::YoloBubbleDetector>>>>,
}

impl BackgroundCoordinator {
    pub fn new() -> Self {
        let (bg_tx, bg_rx) = mpsc::channel();
        let pool = Mutex::new(threadpool::ThreadPool::new(4)); // Default 4 worker threads
        let yolo_bubble = Arc::new(Mutex::new(None));
        Self {
            bg_tx,
            bg_rx,
            pool,
            yolo_bubble,
        }
    }

    pub fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn process_results(
        &self,
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut [SlotRuntimeState],
        err_handler: &crate::core::usecases::error_handler::ErrorHandler,
        translation_cache: &Arc<Mutex<TranslationCache>>,
        settings: &crate::infrastructure::settings::Settings,
    ) {
        while let Ok(result) = self.bg_rx.try_recv() {
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
                } => {
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        let mut model = model_arc.lock();
                        let slot = match model.slots.get_mut(slot_idx) {
                            Some(s) => s,
                            None => continue,
                        };

                        if language_version != slot.language_version {
                            runtime.busy = false;
                            runtime.processing = false;
                            runtime.first_unstable_at = 0; // Reset
                            slot.next_tick_at_ms = 0;
                            continue;
                        }

                        runtime.busy = false;
                        runtime.processing = false;
                        runtime.first_unstable_at = 0; // Reset on success
                        runtime.status = "Ready".to_string();
                        runtime.last_hash = frame_hash;

                        let now = Self::now_ms();
                        slot.next_tick_at_ms = now.saturating_add(slot.refresh_ms.max(500));

                        let new_ocr = ocr_text.trim();
                        let old_ocr = slot.last_ocr_text.trim();
                        let realtime = &settings.realtime;

                        if new_ocr.is_empty() {
                            // ── Persistence Check ──
                            if now < runtime.last_seen_text_at_ms + realtime.subtitle_persistence_ms
                            {
                                // Keep persistent subtitles on screen!
                                let pers_trans = runtime.persistent_translation.lock().clone();
                                let pers_ocr = runtime.persistent_ocr_lines.lock().clone();
                                let pers_trans_lines =
                                    runtime.persistent_trans_lines.lock().clone();

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
                                // Persistence expired, clear fully
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

                            if runtime.identical_frames_count >= realtime.stability_threshold_frames
                            {
                                // Text is fully stable, render new translation!
                                runtime.last_seen_text_at_ms = now; // Update timestamp for persistence

                                if new_ocr != old_ocr {
                                    slot.last_ocr_text = ocr_text.clone();
                                    slot.last_translation = translated.clone();
                                    slot.last_ocr_lines = ocr_lines.clone();
                                    slot.last_trans_lines = trans_lines.clone();
                                    slot.last_yolo_bubbles = yolo_bubbles.clone();

                                    if !translated.trim().is_empty() {
                                        let cap =
                                            settings.realtime.context_window_size.max(1) as usize;
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
                                } else {
                                    if !translated.trim().is_empty() {
                                        slot.last_trans_lines = trans_lines.clone();
                                        slot.last_ocr_lines = ocr_lines.clone();
                                        slot.last_yolo_bubbles = yolo_bubbles.clone();
                                        slot.last_translation = translated.clone();
                                    }
                                }

                                // Save to persistence store
                                if !slot.last_translation.trim().is_empty() {
                                    *runtime.persistent_translation.lock() =
                                        Some(slot.last_translation.clone());
                                    *runtime.persistent_ocr_lines.lock() =
                                        slot.last_ocr_lines.clone();
                                    *runtime.persistent_trans_lines.lock() =
                                        slot.last_trans_lines.clone();
                                }
                            } else {
                                // Still debouncing typewriter animation, keep displaying previous persistent text
                                let pers_trans = runtime.persistent_translation.lock().clone();
                                let pers_ocr = runtime.persistent_ocr_lines.lock().clone();
                                let pers_trans_lines =
                                    runtime.persistent_trans_lines.lock().clone();

                                if let Some(pt) = pers_trans {
                                    slot.last_translation = pt;
                                    slot.last_ocr_lines = pers_ocr;
                                    slot.last_trans_lines = pers_trans_lines;
                                }
                                // Force quick tick to read the next frame of the animation
                                slot.next_tick_at_ms = now.saturating_add(50);
                            }
                        }
                    }
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        runtime.error_streak = 0; // Reset error streak on success
                        if let Some(err_id) = runtime.active_error_id.take() {
                            err_handler.dismiss(err_id);
                        }
                    }
                }
                BgResult::Unchanged { slot_idx } => {
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        runtime.busy = false;
                        runtime.status = "Ready".to_string();
                        runtime.first_unstable_at = 0; // Reset
                    }
                    let now = Self::now_ms();
                    let mut model = model_arc.lock();
                    if let Some(slot) = model.slots.get_mut(slot_idx) {
                        slot.next_tick_at_ms = now.saturating_add(slot.refresh_ms.max(100));
                        // Reduced from 200ms
                    }
                }
                BgResult::HashChanged { slot_idx, new_hash } => {
                    let mut model = model_arc.lock();
                    let now = Self::now_ms();
                    if let Some(slot) = model.slots.get_mut(slot_idx) {
                        slot.stable_hash = new_hash;
                        slot.stable_since_ms = now;
                        slot.next_tick_at_ms = now.saturating_add(30); // Reduced from 150ms to 30ms for instant follow-up
                    }
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        runtime.busy = false;
                        // Initialize first_unstable_at if it's 0
                        if runtime.first_unstable_at == 0 {
                            runtime.first_unstable_at = now;
                        }
                    }
                }
                BgResult::WaitingDebounce { slot_idx } => {
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        runtime.busy = false;
                    }
                    let mut model = model_arc.lock();
                    if let Some(slot) = model.slots.get_mut(slot_idx) {
                        slot.next_tick_at_ms = Self::now_ms() + 16; // 60fps responsive polling (16ms)
                    }
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
                } => {
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        let mut model = model_arc.lock();
                        let slot = match model.slots.get_mut(slot_idx) {
                            Some(s) => s,
                            None => continue,
                        };

                        if language_version != slot.language_version {
                            runtime.busy = false;
                            slot.next_tick_at_ms = 0;
                            continue;
                        }

                        runtime.busy = false;
                        runtime.processing = false;
                        runtime.status = "Ready (Cached)".to_string();
                        runtime.error_streak = 0; // Success case
                        if let Some(err_id) = runtime.active_error_id.take() {
                            err_handler.dismiss(err_id);
                        }
                        runtime.first_unstable_at = 0; // Reset
                        runtime.last_hash = frame_hash;

                        slot.last_ocr_text = ocr_text;
                        slot.last_translation = translated.clone();
                        slot.last_ocr_lines = ocr_lines; // Update positions!
                        slot.last_yolo_bubbles = yolo_bubbles;

                        // Re-align cached translation to the current OCR lines
                        slot.last_trans_lines = trans_lines;

                        slot.next_tick_at_ms = Self::now_ms() + slot.refresh_ms.max(200);
                    }
                }
                BgResult::StatusUpdate { slot_idx, status } => {
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        runtime.status = status;
                        if runtime.status.contains("Scanning") || runtime.status.contains("AI") {
                            runtime.processing = true;
                        }
                    }
                }
                BgResult::Error {
                    slot_idx,
                    language_version,
                    err,
                } => {
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        let mut model = model_arc.lock();
                        let slot = match model.slots.get_mut(slot_idx) {
                            Some(s) => s,
                            None => continue,
                        };

                        runtime.busy = false;
                        runtime.processing = false;
                        runtime.status = "Error".to_string();
                        runtime.error_streak = runtime.error_streak.saturating_add(1);

                        let is_rate_limit = err.contains("quota")
                            || err.contains("429")
                            || err.contains("Too Many Requests");
                        let is_bad_request =
                            err.contains("400") || err.contains("parse") || err.contains("invalid");
                        let is_server_err = err.contains("500")
                            || err.contains("502")
                            || err.contains("503")
                            || err.contains("timeout");

                        let (retry_delay_ms, friendly) = if is_rate_limit {
                            // Exponential backoff for rate limits: 30s, 60s, 120s, 240s... max 10 mins
                            let multiplier =
                                2u64.pow(runtime.error_streak.saturating_sub(1).min(5));
                            let secs = 30 * multiplier;
                            (
                                secs * 1000,
                                format!(
                                    "Region {}: API rate limit hit — retrying in {secs}s",
                                    slot_idx + 1
                                ),
                            )
                        } else if is_bad_request {
                            let secs = 10;
                            (
                                10_000,
                                format!(
                                    "Region {}: Data format error — retrying in {secs}s",
                                    slot_idx + 1
                                ),
                            )
                        } else if is_server_err {
                            let secs = 5;
                            (
                                5_000,
                                format!(
                                    "Region {}: Server/Network error — retrying in {secs}s",
                                    slot_idx + 1
                                ),
                            )
                        } else {
                            let first_line = err.lines().next().unwrap_or(&err).trim().to_string();
                            (3_000, format!("Region {}: {first_line}", slot_idx + 1))
                        };

                        if let Some(old_err_id) = runtime.active_error_id.take() {
                            err_handler.dismiss(old_err_id);
                        }
                        let error_id = err_handler.report_simple(friendly);
                        runtime.active_error_id = Some(error_id);

                        if language_version == slot.language_version {
                            slot.next_tick_at_ms = Self::now_ms() + retry_delay_ms;
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments, clippy::ptr_arg)]
    pub fn tick(
        &self,
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut Vec<SlotRuntimeState>,
        capture: &Arc<dyn FrameSource>,
        ocr_engine: &Arc<dyn OcrEngine>,
        translator: &Option<Arc<dyn Translator + Send + Sync>>,
        translation_cache: &Arc<Mutex<TranslationCache>>,
        text_translation_cache: &Arc<Mutex<TextTranslationCache>>,
        settings: &crate::infrastructure::settings::Settings,
        platform: &Arc<dyn crate::infrastructure::platform::PlatformServices>,
        ctx: egui::Context,
    ) {
        // Dynamically apply user-configured worker thread counts
        self.pool
            .lock()
            .set_num_threads(settings.perf.worker_threads.max(1));

        let now = Self::now_ms();
        let snapshot = { model_arc.lock().clone() };
        if !snapshot.running {
            return;
        }

        let yolo_detector = if settings.use_yolo_bubble {
            let mut guard = self.yolo_bubble.lock();
            if guard.is_none() && crate::infrastructure::asset_manager::check_bubble_yolo_exists() {
                *guard = Some(Arc::new(
                    crate::adapters::ocr::yolo_bubble_detector::YoloBubbleDetector::new(
                        settings.perf.gpu_backend,
                    ),
                ));
            }
            guard.clone()
        } else {
            None
        };

        let parallel_ocr = settings.perf.parallel_ocr;
        let any_busy = !parallel_ocr && slots_runtime.iter().any(|r| r.busy);

        for (i, slot) in snapshot.slots.iter().enumerate() {
            if !slot.enabled || slot.rect.is_none() {
                continue;
            }

            if slots_runtime.len() <= i {
                slots_runtime.push(SlotRuntimeState::new());
            }

            // Language change detection logic
            let cur_src = slot.source_lang.as_ref().map(|l| l.0.clone());
            let cur_tgt = slot.target_lang.0.clone();
            let lang_changed = slots_runtime[i].last_langs != (cur_src.clone(), cur_tgt.clone());
            let model_changed = slots_runtime[i].last_ppocr_model != Some(settings.ppocr_model);

            if lang_changed || model_changed {
                slots_runtime[i].last_langs = (cur_src, cur_tgt);
                slots_runtime[i].last_ppocr_model = Some(settings.ppocr_model);
                slots_runtime[i].last_hash = 0;
                slots_runtime[i].recent_translations.clear();
                translation_cache.lock().clear();
                text_translation_cache.lock().clear();

                let mut model = model_arc.lock();
                if let Some(m_slot) = model.slots.get_mut(i) {
                    m_slot.language_version = m_slot.language_version.wrapping_add(1);
                    m_slot.last_trans_lines.clear();
                    m_slot.last_ocr_lines.clear();
                    m_slot.last_translation.clear();
                    m_slot.last_ocr_text.clear();
                    m_slot.next_tick_at_ms = 0;
                    m_slot.stable_hash = 0;
                    m_slot.stable_since_ms = 0;
                }
                slots_runtime[i].last_hash = 1;
                continue;
            }

            if slots_runtime[i].busy || now < slot.next_tick_at_ms || any_busy {
                continue;
            }

            slots_runtime[i].busy = true;
            slots_runtime[i].processing = false;
            {
                let mut m = model_arc.lock();
                if let Some(s) = m.slots.get_mut(i) {
                    s.next_tick_at_ms = u64::MAX;
                }
            }

            let rect = slot.rect.unwrap();
            let display_id = slot.display_id;
            let source_lang = slot.source_lang.clone();
            let target_lang = slot.target_lang.clone();
            let capture = capture.clone();
            let ocr_engine = ocr_engine.clone();
            let translator = translator.clone();
            let tx = self.bg_tx.clone();
            let prev_hash = slots_runtime[i].last_hash;
            let stable_hash = slot.stable_hash;
            let stable_since_ms = slot.stable_since_ms;
            let language_version = slot.language_version;
            let cache_arc = translation_cache.clone();
            let text_cache_arc = text_translation_cache.clone();
            let first_unstable_at = slots_runtime[i].first_unstable_at;
            let platform = platform.clone();
            let smart_merge = settings.smart_merge;
            let img_proc_cfg = settings.img_proc.clone();
            let txt_proc_cfg = settings.txt_proc.clone();
            let regex_rules = settings.regex_rules.clone();
            let glossary_entries = settings.glossary_entries.clone();
            let last_frame_arc = slots_runtime[i].last_frame.clone();
            let max_cache_entries = settings.perf.max_cache_entries;
            let enable_batching = settings.perf.enable_batching;
            let context_segments: Vec<String> = slots_runtime[i]
                .recent_translations
                .iter()
                .cloned()
                .collect();
            let contextual_translation = settings.trans_behavior.contextual_translation;
            let context_window_size = settings.realtime.context_window_size;
            let th_segmentation = settings.txt_proc.th_segmentation;
            let jp_merge_vertical = settings.txt_proc.jp_merge_vertical;

            let yolo_detector = yolo_detector.clone();
            let ctx_worker = ctx.clone();
            self.pool.lock().execute(move || {
                ctx_worker.request_repaint();
                let tx_for_panic = tx.clone();
                let ctx_for_panic = ctx_worker.clone();
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let tx_inner = tx.clone();
                    let result = TranslationPipeline::execute_slot(
                        crate::core::usecases::pipeline::PipelineContext {
                            slot_idx: i,
                            rect,
                            display_id,
                            source_lang,
                            target_lang,
                            language_version,
                            capture,
                            ocr_engine,
                            translator,
                            platform: platform.clone(),
                            yolo_detector,
                            prev_hash,
                            stable_hash,
                            stable_since_ms,
                            first_unstable_at,
                            cache_arc,
                            text_cache_arc,
                            max_cache_entries,
                            smart_merge,
                            img_proc_cfg,
                            txt_proc_cfg,
                            regex_rules,
                            glossary_entries,
                            jp_merge_vertical,
                            th_segmentation,
                            enable_batching,
                            context_segments,
                            contextual_translation,
                            context_window_size,
                            last_frame_arc,
                            status_tx: tx_inner,
                            ctx: ctx_worker.clone(),
                        },
                    );

                    match result {
                        Ok(res) => {
                            let _ = tx.send(res);
                            ctx_worker.request_repaint();
                        }
                        Err(e) => {
                            let _ = tx.send(BgResult::Error {
                                slot_idx: i,
                                language_version,
                                err: format!("{e:#}"),
                            });
                            ctx_worker.request_repaint();
                        }
                    }
                }));

                if res.is_err() {
                    let _ = tx_for_panic.send(BgResult::Error {
                        slot_idx: i,
                        language_version,
                        err: "Background thread panicked (system error)".to_string(),
                    });
                    ctx_for_panic.request_repaint();
                }
            });
        }
    }
}
