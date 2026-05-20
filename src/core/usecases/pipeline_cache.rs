use crate::core::pipeline_result::BgResult;
use crate::core::types::{TextTranslationCache, TranslationCache};
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
    if let Some((cached_ocr, cached_trans)) = cache.get(cache_key) {
        tracing::debug!(slot = slot_idx, hash = %hash, "Cache hit for whole frame");
        return Some(BgResult::CacheHit {
            slot_idx,
            language_version,
            ocr_text: cached_ocr.clone(),
            translated: cached_trans.clone(),
            frame_hash: hash,
            ocr_lines: Vec::new(),
            trans_lines: vec![cached_trans.clone()],
            yolo_bubbles: Vec::new(),
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
    max_cache_entries: usize,
) {
    let mut frame_cache = cache_arc.lock();
    crate::core::utils::enforce_cache_limit(&mut frame_cache, max_cache_entries);
    frame_cache.insert(cache_key, (ocr_text, translated.clone()));
    drop(frame_cache);

    let mut text_cache = text_cache_arc.lock();
    crate::core::utils::enforce_cache_limit(&mut text_cache, max_cache_entries);
    text_cache.insert(text_cache_key, translated);
}
