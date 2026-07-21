use crate::core::{
    pipeline_execution_result::BgResult,
    ports::{FrameSource, OcrEngine, Translator},
    region_slot_state::AppModel,
    region_slot_state::SlotRuntimeState,
    types::{TextTranslationCache, TranslationCache},
    usecases::pipeline::TranslationPipeline,
};
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};

/// If a slot stays busy longer than this, force-reset it to allow a new attempt.
const BUSY_TIMEOUT_MS: u64 = 20_000;

pub struct BackgroundCoordinator {
    pub bg_tx: mpsc::Sender<BgResult>,
    pub bg_rx: mpsc::Receiver<BgResult>,
    pool: Mutex<threadpool::ThreadPool>,
    yolo_bubble: Arc<
        Mutex<Option<Arc<crate::adapters::ocr::yolo_bubble_detector_adapter::YoloBubbleDetector>>>,
    >,
    craft_text: Arc<
        Mutex<Option<Arc<crate::adapters::ocr::craft_text_detector_adapter::CraftTextDetector>>>,
    >,
}

impl BackgroundCoordinator {
    pub fn new() -> Self {
        let (bg_tx, bg_rx) = mpsc::channel();
        let pool = Mutex::new(threadpool::ThreadPool::new(4)); // Default 4 worker threads
        let yolo_bubble = Arc::new(Mutex::new(None));
        let craft_text = Arc::new(Mutex::new(None));
        Self {
            bg_tx,
            bg_rx,
            pool,
            yolo_bubble,
            craft_text,
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

        let now = crate::core::utils::now_ms();
        let snapshot = { model_arc.lock().clone() };
        if !snapshot.running {
            return;
        }

        let yolo_detector = if settings.text_detector
            == crate::infrastructure::settings::TextDetectorMode::YoloBubble
            || settings.text_detector
                == crate::infrastructure::settings::TextDetectorMode::YoloFullPageHybrid
        {
            let mut guard = self.yolo_bubble.lock();
            if guard.is_none()
                && crate::infrastructure::asset_download_manager::check_bubble_yolo_exists()
            {
                *guard = Some(Arc::new(
                    crate::adapters::ocr::yolo_bubble_detector_adapter::YoloBubbleDetector::new(
                        settings.perf.gpu_backend,
                        settings.perf.vram_limit_mb,
                    ),
                ));
            }
            guard.clone()
        } else {
            None
        };

        let craft_detector = if settings.text_detector
            == crate::infrastructure::settings::TextDetectorMode::CraftRegion
        {
            let mut guard = self.craft_text.lock();
            if guard.is_none()
                && crate::infrastructure::asset_download_manager::check_craft_exists()
            {
                *guard = Some(Arc::new(
                    crate::adapters::ocr::craft_text_detector_adapter::CraftTextDetector::new(
                        settings.perf.gpu_backend,
                        settings.perf.vram_limit_mb,
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

            // --- Busy Timeout Recovery ---
            // If a slot has been stuck as busy for over BUSY_TIMEOUT_MS (e.g. OCR/API hung),
            // force-reset it so the pipeline can attempt a fresh task.
            if slots_runtime[i].busy && slots_runtime[i].busy_since_ms > 0 {
                let busy_duration = now.saturating_sub(slots_runtime[i].busy_since_ms);
                if busy_duration > BUSY_TIMEOUT_MS {
                    tracing::warn!(
                        slot = i,
                        busy_duration_ms = busy_duration,
                        "Slot stuck busy — force resetting after timeout"
                    );
                    slots_runtime[i].busy = false;
                    slots_runtime[i].busy_since_ms = 0;
                    slots_runtime[i].processing = false;
                    slots_runtime[i].status = "Timeout — retrying".to_string();
                    let mut m = model_arc.lock();
                    if let Some(s) = m.slots.get_mut(i) {
                        s.next_tick_at_ms = now; // retry immediately
                    }
                }
            }

            if slots_runtime[i].busy || now < slot.next_tick_at_ms || any_busy {
                continue;
            }

            slots_runtime[i].busy = true;
            slots_runtime[i].busy_since_ms = now;  // record when busy started
            slots_runtime[i].processing = false;
            {
                let mut m = model_arc.lock();
                if let Some(s) = m.slots.get_mut(i) {
                    s.next_tick_at_ms = u64::MAX;
                }
            }

            let task = SlotTask {
                context: crate::core::usecases::pipeline::PipelineContext {
                    slot_idx: i,
                    rect: slot.rect.unwrap(),
                    display_id: slot.display_id,
                    source_lang: slot.source_lang.clone(),
                    target_lang: slot.target_lang.clone(),
                    language_version: slot.language_version,
                    capture: capture.clone(),
                    ocr_engine: ocr_engine.clone(),
                    translator: translator.clone(),
                    platform: platform.clone(),
                    yolo_detector: yolo_detector.clone(),
                    craft_detector: craft_detector.clone(),
                    text_detector_mode: settings.text_detector,
                    prev_hash: slots_runtime[i].last_hash,
                    stable_hash: slot.stable_hash,
                    stable_since_ms: slot.stable_since_ms,
                    first_unstable_at: slots_runtime[i].first_unstable_at,
                    cache_arc: translation_cache.clone(),
                    text_cache_arc: text_translation_cache.clone(),
                    max_cache_entries: settings.perf.max_cache_entries,
                    smart_merge: settings.smart_merge,
                    debounce_timeout_ms: settings.realtime.debounce_timeout_ms,
                    img_proc_cfg: settings.img_proc.clone(),
                    txt_proc_cfg: settings.txt_proc.clone(),
                    regex_rules: settings.regex_rules.clone(),
                    glossary_entries: settings.glossary_entries.clone(),
                    jp_merge_vertical: settings.txt_proc.jp_merge_vertical,
                    th_segmentation: settings.txt_proc.th_segmentation,
                    enable_batching: settings.perf.enable_batching,
                    enable_llm_ocr_correction: settings.enable_llm_ocr_correction,
                    context_segments: slots_runtime[i]
                        .recent_translations
                        .iter()
                        .cloned()
                        .collect(),
                    contextual_translation: settings.trans_behavior.contextual_translation,
                    context_window_size: settings.realtime.context_window_size,
                    last_frame_arc: slots_runtime[i].last_frame.clone(),
                    status_tx: self.bg_tx.clone(),
                    ctx: ctx.clone(),
                },
            };

            self.pool.lock().execute(move || {
                task.run();
            });
        }
    }
}

pub struct SlotTask {
    pub context: crate::core::usecases::pipeline::PipelineContext,
}

impl SlotTask {
    pub fn run(self) {
        let tx = self.context.status_tx.clone();
        let ctx_worker = self.context.ctx.clone();
        let slot_idx = self.context.slot_idx;
        let language_version = self.context.language_version;

        let tx_for_panic = tx.clone();
        let ctx_for_panic = ctx_worker.clone();

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let tx_inner = tx.clone();
            let result = TranslationPipeline::execute_slot(self.context);

            match result {
                Ok(res) => {
                    let _ = tx_inner.send(res);
                    ctx_worker.request_repaint();
                }
                Err(e) => {
                    let _ = tx_inner.send(BgResult::Error {
                        slot_idx,
                        language_version,
                        err: format!("{e:#}"),
                    });
                    ctx_worker.request_repaint();
                }
            }
        }));

        if res.is_err() {
            let _ = tx_for_panic.send(BgResult::Error {
                slot_idx,
                language_version,
                err: "Background thread panicked (system error)".to_string(),
            });
            ctx_for_panic.request_repaint();
        }
    }
}
