use anyhow::Result;
use ort::session::Session;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
use ndarray::Array4;
use image::DynamicImage;
use std::cmp::Ordering;
use crate::infrastructure::settings::GpuBackend;

#[derive(Debug, Clone)]
pub struct BubbleBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub prob: f32,
    #[allow(dead_code)]
    pub class_id: usize,
}


fn nms(mut boxes: Vec<BubbleBox>, iou_threshold: f32) -> Vec<BubbleBox> {
    boxes.sort_by(|a, b| b.prob.partial_cmp(&a.prob).unwrap_or(Ordering::Equal));
    
    let mut suppressed = vec![false; boxes.len()];
    let areas: Vec<f32> = boxes.iter()
        .map(|b| (b.x2 - b.x1) * (b.y2 - b.y1))
        .collect();
        
    // 1. Container/Merged Box Suppression:
    // If box i is significantly larger than box j, and box j is mostly inside box i,
    // then box i is a merged container box and should be suppressed.
    for i in 0..boxes.len() {
        for j in 0..boxes.len() {
            if i == j || suppressed[i] || suppressed[j] {
                continue;
            }
            
            let x_left = boxes[i].x1.max(boxes[j].x1);
            let y_top = boxes[i].y1.max(boxes[j].y1);
            let x_right = boxes[i].x2.min(boxes[j].x2);
            let y_bottom = boxes[i].y2.min(boxes[j].y2);
            
            if x_right > x_left && y_bottom > y_top {
                let intersection_area = (x_right - x_left) * (y_bottom - y_top);
                let area_i = areas[i];
                let area_j = areas[j];
                
                if area_i > area_j * 1.3 && area_j > 0.0 {
                    let containment = intersection_area / area_j;
                    if containment > 0.80 {
                        suppressed[i] = true;
                    }
                }
            }
        }
    }

    // 2. Standard NMS on remaining non-suppressed boxes
    let mut result: Vec<BubbleBox> = Vec::new();
    for i in 0..boxes.len() {
        if suppressed[i] {
            continue;
        }
        let mut keep = true;
        let area_i = areas[i];
        
        for res in &result {
            let x_left = boxes[i].x1.max(res.x1);
            let y_top = boxes[i].y1.max(res.y1);
            let x_right = boxes[i].x2.min(res.x2);
            let y_bottom = boxes[i].y2.min(res.y2);

            if x_right > x_left && y_bottom > y_top {
                let intersection_area = (x_right - x_left) * (y_bottom - y_top);
                let area_res = (res.x2 - res.x1) * (res.y2 - res.y1);
                let iou = intersection_area / (area_i + area_res - intersection_area);
                
                if iou > iou_threshold {
                    keep = false;
                    break;
                }
            }
        }
        if keep {
            result.push(boxes[i].clone());
        }
    }
    result
}

pub struct YoloBubbleDetector {
    session: Arc<Mutex<Option<Session>>>,
    gpu_backend: GpuBackend,
}

impl YoloBubbleDetector {
    pub fn new(gpu_backend: GpuBackend) -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            gpu_backend,
        }
    }

    fn ensure_session(&self) -> Result<()> {
        let mut session_guard = self.session.lock();
        if session_guard.is_some() {
            return Ok(());
        }

        let mut resolved_path = PathBuf::from("models/bubble-yolo/yolo26n.onnx");
        if !resolved_path.exists() {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let exe_relative = exe_dir.join("models/bubble-yolo/yolo26n.onnx");
                    if exe_relative.exists() {
                        resolved_path = exe_relative;
                    }
                }
            }
        }

        if !resolved_path.exists() {
            anyhow::bail!("YOLO Speech Bubble detector model not found at {}. Please download it in settings first.", resolved_path.display());
        }

        let session = super::onnx_engine::OnnxEngine::create_session(&resolved_path, self.gpu_backend)?;
        *session_guard = Some(session);
        Ok(())
    }

    pub fn detect_bubbles(&self, img: &DynamicImage) -> Result<Vec<BubbleBox>> {
        self.ensure_session()?;

        let orig_w = img.width() as f32;
        let orig_h = img.height() as f32;

        // Calculate letterbox scaling factor to preserve aspect ratio (crucial for ultrawide screens)
        let scale = (1280.0 / orig_w).min(1280.0 / orig_h);
        let resized = img.resize(1280, 1280, image::imageops::FilterType::Triangle).to_rgb8();
        
        let mut input = Array4::<f32>::zeros((1, 3, 1280, 1280));

        // Center the resized image in the 1280x1280 canvas
        let pad_x = (1280 - resized.width()) / 2;
        let pad_y = (1280 - resized.height()) / 2;

        for (x, y, pixel) in resized.enumerate_pixels() {
            let tx = x as usize + pad_x as usize;
            let ty = y as usize + pad_y as usize;
            if tx < 1280 && ty < 1280 {
                for c in 0..3 {
                    input[[0, c, ty, tx]] = pixel[c] as f32 / 255.0;
                }
            }
        }

        let input_tensor = ort::value::Value::from_array(input)
            .map_err(|e| anyhow::anyhow!("YOLO input tensor error: {}", e))?;

        let mut session_guard = self.session.lock();
        let session = session_guard.as_mut().ok_or_else(|| anyhow::anyhow!("YOLO session not initialized"))?;
        
        let outputs = session.run(ort::inputs![input_tensor])
            .map_err(|e| anyhow::anyhow!("YOLO inference error: {}", e))?;

        let out = outputs[0].try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("YOLO output extract error: {}", e))?;

        let view = out.view();
        let shape = view.shape();
        
        let mut boxes = Vec::new();

        // 1. Check if End-to-End Head (e.g. YOLOv10/YOLO26 output shape: [1, 300, 6])
        if shape.len() == 3 && shape[2] == 6 {
            let num_detections = shape[1];
            for i in 0..num_detections {
                let x1 = view[[0, i, 0]];
                let y1 = view[[0, i, 1]];
                let x2 = view[[0, i, 2]];
                let y2 = view[[0, i, 3]];
                let prob = view[[0, i, 4]];
                let class_id = view[[0, i, 5]] as usize;

                if prob > 0.20 {
                    // Map back from 1280x1280 padded tensor coordinates to original image coordinates
                    let ox1 = ((x1 - pad_x as f32) / scale).clamp(0.0, orig_w);
                    let oy1 = ((y1 - pad_y as f32) / scale).clamp(0.0, orig_h);
                    let ox2 = ((x2 - pad_x as f32) / scale).clamp(0.0, orig_w);
                    let oy2 = ((y2 - pad_y as f32) / scale).clamp(0.0, orig_h);

                    let box_w = ox2 - ox1;
                    let box_h = oy2 - oy1;
                    let pad_w = (box_w * 0.08).max(8.0);
                    let pad_h = (box_h * 0.08).max(8.0);
                    
                    let ex1 = (ox1 - pad_w).max(0.0);
                    let ey1 = (oy1 - pad_h).max(0.0);
                    let ex2 = (ox2 + pad_w).min(orig_w);
                    let ey2 = (oy2 + pad_h).min(orig_h);

                    boxes.push(BubbleBox {
                        x1: ex1,
                        y1: ey1,
                        x2: ex2,
                        y2: ey2,
                        prob,
                        class_id,
                    });
                }
            }
        } else if shape.len() == 3 && shape[1] >= 4 {
            // 2. Check if standard YOLOv8 output shape: [1, 4 + classes, anchors] (e.g. [1, 5, 8400])
            let num_anchors = shape[2];
            let num_classes = shape[1] - 4;
            
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

                if max_conf > 0.20 {
                    let x1 = cx - w / 2.0;
                    let y1 = cy - h / 2.0;
                    let x2 = cx + w / 2.0;
                    let y2 = cy + h / 2.0;

                    // Map back from 1280x1280 padded tensor coordinates to original image coordinates
                    let ox1 = ((x1 - pad_x as f32) / scale).clamp(0.0, orig_w);
                    let oy1 = ((y1 - pad_y as f32) / scale).clamp(0.0, orig_h);
                    let ox2 = ((x2 - pad_x as f32) / scale).clamp(0.0, orig_w);
                    let oy2 = ((y2 - pad_y as f32) / scale).clamp(0.0, orig_h);

                    let box_w = ox2 - ox1;
                    let box_h = oy2 - oy1;
                    let pad_w = (box_w * 0.08).max(8.0);
                    let pad_h = (box_h * 0.08).max(8.0);
                    
                    let ex1 = (ox1 - pad_w).max(0.0);
                    let ey1 = (oy1 - pad_h).max(0.0);
                    let ex2 = (ox2 + pad_w).min(orig_w);
                    let ey2 = (oy2 + pad_h).min(orig_h);

                    boxes.push(BubbleBox {
                        x1: ex1,
                        y1: ey1,
                        x2: ex2,
                        y2: ey2,
                        prob: max_conf,
                        class_id: max_class,
                    });
                }
            }
        } else {
            anyhow::bail!("Unsupported YOLO output tensor shape: {:?}", shape);
        }

        // Run NMS globally on all detected boxes to eliminate duplicate and heavily overlapping boxes
        boxes = nms(boxes, 0.40);

        // Apply spatial dimension filtering to prevent oversized or microscopic boxes
        boxes.retain(|b| {
            let box_w = b.x2 - b.x1;
            let box_h = b.y2 - b.y1;
            box_w > 10.0 && box_h > 10.0 && box_w < orig_w && box_h < orig_h
        });

        Ok(boxes)
    }
}
