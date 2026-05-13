use std::sync::{mpsc, Arc};
use std::collections::HashMap;
use anyhow::Context;
use parking_lot::Mutex;
use eframe::egui;

use crate::core::{
    ports::{FrameSource, OcrEngine, Translator},
    types::{LanguageTag, Rect},
    worker::{BgResult, smart_hash},
    text_cleaner::TextCleaner,
};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Orchestrates the end-to-end screen translation flow for a single frame.
/// Decouples raw infrastructure access and hashing from task concurrency.
pub struct TranslationPipeline;

impl TranslationPipeline {
    pub fn execute_slot(
        slot_idx: usize,
        rect: Rect,
        display_id: u32,
        source_lang: Option<LanguageTag>,
        target_lang: LanguageTag,
        capture: Arc<dyn FrameSource>,
        ocr_engine: Arc<dyn OcrEngine>,
        translator: Option<Arc<dyn Translator + Send + Sync>>,
        prev_hash: u64,
        stable_hash: u64,
        stable_since_ms: u64,
        language_version: u32,
        cache_arc: Arc<Mutex<HashMap<(u64, Option<String>, String), (String, String)>>>,
        text_cache_arc: Arc<Mutex<HashMap<(u64, Option<String>, String), String>>>,
        first_unstable_at: u64,
        smart_merge: bool,
        status_tx: mpsc::Sender<BgResult>,
        ctx: egui::Context,
    ) -> anyhow::Result<BgResult> {
        let frame = capture.capture_rect(rect, display_id)?;
        ctx.request_repaint();
        let hash = smart_hash(&frame.data);
        let now = now_ms();

        tracing::debug!(slot = slot_idx, hash = %hash, "Frame captured and hashed");

        // Stability Logic
        let is_changing = hash != stable_hash || stable_since_ms == 0;
        let unstable_dur = if stable_since_ms == 0 { 0 } else { now.saturating_sub(stable_since_ms) };
        
        // If we've been unstable for a long time (e.g. 1.5s), FORCE proceed.
        let unstable_since_start = if first_unstable_at == 0 { 0 } else { now.saturating_sub(first_unstable_at) };
        let force_proceed = unstable_since_start > 1500; 

        if is_changing && !force_proceed {
            return Ok(BgResult::HashChanged { slot_idx, new_hash: hash });
        }
        if !force_proceed && unstable_dur < 400 {
            return Ok(BgResult::WaitingDebounce { slot_idx });
        }
        if hash == prev_hash && prev_hash != 0 && !force_proceed {
            return Ok(BgResult::Unchanged { slot_idx });
        }

        tracing::info!(slot = slot_idx, "Proceeding with OCR/Translation");
        let _ = status_tx.send(BgResult::StatusUpdate { slot_idx, status: "Scanning Text...".to_string() });
        ctx.request_repaint();
        let raw_ocr_lines = ocr_engine.recognize_lines(frame, source_lang.as_ref())?;
        
        let blocks = crate::core::layout::build_blocks(raw_ocr_lines, smart_merge);
        
        let mut ocr_lines = Vec::new();
        let mut block_sizes = Vec::new();
        for block in &blocks {
            block_sizes.push(block.lines.len());
            for line in &block.lines {
                ocr_lines.push(line.clone());
            }
        }

        let raw_ocr_text = blocks.iter().map(|b| b.source_text.replace('\n', " ")).collect::<Vec<_>>().join("\n");
        let ocr_text = TextCleaner::clean(&raw_ocr_text);

        // Helper to map block-level translations back to line-level `trans_lines`
        let build_trans_lines = |translated: &str| -> Vec<String> {
            let block_translations = Self::parse_numbered_lines(translated, blocks.len());
            let mut trans_lines = Vec::new();
            for (block_idx, size) in block_sizes.iter().enumerate() {
                trans_lines.push(block_translations.get(block_idx).cloned().unwrap_or_default());
                for _ in 1..*size {
                    trans_lines.push(String::new());
                }
            }
            trans_lines
        };

        let cache_key = (hash, source_lang.as_ref().map(|l| l.0.clone()), target_lang.0.clone());
        {
            let cache = cache_arc.lock();
            if let Some((ocr, tra)) = cache.get(&cache_key) {
                let trans_lines = build_trans_lines(tra);
                return Ok(BgResult::CacheHit {
                    slot_idx, language_version, ocr_text: ocr.clone(), translated: tra.clone(), frame_hash: hash, ocr_lines, trans_lines
                });
            }
        }

        if ocr_text.is_empty() {
            return Ok(BgResult::Done {
                slot_idx, language_version, ocr_text: String::new(), translated: String::new(), frame_hash: hash, ocr_lines: Vec::new(), trans_lines: Vec::new(),
            });
        }

        // --- Aggressive Text-Level Cache Check ---
        {
            let text_hash = {
                let mut h: u64 = 0xcbf29ce484222325;
                for b in ocr_text.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
                h
            };
            let tc_key = (text_hash, source_lang.as_ref().map(|l| l.0.clone()), target_lang.0.clone());
            let cached = { let tc = text_cache_arc.lock(); tc.get(&tc_key).cloned() };
            
            if let Some(cached_trans) = cached {
                let mut fc = cache_arc.lock();
                fc.insert(cache_key, (ocr_text.clone(), cached_trans.clone()));
                
                let trans_lines = build_trans_lines(&cached_trans);
                return Ok(BgResult::Done {
                    slot_idx, language_version, ocr_text, translated: cached_trans, frame_hash: hash, ocr_lines, trans_lines
                });
            }
        }

        if source_lang.as_ref().map(|s| s.0 == target_lang.0).unwrap_or(false) {
            let mut cache = cache_arc.lock();
            cache.insert(cache_key, (ocr_text.clone(), ocr_text.clone()));
            let trans_lines = build_trans_lines(&ocr_text);
            return Ok(BgResult::Done {
                slot_idx, language_version, ocr_text: ocr_text.clone(), translated: ocr_text, frame_hash: hash, ocr_lines, trans_lines
            });
        }

        let _ = status_tx.send(BgResult::StatusUpdate { slot_idx, status: "AI Translating...".to_string() });
        ctx.request_repaint();
        let translated = translator.as_ref()
            .context("No translator provider selected")?
            .translate(&ocr_text, source_lang.as_ref(), &target_lang)?;

        let trans_lines = build_trans_lines(&translated);

        {
            let text_hash = {
                let mut h: u64 = 0xcbf29ce484222325;
                for b in ocr_text.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
                h
            };
            let text_cache_key = (text_hash, source_lang.as_ref().map(|l| l.0.clone()), target_lang.0.clone());
            let mut frame_cache = cache_arc.lock();
            frame_cache.insert(cache_key, (ocr_text.clone(), translated.clone()));
            drop(frame_cache);
            let mut text_cache = text_cache_arc.lock();
            text_cache.insert(text_cache_key, translated.clone());
        }
        Ok(BgResult::Done { slot_idx, language_version, ocr_text, translated, frame_hash: hash, ocr_lines, trans_lines })
    }

    fn parse_numbered_lines(raw: &str, ocr_count: usize) -> Vec<String> {
        let mut result = crate::core::prompt_builder::parse_translation_response(raw, ocr_count);
        for s in result.iter_mut() {
            *s = TextCleaner::clean(s);
        }
        result
    }
}
