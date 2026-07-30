/// Fast background color estimation for manga text removal.
///
/// For each OCR bounding box, samples pixels along the outer border of the box
/// (the surrounding area, not the text area itself) and returns the average color.
/// This gives a very good approximation of the background color which can then
/// be used to fill over the text in the overlay, making it appear "inpainted".
///
/// Works best for manga which typically has white/solid-color backgrounds.
/// For complex backgrounds, consider LaMa ONNX inpainting in future.

use crate::core::ports::{FrameRgba, OcrTextLine};

/// How many pixels outside the OCR box to sample (to avoid sampling text itself)
const BORDER_MARGIN: i32 = 4;
/// How many sample points along each edge
const SAMPLES_PER_EDGE: i32 = 12;

/// Samples the background color around each OCR bounding box.
///
/// Returns a `Vec<[u8; 4]>` of RGBA colors, one per `ocr_line`.
/// If a bounding box is out of bounds or the frame is unavailable, returns
/// the fallback color `[0,0,0,180]`.
pub fn sample_bg_colors(
    frame: &FrameRgba,
    ocr_lines: &[OcrTextLine],
    fallback: [u8; 4],
) -> Vec<[u8; 4]> {
    ocr_lines
        .iter()
        .map(|line| sample_single(frame, line, fallback))
        .collect()
}

fn sample_single(frame: &FrameRgba, line: &OcrTextLine, fallback: [u8; 4]) -> [u8; 4] {
    let fw = frame.width as i32;
    let fh = frame.height as i32;

    // Box in pixel coordinates
    let bx = line.x.round() as i32;
    let by = line.y.round() as i32;
    let bw = line.w.round() as i32;
    let bh = line.h.round() as i32;

    if bw <= 0 || bh <= 0 {
        return fallback;
    }

    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut count: u64 = 0;

    // Sample pixels along all 4 outer borders (BORDER_MARGIN pixels outside the box)
    // Top edge strip
    let strip_y = by - BORDER_MARGIN;
    if strip_y >= 0 && strip_y < fh {
        let step = (bw as f32 / SAMPLES_PER_EDGE as f32).max(1.0) as i32;
        let mut sx = bx;
        while sx < bx + bw {
            if let Some(px) = get_pixel(frame, sx, strip_y, fw, fh) {
                r_sum += px[0] as u64;
                g_sum += px[1] as u64;
                b_sum += px[2] as u64;
                count += 1;
            }
            sx += step;
        }
    }

    // Bottom edge strip
    let strip_y = by + bh + BORDER_MARGIN;
    if strip_y >= 0 && strip_y < fh {
        let step = (bw as f32 / SAMPLES_PER_EDGE as f32).max(1.0) as i32;
        let mut sx = bx;
        while sx < bx + bw {
            if let Some(px) = get_pixel(frame, sx, strip_y, fw, fh) {
                r_sum += px[0] as u64;
                g_sum += px[1] as u64;
                b_sum += px[2] as u64;
                count += 1;
            }
            sx += step;
        }
    }

    // Left edge strip
    let strip_x = bx - BORDER_MARGIN;
    if strip_x >= 0 && strip_x < fw {
        let step = (bh as f32 / SAMPLES_PER_EDGE as f32).max(1.0) as i32;
        let mut sy = by;
        while sy < by + bh {
            if let Some(px) = get_pixel(frame, strip_x, sy, fw, fh) {
                r_sum += px[0] as u64;
                g_sum += px[1] as u64;
                b_sum += px[2] as u64;
                count += 1;
            }
            sy += step;
        }
    }

    // Right edge strip
    let strip_x = bx + bw + BORDER_MARGIN;
    if strip_x >= 0 && strip_x < fw {
        let step = (bh as f32 / SAMPLES_PER_EDGE as f32).max(1.0) as i32;
        let mut sy = by;
        while sy < by + bh {
            if let Some(px) = get_pixel(frame, strip_x, sy, fw, fh) {
                r_sum += px[0] as u64;
                g_sum += px[1] as u64;
                b_sum += px[2] as u64;
                count += 1;
            }
            sy += step;
        }
    }

    if count == 0 {
        return fallback;
    }

    let r = (r_sum / count) as u8;
    let g = (g_sum / count) as u8;
    let b = (b_sum / count) as u8;

    // Compute luminance of sampled background to pick readable text color
    // Returned as alpha=255 (opaque, since we're replacing the background)
    [r, g, b, 255]
}

/// Compute a contrasting text color (black or white) based on background luminance.
/// Uses ITU-R BT.709 luma coefficients.
pub fn contrast_text_color(bg: [u8; 4]) -> [u8; 4] {
    let luma = 0.2126 * bg[0] as f32 + 0.7152 * bg[1] as f32 + 0.0722 * bg[2] as f32;
    if luma > 140.0 {
        [10, 10, 10, 255] // dark text on light background
    } else {
        [245, 245, 245, 255] // light text on dark background
    }
}

#[inline]
fn get_pixel(frame: &FrameRgba, x: i32, y: i32, fw: i32, fh: i32) -> Option<[u8; 4]> {
    if x < 0 || y < 0 || x >= fw || y >= fh {
        return None;
    }
    let idx = ((y as usize) * (fw as usize) + (x as usize)) * 4;
    if idx + 3 >= frame.data.len() {
        return None;
    }
    Some([frame.data[idx], frame.data[idx + 1], frame.data[idx + 2], frame.data[idx + 3]])
}
