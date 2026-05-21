use crate::infrastructure::settings::GpuBackend;
use anyhow::Result;
use image::DynamicImage;
use ndarray::Array4;
use ort::session::Session;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

use super::non_max_suppression_utils::{nms, DetectionBox};

/// CRAFT (Character Region Awareness for Text Detection) adapter.
///
/// Detects precise text regions by predicting character-level region score maps
/// and affinity score maps via an ONNX model, then extracting connected components
/// as bounding boxes. Produces tighter text-level boxes than bubble-level YOLO.
pub struct CraftTextDetector {
    session: Arc<Mutex<Option<Session>>>,
    gpu_backend: GpuBackend,
}

/// CRAFT model input dimensions
const CRAFT_INPUT_SIZE: usize = 384;
/// Region score threshold for binarizing the heatmap
const REGION_THRESHOLD: f32 = 0.4;
/// Minimum area in pixels for a detected text region to be kept
const MIN_REGION_AREA: f32 = 100.0;

impl CraftTextDetector {
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

        let mut resolved_path = PathBuf::from("models/craft/craft.onnx");
        if !resolved_path.exists() {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let exe_relative = exe_dir.join("models/craft/craft.onnx");
                    if exe_relative.exists() {
                        resolved_path = exe_relative;
                    }
                }
            }
        }

        if !resolved_path.exists() {
            anyhow::bail!(
                "CRAFT text detector model not found at {}. Please download it in settings first.",
                resolved_path.display()
            );
        }

        let session = super::onnx_inference_engine::OnnxEngine::create_session(
            &resolved_path,
            self.gpu_backend,
        )?;
        *session_guard = Some(session);
        Ok(())
    }

    /// Detect text regions in the given image.
    ///
    /// Returns bounding boxes in original image coordinates (same format as YOLO output).
    pub fn detect_text_regions(&self, img: &DynamicImage) -> Result<Vec<DetectionBox>> {
        self.ensure_session()?;

        let orig_w = img.width() as f32;
        let orig_h = img.height() as f32;

        // Resize to CRAFT input size while preserving aspect ratio (letterbox)
        let scale = (CRAFT_INPUT_SIZE as f32 / orig_w).min(CRAFT_INPUT_SIZE as f32 / orig_h);
        let new_w = (orig_w * scale) as u32;
        let new_h = (orig_h * scale) as u32;

        let resized = img
            .resize_exact(new_w, new_h, image::imageops::FilterType::Triangle)
            .to_rgb8();

        // Prepare input tensor: [1, 3, H, W] normalized with ImageNet mean/std
        let mean = [0.485_f32, 0.456, 0.406];
        let std_dev = [0.229_f32, 0.224, 0.225];

        let mut input = Array4::<f32>::zeros((1, 3, CRAFT_INPUT_SIZE, CRAFT_INPUT_SIZE));

        let pad_x = (CRAFT_INPUT_SIZE - new_w as usize) / 2;
        let pad_y = (CRAFT_INPUT_SIZE - new_h as usize) / 2;

        for (x, y, pixel) in resized.enumerate_pixels() {
            let tx = x as usize + pad_x;
            let ty = y as usize + pad_y;
            if tx < CRAFT_INPUT_SIZE && ty < CRAFT_INPUT_SIZE {
                for c in 0..3 {
                    input[[0, c, ty, tx]] = (pixel[c] as f32 / 255.0 - mean[c]) / std_dev[c];
                }
            }
        }

        let input_tensor = ort::value::Value::from_array(input)
            .map_err(|e| anyhow::anyhow!("CRAFT input tensor error: {}", e))?;

        let mut session_guard = self.session.lock();
        let session = session_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("CRAFT session not initialized"))?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| anyhow::anyhow!("CRAFT inference error: {}", e))?;

        // CRAFT outputs: [1, H/2, W/2, 2] where channel 0 = region score, channel 1 = affinity score
        let out = outputs[0]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("CRAFT output extract error: {}", e))?;

        let view = out.view();
        let shape = view.shape();

        // The output is typically [1, H/2, W/2, 2] for CRAFT
        let (score_h, score_w) = if shape.len() == 4 {
            (shape[1], shape[2])
        } else {
            anyhow::bail!("Unexpected CRAFT output shape: {:?}", shape);
        };

        // Binarize region score map to find text regions
        let mut binary_map = vec![false; score_h * score_w];
        for y in 0..score_h {
            for x in 0..score_w {
                let region_score = view[[0, y, x, 0]];
                binary_map[y * score_w + x] = region_score > REGION_THRESHOLD;
            }
        }

        // Connected component labeling (4-connectivity flood fill)
        let mut labels = vec![0u32; score_h * score_w];
        let mut label_id = 0u32;
        let mut components: Vec<(f32, f32, f32, f32)> = Vec::new(); // (min_x, min_y, max_x, max_y)

        for y in 0..score_h {
            for x in 0..score_w {
                let idx = y * score_w + x;
                if binary_map[idx] && labels[idx] == 0 {
                    label_id += 1;
                    let mut min_x = x;
                    let mut min_y = y;
                    let mut max_x = x;
                    let mut max_y = y;

                    // BFS flood fill
                    let mut queue = std::collections::VecDeque::new();
                    queue.push_back((x, y));
                    labels[idx] = label_id;

                    while let Some((qx, qy)) = queue.pop_front() {
                        min_x = min_x.min(qx);
                        min_y = min_y.min(qy);
                        max_x = max_x.max(qx);
                        max_y = max_y.max(qy);

                        // 4-connectivity neighbors
                        let neighbors = [
                            (qx.wrapping_sub(1), qy),
                            (qx + 1, qy),
                            (qx, qy.wrapping_sub(1)),
                            (qx, qy + 1),
                        ];
                        for (nx, ny) in neighbors {
                            if nx < score_w && ny < score_h {
                                let ni = ny * score_w + nx;
                                if binary_map[ni] && labels[ni] == 0 {
                                    labels[ni] = label_id;
                                    queue.push_back((nx, ny));
                                }
                            }
                        }
                    }

                    components.push((min_x as f32, min_y as f32, max_x as f32, max_y as f32));
                }
            }
        }

        // Map score-map coordinates back to original image coordinates
        // Score map is at half resolution of the input, so multiply by 2.
        let score_to_input = 2.0_f32;
        let mut boxes = Vec::new();

        for (sx1, sy1, sx2, sy2) in &components {
            // Score map coords -> input tensor coords
            let ix1 = sx1 * score_to_input;
            let iy1 = sy1 * score_to_input;
            let ix2 = (sx2 + 1.0) * score_to_input;
            let iy2 = (sy2 + 1.0) * score_to_input;

            // Remove letterbox padding and un-scale to original image coords
            let ox1 = ((ix1 - pad_x as f32) / scale).clamp(0.0, orig_w);
            let oy1 = ((iy1 - pad_y as f32) / scale).clamp(0.0, orig_h);
            let ox2 = ((ix2 - pad_x as f32) / scale).clamp(0.0, orig_w);
            let oy2 = ((iy2 - pad_y as f32) / scale).clamp(0.0, orig_h);

            let box_w = ox2 - ox1;
            let box_h = oy2 - oy1;

            // Filter out tiny regions
            if box_w * box_h < MIN_REGION_AREA {
                continue;
            }

            // Add small padding for OCR accuracy
            let pad_w = (box_w * 0.05).max(4.0);
            let pad_h = (box_h * 0.05).max(4.0);

            boxes.push(DetectionBox {
                x1: (ox1 - pad_w).max(0.0),
                y1: (oy1 - pad_h).max(0.0),
                x2: (ox2 + pad_w).min(orig_w),
                y2: (oy2 + pad_h).min(orig_h),
                prob: 1.0, // CRAFT does not output per-region confidence
                class_id: 0,
            });
        }

        // Apply NMS to merge overlapping detections
        boxes = nms(boxes, 0.3);

        // Filter out oversized boxes (larger than 95% of the image)
        boxes.retain(|b| {
            let bw = b.x2 - b.x1;
            let bh = b.y2 - b.y1;
            bw > 5.0 && bh > 5.0 && bw < orig_w * 0.95 && bh < orig_h * 0.95
        });

        Ok(boxes)
    }
}
