use anyhow::Context;
use eframe::egui;
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};

use crate::core::{
    pipeline_execution_result::BgResult,
    ports::{FrameSource, OcrEngine, Translator},
    text_cleaning_pipeline::TextCleaner,
    types::{LanguageTag, Rect, TextTranslationCache, TranslationCache},
    utils::smart_hash,
};

/// Force-proceed threshold: if a frame has been unstable for this long, proceed anyway.
const FORCE_PROCEED_MS: u64 = 800;
/// Minimum stable duration before debounce allows OCR/translation to proceed.
const DEBOUNCE_STABLE_MS: u64 = 150;

/// Orchestrates the end-to-end screen translation flow for a single frame.
/// Decouples raw infrastructure access and hashing from task concurrency.
pub struct TranslationPipeline;

/// Groups all parameters needed by `TranslationPipeline::execute_slot`.
///
/// Replaces the previous 33-parameter function signature with a structured context
/// that organizes parameters by their role in the pipeline.
pub struct PipelineContext {
    // --- Slot identity ---
    pub slot_idx: usize,
    pub rect: Rect,
    pub display_id: u32,
    pub source_lang: Option<LanguageTag>,
    pub target_lang: LanguageTag,
    pub language_version: u32,

    // --- Services ---
    pub capture: Arc<dyn FrameSource>,
    pub ocr_engine: Arc<dyn OcrEngine>,
    pub translator: Option<Arc<dyn Translator + Send + Sync>>,
    pub platform: Arc<dyn crate::infrastructure::platform::PlatformServices>,
    pub yolo_detector:
        Option<Arc<crate::adapters::ocr::yolo_bubble_detector_adapter::YoloBubbleDetector>>,
    pub craft_detector:
        Option<Arc<crate::adapters::ocr::craft_text_detector_adapter::CraftTextDetector>>,
    pub text_detector_mode: crate::infrastructure::settings::TextDetectorMode,

    // --- Hash/stability state ---
    pub prev_hash: u64,
    pub stable_hash: u64,
    pub stable_since_ms: u64,
    pub first_unstable_at: u64,

    // --- Caches ---
    pub cache_arc: Arc<Mutex<TranslationCache>>,
    pub text_cache_arc: Arc<Mutex<TextTranslationCache>>,
    pub max_cache_entries: usize,

    // --- Processing config ---
    pub smart_merge: bool,
    pub img_proc_cfg: crate::infrastructure::settings::ImageProcessingSettings,
    pub txt_proc_cfg: crate::infrastructure::settings::TextProcessingSettings,
    pub regex_rules: Vec<crate::infrastructure::settings::RegexRule>,
    pub glossary_entries: Vec<crate::infrastructure::settings::GlossaryEntry>,
    pub jp_merge_vertical: bool,
    pub th_segmentation: crate::infrastructure::settings::ThaiSegmentationMode,
    pub enable_batching: bool,
    pub enable_llm_ocr_correction: bool,

    // --- Context/history ---
    pub context_segments: Vec<String>,
    pub contextual_translation: bool,
    pub context_window_size: u32,

    // --- UI handles ---
    pub last_frame_arc: Arc<Mutex<Option<crate::core::ports::FrameRgba>>>,
    pub status_tx: mpsc::Sender<BgResult>,
    pub ctx: egui::Context,
}

impl TranslationPipeline {
    pub fn execute_slot(p: PipelineContext) -> anyhow::Result<BgResult> {
        // Destructure for convenient local access (preserves all original variable names)
        let PipelineContext {
            slot_idx,
            rect,
            display_id,
            source_lang,
            target_lang,
            language_version,
            capture,
            ocr_engine,
            translator,
            platform,
            yolo_detector,
            craft_detector,
            text_detector_mode,
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
            enable_llm_ocr_correction,
            context_segments,
            contextual_translation,
            context_window_size,
            last_frame_arc,
            status_tx,
            ctx,
        } = p;
        let _span = tracing::info_span!("execute_slot", slot = slot_idx).entered();
        let frame = capture.capture_rect(rect, display_id)?;

        *last_frame_arc.lock() = Some(frame.clone());
        ctx.request_repaint();
        let hash = smart_hash(&frame.data);
        let now = crate::core::utils::now_ms();

        tracing::debug!(slot = slot_idx, hash = %hash, "Frame captured and hashed");

        // Stability Logic
        let is_changing = hash != stable_hash || stable_since_ms == 0;
        let unstable_dur = if stable_since_ms == 0 {
            0
        } else {
            now.saturating_sub(stable_since_ms)
        };

        // If we've been unstable for a long time (e.g. 1.5s), FORCE proceed.
        let unstable_since_start = if first_unstable_at == 0 {
            0
        } else {
            now.saturating_sub(first_unstable_at)
        };
        let force_proceed = unstable_since_start > FORCE_PROCEED_MS;

        // Let's keep logic exactly flow matching:
        let cache_key = (
            hash,
            source_lang.as_ref().map(|l| l.0.clone()),
            target_lang.0.clone(),
        );

        // 1. Unchanged Check
        if hash == prev_hash && prev_hash != 0 && !force_proceed {
            return Ok(BgResult::Unchanged { slot_idx });
        }

        // 2. Stability Hash Tracking
        if is_changing && !force_proceed {
            return Ok(BgResult::HashChanged {
                slot_idx,
                new_hash: hash,
            });
        }

        // 3. Stable Debounce check: Reduced from 400ms to 150ms for hyper-reactive response
        if unstable_dur < DEBOUNCE_STABLE_MS && !force_proceed {
            return Ok(BgResult::WaitingDebounce { slot_idx });
        }

        if let Some(cache_hit) = crate::core::usecases::pipeline_cache::check_cache(
            &cache_arc,
            &cache_key,
            slot_idx,
            language_version,
            hash,
        ) {
            return Ok(cache_hit);
        }

        tracing::info!(slot = slot_idx, "Proceeding with OCR/Translation");
        let _ = status_tx.send(BgResult::StatusUpdate {
            slot_idx,
            status: "Scanning Text...".to_string(),
        });
        ctx.request_repaint();

        let (mut grouped_ocr_lines, yolo_bubbles, _bubble_detection_successful) =
            crate::core::usecases::pipeline_ocr::perform_ocr(
                &frame,
                &ocr_engine,
                source_lang.as_ref(),
                yolo_detector.as_ref(),
                craft_detector.as_ref(),
                text_detector_mode,
                &img_proc_cfg,
                jp_merge_vertical,
            );

        // Line-level Garbage Filtering per group
        for group in &mut grouped_ocr_lines {
            group.retain(|l| TextCleaner::is_line_valid(&l.text, &txt_proc_cfg));
        }
        grouped_ocr_lines.retain(|g| !g.is_empty());

        let mut blocks = Vec::new();
        for group in grouped_ocr_lines {
            let mut group_blocks = crate::core::text_layout_analyzer::build_blocks(
                group,
                smart_merge,
                jp_merge_vertical,
            );
            blocks.append(&mut group_blocks);
        }

        let mut ocr_lines = Vec::new();
        let mut block_sizes = Vec::new();
        for block in &blocks {
            block_sizes.push(block.lines.len());
            for line in &block.lines {
                ocr_lines.push(line.clone());
            }
        }

        let raw_ocr_text = blocks
            .iter()
            .map(|b| b.source_text.replace('\n', " "))
            .collect::<Vec<_>>()
            .join("\n");
        let mut ocr_text_base = TextCleaner::clean(&raw_ocr_text, &txt_proc_cfg);

        // Optional: LLM OCR Correction
        if enable_llm_ocr_correction {
            if let Some(translator_arc) = &translator {
                let _ = status_tx.send(BgResult::StatusUpdate {
                    slot_idx,
                    status: "Correcting OCR with LLM...".to_string(),
                });
                tracing::info!(slot = slot_idx, "Applying LLM OCR correction");
                match translator_arc.correct_text(&ocr_text_base, source_lang.as_ref()) {
                    Ok(corrected) => {
                        tracing::debug!(
                            slot = slot_idx,
                            "OCR Corrected: {} -> {}",
                            ocr_text_base,
                            corrected
                        );
                        ocr_text_base = corrected;
                    }
                    Err(e) => {
                        tracing::warn!(slot = slot_idx, "LLM OCR Correction failed: {:?}", e);
                    }
                }
            }
        }

        // Helper check: Perfect Translation Memory Hit
        if let Some(memory_trans) =
            crate::core::usecases::glossary_replacement_engine::GlossaryEngine::apply_translation_memory(
                &ocr_text_base,
                &glossary_entries,
            )
        {
            tracing::info!(slot = slot_idx, "Translation Memory perfect match hit");
            let mut trans_lines = Vec::new();
            // Simple mapping or full global replacement
            let block_translations =
                Self::parse_numbered_lines(&memory_trans, blocks.len(), &txt_proc_cfg);
            for (block_idx, size) in block_sizes.iter().enumerate() {
                trans_lines.push(
                    block_translations
                        .get(block_idx)
                        .cloned()
                        .unwrap_or_default(),
                );
                for _ in 1..*size {
                    trans_lines.push(String::new());
                }
            }
            return Ok(BgResult::Done {
                slot_idx,
                language_version,
                ocr_text: ocr_text_base,
                translated: memory_trans,
                frame_hash: hash,
                ocr_lines,
                trans_lines,
                yolo_bubbles: yolo_bubbles.clone(),
            });
        }

        // 1. Apply Pre-Translation Regex Engine Rules FIRST
        // This ensures space-separated words like "HA NAZO NO" are merged to "HANAZONO" BEFORE glossary looks for it.
        let (ocr_text_regex, regex_protected_map) =
            crate::core::usecases::regex_replacement_engine::RegexEngine::apply_pre_rules(
                &ocr_text_base,
                &regex_rules,
            );

        // 2. Apply Glossary Engine Overrides and Protected words SECOND
        // Now Glossary will successfully catch "HANAZONO" and apply protection or guidance correctly!
        let (ocr_text, gloss_protected_map) =
            crate::core::usecases::glossary_replacement_engine::GlossaryEngine::apply_pre_override(
                &ocr_text_regex,
                &glossary_entries,
            );

        // Extract active glossary metadata for injection into AI Guidance prompt
        let active_glossary_entries =
            crate::core::usecases::glossary_replacement_engine::GlossaryEngine::filter_active_entries(
                &ocr_text,
                &glossary_entries,
            );
        let glossary_guidance_str =
            crate::core::usecases::glossary_replacement_engine::GlossaryEngine::build_glossary_guidance(
                &active_glossary_entries,
            );

        // Helper to map block-level translations back to line-level `trans_lines`
        let txt_proc_cfg_inner = txt_proc_cfg.clone();
        let regex_rules_inner = regex_rules.clone();
        let glossary_entries_inner = Arc::new(glossary_entries.clone());
        let gloss_protected_map_inner = Arc::new(gloss_protected_map);
        let regex_protected_map_inner = Arc::new(regex_protected_map);
        let platform_inner = platform.clone();
        let target_lang_inner = target_lang.clone();
        let th_segmentation_inner = th_segmentation;

        let build_trans_lines = move |translated: &str| -> Vec<String> {
            // Decode Glossary protected masks first
            let decoded_gloss =
                crate::core::usecases::glossary_replacement_engine::GlossaryEngine::apply_post_override(
                    translated,
                    &gloss_protected_map_inner,
                );

            // Apply Regex Post rules and decode masks
            let mut post_trans =
                crate::core::usecases::regex_replacement_engine::RegexEngine::apply_post_rules(
                    &decoded_gloss,
                    &regex_rules_inner,
                    &regex_protected_map_inner,
                );

            // Forcefully apply any missed Glossary entries (like CharacterName, Terms) on the final text
            // to serve as a 100% reliable post-translation enforcer.
            post_trans = crate::core::usecases::glossary_replacement_engine::GlossaryEngine::apply_post_glossary_overrides(&post_trans, &glossary_entries_inner);

            let mut block_translations =
                Self::parse_numbered_lines(&post_trans, blocks.len(), &txt_proc_cfg_inner);

            // --- Thai Word Segmentation ---
            // Apply segmentation to each individual block only if target is Thai
            if target_lang_inner.0 == "th" || target_lang_inner.0 == "Thai" {
                for text in block_translations.iter_mut() {
                    *text = platform_inner.segment_thai(text, th_segmentation_inner);
                }
            }
            let mut trans_lines = Vec::new();
            for (block_idx, size) in block_sizes.iter().enumerate() {
                trans_lines.push(
                    block_translations
                        .get(block_idx)
                        .cloned()
                        .unwrap_or_default(),
                );
                for _ in 1..*size {
                    trans_lines.push(String::new());
                }
            }
            trans_lines
        };

        if ocr_text.is_empty() {
            return Ok(BgResult::Done {
                slot_idx,
                language_version,
                ocr_text: String::new(),
                translated: String::new(),
                frame_hash: hash,
                ocr_lines: Vec::new(),
                trans_lines: Vec::new(),
                yolo_bubbles: yolo_bubbles.clone(),
            });
        }

        // --- Aggressive Text-Level Cache Check ---
        {
            let text_hash = crate::core::utils::fnv_hash_str(&ocr_text);
            let tc_key = (
                text_hash,
                source_lang.as_ref().map(|l| l.0.clone()),
                target_lang.0.clone(),
            );
            let cached = {
                let tc = text_cache_arc.lock();
                tc.get(&tc_key).cloned()
            };

            if let Some(cached_trans) = cached {
                let mut fc = cache_arc.lock();
                crate::core::utils::enforce_cache_limit(&mut fc, max_cache_entries);
                fc.insert(cache_key, (ocr_text.clone(), cached_trans.clone()));

                let trans_lines = build_trans_lines(&cached_trans);
                return Ok(BgResult::Done {
                    slot_idx,
                    language_version,
                    ocr_text,
                    translated: cached_trans,
                    frame_hash: hash,
                    ocr_lines,
                    trans_lines,
                    yolo_bubbles: yolo_bubbles.clone(),
                });
            }
        }

        if source_lang
            .as_ref()
            .map(|s| s.0 == target_lang.0)
            .unwrap_or(false)
        {
            let mut cache = cache_arc.lock();
            crate::core::utils::enforce_cache_limit(&mut cache, max_cache_entries);
            cache.insert(cache_key, (ocr_text.clone(), ocr_text.clone()));
            let trans_lines = build_trans_lines(&ocr_text);
            return Ok(BgResult::Done {
                slot_idx,
                language_version,
                ocr_text: ocr_text.clone(),
                translated: ocr_text,
                frame_hash: hash,
                ocr_lines,
                trans_lines,
                yolo_bubbles: yolo_bubbles.clone(),
            });
        }

        // --- Pre-Translation Validation Heuristics ---
        // Bypass costly AI translation calls for trivial raw outputs (pure numbers, whitespace, simple icons)
        let is_trivial = ocr_text.chars().all(|c| {
            c.is_ascii_digit() || c.is_ascii_punctuation() || c.is_whitespace() || c == '…'
        });
        if is_trivial {
            let mut cache = cache_arc.lock();
            crate::core::utils::enforce_cache_limit(&mut cache, max_cache_entries);
            cache.insert(cache_key, (ocr_text.clone(), ocr_text.clone()));
            let trans_lines = build_trans_lines(&ocr_text);
            return Ok(BgResult::Done {
                slot_idx,
                language_version,
                ocr_text: ocr_text.clone(),
                translated: ocr_text,
                frame_hash: hash,
                ocr_lines,
                trans_lines,
                yolo_bubbles: yolo_bubbles.clone(),
            });
        }

        let _ = status_tx.send(BgResult::StatusUpdate {
            slot_idx,
            status: "AI Translating...".to_string(),
        });
        ctx.request_repaint();

        // Inject Glossary guidance directly to the translated input stream seamlessly
        let mut text_to_translate = ocr_text.clone();
        if !glossary_guidance_str.is_empty() {
            text_to_translate.push_str(&format!(
                "\n\n[MANDATORY_GLOSSARY_TERMS:\n{}]",
                glossary_guidance_str
            ));
        }
        let context_hint = if contextual_translation {
            crate::core::usecases::translation_runner_usecase::build_context_hint(
                &context_segments,
                context_window_size,
            )
        } else {
            None
        };
        let context_hint_ref = context_hint.as_deref();
        let translator_output = crate::core::usecases::translation_runner_usecase::translate_text(
            translator
                .as_deref()
                .context("No translator provider selected")?,
            &text_to_translate,
            source_lang.as_ref(),
            &target_lang,
            enable_batching,
            context_hint_ref,
        )?;

        // --- Thai Word Segmentation ---
        // If target is Thai, use platform segmenter to insert spaces for proper wrapping.
        let raw_translated = translator_output;

        let trans_lines = build_trans_lines(&raw_translated);
        let translated = trans_lines
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        crate::core::usecases::pipeline_cache::update_cache(
            &cache_arc,
            &text_cache_arc,
            cache_key,
            (
                crate::core::utils::fnv_hash_str(&ocr_text),
                source_lang.as_ref().map(|l| l.0.clone()),
                target_lang.0.clone(),
            ),
            ocr_text.clone(),
            translated.clone(),
            max_cache_entries,
        );
        Ok(BgResult::Done {
            slot_idx,
            language_version,
            ocr_text,
            translated,
            frame_hash: hash,
            ocr_lines,
            trans_lines,
            yolo_bubbles,
        })
    }

    fn parse_numbered_lines(
        raw: &str,
        ocr_count: usize,
        config: &crate::infrastructure::settings::TextProcessingSettings,
    ) -> Vec<String> {
        let mut result =
            crate::core::llm_prompt_builder::parse_translation_response(raw, ocr_count);
        for s in result.iter_mut() {
            *s = TextCleaner::clean(s, config);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[test]
    fn test_enforce_cache_limit_under_limit() {
        let cache = Arc::new(Mutex::new(IndexMap::new()));
        {
            let mut guard = cache.lock();
            guard.insert("key1", "val1");
            guard.insert("key2", "val2");
            crate::core::utils::enforce_cache_limit(&mut guard, 5);
        }
        let guard = cache.lock();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard.get("key1"), Some(&"val1"));
        assert_eq!(guard.get("key2"), Some(&"val2"));
    }

    #[test]
    fn test_enforce_cache_limit_fifo_eviction() {
        let cache = Arc::new(Mutex::new(IndexMap::new()));
        {
            let mut guard = cache.lock();
            // Insert 5 elements
            guard.insert("key1", "val1"); // Oldest
            guard.insert("key2", "val2");
            guard.insert("key3", "val3");
            guard.insert("key4", "val4");
            guard.insert("key5", "val5"); // Newest

            // Enforce limit of 3. It should evict 5 - 3 + 1 = 3 entries (key1, key2, key3).
            crate::core::utils::enforce_cache_limit(&mut guard, 3);
        }

        let guard = cache.lock();
        assert_eq!(guard.len(), 2); // 3 entries evicted, leaving 2

        // key1, key2, key3 should be evicted
        assert_eq!(guard.get("key1"), None);
        assert_eq!(guard.get("key2"), None);
        assert_eq!(guard.get("key3"), None);

        // key4, key5 should remain
        assert_eq!(guard.get("key4"), Some(&"val4"));
        assert_eq!(guard.get("key5"), Some(&"val5"));
    }
}
