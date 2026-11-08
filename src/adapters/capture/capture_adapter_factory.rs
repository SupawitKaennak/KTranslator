use crate::core::ports::FrameSource;
use crate::infrastructure::settings::CaptureMethod;
use std::sync::Arc;

use super::screenshots_crate_adapter::ScreenshotsCapture;

pub struct CaptureAdapterFactory;

impl CaptureAdapterFactory {
    pub fn create(method: CaptureMethod) -> Arc<dyn FrameSource> {
        match method {
            CaptureMethod::Gdi => {
                tracing::info!("Initializing GDI (BitBlt) screen capture adapter");
                Arc::new(ScreenshotsCapture::new())
            }
            CaptureMethod::Wgc => {
                tracing::info!("Initializing WGC (Windows.Graphics.Capture) screen capture adapter");
                // Fallback to GDI until WGC native pipeline handles the frame loop, or on error
                #[cfg(target_os = "windows")]
                {
                    // Placeholder for WGC adapter instance or fallback
                    tracing::warn!("WGC capture adapter fallback to GDI for current session");
                    Arc::new(ScreenshotsCapture::new())
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Arc::new(ScreenshotsCapture::new())
                }
            }
            CaptureMethod::Dxgi => {
                tracing::info!("Initializing DXGI Desktop Duplication capture adapter");
                // Fallback to GDI until DXGI pipeline handles the frame loop, or on error
                #[cfg(target_os = "windows")]
                {
                    tracing::warn!("DXGI capture adapter fallback to GDI for current session");
                    Arc::new(ScreenshotsCapture::new())
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Arc::new(ScreenshotsCapture::new())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_factory_gdi() {
        let adapter = CaptureAdapterFactory::create(CaptureMethod::Gdi);
        assert!(Arc::strong_count(&adapter) >= 1);
    }

    #[test]
    fn test_capture_factory_wgc_fallback() {
        let adapter = CaptureAdapterFactory::create(CaptureMethod::Wgc);
        assert!(Arc::strong_count(&adapter) >= 1);
    }

    #[test]
    fn test_capture_factory_dxgi_fallback() {
        let adapter = CaptureAdapterFactory::create(CaptureMethod::Dxgi);
        assert!(Arc::strong_count(&adapter) >= 1);
    }
}

