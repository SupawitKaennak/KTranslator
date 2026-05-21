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
}
