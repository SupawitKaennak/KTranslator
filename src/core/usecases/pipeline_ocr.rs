use crate::adapters::ocr::craft_text_detector_adapter::CraftTextDetector;
use crate::adapters::ocr::yolo_bubble_detector_adapter::YoloBubbleDetector;
use crate::core::ports::{FrameRgba, OcrEngine, OcrTextLine};
use crate::infrastructure::settings::{ImageProcessingSettings, TextDetectorMode};
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub fn perform_ocr(
    frame: &FrameRgba,
    ocr_engine: &Arc<dyn OcrEngine>,
    source_lang: Option<&crate::core::types::LanguageTag>,
    yolo_detector: Option<&Arc<YoloBubbleDetector>>,
    craft_detector: Option<&Arc<CraftTextDetector>>,
    text_detector_mode: TextDetectorMode,
    img_proc_cfg: &ImageProcessingSettings,
    jp_merge_vertical: bool,
) -> (Vec<OcrTextLine>, Vec<OcrTextLine>, bool) {
    let mut detection_boxes = Vec::new();
    let mut raw_ocr_lines = Vec::new();
    let mut detection_successful = false;

    // Convert frame to DynamicImage once if any detector is active
    let dynamic_img = if matches!(
        text_detector_mode,
        TextDetectorMode::YoloBubble | TextDetectorMode::CraftRegion
    ) {
        image::RgbaImage::from_raw(frame.width, frame.height, (*frame.data).clone())
            .map(image::DynamicImage::ImageRgba8)
    } else {
        None
    };

    // Run the selected text detector
    if let Some(ref dyn_img) = dynamic_img {
        match text_detector_mode {
            TextDetectorMode::YoloBubble => {
                if let Some(detector) = yolo_detector {
                    if let Ok(mut bubbles) = detector.detect_bubbles(dyn_img) {
                        if !bubbles.is_empty() {
                            // Sort bubbles in natural reading order
                            bubbles.sort_by(|a, b| {
                                let a_h = a.y2 - a.y1;
                                let b_h = b.y2 - b.y1;
                                let tolerance = a_h.min(b_h) * 0.4;
                                let y_diff = (a.y1 - b.y1).abs();

                                if y_diff > tolerance {
                                    a.y1.partial_cmp(&b.y1)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                } else if jp_merge_vertical {
                                    b.x1.partial_cmp(&a.x1)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                } else {
                                    a.x1.partial_cmp(&b.x1)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                }
                            });

                            detection_successful = true;
                            for b in &bubbles {
                                detection_boxes.push(OcrTextLine {
                                    text: String::new(),
                                    x: b.x1,
                                    y: b.y1,
                                    w: b.x2 - b.x1,
                                    h: b.y2 - b.y1,
                                });
                            }
                        }
                    }
                }
            }
            TextDetectorMode::CraftRegion => {
                if let Some(detector) = craft_detector {
                    if let Ok(mut regions) = detector.detect_text_regions(dyn_img) {
                        if !regions.is_empty() {
                            // Sort text regions in reading order (top-to-bottom, left-to-right)
                            regions.sort_by(|a, b| {
                                let a_h = a.y2 - a.y1;
                                let b_h = b.y2 - b.y1;
                                let tolerance = a_h.min(b_h) * 0.4;
                                let y_diff = (a.y1 - b.y1).abs();

                                if y_diff > tolerance {
                                    a.y1.partial_cmp(&b.y1)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                } else if jp_merge_vertical {
                                    b.x1.partial_cmp(&a.x1)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                } else {
                                    a.x1.partial_cmp(&b.x1)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                }
                            });

                            detection_successful = true;
                            for r in &regions {
                                detection_boxes.push(OcrTextLine {
                                    text: String::new(),
                                    x: r.x1,
                                    y: r.y1,
                                    w: r.x2 - r.x1,
                                    h: r.y2 - r.y1,
                                });
                            }
                        }
                    }
                }
            }
            TextDetectorMode::None => {}
        }
    }

    // If detection was successful, crop each detected region and run OCR on it
    let yolo_bubbles = detection_boxes.clone();
    if detection_successful {
        for region in &detection_boxes {
            let pad = 6;
            let crop_x = (region.x - pad as f32).max(0.0) as u32;
            let crop_y = (region.y - pad as f32).max(0.0) as u32;
            let crop_w = ((region.x + region.w + pad as f32).min(frame.width as f32) as u32)
                .saturating_sub(crop_x);
            let crop_h = ((region.y + region.h + pad as f32).min(frame.height as f32) as u32)
                .saturating_sub(crop_y);

            if crop_w >= 5 && crop_h >= 5 {
                let cropped_frame =
                    crate::core::usecases::image_processing_usecase::crop_frame(
                        frame, crop_x, crop_y, crop_w, crop_h,
                    );

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

    if !detection_successful {
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

    (raw_ocr_lines, yolo_bubbles, detection_successful)
}

