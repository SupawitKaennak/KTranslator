use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;

use crate::core::{
    ports::{FrameRgba, OcrEngine, OcrTextLine},
    types::LanguageTag,
};
use crate::infrastructure::settings::PpocrModelSuite;

/// Built-in PaddleOCR adapter using oar-ocr ONNX pipeline.
/// Unlike PaddleOcr (subprocess), this runs the detection+recognition models
/// directly in-process via ONNX Runtime, eliminating IPC overhead (~125ms).
pub struct BuiltinPaddleOcr {
    pipeline: Arc<Mutex<Option<(oar_ocr::oarocr::OAROCR, PpocrModelSuite)>>>,
    models_dir: String,
    model_suite: Arc<Mutex<PpocrModelSuite>>,
}

impl BuiltinPaddleOcr {
    pub fn new(models_dir: String, model_suite: PpocrModelSuite) -> Self {
        Self {
            pipeline: Arc::new(Mutex::new(None)),
            models_dir,
            model_suite: Arc::new(Mutex::new(model_suite)),
        }
    }

    /// Update the model suite when settings change (called from factory).
    #[allow(dead_code)]
    pub fn set_model_suite(&self, suite: PpocrModelSuite) {
        *self.model_suite.lock() = suite;
    }

    fn ensure_pipeline(&self) -> Result<()> {
        let mut guard = self.pipeline.lock();
        let current_suite = *self.model_suite.lock();

        if let Some((_, active_suite)) = guard.as_ref() {
            if *active_suite == current_suite {
                return Ok(());
            }
            tracing::info!(
                "Active PaddleOCR model suite changed from {:?} to {:?}. Invalidating cached pipeline...",
                active_suite, current_suite
            );
            *guard = None; // Invalidate the cached pipeline to force reload
        }
        
        let folder_name = current_suite.folder_name();

        let target_suite_dir = Path::new(&self.models_dir).join(folder_name);
        
        // Robust per-file resolver targeting the specific combination subfolder
        let resolve_file = |file_name: &str| -> PathBuf {
            let p = target_suite_dir.join(file_name);
            if p.exists() { return p; }
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    let target_p = dir.join(&target_suite_dir).join(file_name);
                    if target_p.exists() { return target_p; }
                }
            }
            target_suite_dir.join(file_name)
        };

        let det_path = resolve_file("det.onnx");
        let rec_path = resolve_file("rec.onnx");
        let dict_path = resolve_file("dict.txt");

        if !det_path.exists() {
            return Err(anyhow!(
                "PP-OCR detection model not found at {:?}\n\
                 Please place the required models in '{}' or use the Download button in settings.",
                det_path, self.models_dir
            ));
        }
        if !rec_path.exists() {
            return Err(anyhow!("PP-OCR recognition model not found at {:?}", rec_path));
        }
        if !dict_path.exists() {
            return Err(anyhow!("PP-OCR dictionary file not found at {:?}", dict_path));
        }

        tracing::info!("Initializing Built-in PaddleOCR pipeline from {:?}", det_path.parent());

        let det_str = det_path.to_string_lossy().to_string();
        let rec_str = rec_path.to_string_lossy().to_string();
        let dict_str = dict_path.to_string_lossy().to_string();

        let pipeline = oar_ocr::oarocr::OAROCRBuilder::new(&det_str, &rec_str, &dict_str)
            .build()
            .context("Failed to initialize Built-in PaddleOCR pipeline")?;

        *guard = Some((pipeline, current_suite));
        tracing::info!("Built-in PaddleOCR pipeline ready (GPU accelerated via DirectML) for {:?}", current_suite);
        Ok(())
    }
}

impl OcrEngine for BuiltinPaddleOcr {
    fn recognize(&self, frame: FrameRgba, lang_hint: Option<&LanguageTag>) -> Result<String> {
        let lines = self.recognize_lines(frame, lang_hint)?;
        Ok(lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n"))
    }

    fn recognize_lines(&self, frame: FrameRgba, _lang_hint: Option<&LanguageTag>) -> Result<Vec<OcrTextLine>> {
        self.ensure_pipeline()?;

        let start_time = std::time::Instant::now();

        // Convert RGBA frame directly to RGB drop alpha channel blazingly fast in a single linear vector pass
        let src_data = &frame.data;
        let pixel_count = (frame.width * frame.height) as usize;
        let mut rgb_data = Vec::with_capacity(pixel_count * 3);
        
        for chunk in src_data.chunks_exact(4) {
            rgb_data.push(chunk[0]);
            rgb_data.push(chunk[1]);
            rgb_data.push(chunk[2]);
        }
        
        let rgb_img = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_raw(
            frame.width, frame.height, rgb_data,
        ).context("Failed to construct high-speed RGB buffer")?;

        let prep_duration = start_time.elapsed();
        let infer_start = std::time::Instant::now();

        // Run OCR pipeline — predict() takes Vec<ImageBuffer<Rgb<u8>>>
        let guard = self.pipeline.lock();
        let (pipeline, _) = guard.as_ref().context("BuiltinPaddleOcr pipeline not initialized")?;
        let results = pipeline.predict(vec![rgb_img])
            .map_err(|e| anyhow!("Built-in PaddleOCR inference failed: {}", e))?;

        let infer_duration = infer_start.elapsed();

        let mut out = Vec::new();

        for page_result in &results {
            for region in &page_result.text_regions {
                // text is Option<Arc<str>>
                let text = match &region.text {
                    Some(t) if !t.trim().is_empty() => t.trim().to_string(),
                    _ => continue,
                };

                // Extract bounding box using BoundingBox helper methods
                let bbox = &region.bounding_box;
                let min_x = bbox.x_min();
                let min_y = bbox.y_min();
                let max_x = bbox.x_max();
                let max_y = bbox.y_max();

                out.push(OcrTextLine {
                    text,
                    x: min_x,
                    y: min_y,
                    w: (max_x - min_x).max(1.0),
                    h: (max_y - min_y).max(1.0),
                });
            }
        }

        let total_duration = start_time.elapsed();
        tracing::info!(
            "Built-in PaddleOCR processed frame: {} lines found. [Prep: {:.1}ms, Inference: {:.1}ms, Total: {:.1}ms]",
            out.len(),
            prep_duration.as_secs_f64() * 1000.0,
            infer_duration.as_secs_f64() * 1000.0,
            total_duration.as_secs_f64() * 1000.0
        );

        Ok(out)
    }
}
