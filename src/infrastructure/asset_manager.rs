use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use std::io::Write;

#[derive(Clone, Copy)]
pub struct ModelAsset {
    pub name: &'static str,
    pub url: &'static str,
    pub path: &'static str,
}

pub const MANGA_MODELS: [ModelAsset; 10] = [
    ModelAsset {
        name: "YOLO Text Detector",
        url: "https://huggingface.co/deepghs/manga109_yolo/resolve/main/v2023.12.07_s/model.onnx",
        path: "models/manga-ocr/manga109_yolo_s.onnx",
    },
    ModelAsset {
        name: "Manga-OCR Encoder",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/encoder_model.onnx",
        path: "models/manga-ocr/encoder_model.onnx",
    },
    ModelAsset {
        name: "Manga-OCR Decoder",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/decoder_model.onnx",
        path: "models/manga-ocr/decoder_model.onnx",
    },
    ModelAsset {
        name: "Manga-OCR Tokenizer",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/tokenizer.json",
        path: "models/manga-ocr/tokenizer.json",
    },
    ModelAsset {
        name: "Manga-OCR Config",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/config.json",
        path: "models/manga-ocr/config.json",
    },
    ModelAsset {
        name: "Manga-OCR Preprocessor",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/preprocessor_config.json",
        path: "models/manga-ocr/preprocessor_config.json",
    },
    ModelAsset {
        name: "Manga-OCR Generation Config",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/generation_config.json",
        path: "models/manga-ocr/generation_config.json",
    },
    ModelAsset {
        name: "Manga-OCR Special Tokens",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/special_tokens_map.json",
        path: "models/manga-ocr/special_tokens_map.json",
    },
    ModelAsset {
        name: "Manga-OCR Tokenizer Config",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/tokenizer_config.json",
        path: "models/manga-ocr/tokenizer_config.json",
    },
    ModelAsset {
        name: "Manga-OCR Vocab",
        url: "https://huggingface.co/l0wgear/manga-ocr-2025-onnx/resolve/main/vocab.txt",
        path: "models/manga-ocr/vocab.txt",
    },
];

/// PP-OCRv4 Mobile models for Built-in PaddleOCR (det + rec + dict).
/// Total ~15MB — covers Chinese+English with high accuracy.
pub const PPOCR_MOBILE_MODELS: [ModelAsset; 3] = [
    ModelAsset {
        name: "PP-OCRv4 Detection (Mobile)",
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv4_mobile_det.onnx",
        path: "models/ppocr/det.onnx",
    },
    ModelAsset {
        name: "PP-OCRv4 Recognition (Mobile)",
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv4_mobile_rec.onnx",
        path: "models/ppocr/rec.onnx",
    },
    ModelAsset {
        name: "PP-OCR Dictionary (Standard)",
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocr_keys_v1.txt",
        path: "models/ppocr/dict.txt",
    },
];

/// PP-OCRv4 Server models for maximum precision on complex game/manga fonts.
/// Det (~108MB) + Rec (~86MB).
pub const PPOCR_SERVER_MODELS: [ModelAsset; 2] = [
    ModelAsset {
        name: "PP-OCRv4 Detection (Server)",
        url: "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_server_infer.onnx",
        path: "models/ppocr/det_server.onnx",
    },
    ModelAsset {
        name: "PP-OCRv4 Recognition (Server)",
        url: "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_rec_server_infer.onnx",
        path: "models/ppocr/rec_server.onnx",
    },
];

pub const PPOCR_DICT_JAPANESE: ModelAsset = ModelAsset {
    name: "PP-OCR Dictionary (Japanese)",
    url: "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/dict/japan_dict.txt",
    path: "models/ppocr/japan_dict.txt",
};

#[derive(Clone, Default, Debug)]
pub struct DownloadProgress {
    pub current_file: String,
    pub progress: f32, // 0.0 to 1.0
    pub is_downloading: bool,
    pub error: Option<String>,
}

/// Generic download helper that downloads a list of model assets with progress reporting.
async fn download_asset_list(
    assets: &[ModelAsset],
    progress_tx: &tokio::sync::mpsc::Sender<DownloadProgress>,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()?;
    
    for asset in assets {
        let mut dest_path = PathBuf::from(asset.path);
        
        // Ensure we download relative to the EXE directory for portability
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                dest_path = exe_dir.join(asset.path);
            }
        }

        // Skip if exists and is valid size
        if dest_path.exists() {
            if let Ok(meta) = fs::metadata(&dest_path) {
                let is_onnx = dest_path.extension().and_then(|s| s.to_str()) == Some("onnx");
                let threshold = if is_onnx { 10 * 1024 * 1024 } else { 5 * 1024 }; // 10MB for ONNX, 5KB for others
                
                if meta.len() > threshold {
                    continue;
                } else {
                    let _ = fs::remove_file(&dest_path);
                }
            }
        }

        // Create directory
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut progress = DownloadProgress {
            current_file: asset.name.to_string(),
            progress: 0.0,
            is_downloading: true,
            error: None,
        };
        let _ = progress_tx.send(progress.clone()).await;

        let mut response = client.get(asset.url).send().await?;
        let total_size = response.content_length().unwrap_or(0);
        
        let mut file = fs::File::create(&dest_path)?;
        let mut downloaded: u64 = 0;

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            
            if total_size > 0 {
                let new_prog = downloaded as f32 / total_size as f32;
                if (new_prog - progress.progress).abs() > 0.01 {
                    progress.progress = new_prog;
                    let _ = progress_tx.send(progress.clone()).await;
                }
            }
        }
    }

    Ok(())
}

/// Download Manga-OCR models.
pub async fn download_models(progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>) -> Result<()> {
    download_asset_list(&MANGA_MODELS, &progress_tx).await?;

    let _ = progress_tx.send(DownloadProgress {
        current_file: "All files downloaded!".to_string(),
        progress: 1.0,
        is_downloading: false,
        error: None,
    }).await;

    Ok(())
}

/// Download PP-OCR models for Built-in PaddleOCR.
pub async fn download_ppocr_models(progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>) -> Result<()> {
    let settings = crate::infrastructure::settings::load_settings().unwrap_or_default();
    
    let det_url = match settings.ppocr_variant {
        crate::infrastructure::settings::PpocrVariant::Mobile => PPOCR_MOBILE_MODELS[0].url,
        crate::infrastructure::settings::PpocrVariant::Server => PPOCR_SERVER_MODELS[0].url,
    };
    let rec_url = match settings.ppocr_variant {
        crate::infrastructure::settings::PpocrVariant::Mobile => PPOCR_MOBILE_MODELS[1].url,
        crate::infrastructure::settings::PpocrVariant::Server => PPOCR_SERVER_MODELS[1].url,
    };
    let dict_url = match settings.ppocr_dict {
        crate::infrastructure::settings::PpocrDictLanguage::Standard => PPOCR_MOBILE_MODELS[2].url,
        crate::infrastructure::settings::PpocrDictLanguage::Japanese => PPOCR_DICT_JAPANESE.url,
    };

    let folder_name = match (settings.ppocr_variant, settings.ppocr_dict) {
        (crate::infrastructure::settings::PpocrVariant::Mobile, crate::infrastructure::settings::PpocrDictLanguage::Standard) => "cn_en_mobile",
        (crate::infrastructure::settings::PpocrVariant::Mobile, crate::infrastructure::settings::PpocrDictLanguage::Japanese) => "mobile_japanese",
        (crate::infrastructure::settings::PpocrVariant::Server, crate::infrastructure::settings::PpocrDictLanguage::Standard) => "cn_en_server",
        (crate::infrastructure::settings::PpocrVariant::Server, crate::infrastructure::settings::PpocrDictLanguage::Japanese) => "server_japanese",
    };

    // Construct persistent path names within isolated subset directories
    let base_p = format!("models/ppocr/{}", folder_name);
    let det_p: &'static str = Box::leak(format!("{}/det.onnx", base_p).into_boxed_str());
    let rec_p: &'static str = Box::leak(format!("{}/rec.onnx", base_p).into_boxed_str());
    let dict_p: &'static str = Box::leak(format!("{}/dict.txt", base_p).into_boxed_str());

    let assets = [
        ModelAsset { name: "PP-OCR Detection", url: det_url, path: det_p },
        ModelAsset { name: "PP-OCR Recognition", url: rec_url, path: rec_p },
        ModelAsset { name: "PP-OCR Dictionary", url: dict_url, path: dict_p },
    ];

    download_asset_list(&assets, &progress_tx).await?;

    let _ = progress_tx.send(DownloadProgress {
        current_file: format!("PP-OCR suite '{}' downloaded successfully!", folder_name),
        progress: 1.0,
        is_downloading: false,
        error: None,
    }).await;

    Ok(())
}

