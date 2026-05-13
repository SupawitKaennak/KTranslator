use std::sync::{mpsc, Arc};
use std::collections::HashMap;
use parking_lot::Mutex;
use crate::core::{
    model::AppModel,
    ports::{FrameSource, Translator, OcrEngine},
    usecases::pipeline::TranslationPipeline,
    worker::{BgResult, SlotRuntimeState},
};

pub struct BackgroundCoordinator {
    pub bg_tx: mpsc::Sender<BgResult>,
    pub bg_rx: mpsc::Receiver<BgResult>,
    pool: threadpool::ThreadPool,
}

impl BackgroundCoordinator {
    pub fn new() -> Self {
        let (bg_tx, bg_rx) = mpsc::channel();
        let pool = threadpool::ThreadPool::new(4); // 4 persistent worker threads
        Self { bg_tx, bg_rx, pool }
    }

    pub fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    pub fn process_results(
        &self,
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut Vec<SlotRuntimeState>,
        err_handler: &crate::core::usecases::error_handler::ErrorHandler,
        translation_cache: &Arc<Mutex<HashMap<(u64, Option<String>, String), (String, String)>>>,
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

                        if new_ocr.is_empty() {
                            slot.last_ocr_text = String::new();
                            slot.last_translation = String::new();
                            slot.last_ocr_lines.clear();
                            slot.last_trans_lines.clear();
                        } else if new_ocr != old_ocr {
                            slot.last_ocr_text = ocr_text.clone();
                            slot.last_translation = translated.clone();
                            slot.last_ocr_lines = ocr_lines.clone();
                            slot.last_trans_lines = trans_lines.clone();

                            if frame_hash != 0 {
                                let cache_key = (frame_hash, slot.source_lang.as_ref().map(|l| l.0.clone()), slot.target_lang.0.clone());
                                translation_cache.lock().insert(cache_key, (ocr_text, translated));
                            }
                        } else {
                            if !translated.trim().is_empty() {
                                slot.last_trans_lines = trans_lines;
                                slot.last_ocr_lines = ocr_lines;
                                slot.last_translation = translated;
                            }
                        }
                    }
                    err_handler.dismiss(slot_idx);
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
                        slot.next_tick_at_ms = now.saturating_add(slot.refresh_ms.max(200));
                    }
                }
                BgResult::HashChanged { slot_idx, new_hash } => {
                    let mut model = model_arc.lock();
                    let now = Self::now_ms();
                    if let Some(slot) = model.slots.get_mut(slot_idx) {
                        slot.stable_hash = new_hash;
                        slot.stable_since_ms = now;
                        slot.next_tick_at_ms = now.saturating_add(150);
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
                        slot.next_tick_at_ms = Self::now_ms() + 50;
                    }
                }
                BgResult::CacheHit { slot_idx, language_version, ocr_text, translated, frame_hash, ocr_lines, trans_lines } => {
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
                        runtime.status = "Ready (Cached)".to_string();
                        runtime.first_unstable_at = 0; // Reset
                        runtime.last_hash = frame_hash;

                        slot.last_ocr_text = ocr_text;
                        slot.last_translation = translated.clone();
                        slot.last_ocr_lines = ocr_lines; // Update positions!
                        
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
                BgResult::Error { slot_idx, language_version, err } => {
                    if let Some(runtime) = slots_runtime.get_mut(slot_idx) {
                        let mut model = model_arc.lock();
                        let slot = match model.slots.get_mut(slot_idx) {
                            Some(s) => s,
                            None => continue,
                        };

                        runtime.busy = false;
                        runtime.processing = false;
                        runtime.status = "Error".to_string();

                        let friendly = if err.contains("quota") || err.contains("429") {
                            let secs = 30;
                            format!("Region {}: API quota exceeded — retrying in {secs}s", slot_idx + 1)
                        } else {
                            let first_line = err.lines().next().unwrap_or(&err).trim().to_string();
                            format!("Region {}: {first_line}", slot_idx + 1)
                        };
                        err_handler.report_simple(friendly);
                        
                        if language_version == slot.language_version {
                            slot.next_tick_at_ms = Self::now_ms() + 2000;
                        }
                    }
                }
            }
        }
    }

    pub fn tick(
        &self,
        model_arc: &Arc<Mutex<AppModel>>,
        slots_runtime: &mut Vec<SlotRuntimeState>,
        capture: &Arc<dyn FrameSource>,
        ocr_engine: &Arc<dyn OcrEngine>,
        translator: &Option<Arc<dyn Translator + Send + Sync>>,
        translation_cache: &Arc<Mutex<HashMap<(u64, Option<String>, String), (String, String)>>>,
        text_translation_cache: &Arc<Mutex<HashMap<(u64, Option<String>, String), String>>>,
        smart_merge: bool,
        ctx: egui::Context,
    ) {
        let now = Self::now_ms();
        let snapshot = { model_arc.lock().clone() };
        if !snapshot.running { return; }

        for (i, slot) in snapshot.slots.iter().enumerate() {
            if !slot.enabled || slot.rect.is_none() { continue; }

            if slots_runtime.len() <= i {
                slots_runtime.push(SlotRuntimeState::new());
            }

            // Language change detection logic (moved from app.rs)
            let cur_src = slot.source_lang.as_ref().map(|l| l.0.clone());
            let cur_tgt = slot.target_lang.0.clone();
            let lang_changed = slots_runtime[i].last_langs != (cur_src.clone(), cur_tgt.clone());
            if lang_changed {
                slots_runtime[i].last_langs = (cur_src, cur_tgt);
                slots_runtime[i].last_hash = 0;
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

            if slots_runtime[i].busy || now < slot.next_tick_at_ms {
                continue;
            }

            slots_runtime[i].busy = true;
            slots_runtime[i].processing = false; // Reset processing at start of every tick
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
            
            let ctx_worker = ctx.clone();
            self.pool.execute(move || {
                // let _ = tx.send(BgResult::StatusUpdate { slot_idx: i, status: "Taking Screenshot...".to_string() }); // REMOVED: Too fast, causes flickering
                ctx_worker.request_repaint();
                let tx_for_panic = tx.clone();
                let ctx_for_panic = ctx_worker.clone();
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let tx_inner = tx.clone();
                    let result = TranslationPipeline::execute_slot(
                        i, rect, display_id, source_lang, target_lang,
                        capture, ocr_engine, translator, prev_hash, stable_hash,
                        stable_since_ms, language_version, cache_arc, text_cache_arc,
                        first_unstable_at, smart_merge, tx_inner, ctx_worker.clone(),
                    );

                    match result {
                        Ok(res) => { 
                            let _ = tx.send(res); 
                            ctx_worker.request_repaint();
                        }
                        Err(e) => {
                            let _ = tx.send(BgResult::Error { slot_idx: i, language_version, err: format!("{e:#}") });
                            ctx_worker.request_repaint();
                        }
                    }
                }));

                if res.is_err() {
                    let _ = tx_for_panic.send(BgResult::Error {
                        slot_idx: i, language_version, err: "Background thread panicked (system error)".to_string(),
                    });
                    ctx_for_panic.request_repaint();
                }
            });
        }
    }
}

