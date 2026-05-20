use std::cmp::Ordering;

/// Axis-aligned bounding box with confidence score and class ID.
/// Unified type used across YOLO detectors for NMS processing.
#[derive(Debug, Clone)]
pub struct DetectionBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub prob: f32,
    pub class_id: usize,
}

impl DetectionBox {
    #[inline]
    pub fn width(&self) -> f32 {
        self.x2 - self.x1
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.y2 - self.y1
    }

    #[inline]
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }
}

/// Calculates Intersection over Union (IoU) between two bounding boxes.
#[inline]
pub fn iou(a: &DetectionBox, b: &DetectionBox) -> f32 {
    let x_left = a.x1.max(b.x1);
    let y_top = a.y1.max(b.y1);
    let x_right = a.x2.min(b.x2);
    let y_bottom = a.y2.min(b.y2);

    if x_right < x_left || y_bottom < y_top {
        return 0.0;
    }

    let intersection_area = (x_right - x_left) * (y_bottom - y_top);
    let union_area = a.area() + b.area() - intersection_area;

    if union_area <= 0.0 {
        return 0.0;
    }

    intersection_area / union_area
}

/// Non-Maximum Suppression: removes overlapping detections by keeping only
/// the highest-confidence box when two boxes overlap beyond the IoU threshold.
pub fn nms(mut boxes: Vec<DetectionBox>, iou_threshold: f32) -> Vec<DetectionBox> {
    boxes.sort_by(|a, b| b.prob.partial_cmp(&a.prob).unwrap_or(Ordering::Equal));
    let mut result = Vec::new();
    for b in &boxes {
        let mut keep = true;
        for res in &result {
            if iou(b, res) > iou_threshold {
                keep = false;
                break;
            }
        }
        if keep {
            result.push(b.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(x1: f32, y1: f32, x2: f32, y2: f32, prob: f32) -> DetectionBox {
        DetectionBox {
            x1,
            y1,
            x2,
            y2,
            prob,
            class_id: 0,
        }
    }

    #[test]
    fn iou_identical_boxes() {
        let a = bbox(0.0, 0.0, 10.0, 10.0, 0.9);
        assert!((iou(&a, &a) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn iou_non_overlapping() {
        let a = bbox(0.0, 0.0, 10.0, 10.0, 0.9);
        let b = bbox(20.0, 20.0, 30.0, 30.0, 0.8);
        assert_eq!(iou(&a, &b), 0.0);
    }

    #[test]
    fn iou_partial_overlap() {
        let a = bbox(0.0, 0.0, 10.0, 10.0, 0.9);
        let b = bbox(5.0, 5.0, 15.0, 15.0, 0.8);
        // Intersection: 5x5=25, Union: 100+100-25=175
        let expected = 25.0 / 175.0;
        assert!((iou(&a, &b) - expected).abs() < 0.001);
    }

    #[test]
    fn detection_box_dimensions() {
        let b = bbox(10.0, 20.0, 50.0, 80.0, 0.5);
        assert_eq!(b.width(), 40.0);
        assert_eq!(b.height(), 60.0);
        assert_eq!(b.area(), 2400.0);
    }

    #[test]
    fn nms_keeps_highest_confidence() {
        let boxes = vec![
            bbox(0.0, 0.0, 10.0, 10.0, 0.5),
            bbox(1.0, 1.0, 11.0, 11.0, 0.9), // Higher confidence, overlapping
        ];
        let result = nms(boxes, 0.3);
        assert_eq!(result.len(), 1);
        assert!((result[0].prob - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn nms_keeps_non_overlapping() {
        let boxes = vec![
            bbox(0.0, 0.0, 10.0, 10.0, 0.9),
            bbox(50.0, 50.0, 60.0, 60.0, 0.8),
        ];
        let result = nms(boxes, 0.5);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn nms_empty_input() {
        let result = nms(Vec::new(), 0.5);
        assert!(result.is_empty());
    }
}
