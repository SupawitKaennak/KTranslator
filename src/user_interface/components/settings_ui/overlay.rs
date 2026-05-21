use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_overlay(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
    download_progress: &crate::core::types::DownloadProgress,
    download_trigger_tx: &std::sync::mpsc::Sender<crate::infrastructure::settings::OcrEngineType>,
) {
    ui.heading(i18n.tab_overlay);
    ui.add_space(8.0);

    super::section_header(ui, i18n.overlay_customization);
    ui.add_space(4.0);

    egui::Grid::new("overlay_grid")
        .num_columns(2)
        .spacing([20.0, 10.0])
        .show(ui, |ui| {
            ui.label(format!("{}:", i18n.bg_color));
            ui.horizontal(|ui| {
                let mut rgb = [
                    settings.overlay_bg_color[0],
                    settings.overlay_bg_color[1],
                    settings.overlay_bg_color[2],
                ];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    settings.overlay_bg_color[0] = rgb[0];
                    settings.overlay_bg_color[1] = rgb[1];
                    settings.overlay_bg_color[2] = rgb[2];
                }
                ui.add_space(8.0);
                ui.label(format!("{}:", i18n.opacity));
                ui.add(egui::Slider::new(
                    &mut settings.overlay_bg_color[3],
                    0..=255,
                ));
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.text_color));
            ui.horizontal(|ui| {
                let mut rgb = [
                    settings.overlay_text_color[0],
                    settings.overlay_text_color[1],
                    settings.overlay_text_color[2],
                ];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    settings.overlay_text_color[0] = rgb[0];
                    settings.overlay_text_color[1] = rgb[1];
                    settings.overlay_text_color[2] = rgb[2];
                }
                ui.add_space(8.0);
                ui.label(format!("{}:", i18n.opacity));
                ui.add(egui::Slider::new(
                    &mut settings.overlay_text_color[3],
                    0..=255,
                ));
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.font_size));
            ui.add(egui::Slider::new(&mut settings.overlay_font_size, 8.0..=48.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.padding));
            ui.add(egui::Slider::new(&mut settings.overlay_padding, 0.0..=20.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.corner_radius));
            ui.add(egui::Slider::new(&mut settings.overlay_corner_radius, 0.0..=20.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.text_align));
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut settings.overlay_text_align,
                    crate::infrastructure::settings::TextAlign::Left,
                    i18n.align_left,
                );
                ui.radio_value(
                    &mut settings.overlay_text_align,
                    crate::infrastructure::settings::TextAlign::Center,
                    i18n.align_center,
                );
                ui.radio_value(
                    &mut settings.overlay_text_align,
                    crate::infrastructure::settings::TextAlign::Right,
                    i18n.align_right,
                );
            });
            ui.end_row();
        });

    ui.add_space(16.0);
    super::section_header(ui, "Advanced Text Detection Models");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("Text Detector Mode:");
        egui::ComboBox::from_id_source("text_detector_mode")
            .selected_text(match settings.text_detector {
                crate::infrastructure::settings::TextDetectorMode::None => "None (Full Frame)",
                crate::infrastructure::settings::TextDetectorMode::YoloBubble => "YOLO Speech Bubble",
                crate::infrastructure::settings::TextDetectorMode::CraftRegion => "CRAFT Text Region",
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
    settings.use_yolo_bubble = settings.text_detector == crate::infrastructure::settings::TextDetectorMode::YoloBubble;

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
                if download_progress.is_downloading && download_progress.current_file.contains("Bubble") {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Downloading model: {}", download_progress.current_file));
                    });
                    ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
                } else {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(235, 120, 0),
                            "⚠ YOLO Speech Bubble model (yolo26n.onnx) is not installed.",
                        );
                        if ui.button("Download (6MB)").clicked() {
                            let _ = download_trigger_tx.send(crate::infrastructure::settings::OcrEngineType::BubbleYOLO);
                        }
                    });
                }
            } else {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(0, 180, 50), "✅ YOLO Speech Bubble model installed.");
            }
        }
        crate::infrastructure::settings::TextDetectorMode::CraftRegion => {
            let exists = crate::infrastructure::asset_download_manager::check_craft_exists();
            if !exists {
                ui.add_space(8.0);
                if download_progress.is_downloading && download_progress.current_file.contains("CRAFT") {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Downloading model: {}", download_progress.current_file));
                    });
                    ui.add(egui::ProgressBar::new(download_progress.progress).show_percentage());
                } else {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(235, 120, 0),
                            "⚠ CRAFT Text Detector model is not installed.",
                        );
                        if ui.button("Download (20MB)").clicked() {
                            let _ = download_trigger_tx.send(crate::infrastructure::settings::OcrEngineType::CraftDetector);
                        }
                    });
                }
            } else {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(0, 180, 50), "✅ CRAFT Text Detector model installed.");
            }
        }
        _ => {}
    }
}
