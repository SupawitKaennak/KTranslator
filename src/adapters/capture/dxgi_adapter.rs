use crate::core::{
    ports::{FrameRgba, FrameSource},
    types::Rect,
};
use dxgi_capture_rs::{DXGIManager, CaptureError};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct DxgiCapture {
    manager: Mutex<DXGIManager>,
}

impl DxgiCapture {
    pub fn new() -> anyhow::Result<Self> {
        // Initialize manager with a 1000ms timeout
        let manager = DXGIManager::new(1000)
            .map_err(|e| anyhow::anyhow!("Failed to initialize DXGIManager: {:?}", e))?;
            
        Ok(Self {
            manager: Mutex::new(manager),
        })
    }
}

impl FrameSource for DxgiCapture {
    fn capture_rect(
        &self,
        rect: Rect,
        _display_id: u32,
    ) -> anyhow::Result<FrameRgba> {
        let mut manager = self.manager.lock();
        
        let capture_result = manager.capture_frame();
        
        let (pixels, (width, height)) = match capture_result {
            Ok(res) => res,
            Err(CaptureError::AccessLost) => {
                // AccessLost happens if resolution changes, or if Desktop Duplication is invalidated.
                // We must recreate the DXGIManager to recover.
                *manager = DXGIManager::new(1000)
                    .map_err(|e| anyhow::anyhow!("DXGI reinit failed after AccessLost: {:?}", e))?;
                    
                manager.capture_frame()
                    .map_err(|e| anyhow::anyhow!("DXGI capture failed after reinit: {:?}", e))?
            }
            Err(e) => {
                return Err(anyhow::anyhow!("DXGI capture failed: {:?}", e));
            }
        };
            
        // `pixels` from dxgi-capture-rs is Vec<u8> in BGRA8 format.
        // We need RGBA8 for KTranslator FrameRgba.
        let flat_pixels: &[u8] = unsafe {
            std::slice::from_raw_parts(pixels.as_ptr() as *const u8, pixels.len() * 4)
        };
        
        let mut rgba_pixels = vec![0u8; pixels.len() * 4];
        for chunk in flat_pixels.chunks_exact(4).zip(rgba_pixels.chunks_exact_mut(4)) {
            let (src, dst) = (chunk.0, chunk.1);
            dst[0] = src[2]; // R
            dst[1] = src[1]; // G
            dst[2] = src[0]; // B
            dst[3] = src[3]; // A
        }
        
        // We need to crop it to `rect`
        // Ensure bounds are safe
        let start_x = (rect.x.max(0.0) as usize).min(width);
        let start_y = (rect.y.max(0.0) as usize).min(height);
        let end_x = (start_x + rect.w as usize).min(width);
        let end_y = (start_y + rect.h as usize).min(height);
        let crop_w = end_x - start_x;
        let crop_h = end_y - start_y;
        
        let mut cropped_rgba = vec![0u8; crop_w * crop_h * 4];
        
        for y in 0..crop_h {
            let src_row_start = ((start_y + y) * width + start_x) * 4;
            let dst_row_start = y * crop_w * 4;
            let row_bytes = crop_w * 4;
            
            cropped_rgba[dst_row_start..dst_row_start + row_bytes]
                .copy_from_slice(&rgba_pixels[src_row_start..src_row_start + row_bytes]);
        }

        Ok(FrameRgba {
            width: crop_w as u32,
            height: crop_h as u32,
            data: Arc::new(cropped_rgba),
        })
    }
}
