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

/// Enforces cache size limit by removing oldest entries if cache is full
fn enforce_cache_limit<K, V>(cache: &mut parking_lot::MutexGuard<std::collections::HashMap<K, V>>, max_entries: usize)
where
    K: Clone + std::hash::Hash + std::cmp::Eq,
{
    if cache.len() >= max_entries {
        // Remove oldest entries (simple FIFO by removing first few)
        let to_remove = cache.len() - max_entries + 1; // +1 to make room for new entry
        let keys: Vec<K> = cache.keys().take(to_remove).cloned().collect();
        for key in keys {
            cache.remove(&key);
        }
        tracing::warn!("Cache limit reached, removed {} oldest entries", to_remove);
    }
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
        img_proc_cfg: crate::infrastructure::settings::ImageProcessingSettings,
        txt_proc_cfg: crate::infrastructure::settings::TextProcessingSettings,
        regex_rules: Vec<crate::infrastructure::settings::RegexRule>,
        glossary_entries: Vec<crate::infrastructure::settings::GlossaryEntry>,
        last_frame_arc: Arc<Mutex<Option<crate::core::ports::FrameRgba>>>,
        status_tx: mpsc::Sender<BgResult>,
        ctx: egui::Context,
        max_cache_entries: usize,
    ) -> anyhow::Result<BgResult> {
        let mut frame = capture.capture_rect(rect, display_id)?;
        
        *last_frame_arc.lock() = Some(frame.clone());
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

        // Let's keep logic exactly flow matching:
        let cache_key = (hash, source_lang.as_ref().map(|l| l.0.clone()), target_lang.0.clone());
        
        // 1. Unchanged Check
        if hash == prev_hash && prev_hash != 0 && !force_proceed {
            return Ok(BgResult::Unchanged { slot_idx });
        }

        // 2. Stability Hash Tracking
        if is_changing && !force_proceed {
            return Ok(BgResult::HashChanged { slot_idx, new_hash: hash });
        }

        // 3. Stable Debounce check
        if unstable_dur < 400 && !force_proceed {
            return Ok(BgResult::WaitingDebounce { slot_idx });
        }

        // Check Cache
        {
            let cache = cache_arc.lock();
            if let Some((cached_ocr, cached_trans)) = cache.get(&cache_key) {
                tracing::debug!(slot = slot_idx, hash = %hash, "Cache hit for whole frame");
                return Ok(BgResult::CacheHit {
                    slot_idx, language_version, ocr_text: cached_ocr.clone(), translated: cached_trans.clone(), frame_hash: hash,
                    ocr_lines: Vec::new(), trans_lines: vec![cached_trans.clone()]
                });
            }
        }

        tracing::info!(slot = slot_idx, "Proceeding with OCR/Translation");
        let _ = status_tx.send(BgResult::StatusUpdate { slot_idx, status: "Scanning Text...".to_string() });
        ctx.request_repaint();

        // Perform Image pre-processing IN-PLACE on frame
        let (proc_data, proc_w, proc_h) = crate::core::usecases::image_processor::process_image_for_ocr(
            &frame.data, frame.width, frame.height, &img_proc_cfg
        );
        frame.data = proc_data;
        frame.width = proc_w;
        frame.height = proc_h;

        let mut raw_ocr_lines = ocr_engine.recognize_lines(frame, source_lang.as_ref())?;

        // Line-level Garbage Filtering
        raw_ocr_lines.retain(|l| TextCleaner::is_line_valid(&l.text, &txt_proc_cfg));
        
        // Rescale bounding boxes back to screen resolution coordinates if scaled
        if (img_proc_cfg.resize_scale - 1.0).abs() > 0.01 {
            let scale = img_proc_cfg.resize_scale;
            for line in &mut raw_ocr_lines {
                line.x /= scale;
                line.y /= scale;
                line.w /= scale;
                line.h /= scale;
            }
        }
        
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
        let ocr_text_base = TextCleaner::clean(&raw_ocr_text, &txt_proc_cfg);

        // Helper check: Perfect Translation Memory Hit
        if let Some(memory_trans) = crate::core::usecases::glossary_engine::GlossaryEngine::apply_translation_memory(&ocr_text_base, &glossary_entries) {
            tracing::info!(slot = slot_idx, "Translation Memory perfect match hit");
            let mut trans_lines = Vec::new();
            // Simple mapping or full global replacement
            let block_translations = Self::parse_numbered_lines(&memory_trans, blocks.len(), &txt_proc_cfg);
            for (block_idx, size) in block_sizes.iter().enumerate() {
                trans_lines.push(block_translations.get(block_idx).cloned().unwrap_or_default());
                for _ in 1..*size { trans_lines.push(String::new()); }
            }
            return Ok(BgResult::Done {
                slot_idx, language_version, ocr_text: ocr_text_base, translated: memory_trans, frame_hash: hash, ocr_lines, trans_lines
            });
        }
        
        // Apply Glossary Engine Overrides and Protected words
        let (ocr_text_gloss, gloss_protected_map) = crate::core::usecases::glossary_engine::GlossaryEngine::apply_pre_override(&ocr_text_base, &glossary_entries);

        // Apply Pre-Translation Regex Engine Rules
        let (ocr_text, regex_protected_map) = crate::core::usecases::regex_engine::RegexEngine::apply_pre_rules(&ocr_text_gloss, &regex_rules);

        // Extract active glossary metadata for injection into AI Guidance prompt
        let active_glossary_entries = crate::core::usecases::glossary_engine::GlossaryEngine::filter_active_entries(&ocr_text, &glossary_entries);
        let glossary_guidance_str = crate::core::usecases::glossary_engine::GlossaryEngine::build_glossary_guidance(&active_glossary_entries);

        // Helper to map block-level translations back to line-level `trans_lines`
        let txt_proc_cfg_inner = txt_proc_cfg.clone();
        let regex_rules_inner = regex_rules.clone();
        let gloss_protected_map_inner = Arc::new(gloss_protected_map);
        let regex_protected_map_inner = Arc::new(regex_protected_map);
        
        let build_trans_lines = move |translated: &str| -> Vec<String> {
            // Decode Glossary protected masks first
            let decoded_gloss = crate::core::usecases::glossary_engine::GlossaryEngine::apply_post_override(translated, &gloss_protected_map_inner);
            // Apply Regex Post rules and decode masks
            let post_trans = crate::core::usecases::regex_engine::RegexEngine::apply_post_rules(&decoded_gloss, &regex_rules_inner, &regex_protected_map_inner);
            
            let block_translations = Self::parse_numbered_lines(&post_trans, blocks.len(), &txt_proc_cfg_inner);
            let mut trans_lines = Vec::new();
            for (block_idx, size) in block_sizes.iter().enumerate() {
                trans_lines.push(block_translations.get(block_idx).cloned().unwrap_or_default());
                for _ in 1..*size {
                    trans_lines.push(String::new());
                }
            }
            trans_lines
        };

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
                enforce_cache_limit(&mut fc, max_cache_entries);
                fc.insert(cache_key, (ocr_text.clone(), cached_trans.clone()));

                let trans_lines = build_trans_lines(&cached_trans);
                return Ok(BgResult::Done {
                    slot_idx, language_version, ocr_text, translated: cached_trans, frame_hash: hash, ocr_lines, trans_lines
                });
            }
        }

        if source_lang.as_ref().map(|s| s.0 == target_lang.0).unwrap_or(false) {
            let mut cache = cache_arc.lock();
            enforce_cache_limit(&mut cache, max_cache_entries);
            cache.insert(cache_key, (ocr_text.clone(), ocr_text.clone()));
            let trans_lines = build_trans_lines(&ocr_text);
            return Ok(BgResult::Done {
                slot_idx, language_version, ocr_text: ocr_text.clone(), translated: ocr_text, frame_hash: hash, ocr_lines, trans_lines
            });
        }

        // --- Pre-Translation Validation Heuristics ---
        // Bypass costly AI translation calls for trivial raw outputs (pure numbers, whitespace, simple icons)
        let is_trivial = ocr_text.chars().all(|c| c.is_ascii_digit() || c.is_ascii_punctuation() || c.is_whitespace() || c == '…');
        if is_trivial {
            let mut cache = cache_arc.lock();
            enforce_cache_limit(&mut cache, max_cache_entries);
            cache.insert(cache_key, (ocr_text.clone(), ocr_text.clone()));
            let trans_lines = build_trans_lines(&ocr_text);
            return Ok(BgResult::Done {
                slot_idx, language_version, ocr_text: ocr_text.clone(), translated: ocr_text, frame_hash: hash, ocr_lines, trans_lines
            });
        }

        let _ = status_tx.send(BgResult::StatusUpdate { slot_idx, status: "AI Translating...".to_string() });
        ctx.request_repaint();

        // Inject Glossary guidance directly to the translated input stream seamlessly
        let mut text_to_translate = ocr_text.clone();
        if !glossary_guidance_str.is_empty() {
            text_to_translate.push_str(&format!("\n\n[MANDATORY_GLOSSARY_TERMS:\n{}]", glossary_guidance_str));
        }

        let raw_translated = translator.as_ref()
            .context("No translator provider selected")?
            .translate(&text_to_translate, source_lang.as_ref(), &target_lang)?;

        let trans_lines = build_trans_lines(&raw_translated);
        let translated = trans_lines.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join("\n");

        {
            let text_hash = {
                let mut h: u64 = 0xcbf29ce484222325;
                for b in ocr_text.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
                h
            };
            let text_cache_key = (text_hash, source_lang.as_ref().map(|l| l.0.clone()), target_lang.0.clone());
            let mut frame_cache = cache_arc.lock();
            enforce_cache_limit(&mut frame_cache, max_cache_entries);
            frame_cache.insert(cache_key, (ocr_text.clone(), translated.clone()));
            drop(frame_cache);
            let mut text_cache = text_cache_arc.lock();
            enforce_cache_limit(&mut text_cache, max_cache_entries);
            text_cache.insert(text_cache_key, translated.clone());
        }
        Ok(BgResult::Done { slot_idx, language_version, ocr_text, translated, frame_hash: hash, ocr_lines, trans_lines })
    }

    fn parse_numbered_lines(raw: &str, ocr_count: usize, config: &crate::infrastructure::settings::TextProcessingSettings) -> Vec<String> {
        let mut result = crate::core::prompt_builder::parse_translation_response(raw, ocr_count);
        for s in result.iter_mut() {
            *s = TextCleaner::clean(s, config);
        }
        result
    }
}
