use crate::core::{
    ports::{FrameRgba, FrameSource},
    types::Rect,
};
use std::sync::{Arc, atomic::{AtomicU8, Ordering}};

use super::{
    dxgi_adapter::DxgiCapture,
    screenshots_crate_adapter::ScreenshotsCapture,
    wgc_adapter::WgcCapture,
};

pub struct DynamicCaptureAdapter {
    current_method: Arc<AtomicU8>,
    gdi_capture: Arc<ScreenshotsCapture>,
    dxgi_capture: Arc<DxgiCapture>,
    wgc_capture: Arc<WgcCapture>,
}

impl DynamicCaptureAdapter {
    pub fn new(shared_method: Arc<AtomicU8>) -> anyhow::Result<Self> {
        let gdi_capture = Arc::new(ScreenshotsCapture::new());
        
        let dxgi_capture = Arc::new(DxgiCapture::new().map_err(|e| {
            anyhow::anyhow!("Failed to initialize DXGI Capture Engine: {:?}", e)
        })?);
        
        let wgc_capture = Arc::new(WgcCapture::new().map_err(|e| {
            anyhow::anyhow!("Failed to initialize WGC Capture Engine: {:?}", e)
        })?);

        Ok(Self {
            current_method: shared_method,
            gdi_capture,
            dxgi_capture,
            wgc_capture,
        })
    }
}

impl FrameSource for DynamicCaptureAdapter {
    fn capture_rect(
        &self,
        rect: Rect,
        display_id: u32,
    ) -> anyhow::Result<FrameRgba> {
        let method_val = self.current_method.load(Ordering::Relaxed);
        
        match method_val {
            0 => self.gdi_capture.capture_rect(rect, display_id), // Gdi
            1 => self.dxgi_capture.capture_rect(rect, display_id), // Dxgi
            _ => self.wgc_capture.capture_rect(rect, display_id), // Wgc
        }
    }
}
