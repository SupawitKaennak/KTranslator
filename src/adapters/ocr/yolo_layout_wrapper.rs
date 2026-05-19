use anyhow::Result;
use ort::session::Session;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use ndarray::Array4;

use crate::core::ports::{OcrEngine, FrameRgba, OcrTextLine};
use crate::core::types::{LanguageTag, Rect};
use crate::infrastructure::settings::GpuBackend;

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
    boxes.sort_by(|a, b| b.prob.partial_cmp(&a.prob).unwrap_or(std::cmp::Ordering::Equal));
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

pub struct YoloLayoutOcrWrapper {
    underlying: Arc<dyn OcrEngine>,
    yolo: Arc<Mutex<Option<Session>>>,
    last_boxes: Mutex<Vec<Rect>>,
    model_path: PathBuf,
    gpu_backend: GpuBackend,
}

impl YoloLayoutOcrWrapper {
    pub fn new(underlying: Arc<dyn OcrEngine>, model_path: String, gpu_backend: GpuBackend) -> Self {
        Self {
            underlying,
            yolo: Arc::new(Mutex::new(None)),
            last_boxes: Mutex::new(Vec::new()),
            model_path: PathBuf::from(model_path),
            gpu_backend,
        }
    }

    fn ensure_session(&self) -> Result<()> {
        let mut yolo_guard = self.yolo.lock();
        if yolo_guard.is_some() {
            return Ok(());
        }

        let mut resolved_path = self.model_path.clone();

        // 1. Try relative to CWD
        if !resolved_path.exists() {
            // 2. Try relative to EXE
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let exe_relative = exe_dir.join(&self.model_path);
                    if exe_relative.exists() {
                        resolved_path = exe_relative;
                    }
                }
            }
        }

        if !resolved_path.exists() {
            anyhow::bail!(
                "YOLO Layout Model not found at path: {:?}. Please ensure the models directory contains manga-ocr/manga109_yolo_s.onnx",
                self.model_path
            );
        }

        let session = crate::adapters::ocr::onnx_engine::OnnxEngine::create_session(&resolved_path, self.gpu_backend)?;
        *yolo_guard = Some(session);
        Ok(())
    }

    fn detect_text_boxes(&self, img: &image::DynamicImage) -> Result<Vec<BoundingBox>> {
        let orig_w = img.width() as f32;
        let orig_h = img.height() as f32;
        
        let target_size = 1024.0;
        let scale = (target_size / orig_w).min(target_size / orig_h);
        let new_w = (orig_w * scale) as u32;
        let new_h = (orig_h * scale) as u32;
        
        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Triangle).to_rgb8();
        let mut input = Array4::<f32>::zeros((1, 3, 1024, 1024));
        
        input.fill(114.0 / 255.0);
        
        for (x, y, pixel) in resized.enumerate_pixels() {
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = pixel[c] as f32 / 255.0;
            }
        }

        self.ensure_session()?;

        let input_tensor = ort::value::Value::from_array(input)
            .map_err(|e| anyhow::anyhow!("YOLO input error: {}", e))?;
            
        let mut yolo_guard = self.yolo.lock();
        let yolo = yolo_guard.as_mut().ok_or_else(|| anyhow::anyhow!("YOLO session not initialized"))?;
        let outputs = yolo.run(ort::inputs![input_tensor])
            .map_err(|e| anyhow::anyhow!("YOLO run error: {}", e))?;
            
        let out = outputs[0].try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("YOLO extract error: {}", e))?;
            
        let view = out.view();
        let shape = view.shape();
        let num_anchors = shape[2];
        let num_classes = shape[1] - 4;
        
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
        
        let mut nms_boxes = nms(boxes, 0.45);
        nms_boxes.retain(|b| {
            let box_w = b.x2 - b.x1;
            let box_h = b.y2 - b.y1;
            let is_bubble_size = box_w > 15.0 && box_h > 15.0
                && box_w < (orig_w * 0.95)
                && box_h < (orig_h * 0.95);
            b.class_id == text_class_id && is_bubble_size
        });
        
        Ok(nms_boxes)
    }
}

impl OcrEngine for YoloLayoutOcrWrapper {
    fn recognize(&self, frame: FrameRgba, lang_hint: Option<&LanguageTag>) -> Result<String> {
        let lines = self.recognize_lines(frame, lang_hint)?;
        let joined = lines.into_iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
        Ok(joined)
    }

    fn recognize_lines(&self, frame: FrameRgba, lang_hint: Option<&LanguageTag>) -> Result<Vec<OcrTextLine>> {
        let img = match image::RgbaImage::from_raw(frame.width, frame.height, frame.data.clone()) {
            Some(i) => image::DynamicImage::ImageRgba8(i),
            None => return Err(anyhow::anyhow!("Failed to convert frame to image")),
        };

        let boxes = match self.detect_text_boxes(&img) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("YOLO Layout detection failed, falling back to full-frame OCR. Error: {:?}", e);
                return self.underlying.recognize_lines(frame, lang_hint);
            }
        };

        if boxes.is_empty() {
            return self.underlying.recognize_lines(frame, lang_hint);
        }

        let mut sorted_boxes = boxes;
        // Step 1: Sort by x descending (RTL: rightmost first)
        sorted_boxes.sort_by(|a, b| b.x1.partial_cmp(&a.x1).unwrap_or(std::cmp::Ordering::Equal));

        // Step 2: Group boxes into columns based on x distance
        let mut columns: Vec<Vec<BoundingBox>> = Vec::new();
        for bbox in sorted_boxes {
            let mut added = false;
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
            col.sort_by(|a, b| a.y1.partial_cmp(&b.y1).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Step 4: Flatten columns back to a sorted vector
        let sorted_boxes: Vec<BoundingBox> = columns.into_iter().flatten().collect();

        // Save layout boxes for rendering overlay borders
        let mut rect_boxes = Vec::new();
        for bbox in &sorted_boxes {
            rect_boxes.push(Rect {
                x: bbox.x1,
                y: bbox.y1,
                w: bbox.x2 - bbox.x1,
                h: bbox.y2 - bbox.y1,
            });
        }
        *self.last_boxes.lock() = rect_boxes;

        let mut result = Vec::new();

        for bbox in sorted_boxes {
            let pad = 10.0;
            let x1 = (bbox.x1 - pad).max(0.0) as u32;
            let y1 = (bbox.y1 - pad).max(0.0) as u32;
            let x2 = (bbox.x2 + pad).min(frame.width as f32) as u32;
            let y2 = (bbox.y2 + pad).min(frame.height as f32) as u32;

            if x2 <= x1 || y2 <= y1 { continue; }

            let cropped = img.crop_imm(x1, y1, x2 - x1, y2 - y1);
            let cropped_rgba = cropped.to_rgba8();
            let cropped_frame = FrameRgba {
                width: cropped_rgba.width(),
                height: cropped_rgba.height(),
                data: cropped_rgba.into_raw(),
            };

            if let Ok(lines) = self.underlying.recognize_lines(cropped_frame, lang_hint) {
                for line in lines {
                    let mut offset_line = line;
                    offset_line.x += x1 as f32;
                    offset_line.y += y1 as f32;
                    result.push(offset_line);
                }
            }
        }

        Ok(result)
    }

    fn get_last_yolo_boxes(&self) -> Vec<Rect> {
        self.last_boxes.lock().clone()
    }
}
