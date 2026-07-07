use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_overlay(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
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
                ui.color_edit_button_srgba_unmultiplied(&mut settings.overlay_bg_color);
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.text_color));
            ui.horizontal(|ui| {
                ui.color_edit_button_srgba_unmultiplied(&mut settings.overlay_text_color);
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.font_size));
            ui.add(egui::Slider::new(&mut settings.overlay_font_size, 8.0..=48.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.padding));
            ui.add(egui::Slider::new(&mut settings.overlay_padding, 0.0..=20.0).suffix("px"));
            ui.end_row();

            ui.label(format!("{}:", i18n.corner_radius));
            ui.add(
                egui::Slider::new(&mut settings.overlay_corner_radius, 0.0..=20.0).suffix("px"),
            );
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
}
