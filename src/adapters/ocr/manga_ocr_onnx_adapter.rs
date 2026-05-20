use anyhow::Result;
use ndarray::{Array2, Array4};
use ort::session::Session;
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;
use tokenizers::Tokenizer;

use crate::core::ports::{FrameRgba, OcrEngine, OcrTextLine};
use crate::core::types::LanguageTag;
use crate::infrastructure::settings::GpuBackend;
use std::cmp::Ordering;

use super::non_max_suppression_utils::{nms, DetectionBox};

pub struct OnnxMangaRecognizer {
    encoder: Arc<Mutex<Option<Session>>>,
    decoder: Arc<Mutex<Option<Session>>>,
    yolo: Arc<Mutex<Option<Session>>>,
    tokenizer: Arc<Mutex<Option<Tokenizer>>>,
    models_dir: std::path::PathBuf,
    gpu_backend: GpuBackend,
    decoder_start_token_id: i64,
    eos_token_id: i64,
}

impl OnnxMangaRecognizer {
    pub fn new<P: AsRef<Path>>(models_dir: P, gpu_backend: GpuBackend) -> Self {
        Self {
            encoder: Arc::new(Mutex::new(None)),
            decoder: Arc::new(Mutex::new(None)),
            yolo: Arc::new(Mutex::new(None)),
            tokenizer: Arc::new(Mutex::new(None)),
            models_dir: models_dir.as_ref().to_path_buf(),
            gpu_backend,
            decoder_start_token_id: 2,
            eos_token_id: 3,
        }
    }

    fn ensure_sessions(&self) -> Result<()> {
        let mut enc_guard = self.encoder.lock();
        let mut dec_guard = self.decoder.lock();
        let mut yolo_guard = self.yolo.lock();
        let mut tok_guard = self.tokenizer.lock();

        if enc_guard.is_some() && dec_guard.is_some() && yolo_guard.is_some() && tok_guard.is_some()
        {
            return Ok(());
        }

        let mut resolved_path = self.models_dir.clone();

        // 1. Try relative to CWD
        if !resolved_path.exists() {
            // 2. Try relative to EXE
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let exe_relative = exe_dir.join(&self.models_dir);
                    if exe_relative.exists() {
                        resolved_path = exe_relative;
                    }
                }
            }
        }

        let encoder_path = resolved_path.join("encoder_model.onnx");
        let decoder_path = resolved_path.join("decoder_model.onnx");
        let tokenizer_path = resolved_path.join("tokenizer.json");
        let yolo_path = resolved_path.join("manga109_yolo_s.onnx");

        if !encoder_path.exists()
            || !decoder_path.exists()
            || !tokenizer_path.exists()
            || !yolo_path.exists()
        {
            anyhow::bail!("Manga-OCR models not found. Please ensure the 'models' folder is present at {:?} or next to the .exe", resolved_path);
        }

        let encoder =
            super::onnx_inference_engine::OnnxEngine::create_session(&encoder_path, self.gpu_backend)?;
        let decoder =
            super::onnx_inference_engine::OnnxEngine::create_session(&decoder_path, self.gpu_backend)?;
        let yolo = super::onnx_inference_engine::OnnxEngine::create_session(&yolo_path, self.gpu_backend)?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer Error: {}", e))?;

        *enc_guard = Some(encoder);
        *dec_guard = Some(decoder);
        *yolo_guard = Some(yolo);
        *tok_guard = Some(tokenizer);

        Ok(())
    }

    fn detect_text_boxes(&self, img: &image::DynamicImage) -> Result<Vec<DetectionBox>> {
        let orig_w = img.width() as f32;
        let orig_h = img.height() as f32;

        let target_size = 1024.0;
        let scale = (target_size / orig_w).min(target_size / orig_h);
        let new_w = (orig_w * scale) as u32;
        let new_h = (orig_h * scale) as u32;

        let resized = img
            .resize_exact(new_w, new_h, image::imageops::FilterType::Triangle)
            .to_rgb8();
        let mut input = Array4::<f32>::zeros((1, 3, 1024, 1024));

        input.fill(114.0 / 255.0);

        for (x, y, pixel) in resized.enumerate_pixels() {
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = pixel[c] as f32 / 255.0;
            }
        }

        self.ensure_sessions()?;

        let input_tensor = ort::value::Value::from_array(input)
            .map_err(|e| anyhow::anyhow!("YOLO input error: {}", e))?;

        let mut yolo_guard = self.yolo.lock();
        let yolo = yolo_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("YOLO session not initialized"))?;
        let outputs = yolo
            .run(ort::inputs![input_tensor])
            .map_err(|e| anyhow::anyhow!("YOLO run error: {}", e))?;

        let out = outputs[0]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("YOLO extract error: {}", e))?;

        let view = out.view();
        let shape = view.shape();
        let num_anchors = shape[2];
        let num_classes = shape[1] - 4;
        tracing::debug!(
            "YOLO model: {} anchors, {} classes",
            num_anchors,
            num_classes
        );

        // Determine which class_id represents "text" based on number of classes in the model
        // Manga109 4-class: 0=body, 1=face, 2=frame, 3=text → text is class 3
        // Single-class text detector: text is class 0
        let text_class_id: usize = if num_classes >= 4 { 3 } else { 0 };

        let mut boxes = Vec::new();

        for i in 0..num_anchors {
            let cx = view[[0, 0, i]];
            let cy = view[[0, 1, i]];
            let w = view[[0, 2, i]];
            let h = view[[0, 3, i]];

            let mut max_conf = 0.0;
            let mut max_class = 0;
            for c in 0..num_classes {
                let conf = view[[0, 4 + c, i]];
                if conf > max_conf {
                    max_conf = conf;
                    max_class = c;
                }
            }

            if max_conf > 0.25 {
                let x1 = cx - w / 2.0;
                let y1 = cy - h / 2.0;
                let x2 = cx + w / 2.0;
                let y2 = cy + h / 2.0;

                boxes.push(DetectionBox {
                    x1: x1 / scale,
                    y1: y1 / scale,
                    x2: x2 / scale,
                    y2: y2 / scale,
                    prob: max_conf,
                    class_id: max_class,
                });
            }
        }

        tracing::debug!(
            "YOLO raw boxes before NMS: {}, text_class_id: {}",
            boxes.len(),
            text_class_id
        );

        let mut nms_boxes = nms(boxes, 0.45);
        // Only keep text class boxes with plausible manga text bubble dimensions
        nms_boxes.retain(|b| {
            let box_w = b.x2 - b.x1;
            let box_h = b.y2 - b.y1;
            // Text bubbles in manga are relatively small. Filter out oversized detections.
            let is_bubble_size =
                box_w > 15.0 && box_h > 15.0 && box_w < (orig_w * 0.40) && box_h < (orig_h * 0.50);
            b.class_id == text_class_id && is_bubble_size
        });
        tracing::debug!("YOLO final text boxes: {}", nms_boxes.len());

        Ok(nms_boxes)
    }

    fn recognize_internal(&self, img: &image::DynamicImage) -> Result<String> {
        self.ensure_sessions()?;

        let img = img.resize_exact(224, 224, image::imageops::FilterType::Triangle);
        let rgb = img.to_rgb8();

        let mut input = Array4::<f32>::zeros((1, 3, 224, 224));
        for (x, y, pixel) in rgb.enumerate_pixels() {
            for c in 0..3 {
                let val = (pixel[c] as f32 / 255.0 - 0.5) / 0.5;
                input[[0, c, y as usize, x as usize]] = val;
            }
        }

        let input_tensor = ort::value::Value::from_array(input)
            .map_err(|e| anyhow::anyhow!("Value creation error: {}", e))?;

        let mut encoder_guard = self.encoder.lock();
        let encoder = encoder_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Encoder session not initialized"))?;
        let encoder_outputs = encoder
            .run(ort::inputs![input_tensor])
            .map_err(|e| anyhow::anyhow!("Encoder run error: {}", e))?;

        let last_hidden_state = encoder_outputs[0]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("Encoder extract error: {}", e))?;

        let mut tokens = vec![self.decoder_start_token_id];
        let max_length = 128;

        let mut decoder_guard = self.decoder.lock();
        let decoder = decoder_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Decoder session not initialized"))?;

        for _ in 0..max_length {
            let seq_len = tokens.len();
            let input_ids = Array2::from_shape_vec((1, seq_len), tokens.clone())?;

            let input_ids_tensor = ort::value::Value::from_array(input_ids)
                .map_err(|e| anyhow::anyhow!("Value creation error: {}", e))?;
            let encoder_hidden_tensor = ort::value::Value::from_array(last_hidden_state.to_owned())
                .map_err(|e| anyhow::anyhow!("Value creation error: {}", e))?;

            let decoder_outputs = decoder
                .run(ort::inputs![input_ids_tensor, encoder_hidden_tensor])
                .map_err(|e| anyhow::anyhow!("Decoder run error: {}", e))?;

            let logits = decoder_outputs[0]
                .try_extract_array::<f32>()
                .map_err(|e| anyhow::anyhow!("Decoder extract error: {}", e))?;

            let last_token_logits = logits.slice(ndarray::s![0, seq_len - 1, ..]);
            let mut next_token = 0;
            let mut max_logit = f32::NEG_INFINITY;
            for (idx, &logit) in last_token_logits.iter().enumerate() {
                if logit > max_logit {
                    max_logit = logit;
                    next_token = idx as i64;
                }
            }

            if next_token == self.eos_token_id {
                break;
            }
            tokens.push(next_token);
        }

        let u32_tokens: Vec<u32> = tokens.iter().map(|&t| t as u32).collect();
        let tok_guard = self.tokenizer.lock();
        let tokenizer = tok_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tokenizer not initialized"))?;
        let decoded = tokenizer
            .decode(&u32_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decode error: {}", e))?;

        Ok(decoded)
    }
}

impl OcrEngine for OnnxMangaRecognizer {
    fn recognize(
        &self,
        frame: FrameRgba,
        _lang_hint: Option<&LanguageTag>,
    ) -> Result<String, crate::core::error::KError> {
        let img = image::RgbaImage::from_raw(frame.width, frame.height, (*frame.data).clone())
            .ok_or_else(|| {
                crate::core::error::KError::Ocr("Failed to create image from frame".to_string())
            })?;
        let dynamic_img = image::DynamicImage::ImageRgba8(img);
        self.recognize_internal(&dynamic_img).map_err(|e| {
            crate::core::error::KError::Ocr(format!("Manga109 OCR recognize failed: {:?}", e))
        })
    }

    fn recognize_lines(
        &self,
        frame: FrameRgba,
        _lang_hint: Option<&LanguageTag>,
    ) -> Result<Vec<OcrTextLine>, crate::core::error::KError> {
        let img = image::RgbaImage::from_raw(frame.width, frame.height, (*frame.data).clone())
            .ok_or_else(|| {
                crate::core::error::KError::Ocr("Failed to create image from frame".to_string())
            })?;
        let dynamic_img = image::DynamicImage::ImageRgba8(img);

        let boxes = self.detect_text_boxes(&dynamic_img).map_err(|e| {
            crate::core::error::KError::Ocr(format!(
                "Manga109 OCR text box detection failed: {:?}",
                e
            ))
        })?;

        if boxes.is_empty() {
            return Ok(vec![]);
        }

        let mut result = Vec::new();
        let mut sorted_boxes = boxes;
        // Step 1: Sort by x descending (RTL: rightmost first)
        sorted_boxes.sort_by(|a, b| b.x1.partial_cmp(&a.x1).unwrap_or(Ordering::Equal));

        // Step 2: Group boxes into columns based on x distance
        let mut columns: Vec<Vec<DetectionBox>> = Vec::new();
        for bbox in sorted_boxes {
            let mut added = false;
            // Try to find a column group where this box fits (x-coordinate is within 120px of the column's first element)
            for col in &mut columns {
                if let Some(first_in_col) = col.first() {
                    if (bbox.x1 - first_in_col.x1).abs() <= 120.0 {
                        col.push(bbox.clone());
                        added = true;
                        break;
                    }
                }
            }
            if !added {
                columns.push(vec![bbox]);
            }
        }

        // Step 3: Sort each column top-to-bottom (y ascending)
        for col in &mut columns {
            col.sort_by(|a, b| a.y1.partial_cmp(&b.y1).unwrap_or(Ordering::Equal));
        }

        // Step 4: Flatten columns back to a sorted vector
        let sorted_boxes: Vec<DetectionBox> = columns.into_iter().flatten().collect();

        for bbox in sorted_boxes {
            let pad = 10.0;
            let x1 = (bbox.x1 - pad).max(0.0) as u32;
            let y1 = (bbox.y1 - pad).max(0.0) as u32;
            let x2 = (bbox.x2 + pad).min(frame.width as f32) as u32;
            let y2 = (bbox.y2 + pad).min(frame.height as f32) as u32;

            if x2 <= x1 || y2 <= y1 {
                continue;
            }

            let cropped = dynamic_img.crop_imm(x1, y1, x2 - x1, y2 - y1);
            let text = self.recognize_internal(&cropped).map_err(|e| {
                crate::core::error::KError::Ocr(format!(
                    "Manga109 OCR box text recognize failed: {:?}",
                    e
                ))
            })?;

            let mut cleaned = text.trim().to_string();
            while cleaned.contains("....") {
                cleaned = cleaned.replace("....", "..");
            }
            // Drop dust or single punctuation artifacts recognized from screentones to prevent downstream block-mapping misalignment
            let valid_chars = cleaned
                .chars()
                .filter(|c| c.is_alphanumeric() || (*c as u32) > 0x3000)
                .count();
            if cleaned.len() < 2 || valid_chars == 0 {
                continue;
            }

            let final_w = bbox.x2 - bbox.x1;
            let final_h = bbox.y2 - bbox.y1;
            let final_x = bbox.x1;
            let final_y = bbox.y1;

            result.push(OcrTextLine {
                text: cleaned,
                x: final_x,
                y: final_y,
                w: final_w,
                h: final_h,
            });
        }

        Ok(result)
    }
}
