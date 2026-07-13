use crate::core::pipeline_execution_result::BgResult;
use crate::core::types::{CachedFrame, TextTranslationCache, TranslationCache};
use parking_lot::Mutex;
use std::sync::Arc;

pub fn check_cache(
    cache_arc: &Arc<Mutex<TranslationCache>>,
    cache_key: &(u64, Option<String>, String),
    slot_idx: usize,
    language_version: u32,
    hash: u64,
) -> Option<BgResult> {
    let cache = cache_arc.lock();
    if let Some(cached) = cache.get(cache_key) {
        tracing::debug!(slot = slot_idx, hash = %hash, "Cache hit for whole frame");
        return Some(BgResult::CacheHit {
            slot_idx,
            language_version,
            ocr_text: cached.ocr_text.clone(),
            translated: cached.translated.clone(),
            frame_hash: hash,
            ocr_lines: cached.ocr_lines.clone(),
            trans_lines: cached.trans_lines.clone(),
            yolo_bubbles: cached.yolo_bubbles.clone(),
        });
    }
    None
}

pub fn update_cache(
    cache_arc: &Arc<Mutex<TranslationCache>>,
    text_cache_arc: &Arc<Mutex<TextTranslationCache>>,
    cache_key: (u64, Option<String>, String),
    text_cache_key: (u64, Option<String>, String),
    ocr_text: String,
    translated: String,
    ocr_lines: Vec<crate::core::ports::OcrTextLine>,
    trans_lines: Vec<String>,
    yolo_bubbles: Vec<crate::core::ports::OcrTextLine>,
    max_cache_entries: usize,
) {
    let mut frame_cache = cache_arc.lock();
    crate::core::utils::enforce_cache_limit(&mut frame_cache, max_cache_entries);
    frame_cache.insert(cache_key, CachedFrame {
        ocr_text,
        translated: translated.clone(),
        ocr_lines,
        trans_lines,
        yolo_bubbles,
    });
    drop(frame_cache);

    let mut text_cache = text_cache_arc.lock();
    crate::core::utils::enforce_cache_limit(&mut text_cache, max_cache_entries);
    text_cache.insert(text_cache_key, translated);
}
