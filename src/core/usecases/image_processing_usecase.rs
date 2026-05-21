use crate::core::ports::FrameRgba;
use crate::infrastructure::settings::{ImageProcessingSettings, MorphologyOp};
use std::cell::RefCell;
use std::thread_local;

pub fn crop_frame(frame: &FrameRgba, x: u32, y: u32, w: u32, h: u32) -> FrameRgba {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for row in y..(y + h) {
        let src_idx = (row * frame.width + x) as usize * 4;
        let length = (w * 4) as usize;
        if src_idx + length <= frame.data.len() {
            data.extend_from_slice(&frame.data[src_idx..(src_idx + length)]);
        } else {
            data.resize(data.len() + length, 0);
        }
    }
    FrameRgba {
        width: w,
        height: h,
        data: std::sync::Arc::new(data),
    }
}

/// Applies requested image processing filters to Raw RGBA buffer before OCR.
/// Returns the processed image buffer (RGBA format) along with new width and height.
pub fn process_image_for_ocr(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    config: &ImageProcessingSettings,
) -> (Vec<u8>, u32, u32) {
    type DoubleBuffer = (Option<Vec<u8>>, Option<Vec<u8>>);
    // Thread‑local reusable double-buffers to avoid repeated vector allocations/clones
    thread_local! {
        static REUSE_BUFFERS: RefCell<DoubleBuffer> = const { RefCell::new((None, None)) };
    }

    // Return pristine buffer instantly if all filters are deactivated
    let is_active = config.grayscale
        || config.invert
        || (config.contrast - 1.0).abs() > 0.01
        || config.brightness != 0
        || (config.gamma - 1.0).abs() > 0.01
        || config.binarize
        || config.adaptive_threshold
        || config.denoise
        || config.sharpen
        || config.morphology != MorphologyOp::None
        || (config.resize_scale - 1.0).abs() > 0.01
        || config.deskew
        || config.anti_alias_removal;

    if !is_active || rgba_data.is_empty() {
        return (rgba_data.to_vec(), width, height);
    }

    // Extract double buffers
    let (mut buf_a, mut buf_b) = REUSE_BUFFERS.with(|bufs| {
        let mut b = bufs.borrow_mut();
        let a =
            b.0.take()
                .unwrap_or_else(|| Vec::with_capacity(rgba_data.len()));
        let b_buf =
            b.1.take()
                .unwrap_or_else(|| Vec::with_capacity(rgba_data.len()));
        (a, b_buf)
    });

    buf_a.clear();
    buf_a.extend_from_slice(rgba_data);
    buf_b.resize(buf_a.len(), 0);

    // 1. Grayscale Conversion
    if config.grayscale || config.binarize || config.adaptive_threshold {
        for chunk in buf_a.chunks_exact_mut(4) {
            let r = chunk[0] as u32;
            let g = chunk[1] as u32;
            let b = chunk[2] as u32;
            let gray = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
            chunk[0] = gray;
            chunk[1] = gray;
            chunk[2] = gray;
        }
    }

    // 2. Invert Colors
    if config.invert {
        for chunk in buf_a.chunks_exact_mut(4) {
            chunk[0] = 255 - chunk[0];
            chunk[1] = 255 - chunk[1];
            chunk[2] = 255 - chunk[2];
        }
    }

    // 3. Contrast, Brightness, and Gamma Correction using precomputed LUT
    if (config.contrast - 1.0).abs() > 0.01
        || config.brightness != 0
        || (config.gamma - 1.0).abs() > 0.01
    {
        let mut lut = [0u8; 256];
        let inv_gamma = if config.gamma > 0.0 {
            1.0 / config.gamma
        } else {
            1.0
        };
        for (p, val_lut) in lut.iter_mut().enumerate() {
            // Apply contrast and brightness mapping
            let mut val = ((p as f32 - 127.5) * config.contrast + 127.5) + config.brightness as f32;
            val = val.clamp(0.0, 255.0);

            // Apply gamma correction curve
            if (config.gamma - 1.0).abs() > 0.01 {
                val = 255.0 * (val / 255.0).powf(inv_gamma);
            }
            *val_lut = val.clamp(0.0, 255.0) as u8;
        }
        for chunk in buf_a.chunks_exact_mut(4) {
            chunk[0] = lut[chunk[0] as usize];
            chunk[1] = lut[chunk[1] as usize];
            chunk[2] = lut[chunk[2] as usize];
        }
    }

    // 4. Anti-alias Removal (Sharp dynamic boundary quantization)
    if config.anti_alias_removal {
        for chunk in buf_a.chunks_exact_mut(4) {
            for val in chunk.iter_mut().take(3) {
                let v = *val;
                *val = if v > 160 {
                    255
                } else if v < 96 {
                    0
                } else {
                    v
                };
            }
        }
    }

    // 5. Binary Threshold / Adaptive Threshold
    if config.binarize && !config.adaptive_threshold {
        let thresh = config.binary_threshold;
        for chunk in buf_a.chunks_exact_mut(4) {
            let gray = chunk[0];
            let val = if gray >= thresh { 255 } else { 0 };
            chunk[0] = val;
            chunk[1] = val;
            chunk[2] = val;
        }
    } else if config.adaptive_threshold {
        // High-performance O(W * H) windowed local-mean adaptive binarization using Integral Image
        let w = width as usize;
        let h = height as usize;
        let radius = 7;

        let mut integral = vec![0u32; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row_sum = 0;
            let row_idx = y * w;
            let int_row_idx = y * (w + 1);
            let next_int_row_idx = (y + 1) * (w + 1);
            for x in 0..w {
                row_sum += buf_a[(row_idx + x) * 4] as u32;
                integral[next_int_row_idx + x + 1] = row_sum + integral[int_row_idx + x + 1];
            }
        }

        for y in 0..h {
            let row_idx = y * w;
            let y_min = y.saturating_sub(radius);
            let y_max = (y + radius).min(h - 1);
            for x in 0..w {
                let x_min = x.saturating_sub(radius);
                let x_max = (x + radius).min(w - 1);

                // O(1) sum calculation using the integral image
                let i_y_max = y_max + 1;
                let i_x_max = x_max + 1;
                let i_w = w + 1;

                let sum = integral[i_y_max * i_w + i_x_max]
                    - integral[y_min * i_w + i_x_max]
                    - integral[i_y_max * i_w + x_min]
                    + integral[y_min * i_w + x_min];

                let count = (x_max - x_min + 1) * (y_max - y_min + 1);
                let avg = (sum / count as u32) as u8;
                let current = buf_a[(row_idx + x) * 4];
                let res = if current as i32 <= avg as i32 - 10 {
                    0
                } else {
                    255
                };

                let out_idx = (row_idx + x) * 4;
                buf_b[out_idx] = res;
                buf_b[out_idx + 1] = res;
                buf_b[out_idx + 2] = res;
                buf_b[out_idx + 3] = buf_a[out_idx + 3]; // preserve alpha
            }
        }
        std::mem::swap(&mut buf_a, &mut buf_b);
    }

    // 6. Sharpening Convolution using 3x3 spatial kernel
    if config.sharpen {
        buf_b.copy_from_slice(&buf_a);
        let w = width as usize;
        let h = height as usize;
        for y in 1..(h.saturating_sub(1)) {
            let row_idx = y * w;
            let prev_row_idx = (y - 1) * w;
            let next_row_idx = (y + 1) * w;
            for x in 1..(w.saturating_sub(1)) {
                let idx = (row_idx + x) * 4;
                let top_idx = (prev_row_idx + x) * 4;
                let bottom_idx = (next_row_idx + x) * 4;
                let left_idx = (row_idx + x - 1) * 4;
                let right_idx = (row_idx + x + 1) * 4;
                for c in 0..3 {
                    let center = buf_a[idx + c] as i32;
                    let top = buf_a[top_idx + c] as i32;
                    let bottom = buf_a[bottom_idx + c] as i32;
                    let left = buf_a[left_idx + c] as i32;
                    let right = buf_a[right_idx + c] as i32;

                    let sharpened = (center * 5) - top - bottom - left - right;
                    buf_b[idx + c] = sharpened.clamp(0, 255) as u8;
                }
            }
        }
        std::mem::swap(&mut buf_a, &mut buf_b);
    }

    // 7. Morphology Dilation / Erosion
    if config.morphology != MorphologyOp::None {
        buf_b.copy_from_slice(&buf_a);
        let w = width as usize;
        let h = height as usize;
        let is_dilation = config.morphology == MorphologyOp::Dilation;
        for y in 1..(h.saturating_sub(1)) {
            let row_idx = y * w;
            for x in 1..(w.saturating_sub(1)) {
                let out_idx = (row_idx + x) * 4;
                for c in 0..3 {
                    let mut extreme = buf_a[out_idx + c];
                    for dy in -1..=1 {
                        let ny_row = ((y as isize + dy) as usize) * w;
                        for dx in -1..=1 {
                            let nx = (x as isize + dx) as usize;
                            let val = buf_a[(ny_row + nx) * 4 + c];
                            if is_dilation {
                                extreme = extreme.max(val);
                            } else {
                                extreme = extreme.min(val);
                            }
                        }
                    }
                    buf_b[out_idx + c] = extreme;
                }
            }
        }
        std::mem::swap(&mut buf_a, &mut buf_b);
    }

    // 8. Denoise Smoothing filter
    if config.denoise {
        buf_b.copy_from_slice(&buf_a);
        let w = width as usize;
        let h = height as usize;
        for y in 1..(h.saturating_sub(1)) {
            let row_idx = y * w;
            let prev_row_idx = (y - 1) * w;
            let next_row_idx = (y + 1) * w;
            for x in 1..(w.saturating_sub(1)) {
                let idx = (row_idx + x) * 4;
                let top_idx = (prev_row_idx + x) * 4;
                let bottom_idx = (next_row_idx + x) * 4;
                let left_idx = (row_idx + x - 1) * 4;
                let right_idx = (row_idx + x + 1) * 4;
                for c in 0..3 {
                    let mut sum = buf_a[idx + c] as u32;
                    sum += buf_a[top_idx + c] as u32;
                    sum += buf_a[bottom_idx + c] as u32;
                    sum += buf_a[left_idx + c] as u32;
                    sum += buf_a[right_idx + c] as u32;
                    // Corners
                    sum += buf_a[(prev_row_idx + x - 1) * 4 + c] as u32;
                    sum += buf_a[(prev_row_idx + x + 1) * 4 + c] as u32;
                    sum += buf_a[(next_row_idx + x - 1) * 4 + c] as u32;
                    sum += buf_a[(next_row_idx + x + 1) * 4 + c] as u32;
                    buf_b[idx + c] = (sum / 9) as u8;
                }
            }
        }
        std::mem::swap(&mut buf_a, &mut buf_b);
    }

    // 9. Resize Scale mapping (Bilinear Interpolation)
    let mut final_w = width;
    let mut final_h = height;
    if (config.resize_scale - 1.0).abs() > 0.01 {
        final_w = (width as f32 * config.resize_scale).max(1.0) as u32;
        final_h = (height as f32 * config.resize_scale).max(1.0) as u32;
        buf_b.resize((final_w * final_h * 4) as usize, 0);
        let orig_w = width as usize;
        let orig_h = height as usize;
        let inv_scale = 1.0 / config.resize_scale;
        for y in 0..final_h {
            let fy = (y as f32 * inv_scale).min((orig_h - 1) as f32);
            let y0 = fy as usize;
            let y1 = (y0 + 1).min(orig_h - 1);
            let wy = fy - y0 as f32;
            let wy_inv = 1.0 - wy;
            for x in 0..final_w {
                let fx = (x as f32 * inv_scale).min((orig_w - 1) as f32);
                let x0 = fx as usize;
                let x1 = (x0 + 1).min(orig_w - 1);
                let wx = fx - x0 as f32;
                let wx_inv = 1.0 - wx;

                let i00 = (y0 * orig_w + x0) * 4;
                let i10 = (y0 * orig_w + x1) * 4;
                let i01 = (y1 * orig_w + x0) * 4;
                let i11 = (y1 * orig_w + x1) * 4;

                let dst_idx = ((y * final_w + x) * 4) as usize;
                for c in 0..4 {
                    let v = buf_a[i00 + c] as f32 * wx_inv * wy_inv
                        + buf_a[i10 + c] as f32 * wx * wy_inv
                        + buf_a[i01 + c] as f32 * wx_inv * wy
                        + buf_a[i11 + c] as f32 * wx * wy;
                    buf_b[dst_idx + c] = v.clamp(0.0, 255.0) as u8;
                }
            }
        }
        std::mem::swap(&mut buf_a, &mut buf_b);
    }

    // 10. Deskew via Projection-Profile Angle Estimation
    // Finds the dominant text skew angle by maximizing horizontal projection variance,
    // then rotates the image back to horizontal using affine transform with bilinear interpolation.
    if config.deskew && final_w > 4 && final_h > 4 {
        let w = final_w as usize;
        let h = final_h as usize;

        // Build grayscale luminance buffer for angle estimation
        let mut gray = vec![0u8; w * h];
        for i in 0..(w * h) {
            let idx = i * 4;
            gray[i] = ((buf_a[idx] as u32 * 77
                + buf_a[idx + 1] as u32 * 150
                + buf_a[idx + 2] as u32 * 29)
                >> 8) as u8;
        }

        // Sweep angles from -15.0 to +15.0 degrees in 0.5-degree steps.
        // For each angle, compute the variance of the horizontal projection profile.
        // The angle that yields the highest variance aligns text rows horizontally.
        let mut best_angle: f32 = 0.0;
        let mut best_variance: f64 = 0.0;

        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;

        // Iterate candidate angles
        let mut angle = -15.0_f32;
        while angle <= 15.0 {
            let rad = angle.to_radians();
            let cos_a = rad.cos();
            let sin_a = rad.sin();

            // Accumulate horizontal projection (sum of dark pixels per row)
            let mut projection = vec![0u32; h];
            for row in 0..h {
                let mut row_sum = 0u32;
                for col in 0..w {
                    // Reverse-map destination pixel to source
                    let dx = col as f32 - cx;
                    let dy = row as f32 - cy;
                    let sx = (cos_a * dx + sin_a * dy + cx) as isize;
                    let sy = (-sin_a * dx + cos_a * dy + cy) as isize;
                    if sx >= 0 && sx < w as isize && sy >= 0 && sy < h as isize {
                        // Invert: dark pixels contribute more (text is typically dark)
                        row_sum += 255 - gray[sy as usize * w + sx as usize] as u32;
                    }
                }
                projection[row] = row_sum;
            }

            // Compute variance of the projection
            let n = h as f64;
            let sum: f64 = projection.iter().map(|v| *v as f64).sum();
            let mean = sum / n;
            let variance: f64 = projection
                .iter()
                .map(|v| {
                    let diff = *v as f64 - mean;
                    diff * diff
                })
                .sum::<f64>()
                / n;

            if variance > best_variance {
                best_variance = variance;
                best_angle = angle;
            }

            angle += 0.5;
        }

        // Only apply rotation if the detected skew exceeds 0.3 degrees
        if best_angle.abs() > 0.3 {
            tracing::debug!(angle = %best_angle, "Deskew: rotating image to correct skew");

            let rad = (-best_angle).to_radians(); // negate to correct
            let cos_a = rad.cos();
            let sin_a = rad.sin();

            buf_b.resize(buf_a.len(), 255);
            // Fill with white background
            for chunk in buf_b.chunks_exact_mut(4) {
                chunk[0] = 255;
                chunk[1] = 255;
                chunk[2] = 255;
                chunk[3] = 255;
            }

            for dy in 0..h {
                for dx in 0..w {
                    let fx = cos_a * (dx as f32 - cx) + sin_a * (dy as f32 - cy) + cx;
                    let fy = -sin_a * (dx as f32 - cx) + cos_a * (dy as f32 - cy) + cy;

                    // Bilinear sample from source
                    if fx >= 0.0 && fx < (w - 1) as f32 && fy >= 0.0 && fy < (h - 1) as f32 {
                        let x0 = fx as usize;
                        let y0 = fy as usize;
                        let x1 = x0 + 1;
                        let y1 = y0 + 1;
                        let wx = fx - x0 as f32;
                        let wy = fy - y0 as f32;
                        let wx_inv = 1.0 - wx;
                        let wy_inv = 1.0 - wy;

                        let i00 = (y0 * w + x0) * 4;
                        let i10 = (y0 * w + x1) * 4;
                        let i01 = (y1 * w + x0) * 4;
                        let i11 = (y1 * w + x1) * 4;
                        let dst = (dy * w + dx) * 4;

                        for c in 0..4 {
                            let v = buf_a[i00 + c] as f32 * wx_inv * wy_inv
                                + buf_a[i10 + c] as f32 * wx * wy_inv
                                + buf_a[i01 + c] as f32 * wx_inv * wy
                                + buf_a[i11 + c] as f32 * wx * wy;
                            buf_b[dst + c] = v.clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
            std::mem::swap(&mut buf_a, &mut buf_b);
        }
    }

    let result = (buf_a.clone(), final_w, final_h);
    // Store buffers for next call
    REUSE_BUFFERS.with(|bufs| {
        *bufs.borrow_mut() = (Some(buf_a), Some(buf_b));
    });
    result
}
