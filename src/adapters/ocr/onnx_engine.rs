use anyhow::Result;
use ort::session::{Session, builder::SessionBuilder};
use ort::ep::DirectMLExecutionProvider;
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;
use ndarray::{Array2, Array4};
use tokenizers::Tokenizer;

use crate::core::ports::{OcrEngine, FrameRgba, OcrTextLine};
use crate::core::types::LanguageTag;

use std::cmp::Ordering;

#[derive(Debug, Clone)]
struct BoundingBox {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    prob: f32,
    class_id: usize,
}

fn iou(a: &BoundingBox, b: &BoundingBox) -> f32 {
    let x_left = a.x1.max(b.x1);
    let y_top = a.y1.max(b.y1);
    let x_right = a.x2.min(b.x2);
    let y_bottom = a.y2.min(b.y2);

    if x_right < x_left || y_bottom < y_top {
        return 0.0;
    }

    let intersection_area = (x_right - x_left) * (y_bottom - y_top);
    let a_area = (a.x2 - a.x1) * (a.y2 - a.y1);
    let b_area = (b.x2 - b.x1) * (b.y2 - b.y1);

    intersection_area / (a_area + b_area - intersection_area)
}

fn nms(mut boxes: Vec<BoundingBox>, iou_threshold: f32) -> Vec<BoundingBox> {
    boxes.sort_by(|a, b| b.prob.partial_cmp(&a.prob).unwrap_or(Ordering::Equal));
    let mut result = Vec::new();
    for i in 0..boxes.len() {
        let mut keep = true;
        for res in &result {
            if iou(&boxes[i], res) > iou_threshold {
                keep = false;
                break;
            }
        }
        if keep {
            result.push(boxes[i].clone());
        }
    }
    result
}

pub struct OnnxMangaRecognizer {
    encoder: Arc<Mutex<Session>>,
    decoder: Arc<Mutex<Session>>,
    yolo: Arc<Mutex<Session>>,
    tokenizer: Tokenizer,
    decoder_start_token_id: i64,
    eos_token_id: i64,
}

impl OnnxMangaRecognizer {
    pub fn new<P: AsRef<Path>>(models_dir: P) -> Result<Self> {
        let models_dir = models_dir.as_ref();
        
        let encoder_path = models_dir.join("encoder_model.onnx");
        let decoder_path = models_dir.join("decoder_model.onnx");
        let tokenizer_path = models_dir.join("tokenizer.json");
        let yolo_path = models_dir.join("manga109_yolo_s.onnx");

        if !encoder_path.exists() || !decoder_path.exists() || !tokenizer_path.exists() || !yolo_path.exists() {
            anyhow::bail!("Manga-OCR models not found in {:?}. Please ensure encoder_model.onnx, decoder_model.onnx, tokenizer.json, and manga109_yolo_s.onnx are present.", models_dir);
        }

        let encoder = SessionBuilder::new()
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?
            .with_execution_providers([DirectMLExecutionProvider::default().build()])
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?
            .commit_from_file(encoder_path)
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?;

        let decoder = SessionBuilder::new()
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?
            .with_execution_providers([DirectMLExecutionProvider::default().build()])
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?
            .commit_from_file(decoder_path)
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?;

        let yolo = SessionBuilder::new()
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?
            .with_execution_providers([DirectMLExecutionProvider::default().build()])
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?
            .commit_from_file(yolo_path)
            .map_err(|e| anyhow::anyhow!("ORT Error: {}", e))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer Error: {}", e))?;

        Ok(Self {
            encoder: Arc::new(Mutex::new(encoder)),
            decoder: Arc::new(Mutex::new(decoder)),
            yolo: Arc::new(Mutex::new(yolo)),
            tokenizer,
            decoder_start_token_id: 2, 
            eos_token_id: 3, 
        })
    }

    fn detect_text_boxes(&self, img: &image::DynamicImage) -> Result<Vec<BoundingBox>> {
        let orig_w = img.width() as f32;
        let orig_h = img.height() as f32;
        
        // YOLOv8 usually uses 640x640 or 1024x1024. Let's try 1024x1024 for Manga.
        let target_size = 1024.0;
        let scale = (target_size / orig_w).min(target_size / orig_h);
        let new_w = (orig_w * scale) as u32;
        let new_h = (orig_h * scale) as u32;
        
        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Triangle).to_rgb8();
        let mut input = Array4::<f32>::zeros((1, 3, 1024, 1024));
        
        // Pad with 114 (standard YOLO gray padding)
        input.fill(114.0 / 255.0);
        
        for (x, y, pixel) in resized.enumerate_pixels() {
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = pixel[c] as f32 / 255.0;
            }
        }

        let input_tensor = ort::value::Value::from_array(input)
            .map_err(|e| anyhow::anyhow!("YOLO input error: {}", e))?;
            
        let mut yolo = self.yolo.lock();
        let outputs = yolo.run(ort::inputs![input_tensor])
            .map_err(|e| anyhow::anyhow!("YOLO run error: {}", e))?;
            
        let out = outputs[0].try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("YOLO extract error: {}", e))?;
            
        // out shape: [1, 4 + classes, anchors] (e.g., [1, 8, 21504])
        let view = out.view();
        let shape = view.shape();
        let num_anchors = shape[2];
        let num_classes = shape[1] - 4;
        
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
            
            // Confidence threshold 0.25
            if max_conf > 0.25 {
                let x1 = cx - w / 2.0;
                let y1 = cy - h / 2.0;
                let x2 = cx + w / 2.0;
                let y2 = cy + h / 2.0;
                
                // Map back to original image
                boxes.push(BoundingBox {
                    x1: x1 / scale,
                    y1: y1 / scale,
                    x2: x2 / scale,
                    y2: y2 / scale,
                    prob: max_conf,
                    class_id: max_class,
                });
            }
        }
        
        // NMS
        let mut nms_boxes = nms(boxes, 0.45);
        
        // In manga109_yolo, classes are [body, face, frame, text]. text is index 3.
        nms_boxes.retain(|b| b.class_id == 3);
        
        Ok(nms_boxes)
    }

    fn recognize_internal(&self, img: &image::DynamicImage) -> Result<String> {
        let img = img.resize_exact(224, 224, image::imageops::FilterType::Triangle);
        let rgb = img.to_rgb8();
        
        // 1. Preprocessing (Normalize to [-1, 1] as per mean=0.5, std=0.5)
        let mut input = Array4::<f32>::zeros((1, 3, 224, 224));
        for (x, y, pixel) in rgb.enumerate_pixels() {
            for c in 0..3 {
                let val = (pixel[c] as f32 / 255.0 - 0.5) / 0.5;
                input[[0, c, y as usize, x as usize]] = val;
            }
        }

        // 2. Encoder
        let input_tensor = ort::value::Value::from_array(input)
            .map_err(|e| anyhow::anyhow!("Value creation error: {}", e))?;
            
        let mut encoder = self.encoder.lock();
        let encoder_outputs = encoder.run(ort::inputs![input_tensor])
            .map_err(|e| anyhow::anyhow!("Encoder run error: {}", e))?;
            
        let last_hidden_state = encoder_outputs[0].try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("Encoder extract error: {}", e))?;
        
        // 3. Autoregressive Decoding
        let mut tokens = vec![self.decoder_start_token_id];
        let max_length = 128;

        for _ in 0..max_length {
            let seq_len = tokens.len();
            let input_ids = Array2::from_shape_vec((1, seq_len), tokens.clone())?;
            
            let input_ids_tensor = ort::value::Value::from_array(input_ids)
                .map_err(|e| anyhow::anyhow!("Value creation error: {}", e))?;
            let encoder_hidden_tensor = ort::value::Value::from_array(last_hidden_state.to_owned())
                .map_err(|e| anyhow::anyhow!("Value creation error: {}", e))?;

            let mut decoder = self.decoder.lock();
            let decoder_outputs = decoder.run(ort::inputs![
                input_ids_tensor,
                encoder_hidden_tensor
            ])
            .map_err(|e| anyhow::anyhow!("Decoder run error: {}", e))?;
            
            let logits = decoder_outputs[0].try_extract_array::<f32>()
                .map_err(|e| anyhow::anyhow!("Decoder extract error: {}", e))?;
            
            // Greedy search
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
        let decoded = self.tokenizer.decode(&u32_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decode error: {}", e))?;
            
        Ok(decoded)
    }
}

impl OcrEngine for OnnxMangaRecognizer {
    fn recognize(&self, frame: FrameRgba, _lang_hint: Option<&LanguageTag>) -> Result<String> {
        let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from frame"))?;
        let dynamic_img = image::DynamicImage::ImageRgba8(img);
        self.recognize_internal(&dynamic_img)
    }

    fn recognize_lines(&self, frame: FrameRgba, _lang_hint: Option<&LanguageTag>) -> Result<Vec<OcrTextLine>> {
        let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.data.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from frame"))?;
        let dynamic_img = image::DynamicImage::ImageRgba8(img);

        // 1. Detect Text Boxes using YOLO
        let boxes = match self.detect_text_boxes(&dynamic_img) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("YOLO Detection failed: {}", e);
                // Fallback to reading the whole image if YOLO fails
                let text = self.recognize_internal(&dynamic_img)?;
                return Ok(vec![OcrTextLine {
                    text,
                    x: 0.0,
                    y: 0.0,
                    w: frame.width as f32,
                    h: frame.height as f32,
                }]);
            }
        };

        if boxes.is_empty() {
            // Fallback
            let text = self.recognize_internal(&dynamic_img)?;
            if text.trim().is_empty() { return Ok(vec![]); }
            return Ok(vec![OcrTextLine {
                text,
                x: 0.0,
                y: 0.0,
                w: frame.width as f32,
                h: frame.height as f32,
            }]);
        }

        let mut result = Vec::new();
        let mut sorted_boxes = boxes;
        // Sort boxes top-to-bottom, right-to-left (Manga reading order)
        sorted_boxes.sort_by(|a, b| {
            // Group by row roughly
            let row_diff = (a.y1 - b.y1).abs();
            if row_diff > 40.0 {
                a.y1.partial_cmp(&b.y1).unwrap_or(Ordering::Equal)
            } else {
                b.x1.partial_cmp(&a.x1).unwrap_or(Ordering::Equal) // Right to left
            }
        });

        for bbox in sorted_boxes {
            // Add padding to crop
            let pad = 10.0;
            let x1 = (bbox.x1 - pad).max(0.0) as u32;
            let y1 = (bbox.y1 - pad).max(0.0) as u32;
            let x2 = (bbox.x2 + pad).min(frame.width as f32) as u32;
            let y2 = (bbox.y2 + pad).min(frame.height as f32) as u32;
            
            if x2 <= x1 || y2 <= y1 { continue; }
            
            let cropped = dynamic_img.crop_imm(x1, y1, x2 - x1, y2 - y1);
            let text = match self.recognize_internal(&cropped) {
                Ok(t) => t,
                Err(_) => continue,
            };
            
            let mut cleaned = text.trim().to_string();
            while cleaned.contains("....") { cleaned = cleaned.replace("....", ".."); }
            if cleaned.is_empty() { continue; }
            
            // เทคนิคช่วยตัดคำไทย: ใส่ Zero Width Space (U+200B) หลังทุกตัวอักษร
            // เพื่อให้ egui สามารถ wrap ข้อความแนวนอนลงมาเป็นบรรทัดใหม่ได้ทุกจุด
            let mut word_wrap_friendly = String::new();
            for c in cleaned.chars() {
                word_wrap_friendly.push(c);
                // ใส่ตัวคั่นล่องหนหลังตัวอักษรไทย/ญี่ปุ่น (ยกเว้นเว้นวรรค)
                if !c.is_whitespace() {
                    word_wrap_friendly.push('\u{200B}');
                }
            }

            // ใช้ขนาดที่แคบกว่าลูกโป่งนิดหน่อย (90%) เพื่อให้ข้อความไม่ติดขอบลูกโป่ง
            // และจัดวางกึ่งกลาง
            let mut final_w = (bbox.x2 - bbox.x1) * 0.9; 
            let mut final_h = bbox.y2 - bbox.y1;
            
            // ป้องกันกล่องแคบเกินไปสำหรับภาษาไทย
            if final_w < 60.0 { final_w = 60.0; }
            if final_h < 30.0 { final_h = 30.0; }
            
            let cx = (bbox.x1 + bbox.x2) / 2.0;
            let cy = (bbox.y1 + bbox.y2) / 2.0;
            let final_x = cx - final_w / 2.0;
            let final_y = cy - final_h / 2.0;

            result.push(OcrTextLine {
                text: word_wrap_friendly,
                x: final_x,
                y: final_y,
                w: final_w,
                h: final_h,
            });
        }
        
        Ok(result)
    }
}
