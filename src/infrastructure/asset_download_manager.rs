use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub struct ModelAsset<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub path: &'a str,
}

pub const MANGA_MODELS: [ModelAsset<'static>; 10] = [
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
pub const PPOCR_MOBILE_MODELS: [ModelAsset<'static>; 3] = [
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



pub const PPOCR_DICT_JAPANESE: ModelAsset<'static> = ModelAsset {
    name: "PP-OCR Dictionary (Japanese)",
    url: "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/dict/japan_dict.txt",
    path: "models/ppocr/japan_dict.txt",
};

pub const PPOCR_DICT_KOREAN: ModelAsset<'static> = ModelAsset {
    name: "PP-OCR Dictionary (Korean)",
    url: "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/dict/korean_dict.txt",
    path: "models/ppocr/korean_dict.txt",
};

pub const PPOCR_DICT_THAI: ModelAsset<'static> = ModelAsset {
    name: "PP-OCR Dictionary (Thai)",
    url:
        "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/th_dict.txt",
    path: "models/ppocr/thai_dict.txt",
};

pub const PPOCR_DICT_LATIN: ModelAsset<'static> = ModelAsset {
    name: "PP-OCR Dictionary (Latin)",
    url: "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/dict/latin_dict.txt",
    path: "models/ppocr/latin_dict.txt",
};

pub const PPOCR_DICT_CYRILLIC: ModelAsset<'static> = ModelAsset {
    name: "PP-OCR Dictionary (Cyrillic)",
    url: "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/dict/cyrillic_dict.txt",
    path: "models/ppocr/cyrillic_dict.txt",
};

pub const BUBBLE_YOLO_MODEL: ModelAsset<'static> = ModelAsset {
    name: "YOLO Bubble Detector (Manga-Bubble-YOLO)",
    url: "https://huggingface.co/Kiuyha/Manga-Bubble-YOLO/resolve/main/onnx/yolo26n.onnx",
    path: "models/bubble-yolo/yolo26n.onnx",
};

pub const CRAFT_TEXT_DETECTOR_MODEL: ModelAsset<'static> = ModelAsset {
    name: "CRAFT Text Detector",
    url: "https://huggingface.co/ml6team/craft-onnx/resolve/main/craft.onnx",
    path: "models/craft/craft.onnx",
};

pub fn check_bubble_yolo_exists() -> bool {
    let mut p = PathBuf::from(BUBBLE_YOLO_MODEL.path);
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            p = exe_dir.join(BUBBLE_YOLO_MODEL.path);
        }
    }
    p.exists()
        && fs::metadata(&p)
            .map(|m| m.len() > 5 * 1024 * 1024)
            .unwrap_or(false)
}

pub fn check_craft_exists() -> bool {
    let mut p = PathBuf::from(CRAFT_TEXT_DETECTOR_MODEL.path);
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            p = exe_dir.join(CRAFT_TEXT_DETECTOR_MODEL.path);
        }
    }
    match fs::metadata(&p) {
        Ok(m) => m.is_file() && m.len() > 1024 * 1024,
        Err(_) => false,
    }
}

pub use crate::core::types::DownloadProgress;

/// Generic download helper that downloads a list of model assets with progress reporting.
async fn download_asset_list(
    assets: &[ModelAsset<'_>],
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
pub async fn download_models(
    progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
) -> Result<()> {
    download_asset_list(&MANGA_MODELS, &progress_tx).await?;

    let _ = progress_tx
        .send(DownloadProgress {
            current_file: "All files downloaded!".to_string(),
            progress: 1.0,
            is_downloading: false,
            error: None,
        })
        .await;

    Ok(())
}

/// Download PP-OCR models for Built-in PaddleOCR.
pub async fn download_ppocr_models(
    progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
) -> Result<()> {
    let settings = crate::infrastructure::settings::load_settings().unwrap_or_default();

    // 1. Detection Model URL
    let det_url = PPOCR_MOBILE_MODELS[0].url;

    // 2. Recognition Model URL
    let rec_url = match settings.ppocr_model {
        crate::infrastructure::settings::PpocrModelSuite::CnEnMobile => PPOCR_MOBILE_MODELS[1].url,
        
        crate::infrastructure::settings::PpocrModelSuite::JapaneseMobile =>
            "https://huggingface.co/cycloneboy/japan_PP-OCRv4_rec_infer/resolve/main/model.onnx",

        crate::infrastructure::settings::PpocrModelSuite::KoreanMobile =>
            "https://huggingface.co/cycloneboy/korean_PP-OCRv4_rec_infer/resolve/main/model.onnx",

        crate::infrastructure::settings::PpocrModelSuite::ThaiMobile =>
            "https://huggingface.co/itextresearch/itext-th_PP-OCRv5_mobile_rec_infer/resolve/main/inference.onnx",

        crate::infrastructure::settings::PpocrModelSuite::LatinMobile =>
            "https://huggingface.co/cycloneboy/latin_PP-OCRv3_rec_infer/resolve/main/model.onnx",

        crate::infrastructure::settings::PpocrModelSuite::CyrillicMobile =>
            "https://huggingface.co/cycloneboy/cyrillic_PP-OCRv3_rec_infer/resolve/main/model.onnx",
    };

    // 3. Dictionary URL
    let dict_url = match settings.ppocr_model {
        crate::infrastructure::settings::PpocrModelSuite::CnEnMobile => {
            PPOCR_MOBILE_MODELS[2].url
        }

        crate::infrastructure::settings::PpocrModelSuite::JapaneseMobile => {
            PPOCR_DICT_JAPANESE.url
        }

        crate::infrastructure::settings::PpocrModelSuite::KoreanMobile => PPOCR_DICT_KOREAN.url,

        crate::infrastructure::settings::PpocrModelSuite::ThaiMobile => PPOCR_DICT_THAI.url,

        crate::infrastructure::settings::PpocrModelSuite::LatinMobile => PPOCR_DICT_LATIN.url,

        crate::infrastructure::settings::PpocrModelSuite::CyrillicMobile => {
            PPOCR_DICT_CYRILLIC.url
        }
    };

    let folder_name = settings.ppocr_model.folder_name();

    // Construct persistent path names within isolated subset directories
    let base_p = format!("models/ppocr/{}", folder_name);
    let det_p_owned = format!("{}/det.onnx", base_p);
    let rec_p_owned = format!("{}/rec.onnx", base_p);
    let dict_p_owned = format!("{}/dict.txt", base_p);

    let assets = [
        ModelAsset {
            name: "PP-OCR Detection",
            url: det_url,
            path: &det_p_owned,
        },
        ModelAsset {
            name: "PP-OCR Recognition",
            url: rec_url,
            path: &rec_p_owned,
        },
        ModelAsset {
            name: "PP-OCR Dictionary",
            url: dict_url,
            path: &dict_p_owned,
        },
    ];

    download_asset_list(&assets, &progress_tx).await?;

    let _ = progress_tx
        .send(DownloadProgress {
            current_file: format!("PP-OCR suite '{}' downloaded successfully!", folder_name),
            progress: 1.0,
            is_downloading: false,
            error: None,
        })
        .await;

    Ok(())
}

pub async fn download_bubble_yolo_model(
    progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
) -> Result<()> {
    download_asset_list(&[BUBBLE_YOLO_MODEL], &progress_tx).await?;

    let _ = progress_tx
        .send(DownloadProgress {
            current_file: "Bubble YOLO model downloaded successfully!".to_string(),
            progress: 1.0,
            is_downloading: false,
            error: None,
        })
        .await;

    Ok(())
}

pub async fn download_craft_model(
    progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
) -> Result<()> {
    download_asset_list(&[CRAFT_TEXT_DETECTOR_MODEL], &progress_tx).await?;

    let _ = progress_tx
        .send(DownloadProgress {
            current_file: "CRAFT Text Detector model downloaded successfully!".to_string(),
            progress: 1.0,
            is_downloading: false,
            error: None,
        })
        .await;

    Ok(())
}
