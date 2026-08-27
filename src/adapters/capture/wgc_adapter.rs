use crate::core::{
    ports::{FrameRgba, FrameSource},
    types::Rect,
};
use std::sync::Arc;
use parking_lot::Mutex;
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};
use std::thread;

pub struct WgcCapture {
    latest_frame: Arc<Mutex<Option<FrameRgba>>>,
}

struct WgcHandler {
    latest_frame: Arc<Mutex<Option<FrameRgba>>>,
    temp_buffer: Vec<u8>,
}

impl GraphicsCaptureApiHandler for WgcHandler {
    type Flags = Arc<Mutex<Option<FrameRgba>>>;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { 
            latest_frame: ctx.flags,
            temp_buffer: Vec::new(),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let width = frame.width();
        let height = frame.height();
        
        let buffer = frame.buffer()?;
        let bgra_slice = buffer.as_nopadding_buffer(&mut self.temp_buffer);
        
        let mut rgba_pixels = vec![0u8; bgra_slice.len()];
        
        for chunk in bgra_slice.chunks_exact(4).zip(rgba_pixels.chunks_exact_mut(4)) {
            let (src, dst) = (chunk.0, chunk.1);
            dst[0] = src[2]; // R
            dst[1] = src[1]; // G
            dst[2] = src[0]; // B
            dst[3] = src[3]; // A
        }

        let new_frame = FrameRgba {
            width,
            height,
            data: Arc::new(rgba_pixels),
        };

        *self.latest_frame.lock() = Some(new_frame);
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl WgcCapture {
    pub fn new() -> anyhow::Result<Self> {
        let latest_frame = Arc::new(Mutex::new(None));
        let frame_clone = latest_frame.clone();

        thread::spawn(move || {
            let primary_monitor = Monitor::primary().expect("No primary monitor");
            let settings = Settings::new(
                primary_monitor,
                CursorCaptureSettings::WithoutCursor,
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                frame_clone,
            );
            
            // Start capture blocks the thread until closed
            let _ = WgcHandler::start(settings);
        });

        // Give it a short moment to capture the first frame
        std::thread::sleep(std::time::Duration::from_millis(200));

        Ok(Self { latest_frame })
    }
}

impl FrameSource for WgcCapture {
    fn capture_rect(
        &self,
        rect: Rect,
        _display_id: u32,
    ) -> anyhow::Result<FrameRgba> {
        let frame_opt = self.latest_frame.lock().clone();
        let frame = frame_opt.ok_or_else(|| anyhow::anyhow!("WGC frame not ready yet"))?;
        
        // Crop the frame
        let width = frame.width as usize;
        let height = frame.height as usize;
        
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
                .copy_from_slice(&frame.data[src_row_start..src_row_start + row_bytes]);
        }

        Ok(FrameRgba {
            width: crop_w as u32,
            height: crop_h as u32,
            data: Arc::new(cropped_rgba),
        })
    }
}
