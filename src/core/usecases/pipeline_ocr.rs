use crate::adapters::ocr::yolo_bubble_detector_adapter::YoloBubbleDetector;
use crate::core::ports::{FrameRgba, OcrEngine, OcrTextLine};
use crate::infrastructure::settings::ImageProcessingSettings;
use std::sync::Arc;

pub fn perform_ocr(
    frame: &FrameRgba,
    ocr_engine: &Arc<dyn OcrEngine>,
    source_lang: Option<&crate::core::types::LanguageTag>,
    yolo_detector: Option<&Arc<YoloBubbleDetector>>,
    img_proc_cfg: &ImageProcessingSettings,
    jp_merge_vertical: bool,
) -> (Vec<OcrTextLine>, Vec<OcrTextLine>, bool) {
    let mut yolo_bubbles = Vec::new();
    let mut raw_ocr_lines = Vec::new();
    let mut bubble_detection_successful = false;

    if let Some(detector) = yolo_detector {
        if let Some(rgba_img) =
            image::RgbaImage::from_raw(frame.width, frame.height, (*frame.data).clone())
        {
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
                        let crop_w = ((b.x2 + pad as f32).min(frame.width as f32) as u32)
                            .saturating_sub(crop_x);
                        let crop_h = ((b.y2 + pad as f32).min(frame.height as f32) as u32)
                            .saturating_sub(crop_y);

                        if crop_w >= 5 && crop_h >= 5 {
                            let cropped_frame = crate::core::usecases::image_processing_usecase::crop_frame(
                                frame, crop_x, crop_y, crop_w, crop_h,
                            );

                            // Perform full image pre-processing on the cropped speech bubble
                            let (proc_data, proc_w, proc_h) =
                                crate::core::usecases::image_processing_usecase::process_image_for_ocr(
                                    &cropped_frame.data,
                                    cropped_frame.width,
                                    cropped_frame.height,
                                    img_proc_cfg,
                                );
                            let mut processed_crop = cropped_frame.clone();
                            processed_crop.data = std::sync::Arc::new(proc_data);
                            processed_crop.width = proc_w;
                            processed_crop.height = proc_h;

                            if let Ok(mut lines) =
                                ocr_engine.recognize_lines(processed_crop, source_lang)
                            {
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
        let (proc_data, proc_w, proc_h) =
            crate::core::usecases::image_processing_usecase::process_image_for_ocr(
                &frame.data,
                frame.width,
                frame.height,
                img_proc_cfg,
            );
        let mut processed_frame = frame.clone();
        processed_frame.data = std::sync::Arc::new(proc_data);
        processed_frame.width = proc_w;
        processed_frame.height = proc_h;

        if let Ok(mut lines) = ocr_engine.recognize_lines(processed_frame, source_lang) {
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

    (raw_ocr_lines, yolo_bubbles, bubble_detection_successful)
}
