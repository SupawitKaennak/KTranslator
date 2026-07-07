use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_custom_rules(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
) {
    ui.heading(i18n.tab_custom_rules);
    ui.add_space(8.0);

    // ── Regex Engine ──
    super::section_header(ui, i18n.txt_regex);
    ui.label(egui::RichText::new(i18n.regex_adv_desc).italics());
    ui.add_space(6.0);

    let mut remove_idx = None;

    for (idx, rule) in settings.regex_rules.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut rule.enabled, format!("#{idx}"));

                egui::ComboBox::from_id_salt(format!("type_{idx}"))
                    .selected_text(format!("{:?}", rule.rule_type))
                    .show_ui(ui, |ui| {
                        use crate::infrastructure::settings::RegexRuleType::*;
                        ui.selectable_value(&mut rule.rule_type, Ignore, "Ignore (Strip pattern)");
                        ui.selectable_value(
                            &mut rule.rule_type,
                            PreTranslation,
                            "PreTranslation (Replace before AI)",
                        );
                        ui.selectable_value(
                            &mut rule.rule_type,
                            Protected,
                            "Protected (Mask word from AI)",
                        );
                        ui.selectable_value(
                            &mut rule.rule_type,
                            Replace,
                            "Replace (General cleanup)",
                        );
                        ui.selectable_value(&mut rule.rule_type, Split, "Split (Match -> Newline)");
                        ui.selectable_value(
                            &mut rule.rule_type,
                            PostTranslation,
                            "PostTranslation (Repair output)",
                        );
                    });

                if ui.button("🗑").clicked() {
                    remove_idx = Some(idx);
                }
            });

            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.pattern));
                ui.add(egui::TextEdit::singleline(&mut rule.pattern).desired_width(140.0));

                let requires_replacement = !matches!(
                    rule.rule_type,
                    crate::infrastructure::settings::RegexRuleType::Ignore
                        | crate::infrastructure::settings::RegexRuleType::Split
                        | crate::infrastructure::settings::RegexRuleType::Protected
                );

                if requires_replacement {
                    ui.label(format!("{}:", i18n.replace));
                    ui.add(egui::TextEdit::singleline(&mut rule.replacement).desired_width(100.0));
                }
            });
        });
        ui.add_space(4.0);
    }

    if let Some(idx) = remove_idx {
        settings.regex_rules.remove(idx);
    }

    if ui.button(i18n.add_regex).clicked() {
        settings
            .regex_rules
            .push(crate::infrastructure::settings::RegexRule {
                enabled: true,
                pattern: String::new(),
                replacement: String::new(),
                rule_type: crate::infrastructure::settings::RegexRuleType::PreTranslation,
            });
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Glossary / Custom Dictionary ──
    super::section_header(ui, "Custom Dictionary / Glossary Engine");
    ui.label(egui::RichText::new(i18n.gloss_adv_desc).italics());
    ui.add_space(6.0);

    let mut remove_gloss_idx = None;

    for (idx, entry) in settings.glossary_entries.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut entry.enabled, format!("#{idx}"));

                egui::ComboBox::from_id_salt(format!("gtype_{idx}"))
                    .selected_text(format!("{:?}", entry.entry_type))
                    .show_ui(ui, |ui| {
                        use crate::infrastructure::settings::GlossaryType::*;
                        ui.selectable_value(
                            &mut entry.entry_type,
                            CharacterName,
                            i18n.gloss_char_name,
                        );
                        ui.selectable_value(
                            &mut entry.entry_type,
                            GameTerminology,
                            i18n.gloss_game_term,
                        );
                        ui.selectable_value(&mut entry.entry_type, SlangJargon, i18n.gloss_slang);
                        ui.selectable_value(
                            &mut entry.entry_type,
                            ProtectedWord,
                            i18n.gloss_protected,
                        );
                        ui.selectable_value(
                            &mut entry.entry_type,
                            PhraseOverride,
                            i18n.gloss_phrase,
                        );
                        ui.selectable_value(
                            &mut entry.entry_type,
                            TranslationMemory,
                            i18n.gloss_tm,
                        );
                    });

                ui.label(format!("{}:", i18n.prio));
                ui.add(egui::DragValue::new(&mut entry.priority).range(0..=100));

                if ui.button("🗑").clicked() {
                    remove_gloss_idx = Some(idx);
                }
            });

            ui.horizontal(|ui| {
                ui.label(format!("{}:", i18n.source));
                ui.add(egui::TextEdit::singleline(&mut entry.source).desired_width(120.0));

                ui.label(format!("{}:", i18n.target));
                ui.add(egui::TextEdit::singleline(&mut entry.target).desired_width(120.0));
            });
        });
        ui.add_space(4.0);
    }

    if let Some(idx) = remove_gloss_idx {
        settings.glossary_entries.remove(idx);
    }

    if ui.button(i18n.add_glossary).clicked() {
        settings
            .glossary_entries
            .push(crate::infrastructure::settings::GlossaryEntry {
                enabled: true,
                source: String::new(),
                target: String::new(),
                entry_type: crate::infrastructure::settings::GlossaryType::GameTerminology,
                priority: 10,
            });
    }
}
