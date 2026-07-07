use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_ocr(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
    download_progress: &crate::core::types::DownloadProgress,
    download_trigger_tx: &std::sync::mpsc::Sender<crate::infrastructure::settings::OcrEngineType>,
) {
    ui.heading(i18n.tab_ocr);
    ui.add_space(8.0);

    super::section_header(ui, i18n.ocr_engine_mode_setup);
    ui.add_space(4.0);

    let modes = [
        (
            crate::infrastructure::settings::OcrMode::Game,
            i18n.mode_game,
        ),
        (
            crate::infrastructure::settings::OcrMode::Manga,
            i18n.mode_manga,
        ),
        (
            crate::infrastructure::settings::OcrMode::Document,
            i18n.mode_document,
        ),
    ];
    for (mode, label) in modes {
        ui.radio_value(&mut settings.ocr_mode, mode, label);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // Show engine config for the selected mode
    let (engine_ref, mode_name) = match settings.ocr_mode {
        crate::infrastructure::settings::OcrMode::Game => {
            (&mut settings.game_ocr_engine, i18n.mode_game)
        }
        crate::infrastructure::settings::OcrMode::Manga => {
            (&mut settings.manga_ocr_engine, i18n.mode_manga)
        }
        crate::infrastructure::settings::OcrMode::Document => {
            (&mut settings.document_ocr_engine, i18n.mode_document)
        }
    };

    super::section_header(ui, &format!("{} — {}", i18n.choose_ocr, mode_name));
    ui.add_space(4.0);
    ui.radio_value(
        engine_ref,
        crate::infrastructure::settings::OcrEngineType::Windows,
        i18n.ocr_windows_desc,
    );
    ui.radio_value(
        engine_ref,
        crate::infrastructure::settings::OcrEngineType::BuiltinPaddle,
        i18n.ocr_builtin_paddle_desc,
    );
    ui.radio_value(
        engine_ref,
        crate::infrastructure::settings::OcrEngineType::MangaOCR,
        i18n.ocr_manga_desc,
    );

    // MangaOCR: download section
    if *engine_ref == crate::infrastructure::settings::OcrEngineType::MangaOCR {
        ui.add_space(8.0);
        if download_progress.is_downloading {
            ui.label(format!(
                "{}: {}",
                i18n.downloading, download_progress.current_file
            ));
            ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
        } else {
            if let Some(err) = &download_progress.error {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 100, 100),
                    format!("Error: {}", err),
                );
            }
            let enc_p = "models/manga-ocr/encoder_model.onnx";
            let dec_p = "models/manga-ocr/decoder_model.onnx";
            let tok_p = "models/manga-ocr/tokenizer.json";
            let yolo_p = "models/manga-ocr/manga109_yolo_s.onnx";

            let models_exist = super::check_file_exists(enc_p)
                && super::check_file_exists(dec_p)
                && super::check_file_exists(tok_p)
                && super::check_file_exists(yolo_p);

            if !models_exist {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 100, 100),
                    i18n.models_not_found,
                );
                if ui.button(i18n.download_install).clicked() {
                    let _ = download_trigger_tx
                        .send(crate::infrastructure::settings::OcrEngineType::MangaOCR);
                    ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(100, 255, 100),
                    i18n.models_installed,
                );
                if ui.button(i18n.reinstall_update).clicked() {
                    let _ = download_trigger_tx
                        .send(crate::infrastructure::settings::OcrEngineType::MangaOCR);
                    ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }
            }
        }
    }

    // Built-in PaddleOCR: configuration & downloader
    if *engine_ref == crate::infrastructure::settings::OcrEngineType::BuiltinPaddle {
        ui.add_space(8.0);

        // Configuration dropdowns
        ui.horizontal(|ui| {
            ui.label(i18n.ppocr_variant_label);
            egui::ComboBox::from_id_salt("ppocr_model_suite_combo")
                .selected_text(match settings.ppocr_model {
                    crate::infrastructure::settings::PpocrModelSuite::CnEnMobile => {
                        i18n.ppocr_suite_cnen_mobile
                    }
                    crate::infrastructure::settings::PpocrModelSuite::JapaneseMobile => {
                        i18n.ppocr_suite_jp_mobile
                    }
                    crate::infrastructure::settings::PpocrModelSuite::KoreanMobile => {
                        i18n.ppocr_suite_ko_mobile
                    }
                    crate::infrastructure::settings::PpocrModelSuite::ThaiMobile => {
                        i18n.ppocr_suite_th_mobile
                    }
                    crate::infrastructure::settings::PpocrModelSuite::LatinMobile => {
                        i18n.ppocr_suite_latin_mobile
                    }
                    crate::infrastructure::settings::PpocrModelSuite::CyrillicMobile => {
                        i18n.ppocr_suite_cyrillic_mobile
                    }
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut settings.ppocr_model,
                        crate::infrastructure::settings::PpocrModelSuite::CnEnMobile,
                        i18n.ppocr_suite_cnen_mobile,
                    );
                    ui.selectable_value(
                        &mut settings.ppocr_model,
                        crate::infrastructure::settings::PpocrModelSuite::JapaneseMobile,
                        i18n.ppocr_suite_jp_mobile,
                    );
                    ui.selectable_value(
                        &mut settings.ppocr_model,
                        crate::infrastructure::settings::PpocrModelSuite::KoreanMobile,
                        i18n.ppocr_suite_ko_mobile,
                    );
                    ui.selectable_value(
                        &mut settings.ppocr_model,
                        crate::infrastructure::settings::PpocrModelSuite::ThaiMobile,
                        i18n.ppocr_suite_th_mobile,
                    );
                    ui.selectable_value(
                        &mut settings.ppocr_model,
                        crate::infrastructure::settings::PpocrModelSuite::LatinMobile,
                        i18n.ppocr_suite_latin_mobile,
                    );
                    ui.selectable_value(
                        &mut settings.ppocr_model,
                        crate::infrastructure::settings::PpocrModelSuite::CyrillicMobile,
                        i18n.ppocr_suite_cyrillic_mobile,
                    );
                });
        });

        ui.add_space(6.0);

        if download_progress.is_downloading {
            ui.label(format!(
                "{}: {}",
                i18n.downloading, download_progress.current_file
            ));
            ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
        } else {
            if let Some(err) = &download_progress.error {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 100, 100),
                    format!("Error: {}", err),
                );
            }

            // Check status dynamically based on current configuration suite folder
            let folder_name = settings.ppocr_model.folder_name();

            let base_p = format!("models/ppocr/{}", folder_name);
            let det_path = format!("{}/det.onnx", base_p);
            let rec_path = format!("{}/rec.onnx", base_p);
            let dict_path = format!("{}/dict.txt", base_p);

            let det_exists = super::check_file_exists(&det_path);
            let rec_exists = super::check_file_exists(&rec_path);
            let dict_exists = super::check_file_exists(&dict_path);

            if det_exists && rec_exists && dict_exists {
                ui.colored_label(
                    egui::Color32::from_rgb(100, 255, 100),
                    format!("✔ {} ({})", i18n.models_found, folder_name),
                );
                if ui.button(i18n.reinstall_update).clicked() {
                    let _ = download_trigger_tx
                        .send(crate::infrastructure::settings::OcrEngineType::BuiltinPaddle);
                    ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 100, 100),
                    format!("⚠ {}: models/ppocr/{}", i18n.missing_models, folder_name),
                );
                ui.label(i18n.ppocr_download_hint);
                if ui.button(i18n.download_install).clicked() {
                    let _ = download_trigger_tx
                        .send(crate::infrastructure::settings::OcrEngineType::BuiltinPaddle);
                    ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }
            }
        }
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    super::section_header(ui, "LLM OCR Post-processing");
    ui.add_space(4.0);

    ui.checkbox(
        &mut settings.enable_llm_ocr_correction,
        "Use LLM to correct OCR typos before translation",
    );
    if settings.enable_llm_ocr_correction {
        ui.colored_label(
            egui::Color32::from_rgb(255, 180, 100),
            "⚠ Warning: This requires calling the LLM API twice per frame, which doubles latency and token usage.",
        );
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Advanced Text Detection Models ──
    super::section_header(ui, "Advanced Text Detection Models");
    ui.label(
        egui::RichText::new("AI-powered pre-processing to locate text regions before OCR.")
            .small()
            .color(egui::Color32::GRAY),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("Text Detector Mode:");
        egui::ComboBox::from_id_salt("text_detector_mode")
            .selected_text(match settings.text_detector {
                crate::infrastructure::settings::TextDetectorMode::None => "None (Full Frame)",
                crate::infrastructure::settings::TextDetectorMode::YoloBubble => {
                    "YOLO Speech Bubble"
                }
                crate::infrastructure::settings::TextDetectorMode::CraftRegion => {
                    "CRAFT Text Region"
                }
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut settings.text_detector,
                    crate::infrastructure::settings::TextDetectorMode::None,
                    "None (Full Frame)",
                );
                ui.selectable_value(
                    &mut settings.text_detector,
                    crate::infrastructure::settings::TextDetectorMode::YoloBubble,
                    "YOLO Speech Bubble",
                );
                ui.selectable_value(
                    &mut settings.text_detector,
                    crate::infrastructure::settings::TextDetectorMode::CraftRegion,
                    "CRAFT Text Region",
                );
            });
    });

    // Synchronize legacy `use_yolo_bubble` setting
    settings.use_yolo_bubble =
        settings.text_detector == crate::infrastructure::settings::TextDetectorMode::YoloBubble;

    ui.add_space(4.0);
    ui.checkbox(
        &mut settings.show_yolo_debug_borders,
        "Show Detection Borders (Debug)",
    );

    match settings.text_detector {
        crate::infrastructure::settings::TextDetectorMode::YoloBubble => {
            let exists = crate::infrastructure::asset_download_manager::check_bubble_yolo_exists();
            if !exists {
                ui.add_space(8.0);
                if download_progress.is_downloading
                    && download_progress.current_file.contains("Bubble")
                {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!(
                            "Downloading model: {}",
                            download_progress.current_file
                        ));
                    });
                    ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
                } else {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(235, 120, 0),
                            "⚠ YOLO Speech Bubble model (yolo26n.onnx) is not installed.",
                        );
                        if ui.button("Download (6MB)").clicked() {
                            let _ = download_trigger_tx
                                .send(crate::infrastructure::settings::OcrEngineType::BubbleYOLO);
                        }
                    });
                }
            } else {
                ui.add_space(8.0);
                ui.colored_label(
                    egui::Color32::from_rgb(0, 180, 50),
                    "✅ YOLO Speech Bubble model installed.",
                );
            }
        }
        crate::infrastructure::settings::TextDetectorMode::CraftRegion => {
            let exists = crate::infrastructure::asset_download_manager::check_craft_exists();
            if !exists {
                ui.add_space(8.0);
                if download_progress.is_downloading
                    && download_progress.current_file.contains("CRAFT")
                {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!(
                            "Downloading model: {}",
                            download_progress.current_file
                        ));
                    });
                    ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
                } else {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(235, 120, 0),
                            "⚠ CRAFT Text Detector model is not installed.",
                        );
                        if ui.button("Download (83MB)").clicked() {
                            let _ = download_trigger_tx.send(
                                crate::infrastructure::settings::OcrEngineType::CraftDetector,
                            );
                        }
                    });
                }
            } else {
                ui.add_space(8.0);
                ui.colored_label(
                    egui::Color32::from_rgb(0, 180, 50),
                    "✅ CRAFT Text Detector model installed.",
                );
            }
        }
        _ => {}
    }
}
