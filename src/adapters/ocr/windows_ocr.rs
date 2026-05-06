use anyhow::Result;
use image::{ImageBuffer, Rgba};
use std::sync::Arc;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Storage::Streams::InMemoryRandomAccessStream;
use windows::Globalization::Language;
use windows::Media::Ocr::OcrEngine;
use std::future::IntoFuture;

use crate::core::{
    ports::{FrameRgba, OcrEngine as OcrEngineTrait, OcrTextLine},
    types::LanguageTag,
};

pub struct WindowsOcr {
    engine: Arc<OcrEngine>,
}

impl WindowsOcr {
    pub fn new() -> Self {
        let lang = Language::CreateLanguage(&windows::core::HSTRING::from("en-US")).unwrap();
        let engine = OcrEngine::TryCreateFromLanguage(&lang).unwrap();
        Self { engine: Arc::new(engine) }
    }

    fn preprocess(frame: FrameRgba) -> (FrameRgba, f32) {
        let Some(img) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(frame.width, frame.height, frame.data) else {
            return (FrameRgba { width: frame.width, height: frame.height, data: Vec::new() }, 1.0);
        };

        let gray_img_base = image::DynamicImage::ImageRgba8(img).into_luma8();
        
        let (processed_img, final_scale) = if frame.height < 1000 {
            let scale = 3.0;
            let new_w = (frame.width as f32 * scale) as u32;
            let new_h = (frame.height as f32 * scale) as u32;
            let resized = image::imageops::resize(&gray_img_base, new_w, new_h, image::imageops::FilterType::Triangle);
            (resized, scale)
        } else {
            (gray_img_base, 1.0)
        };

        let mut final_img = processed_img;
        let mut min_v = 255u8;
        let mut max_v = 0u8;
        let mut sum_v = 0u64;
        
        for pixel in final_img.pixels() {
            let v = pixel.0[0];
            if v < min_v { min_v = v; }
            if v > max_v { max_v = v; }
            sum_v += v as u64;
        }
        
        let total_pixels = final_img.width() as u64 * final_img.height() as u64;
        let avg_v = if total_pixels > 0 { (sum_v / total_pixels) as u8 } else { 128 };
        let should_invert = avg_v < 100;

        if max_v > min_v {
            let range = (max_v - min_v) as f32;
            for pixel in final_img.pixels_mut() {
                let v = pixel.0[0];
                let mut normalized = ((v - min_v) as f32 / range * 255.0) as u8;
                if should_invert {
                    normalized = 255 - normalized;
                }
                pixel.0[0] = normalized;
            }
        }

        let width = final_img.width();
        let height = final_img.height();
        let final_rgba = image::DynamicImage::ImageLuma8(final_img).to_rgba8();
        
        (FrameRgba { width, height, data: final_rgba.into_raw() }, final_scale)
    }
}

use std::sync::LazyLock;

// Robust global runtime to handle Windows async calls from any thread (including non-tokio threads)
static GLOBAL_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create global OCR tokio runtime")
});

fn wait_for<F: std::future::Future>(f: F) -> F::Output {
    GLOBAL_RUNTIME.block_on(f)
}

impl OcrEngineTrait for WindowsOcr {
    fn recognize(&self, frame: FrameRgba, _lang_hint: Option<&LanguageTag>) -> Result<String> {
        let (processed, _) = Self::preprocess(frame);
        
        // Encode raw pixels to PNG in memory
        let mut png_buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_buffer);
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(processed.width, processed.height, processed.data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        image::DynamicImage::ImageRgba8(img).write_to(&mut cursor, image::ImageFormat::Png)?;

        let stream = InMemoryRandomAccessStream::new()?;
        let writer = stream.GetOutputStreamAt(0)?;
        {
            let data_writer = windows::Storage::Streams::DataWriter::CreateDataWriter(&writer)?;
            data_writer.WriteBytes(&png_buffer)?;
            wait_for(data_writer.StoreAsync()?.into_future())?;
            wait_for(data_writer.FlushAsync()?.into_future())?;
        }

        let decoder = wait_for(BitmapDecoder::CreateWithIdAsync(windows::Graphics::Imaging::BitmapDecoder::PngDecoderId()?, &stream)?.into_future())?;
        let bitmap = wait_for(decoder.GetSoftwareBitmapAsync()?.into_future())?;

        let result = wait_for(self.engine.RecognizeAsync(&bitmap)?.into_future())?;
        Ok(result.Text()?.to_string())
    }

    fn recognize_lines(&self, frame: FrameRgba, _lang_hint: Option<&LanguageTag>) -> Result<Vec<OcrTextLine>> {
        let (processed, scale) = Self::preprocess(frame);
        
        // Encode raw pixels to PNG in memory
        let mut png_buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_buffer);
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(processed.width, processed.height, processed.data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        image::DynamicImage::ImageRgba8(img).write_to(&mut cursor, image::ImageFormat::Png)?;

        let stream = InMemoryRandomAccessStream::new()?;
        let writer = stream.GetOutputStreamAt(0)?;
        {
            let data_writer = windows::Storage::Streams::DataWriter::CreateDataWriter(&writer)?;
            data_writer.WriteBytes(&png_buffer)?;
            wait_for(data_writer.StoreAsync()?.into_future())?;
            wait_for(data_writer.FlushAsync()?.into_future())?;
        }

        let decoder = wait_for(BitmapDecoder::CreateWithIdAsync(windows::Graphics::Imaging::BitmapDecoder::PngDecoderId()?, &stream)?.into_future())?;
        let bitmap = wait_for(decoder.GetSoftwareBitmapAsync()?.into_future())?;

        let result = wait_for(self.engine.RecognizeAsync(&bitmap)?.into_future())?;
        let lines_api = result.Lines()?;

        let mut out_lines = Vec::new();
        for line in lines_api {
            let text = line.Text()?.to_string();
            
            let words = line.Words()?;
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;
            
            let mut has_words = false;
            for word in words {
                let rect = word.BoundingRect()?;
                min_x = min_x.min(rect.X);
                min_y = min_y.min(rect.Y);
                max_x = max_x.max(rect.X + rect.Width);
                max_y = max_y.max(rect.Y + rect.Height);
                has_words = true;
            }
            
            if has_words {
                out_lines.push(OcrTextLine {
                    text,
                    x: min_x / scale,
                    y: min_y / scale,
                    w: (max_x - min_x) / scale,
                    h: (max_y - min_y) / scale,
                });
            }
        }

        Ok(out_lines)
    }
}
