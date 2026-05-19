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

fn iou(a: &BubbleBox, b: &BubbleBox) -> f32 {
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

fn nms(mut boxes: Vec<BubbleBox>, iou_threshold: f32) -> Vec<BubbleBox> {
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

        // YOLO26n / YOLOv10 typically uses 1280x1280
        let target_size = 1280.0;
        let scale = (target_size / orig_w).min(target_size / orig_h);
        let new_w = (orig_w * scale) as u32;
        let new_h = (orig_h * scale) as u32;

        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Triangle).to_rgb8();
        let mut input = Array4::<f32>::zeros((1, 3, 1280, 1280));
        input.fill(114.0 / 255.0); // padding color

        for (x, y, pixel) in resized.enumerate_pixels() {
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] = pixel[c] as f32 / 255.0;
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

                if prob > 0.25 {
                    boxes.push(BubbleBox {
                        x1: x1 / scale,
                        y1: y1 / scale,
                        x2: x2 / scale,
                        y2: y2 / scale,
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

                if max_conf > 0.25 {
                    let x1 = cx - w / 2.0;
                    let y1 = cy - h / 2.0;
                    let x2 = cx + w / 2.0;
                    let y2 = cy + h / 2.0;

                    boxes.push(BubbleBox {
                        x1: x1 / scale,
                        y1: y1 / scale,
                        x2: x2 / scale,
                        y2: y2 / scale,
                        prob: max_conf,
                        class_id: max_class,
                    });
                }
            }
            // YOLOv8 requires NMS post-processing
            boxes = nms(boxes, 0.45);
        } else {
            anyhow::bail!("Unsupported YOLO output tensor shape: {:?}", shape);
        }

        // Apply spatial dimension filtering to prevent oversized or microscopic boxes
        boxes.retain(|b| {
            let box_w = b.x2 - b.x1;
            let box_h = b.y2 - b.y1;
            box_w > 10.0 && box_h > 10.0 && box_w < orig_w && box_h < orig_h
        });

        Ok(boxes)
    }
}
