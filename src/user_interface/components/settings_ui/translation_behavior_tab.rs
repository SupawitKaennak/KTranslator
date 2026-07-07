use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_translation_behavior(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
) {
    ui.heading(i18n.tab_translation_behavior);
    ui.add_space(8.0);

    let is_llm = settings.provider != crate::infrastructure::settings::TranslationProvider::Google;

    ui.add_enabled_ui(is_llm, |ui| {
        let beh = &mut settings.trans_behavior;

        super::section_header(ui, i18n.beh_preset_modes);
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut beh.preset,
                crate::infrastructure::settings::TranslationStylePreset::Standard,
                "Standard",
            );
            ui.radio_value(
                &mut beh.preset,
                crate::infrastructure::settings::TranslationStylePreset::JrpgMode,
                "JRPG Mode",
            );
            ui.radio_value(
                &mut beh.preset,
                crate::infrastructure::settings::TranslationStylePreset::AnimeSubtitle,
                "Anime Subtitle",
            );
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut beh.preset,
                crate::infrastructure::settings::TranslationStylePreset::VisualNovel,
                "Visual Novel",
            );
            ui.radio_value(
                &mut beh.preset,
                crate::infrastructure::settings::TranslationStylePreset::StreamerMode,
                "Streamer Mode",
            );
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        super::section_header(ui, i18n.beh_sliders);
        egui::Grid::new("behavior_sliders_grid")
            .num_columns(2)
            .spacing([20.0, 10.0])
            .show(ui, |ui| {
                ui.label("Style Balance:");
                ui.add(
                    egui::Slider::new(&mut beh.literal_natural_slider, 0.0..=1.0)
                        .text("Literal ↔ Natural"),
                );
                ui.end_row();

                ui.label("AI Creativity:");
                ui.add(
                    egui::Slider::new(&mut beh.creativity, 0.0..=1.0).text("Low (Strict) ↔ High"),
                );
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        super::section_header(ui, i18n.beh_tone_rules);
        ui.horizontal(|ui| {
            ui.label("Voice Tone:");
            egui::ComboBox::from_id_salt("tone_combobox")
                .selected_text(format!("{:?}", beh.tone))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut beh.tone,
                        crate::infrastructure::settings::TranslationTone::Auto,
                        "Auto",
                    );
                    ui.selectable_value(
                        &mut beh.tone,
                        crate::infrastructure::settings::TranslationTone::Formal,
                        "Formal / Polite",
                    );
                    ui.selectable_value(
                        &mut beh.tone,
                        crate::infrastructure::settings::TranslationTone::Casual,
                        "Casual / Lively",
                    );
                    ui.selectable_value(
                        &mut beh.tone,
                        crate::infrastructure::settings::TranslationTone::Polite,
                        "Standard Public Polite",
                    );
                });
        });

        ui.add_space(10.0);
        super::section_header(ui, i18n.beh_strict_pres);
        egui::Grid::new("preservations_grid")
            .num_columns(2)
            .spacing([15.0, 8.0])
            .show(ui, |ui| {
                ui.checkbox(&mut beh.preserve_formatting, "Preserve Formatting");
                ui.checkbox(&mut beh.preserve_line_breaks, "Preserve Line Breaks");
                ui.end_row();

                ui.checkbox(&mut beh.preserve_punctuation, "Preserve Punctuation");
                ui.checkbox(&mut beh.preserve_honorifics, "Preserve Honorifics (-san)");
                ui.end_row();

                ui.checkbox(&mut beh.preserve_emojis, "Preserve Emojis / Kaomojis");
                ui.checkbox(&mut beh.contextual_translation, "Contextual Adaptation");
                ui.end_row();

                ui.checkbox(&mut beh.profanity_filter, "Safe Profanity Filter");
                ui.end_row();
            });
    });

}
