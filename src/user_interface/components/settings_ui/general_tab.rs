use crate::infrastructure::settings::{Settings, UiLanguage};
use eframe::egui;

pub fn render_tab_general(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
) {
    ui.heading(i18n.tab_general);
    ui.add_space(8.0);

    super::section_header(ui, i18n.ui_language);
    egui::ComboBox::from_id_salt("ui_lang_combo")
        .width(200.0)
        .selected_text(match settings.ui_language {
            UiLanguage::System => i18n.system_default,
            UiLanguage::Thai => "ไทย",
            UiLanguage::English => "English",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut settings.ui_language,
                UiLanguage::System,
                i18n.system_default,
            );
            ui.selectable_value(&mut settings.ui_language, UiLanguage::Thai, "ไทย");
            ui.selectable_value(&mut settings.ui_language, UiLanguage::English, "English");
        });

    ui.add_space(12.0);
    super::section_header(ui, i18n.capture_section);
    let mut allow = !settings.hide_from_capture;
    if ui.checkbox(&mut allow, i18n.allow_capture).changed() {
        settings.hide_from_capture = !allow;
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Realtime Stability ──
    super::section_header(ui, i18n.beh_stability);
    ui.label(
        egui::RichText::new(
            "Prevent screen flickering and stabilize typewriter subtitles in games.",
        )
        .small()
        .color(egui::Color32::GRAY),
    );
    ui.add_space(6.0);

    let real = &mut settings.realtime;
    egui::Grid::new("realtime_stability_grid")
        .num_columns(2)
        .spacing([20.0, 12.0])
        .show(ui, |ui| {
            ui.label(i18n.gen_debounce_frames);
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut real.stability_threshold_frames, 1..=10).text("Frames"),
                );
                ui.label(
                    egui::RichText::new("Wait for scrolling text to stop")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.end_row();

            ui.label(i18n.gen_sub_persistence);
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut real.subtitle_persistence_ms, 0..=10000)
                        .step_by(500.0)
                        .text("ms"),
                );
                ui.label(
                    egui::RichText::new("Hold text after dialogue disappears")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.end_row();
        });
}
