use std::sync::{mpsc, Arc};
use std::collections::HashMap;
use anyhow::Context;
use parking_lot::Mutex;
use eframe::egui;

use crate::core::{
    ports::{FrameSource, OcrEngine, Translator, FrameRgba, OcrTextLine},
    types::{LanguageTag, Rect},
    worker::{BgResult, smart_hash},
    text_cleaner::TextCleaner,
};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
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
    pub yolo_detector: Option<Arc<crate::adapters::ocr::yolo_bubble_detector::YoloBubbleDetector>>,

    // --- Hash/stability state ---
    pub prev_hash: u64,
    pub stable_hash: u64,
    pub stable_since_ms: u64,
    pub first_unstable_at: u64,

    // --- Caches ---
    pub cache_arc: Arc<Mutex<HashMap<(u64, Option<String>, String), (String, String)>>>,
    pub text_cache_arc: Arc<Mutex<HashMap<(u64, Option<String>, String), String>>>,
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
            slot_idx, rect, display_id, source_lang, target_lang, language_version,
            capture, ocr_engine, translator, platform, yolo_detector,
            prev_hash, stable_hash, stable_since_ms, first_unstable_at,
            cache_arc, text_cache_arc, max_cache_entries,
            smart_merge, img_proc_cfg, txt_proc_cfg, regex_rules, glossary_entries,
            jp_merge_vertical, th_segmentation, enable_batching,
            context_segments, contextual_translation, context_window_size,
            last_frame_arc, status_tx, ctx,
        } = p;
        let frame = capture.capture_rect(rect, display_id)?;
        
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
        let force_proceed = unstable_since_start > 800; 

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

        // 3. Stable Debounce check: Reduced from 400ms to 150ms for hyper-reactive response
        if unstable_dur < 150 && !force_proceed {
            return Ok(BgResult::WaitingDebounce { slot_idx });
        }

        // Check Cache
        {
            let cache = cache_arc.lock();
            if let Some((cached_ocr, cached_trans)) = cache.get(&cache_key) {
                tracing::debug!(slot = slot_idx, hash = %hash, "Cache hit for whole frame");
                return Ok(BgResult::CacheHit {
                    slot_idx, language_version, ocr_text: cached_ocr.clone(), translated: cached_trans.clone(), frame_hash: hash,
                    ocr_lines: Vec::new(), trans_lines: vec![cached_trans.clone()], yolo_bubbles: Vec::new()
                });
            }
        }

        tracing::info!(slot = slot_idx, "Proceeding with OCR/Translation");
        let _ = status_tx.send(BgResult::StatusUpdate { slot_idx, status: "Scanning Text...".to_string() });
        ctx.request_repaint();

        let mut yolo_bubbles = Vec::new();
        let mut raw_ocr_lines = Vec::new();
        let mut bubble_detection_successful = false;

        if let Some(ref detector) = yolo_detector {
            if let Some(rgba_img) = image::RgbaImage::from_raw(frame.width, frame.height, frame.data.clone()) {
                let dynamic_img = image::DynamicImage::ImageRgba8(rgba_img);
                if let Ok(mut bubbles) = detector.detect_bubbles(&dynamic_img) {
                    if !bubbles.is_empty() {
                        // Sort bubbles in natural reading order (Right-to-Left for CJK, Left-to-Right otherwise)
                        bubbles.sort_by(|a, b| {
                            let a_h = a.y2 - a.y1;
                            let b_h = b.y2 - b.y1;
                            let tolerance = a_h.min(b_h) * 0.4;
                            let y_diff = (a.y1 - b.y1).abs();

                            if y_diff > tolerance {
                                a.y1.partial_cmp(&b.y1).unwrap_or(std::cmp::Ordering::Equal)
                            } else {
                                if jp_merge_vertical {
                                    b.x1.partial_cmp(&a.x1).unwrap_or(std::cmp::Ordering::Equal)
                                } else {
                                    a.x1.partial_cmp(&b.x1).unwrap_or(std::cmp::Ordering::Equal)
                                }
                            }
                        });

                        bubble_detection_successful = true;
                        for b in &bubbles {
                            yolo_bubbles.push(OcrTextLine {
                                text: String::new(),
                                x: b.x1,
                                y: b.y1,
                                w: b.x2 - b.x1,
                                h: b.y2 - b.y1,
                            });

                            // Add a small 6px padding to prevent boundaries clipping
                            let pad = 6;
                            let crop_x = (b.x1 - pad as f32).max(0.0) as u32;
                            let crop_y = (b.y1 - pad as f32).max(0.0) as u32;
                            let crop_w = ((b.x2 + pad as f32).min(frame.width as f32) as u32).saturating_sub(crop_x);
                            let crop_h = ((b.y2 + pad as f32).min(frame.height as f32) as u32).saturating_sub(crop_y);

                            if crop_w >= 5 && crop_h >= 5 {
                                let cropped_frame = crop_frame(&frame, crop_x, crop_y, crop_w, crop_h);
                                
                                // Perform full image pre-processing on the cropped speech bubble
                                let (proc_data, proc_w, proc_h) = crate::core::usecases::image_processor::process_image_for_ocr(
                                    &cropped_frame.data, cropped_frame.width, cropped_frame.height, &img_proc_cfg
                                );
                                let mut processed_crop = cropped_frame.clone();
                                processed_crop.data = proc_data;
                                processed_crop.width = proc_w;
                                processed_crop.height = proc_h;

                                if let Ok(mut lines) = ocr_engine.recognize_lines(processed_crop, source_lang.as_ref()) {
                                    let scale = img_proc_cfg.resize_scale;
                                    for line in &mut lines {
                                        if (scale - 1.0).abs() > 0.01 {
                                            line.x /= scale;
                                            line.y /= scale;
                                            line.w /= scale;
                                            line.h /= scale;
                                        }
                                        line.x += crop_x as f32;
                                        line.y += crop_y as f32;
                                        raw_ocr_lines.push(line.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if !bubble_detection_successful {
            // Fallback: Perform Image pre-processing IN-PLACE on frame
            let (proc_data, proc_w, proc_h) = crate::core::usecases::image_processor::process_image_for_ocr(
                &frame.data, frame.width, frame.height, &img_proc_cfg
            );
            let mut processed_frame = frame.clone();
            processed_frame.data = proc_data;
            processed_frame.width = proc_w;
            processed_frame.height = proc_h;

            if let Ok(mut lines) = ocr_engine.recognize_lines(processed_frame, source_lang.as_ref()) {
                if (img_proc_cfg.resize_scale - 1.0).abs() > 0.01 {
                    let scale = img_proc_cfg.resize_scale;
                    for line in &mut lines {
                        line.x /= scale;
                        line.y /= scale;
                        line.w /= scale;
                        line.h /= scale;
                    }
                }
                raw_ocr_lines = lines;
            }
        }

        // Line-level Garbage Filtering
        raw_ocr_lines.retain(|l| TextCleaner::is_line_valid(&l.text, &txt_proc_cfg));
        
        let blocks = crate::core::layout::build_blocks(raw_ocr_lines, smart_merge, jp_merge_vertical);
        
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
                slot_idx, language_version, ocr_text: ocr_text_base, translated: memory_trans, frame_hash: hash, ocr_lines, trans_lines, yolo_bubbles: yolo_bubbles.clone()
            });
        }
        
        // 1. Apply Pre-Translation Regex Engine Rules FIRST
        // This ensures space-separated words like "HA NAZO NO" are merged to "HANAZONO" BEFORE glossary looks for it.
        let (ocr_text_regex, regex_protected_map) = crate::core::usecases::regex_engine::RegexEngine::apply_pre_rules(&ocr_text_base, &regex_rules);

        // 2. Apply Glossary Engine Overrides and Protected words SECOND
        // Now Glossary will successfully catch "HANAZONO" and apply protection or guidance correctly!
        let (ocr_text, gloss_protected_map) = crate::core::usecases::glossary_engine::GlossaryEngine::apply_pre_override(&ocr_text_regex, &glossary_entries);

        // Extract active glossary metadata for injection into AI Guidance prompt
        let active_glossary_entries = crate::core::usecases::glossary_engine::GlossaryEngine::filter_active_entries(&ocr_text, &glossary_entries);
        let glossary_guidance_str = crate::core::usecases::glossary_engine::GlossaryEngine::build_glossary_guidance(&active_glossary_entries);

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
            let decoded_gloss = crate::core::usecases::glossary_engine::GlossaryEngine::apply_post_override(translated, &gloss_protected_map_inner);
            
            // Apply Regex Post rules and decode masks
            let mut post_trans = crate::core::usecases::regex_engine::RegexEngine::apply_post_rules(&decoded_gloss, &regex_rules_inner, &regex_protected_map_inner);
            
            // Forcefully apply any missed Glossary entries (like CharacterName, Terms) on the final text
            // to serve as a 100% reliable post-translation enforcer.
            post_trans = crate::core::usecases::glossary_engine::GlossaryEngine::apply_post_glossary_overrides(&post_trans, &glossary_entries_inner);

            let mut block_translations = Self::parse_numbered_lines(&post_trans, blocks.len(), &txt_proc_cfg_inner);
            
            // --- Thai Word Segmentation ---
            // Apply segmentation to each individual block only if target is Thai
            if target_lang_inner.0 == "th" || target_lang_inner.0 == "Thai" {
                for text in block_translations.iter_mut() {
                    *text = platform_inner.segment_thai(text, th_segmentation_inner);
                }
            }
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
                slot_idx, language_version, ocr_text: String::new(), translated: String::new(), frame_hash: hash, ocr_lines: Vec::new(), trans_lines: Vec::new(), yolo_bubbles: yolo_bubbles.clone(),
            });
        }

        // --- Aggressive Text-Level Cache Check ---
        {
            let text_hash = crate::core::utils::fnv_hash_str(&ocr_text);
            let tc_key = (text_hash, source_lang.as_ref().map(|l| l.0.clone()), target_lang.0.clone());
            let cached = { let tc = text_cache_arc.lock(); tc.get(&tc_key).cloned() };

            if let Some(cached_trans) = cached {
                let mut fc = cache_arc.lock();
                enforce_cache_limit(&mut fc, max_cache_entries);
                fc.insert(cache_key, (ocr_text.clone(), cached_trans.clone()));

                let trans_lines = build_trans_lines(&cached_trans);
                return Ok(BgResult::Done {
                    slot_idx, language_version, ocr_text, translated: cached_trans, frame_hash: hash, ocr_lines, trans_lines, yolo_bubbles: yolo_bubbles.clone()
                });
            }
        }

        if source_lang.as_ref().map(|s| s.0 == target_lang.0).unwrap_or(false) {
            let mut cache = cache_arc.lock();
            enforce_cache_limit(&mut cache, max_cache_entries);
            cache.insert(cache_key, (ocr_text.clone(), ocr_text.clone()));
            let trans_lines = build_trans_lines(&ocr_text);
            return Ok(BgResult::Done {
                slot_idx, language_version, ocr_text: ocr_text.clone(), translated: ocr_text, frame_hash: hash, ocr_lines, trans_lines, yolo_bubbles: yolo_bubbles.clone()
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
                slot_idx, language_version, ocr_text: ocr_text.clone(), translated: ocr_text, frame_hash: hash, ocr_lines, trans_lines, yolo_bubbles: yolo_bubbles.clone()
            });
        }

        let _ = status_tx.send(BgResult::StatusUpdate { slot_idx, status: "AI Translating...".to_string() });
        ctx.request_repaint();

        // Inject Glossary guidance directly to the translated input stream seamlessly
        let mut text_to_translate = ocr_text.clone();
        if !glossary_guidance_str.is_empty() {
            text_to_translate.push_str(&format!("\n\n[MANDATORY_GLOSSARY_TERMS:\n{}]", glossary_guidance_str));
        }
        let context_hint = if contextual_translation {
            crate::core::usecases::translation_runner::build_context_hint(
                &context_segments,
                context_window_size,
            )
        } else {
            None
        };
        let context_hint_ref = context_hint.as_deref();
        let translator_output = crate::core::usecases::translation_runner::translate_text(
            translator.as_deref().context("No translator provider selected")?,
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
        let translated = trans_lines.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join("\n");

        {
            let text_hash = crate::core::utils::fnv_hash_str(&ocr_text);
            let text_cache_key = (text_hash, source_lang.as_ref().map(|l| l.0.clone()), target_lang.0.clone());
            let mut frame_cache = cache_arc.lock();
            enforce_cache_limit(&mut frame_cache, max_cache_entries);
            frame_cache.insert(cache_key, (ocr_text.clone(), translated.clone()));
            drop(frame_cache);
            let mut text_cache = text_cache_arc.lock();
            enforce_cache_limit(&mut text_cache, max_cache_entries);
            text_cache.insert(text_cache_key, translated.clone());
        }
        Ok(BgResult::Done { slot_idx, language_version, ocr_text, translated, frame_hash: hash, ocr_lines, trans_lines, yolo_bubbles })
    }

    fn parse_numbered_lines(raw: &str, ocr_count: usize, config: &crate::infrastructure::settings::TextProcessingSettings) -> Vec<String> {
        let mut result = crate::core::prompt_builder::parse_translation_response(raw, ocr_count);
        for s in result.iter_mut() {
            *s = TextCleaner::clean(s, config);
        }
        result
    }
}

fn crop_frame(frame: &FrameRgba, x: u32, y: u32, w: u32, h: u32) -> FrameRgba {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for row in y..(y + h) {
        let src_idx = (row * frame.width + x) as usize * 4;
        let length = (w * 4) as usize;
        if src_idx + length <= frame.data.len() {
            data.extend_from_slice(&frame.data[src_idx..(src_idx + length)]);
        } else {
            data.resize(data.len() + length, 0);
        }
    }
    FrameRgba {
        width: w,
        height: h,
        data,
    }
}
