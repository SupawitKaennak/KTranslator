use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;

use crate::core::{
    ports::{FrameRgba, OcrEngine, OcrTextLine},
    types::LanguageTag,
};

/// Built-in PaddleOCR adapter using oar-ocr ONNX pipeline.
/// Unlike PaddleOcr (subprocess), this runs the detection+recognition models
/// directly in-process via ONNX Runtime, eliminating IPC overhead (~125ms).
pub struct BuiltinPaddleOcr {
    pipeline: Arc<Mutex<Option<oar_ocr::oarocr::OAROCR>>>,
    models_dir: String,
}

impl BuiltinPaddleOcr {
    pub fn new(models_dir: String) -> Self {
        Self {
            pipeline: Arc::new(Mutex::new(None)),
            models_dir,
        }
    }

    fn ensure_pipeline(&self) -> Result<()> {
        let mut guard = self.pipeline.lock();
        if guard.is_some() {
            return Ok(());
        }

        let dir = Path::new(&self.models_dir);

        // Try relative to CWD first, then relative to EXE
        let resolved = if dir.exists() {
            dir.to_path_buf()
        } else if let Ok(exe) = std::env::current_exe() {
            let exe_rel = exe.parent().unwrap_or(Path::new(".")).join(dir);
            if exe_rel.exists() { exe_rel } else { dir.to_path_buf() }
        } else {
            dir.to_path_buf()
        };

        let det_path = resolved.join("det.onnx");
        let rec_path = resolved.join("rec.onnx");
        let dict_path = resolved.join("dict.txt");

        if !det_path.exists() {
            return Err(anyhow!(
                "PP-OCR detection model not found at {:?}\n\
                 Please place det.onnx, rec.onnx, and dict.txt in the '{}' folder.\n\
                 Download from: https://github.com/PaddlePaddle/PaddleOCR",
                det_path, self.models_dir
            ));
        }
        if !rec_path.exists() {
            return Err(anyhow!("PP-OCR recognition model not found at {:?}", rec_path));
        }
        if !dict_path.exists() {
            return Err(anyhow!("PP-OCR dictionary file not found at {:?}", dict_path));
        }

        tracing::info!("Initializing Built-in PaddleOCR pipeline from {:?}", resolved);

        let det_str = det_path.to_string_lossy().to_string();
        let rec_str = rec_path.to_string_lossy().to_string();
        let dict_str = dict_path.to_string_lossy().to_string();

        let pipeline = oar_ocr::oarocr::OAROCRBuilder::new(&det_str, &rec_str, &dict_str)
            .build()
            .context("Failed to initialize Built-in PaddleOCR pipeline")?;

        *guard = Some(pipeline);
        tracing::info!("Built-in PaddleOCR pipeline ready (GPU accelerated via DirectML)");
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

        // Convert RGBA frame to RGB ImageBuffer (oar-ocr expects Rgb<u8>)
        let img_buf = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
            frame.width, frame.height, frame.data,
        ).context("Failed to create image buffer from frame")?;
        let rgb_img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> = image::DynamicImage::ImageRgba8(img_buf).to_rgb8();

        // Run OCR pipeline — predict() takes Vec<ImageBuffer<Rgb<u8>>>
        let guard = self.pipeline.lock();
        let pipeline = guard.as_ref().unwrap();
        let results = pipeline.predict(vec![rgb_img])
            .map_err(|e| anyhow!("Built-in PaddleOCR inference failed: {}", e))?;

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

        Ok(out)
    }
}
