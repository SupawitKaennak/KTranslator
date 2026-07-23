use image::{ImageBuffer, Rgba};
use std::future::IntoFuture;
use std::sync::Arc;
use windows::Globalization::Language;
use windows::Media::Ocr::OcrEngine;

use crate::core::{
    ports::{FrameRgba, OcrEngine as OcrEngineTrait, OcrTextLine},
    types::LanguageTag,
};

use parking_lot::Mutex;
use std::collections::HashMap;

pub struct WindowsOcr {
    /// Maps the resolved final language tag → cached OcrEngine
    engines: Mutex<HashMap<String, Arc<OcrEngine>>>,
    /// Maps requested tag (from user settings) → final resolved tag
    /// Avoids re-enumerating AvailableRecognizerLanguages() on every call.
    tag_cache: Mutex<HashMap<String, String>>,
}

impl WindowsOcr {
    pub fn new() -> Self {
        Self {
            engines: Mutex::new(HashMap::new()),
            tag_cache: Mutex::new(HashMap::new()),
        }
    }

    fn get_engine(
        &self,
        lang_tag: Option<&LanguageTag>,
    ) -> anyhow::Result<Arc<OcrEngine>> {
        let is_auto = lang_tag.is_none();
        let requested_tag = if let Some(t) = lang_tag {
            t.0.as_str().to_lowercase()
        } else {
            Language::CurrentInputMethodLanguageTag()
                .map(|h| h.to_string().to_lowercase())
                .unwrap_or_else(|_| "en-us".to_string())
        };

        // Fast path: if we already resolved this tag, skip the expensive enumeration.
        {
            let tag_cache = self.tag_cache.lock();
            if let Some(cached_final) = tag_cache.get(&requested_tag) {
                let engines = self.engines.lock();
                if let Some(engine) = engines.get(cached_final) {
                    return Ok(engine.clone());
                }
            }
        }

        tracing::info!(
            "WindowsOCR requested language: {} (auto={}) — resolving for first time",
            requested_tag,
            is_auto
        );

        let available_langs = OcrEngine::AvailableRecognizerLanguages()
            .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?;

        // Log all available languages for debugging (only on first resolve)
        let mut all_tags = Vec::new();
        for lang in &available_langs {
            if let Ok(tag) = lang.LanguageTag() {
                all_tags.push(tag.to_string().to_lowercase());
            }
        }
        tracing::info!("WindowsOCR available languages in system: {:?}", all_tags);

        let mut best_match = None;

        // 1. Exact match check
        for lang in &available_langs {
            if let Ok(tag) = lang.LanguageTag() {
                let tag_str = tag.to_string().to_lowercase();
                if tag_str == requested_tag {
                    best_match = Some(lang.clone());
                    break;
                }
            }
        }

        // 2. Prefix match check (e.g. "ru" matches "ru-RU")
        if best_match.is_none() {
            for lang in &available_langs {
                if let Ok(tag) = lang.LanguageTag() {
                    let tag_str = tag.to_string().to_lowercase();
                    if tag_str.starts_with(&requested_tag) || requested_tag.starts_with(&tag_str) {
                        best_match = Some(lang.clone());
                        break;
                    }
                }
            }
        }

        // 3. Fallback logic
        let final_lang = match best_match {
            Some(l) => l,
            None => {
                if is_auto {
                    // If Auto Detect failed to find the input language, try English or first available
                    tracing::warn!(
                        "Auto detect could not find {}, trying English or first available",
                        requested_tag
                    );
                    let mut fallback = None;
                    for lang in &available_langs {
                        if let Ok(tag) = lang.LanguageTag() {
                            let tag_str = tag.to_string().to_lowercase();
                            if tag_str.starts_with("en") {
                                fallback = Some(lang.clone());
                                break;
                            }
                        }
                    }

                    if let Some(f) = fallback {
                        f
                    } else if let Some(first) = available_langs
                        .First()
                        .ok()
                        .and_then(|i| i.into_iter().next())
                    {
                        first
                    } else {
                        return Err(anyhow::anyhow!("No Windows OCR languages installed. Please install a Language Pack in Windows Settings.".to_string()));
                    }
                } else {
                    // Manual selection failed - this is a hard error
                    let available = all_tags.join(", ");
                    return Err(anyhow::anyhow!(format!(
                        "Windows OCR does not have the language pack for '{}' installed.\n\nAvailable languages: {}\n\nPlease go to Windows Settings -> Time & Language -> Language & Region and add the language pack for '{}' (ensure OCR/Optical Character Recognition is checked).",
                        requested_tag, available, requested_tag
                    )));
                }
            }
        };

        let final_tag = final_lang
            .LanguageTag()
            .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?
            .to_string();
        tracing::info!("WindowsOCR selected engine language: {}", final_tag);

        let engine = OcrEngine::TryCreateFromLanguage(&final_lang).map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to create Windows OCR engine for {}: {}",
                final_tag, e
            ))
        })?;

        let engine_arc = Arc::new(engine);
        // Store both the resolved engine and the tag mapping for fast future lookups
        self.engines.lock().insert(final_tag.clone(), engine_arc.clone());
        self.tag_cache.lock().insert(requested_tag, final_tag);
        Ok(engine_arc)
    }

    fn preprocess(frame: FrameRgba) -> (FrameRgba, f32) {
        let Some(img) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
            frame.width,
            frame.height,
            (*frame.data).clone(),
        ) else {
            return (
                FrameRgba {
                    width: frame.width,
                    height: frame.height,
                    data: std::sync::Arc::new(Vec::new()),
                },
                1.0,
            );
        };

        let gray_img_base = image::DynamicImage::ImageRgba8(img).into_luma8();

        let (processed_img, final_scale) = if frame.height < 1000 {
            let scale = 3.0;
            let new_w = (frame.width as f32 * scale) as u32;
            let new_h = (frame.height as f32 * scale) as u32;
            let resized = image::imageops::resize(
                &gray_img_base,
                new_w,
                new_h,
                image::imageops::FilterType::Triangle,
            );
            (resized, scale)
        } else {
            (gray_img_base, 1.0)
        };

        let mut final_img = processed_img;
        let mut min_v = 255u8;
        let mut max_v = 0u8;
        let mut sum_v = 0u64;

        for pixel in final_img.pixels() {
            let v = pixel.0[0];
            if v < min_v {
                min_v = v;
            }
            if v > max_v {
                max_v = v;
            }
            sum_v += v as u64;
        }

        let total_pixels = final_img.width() as u64 * final_img.height() as u64;
        let avg_v = if total_pixels > 0 {
            (sum_v / total_pixels) as u8
        } else {
            128
        };
        let should_invert = avg_v < 100;

        if max_v > min_v {
            let range = (max_v - min_v) as f32;
            for pixel in final_img.pixels_mut() {
                let v = pixel.0[0];
                let mut normalized = ((v - min_v) as f32 / range * 255.0) as u8;
                if should_invert {
                    normalized = 255 - normalized;
                }
                pixel.0[0] = normalized;
            }
        }

        let width = final_img.width();
        let height = final_img.height();
        let final_rgba = image::DynamicImage::ImageLuma8(final_img).to_rgba8();

        (
            FrameRgba {
                width,
                height,
                data: std::sync::Arc::new(final_rgba.into_raw()),
            },
            final_scale,
        )
    }

    fn frame_to_bitmap(
        processed: FrameRgba,
    ) -> anyhow::Result<windows::Graphics::Imaging::SoftwareBitmap> {
        use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
        use windows::Storage::Streams::DataWriter;

        let width = processed.width as i32;
        let height = processed.height as i32;
        let raw_data = Arc::try_unwrap(processed.data).unwrap_or_else(|arc| (*arc).clone());

        // Build an IBuffer from the raw pixel bytes via DataWriter.
        // This completely avoids the PNG encode→decode roundtrip (~20–80ms saved per frame).
        let writer = DataWriter::new()
            .map_err(|e| anyhow::anyhow!(format!("DataWriter::new failed: {:?}", e)))?;
        writer
            .WriteBytes(&raw_data)
            .map_err(|e| anyhow::anyhow!(format!("WriteBytes failed: {:?}", e)))?;
        let buffer = writer
            .DetachBuffer()
            .map_err(|e| anyhow::anyhow!(format!("DetachBuffer failed: {:?}", e)))?;

        // Create SoftwareBitmap directly from the raw-pixel IBuffer — no codec overhead!
        let bitmap =
            SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Rgba8, width, height)
                .map_err(|e| anyhow::anyhow!(format!("CreateCopyFromBuffer failed: {:?}", e)))?;

        Ok(bitmap)
    }
}

use std::sync::LazyLock;

// Robust global runtime to handle Windows async calls from any thread (including non-tokio threads)
// Uses current_thread since we only block_on() single WinRT async operations sequentially.
static GLOBAL_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create global OCR tokio runtime")
});

fn wait_for<F: std::future::Future>(f: F) -> F::Output {
    GLOBAL_RUNTIME.block_on(f)
}

impl OcrEngineTrait for WindowsOcr {
    fn recognize(
        &self,
        frame: FrameRgba,
        lang_hint: Option<&LanguageTag>,
    ) -> anyhow::Result<String> {
        let engine = self.get_engine(lang_hint)?;
        let (processed, _) = Self::preprocess(frame);
        let bitmap = Self::frame_to_bitmap(processed)?;

        let result = wait_for(
            engine
                .RecognizeAsync(&bitmap)
                .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?
                .into_future(),
        )
        .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?;
        Ok(result
            .Text()
            .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?
            .to_string())
    }

    fn recognize_lines(
        &self,
        frame: FrameRgba,
        lang_hint: Option<&LanguageTag>,
    ) -> anyhow::Result<Vec<OcrTextLine>> {
        let engine = self.get_engine(lang_hint)?;
        let (processed, scale) = Self::preprocess(frame);
        let bitmap = Self::frame_to_bitmap(processed)?;

        let result = wait_for(
            engine
                .RecognizeAsync(&bitmap)
                .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?
                .into_future(),
        )
        .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?;
        let lines_api = result
            .Lines()
            .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?;

        let mut out_lines = Vec::new();
        for line in lines_api {
            let text = line
                .Text()
                .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?
                .to_string();

            let words = line
                .Words()
                .map_err(|e| anyhow::anyhow!(format!("WinRT error: {:?}", e)))?;
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;

            let mut has_words = false;
            for word in words {
                let rect = word.BoundingRect().map_err(|e| {
                    anyhow::anyhow!(format!("WinRT error: {:?}", e))
                })?;
                min_x = min_x.min(rect.X);
                min_y = min_y.min(rect.Y);
                max_x = max_x.max(rect.X + rect.Width);
                max_y = max_y.max(rect.Y + rect.Height);
                has_words = true;
            }

            if has_words {
                out_lines.push(OcrTextLine {
                    text,
                    x: min_x / scale,
                    y: min_y / scale,
                    w: (max_x - min_x) / scale,
                    h: (max_y - min_y) / scale,
                    bubble_idx: None,
                });
            }
        }

        Ok(out_lines)
    }
}
