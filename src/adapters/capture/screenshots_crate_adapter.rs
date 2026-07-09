use parking_lot::Mutex;
use screenshots::Screen;
use std::time::{Duration, Instant};

use crate::core::{
    ports::{FrameRgba, FrameSource},
    types::Rect,
};

/// Universal screen-capture adapter.
/// Uses `screenshots` crate which leverages GDI (BitBlt) on Windows.
/// GDI is highly stable and does not conflict with DXGI Desktop Duplication 
/// sessions opened by streaming apps like Discord or OBS.
pub struct ScreenshotsCapture {
    screen_cache: Mutex<Option<(Instant, Vec<Screen>)>>,
}

impl ScreenshotsCapture {
    pub fn new() -> Self {
        Self {
            screen_cache: Mutex::new(None),
        }
    }

    fn capture_impl(
        &self,
        rect: Rect,
        display_id: u32,
        screens: &[Screen],
    ) -> anyhow::Result<FrameRgba> {
        let screen = screens
            .iter()
            .find(|s| s.display_info.id == display_id)
            .or_else(|| screens.iter().find(|s| s.display_info.is_primary))
            .or_else(|| screens.first())
            .ok_or_else(|| anyhow::anyhow!("no display found".to_string()))?;

        // GDI/Cross-platform capture
        let rel_x = (rect.x - screen.display_info.x as f32).max(0.0) as i32;
        let rel_y = (rect.y - screen.display_info.y as f32).max(0.0) as i32;
        
        let image = screen
            .capture_area(rel_x, rel_y, rect.w as u32, rect.h as u32)
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "GDI/Cross-platform capture failed: {:?}",
                    e
                ))
            })?;

        Ok(FrameRgba {
            width: image.width(),
            height: image.height(),
            data: std::sync::Arc::new(image.into_raw()),
        })
    }
}

impl FrameSource for ScreenshotsCapture {
    fn capture_rect(
        &self,
        rect: Rect,
        display_id: u32,
    ) -> anyhow::Result<FrameRgba> {
        let now = Instant::now();
        let mut screen_guard = self.screen_cache.lock();

        let screens = if let Some((last_refresh, cached_screens)) = &mut *screen_guard {
            if now.duration_since(*last_refresh) > Duration::from_secs(2) {
                if let Ok(fresh) = Screen::all() {
                    *last_refresh = now;
                    *cached_screens = fresh;
                }
            }
            cached_screens
        } else {
            let fresh = Screen::all().map_err(|e| {
                anyhow::anyhow!(format!("enumerate screens: {:?}", e))
            })?;
            let (_, cached) = screen_guard.insert((now, fresh));
            cached
        };

        self.capture_impl(rect, display_id, screens)
    }
}
