use anyhow::{Context, Result};
use std::sync::Mutex;
use crate::core::{
    ports::{FrameRgba, OcrEngine as OcrEngineTrait, OcrTextLine},
    types::LanguageTag,
};

#[cfg(feature = "tesseract")]
use leptess::LepTess;

pub struct TesseractOcr {
    #[cfg(feature = "tesseract")]
    api: Mutex<LepTess>,
    #[cfg(not(feature = "tesseract"))]
    _stub: (),
}

impl TesseractOcr {
    #[allow(unused_variables)]
    pub fn new(lang: &str) -> Result<Self> {
        #[cfg(feature = "tesseract")]
        {
            let api = LepTess::new(None, lang).context("Failed to initialize Tesseract")?;
            Ok(Self {
                api: Mutex::new(api),
            })
        }
        #[cfg(not(feature = "tesseract"))]
        {
            Ok(Self { _stub: () })
        }
    }

    #[allow(dead_code)]
    fn lang_tag_to_tess(tag: Option<&LanguageTag>) -> &str {
        match tag.map(|t| t.0.as_str()) {
            Some("en") => "eng",
            Some("ja") => "jpn",
            Some("th") => "tha",
            Some("zh") => "chi_sim",
            _ => "eng", // default
        }
    }
}

impl OcrEngineTrait for TesseractOcr {
    #[allow(unused_variables)]
    fn recognize(&self, frame: FrameRgba, lang_hint: Option<&LanguageTag>) -> Result<String> {
        #[cfg(feature = "tesseract")]
        {
            let mut api = self.api.lock().unwrap();
            
            // Set image from memory (RGBA8)
            api.set_image_from_mem(&frame.data, frame.width as i32, frame.height as i32, 4, (frame.width * 4) as i32)
                .context("Failed to set image for Tesseract")?;
                
            let text = api.get_utf8_text().context("Tesseract failed to extract text")?;
            Ok(text)
        }
        #[cfg(not(feature = "tesseract"))]
        {
            anyhow::bail!("Tesseract feature is not enabled")
        }
    }

    #[allow(unused_variables)]
    fn recognize_lines(&self, frame: FrameRgba, lang_hint: Option<&LanguageTag>) -> Result<Vec<OcrTextLine>> {
        #[cfg(feature = "tesseract")]
        {
            let text = self.recognize(frame, lang_hint)?;
            if text.trim().is_empty() { return Ok(vec![]); }
            
            Ok(vec![OcrTextLine {
                text,
                x: 0.0, y: 0.0, w: frame.width as f32, h: frame.height as f32,
            }])
        }
        #[cfg(not(feature = "tesseract"))]
        {
            anyhow::bail!("Tesseract feature is not enabled")
        }
    }
}
