use eframe::egui;

pub fn render_tab_debugging(
    ui: &mut egui::Ui,
    settings: &mut crate::infrastructure::settings::Settings,
    debug_infos: &[super::SlotDebugInfo],
    i18n: &crate::user_interface::i18n::I18n,
) {
    ui.heading(i18n.tab_debugging);
    ui.add_space(8.0);

    super::section_header(ui, i18n.dbg_telemetry);
    ui.label(
        egui::RichText::new(i18n.dbg_desc)
            .small()
            .color(egui::Color32::GRAY),
    );
    ui.add_space(6.0);

    ui.checkbox(
        &mut settings.show_yolo_debug_borders,
        "Show Text Detection Borders (YOLO/CRAFT Debug)",
    );
    ui.add_space(10.0);

    if debug_infos.is_empty() {
        ui.label(egui::RichText::new(i18n.dbg_no_active).color(egui::Color32::DARK_GRAY));
        return;
    }

    for (idx, info) in debug_infos.iter().enumerate() {
        egui::CollapsingHeader::new(format!("{} #{} [{}]", i18n.region, idx + 1, info.status))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new(format!("debug_grid_{}", idx))
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(format!("{}:", i18n.dbg_worker_state));
                        ui.horizontal(|ui| {
                            if info.busy {
                                ui.label(
                                    egui::RichText::new(i18n.dbg_capturing)
                                        .color(egui::Color32::GOLD),
                                );
                            } else if info.processing {
                                ui.label(
                                    egui::RichText::new(i18n.dbg_waiting_ai)
                                        .color(egui::Color32::LIGHT_BLUE),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(i18n.idle).color(egui::Color32::GREEN),
                                );
                            }
                            ui.label(format!("({})", info.status));
                        });
                        ui.end_row();

                        ui.label(format!("{}:", i18n.dbg_debounce));
                        ui.label(format!(
                            "{} {}",
                            info.identical_frames, i18n.dbg_frames_ident
                        ));
                        ui.end_row();

                        ui.label(format!("{}:", i18n.dbg_ocr_lines));
                        ui.label(format!(
                            "{} {}",
                            info.ocr_lines_count, i18n.dbg_entries_mapped
                        ));
                        ui.end_row();

                        ui.label(format!("{}:", i18n.dbg_trans_lines));
                        ui.label(format!(
                            "{} {}",
                            info.trans_lines_count, i18n.dbg_entries_mapped
                        ));
                        ui.end_row();

                        ui.label(format!("{}:", i18n.dbg_processed_ocr));
                        ui.end_row();
                    });

                ui.add_space(4.0);
                egui::Frame::default()
                    .fill(ui.visuals().extreme_bg_color)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        let text_to_show = if info.ocr_text.is_empty() {
                            "<Empty>"
                        } else {
                            &info.ocr_text
                        };
                        ui.label(egui::RichText::new(text_to_show).monospace().size(12.0));
                    });

                ui.add_space(8.0);
            });
        ui.add_space(6.0);
    }
}
