use anyhow::{Context, Result};
use screenshots::Screen;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::core::{
    ports::{FrameRgba, FrameSource},
    types::Rect,
};

#[cfg(target_os = "windows")]
struct SafeManager {
    manager: dxgcap::DXGIManager,
    last_pixels: Vec<dxgcap::BGRA8>,
    last_dims: (usize, usize),
}

#[cfg(target_os = "windows")]
unsafe impl Send for SafeManager {}
#[cfg(target_os = "windows")]
unsafe impl Sync for SafeManager {}

/// Hybrid screen-capture adapter.
/// Uses `dxgcap` (DXGI) for high-performance capture on Windows.
/// Falls back to `screenshots` crate for cross-platform support.
pub struct ScreenshotsCapture {
    screen_cache: Mutex<Option<(Instant, Vec<Screen>)>>,
    #[cfg(target_os = "windows")]
    dxgi_cache: Mutex<HashMap<u32, SafeManager>>,
}

impl ScreenshotsCapture {
    pub fn new() -> Self {
        Self {
            screen_cache: Mutex::new(None),
            #[cfg(target_os = "windows")]
            dxgi_cache: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(target_os = "windows")]
    fn get_dxgi_index(target_id: u32, screens: &[Screen]) -> usize {
        if let Some(s) = screens.iter().find(|s| s.display_info.id == target_id) {
            if s.display_info.is_primary {
                return 0;
            }
        }
        let mut non_primary: Vec<_> = screens.iter().filter(|s| !s.display_info.is_primary).collect();
        non_primary.sort_by_key(|s| s.display_info.id);
        if let Some(idx) = non_primary.iter().position(|s| s.display_info.id == target_id) {
            return idx + 1;
        }
        0
    }

    #[cfg(target_os = "windows")]
    fn capture_windows(&self, rect: Rect, display_id: u32, screens: &[Screen]) -> Result<FrameRgba> {
        let screen = screens
            .iter()
            .find(|s| s.display_info.id == display_id)
            .or_else(|| screens.iter().find(|s| s.display_info.is_primary))
            .or_else(|| screens.first())
            .ok_or_else(|| anyhow::anyhow!("no display found"))?;
            
        let dxgi_idx = Self::get_dxgi_index(screen.display_info.id, screens);

        let mut dxgi_guard = self.dxgi_cache.lock().unwrap();
        let safe_m = dxgi_guard.entry(display_id).or_insert_with(|| {
            let mut m = dxgcap::DXGIManager::new(50).unwrap_or_else(|_| dxgcap::DXGIManager::new(500).unwrap());
            m.set_capture_source_index(dxgi_idx);
            SafeManager {
                manager: m,
                last_pixels: Vec::new(),
                last_dims: (0, 0),
            }
        });

        match safe_m.manager.capture_frame() {
            Ok(res) => {
                safe_m.last_pixels = res.0;
                safe_m.last_dims = res.1;
            }
            Err(e) => {
                let err_str = format!("{:?}", e);
                if err_str.contains("Timeout") && !safe_m.last_pixels.is_empty() {
                    // Reuse last frame
                } else if err_str.contains("AccessLost") || err_str.contains("AccessDenied") {
                    if let Ok(mut m) = dxgcap::DXGIManager::new(100) {
                        m.set_capture_source_index(dxgi_idx);
                        if let Ok(res) = m.capture_frame() {
                            safe_m.manager = m;
                            safe_m.last_pixels = res.0;
                            safe_m.last_dims = res.1;
                        }
                    }
                } else if safe_m.last_pixels.is_empty() {
                    anyhow::bail!("DXGI capture failed: {:?}", e);
                }
            }
        }

        let pixels = &safe_m.last_pixels;
        let (img_w, img_h) = safe_m.last_dims;
        if img_h == 0 || pixels.is_empty() { anyhow::bail!("Captured empty frame"); }

        let rel_x = (rect.x - screen.display_info.x as f32).max(0.0) as u32;
        let rel_y = (rect.y - screen.display_info.y as f32).max(0.0) as u32;
        let crop_w = rect.w.max(1.0) as u32;
        let crop_h = rect.h.max(1.0) as u32;

        let img_w = img_w as u32;
        let img_h = img_h as u32;
        let padded_w = (pixels.len() / img_h as usize) as u32;

        let safe_x = rel_x.min(img_w.saturating_sub(1));
        let safe_y = rel_y.min(img_h.saturating_sub(1));
        let safe_w = crop_w.min(img_w - safe_x);
        let safe_h = crop_h.min(img_h - safe_y);

        let mut cropped_data = Vec::with_capacity((safe_w * safe_h * 4) as usize);
        for row in 0..safe_h {
            let start = ((safe_y + row) * padded_w + safe_x) as usize;
            let end = start + (safe_w as usize);
            if end <= pixels.len() {
                for p in &pixels[start..end] {
                    cropped_data.push(p.r);
                    cropped_data.push(p.g);
                    cropped_data.push(p.b);
                    cropped_data.push(255);
                }
            } else {
                cropped_data.resize(cropped_data.len() + (safe_w as usize * 4), 0);
            }
        }

        Ok(FrameRgba { width: safe_w, height: safe_h, data: cropped_data })
    }

    #[allow(dead_code)]
    fn capture_cross_platform(&self, rect: Rect, display_id: u32, screens: &[Screen]) -> Result<FrameRgba> {
        let screen = screens
            .iter()
            .find(|s| s.display_info.id == display_id)
            .or_else(|| screens.iter().find(|s| s.display_info.is_primary))
            .or_else(|| screens.first())
            .ok_or_else(|| anyhow::anyhow!("no display found"))?;

        // Fallback to the 'screenshots' crate which is cross-platform but slower.
        let rel_x = (rect.x - screen.display_info.x as f32).max(0.0) as i32;
        let rel_y = (rect.y - screen.display_info.y as f32).max(0.0) as i32;
        let image = screen.capture_area(rel_x, rel_y, rect.w as u32, rect.h as u32)
            .context("cross-platform capture failed")?;

        Ok(FrameRgba {
            width: image.width(),
            height: image.height(),
            data: image.into_raw(),
        })
    }
}

impl FrameSource for ScreenshotsCapture {
    fn capture_rect(&self, rect: Rect, display_id: u32) -> Result<FrameRgba> {
        let now = Instant::now();
        let mut screen_guard = self.screen_cache.lock().unwrap();
        
        let screens = if let Some((last_refresh, cached_screens)) = &*screen_guard {
            if now.duration_since(*last_refresh) > Duration::from_secs(2) {
                let fresh = Screen::all().context("enumerate screens")?;
                *screen_guard = Some((now, fresh));
                &screen_guard.as_ref().unwrap().1
            } else {
                cached_screens
            }
        } else {
            let fresh = Screen::all().context("enumerate screens")?;
            *screen_guard = Some((now, fresh));
            &screen_guard.as_ref().unwrap().1
        };

        #[cfg(target_os = "windows")]
        {
            self.capture_windows(rect, display_id, screens)
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.capture_cross_platform(rect, display_id, screens)
        }
    }
}
