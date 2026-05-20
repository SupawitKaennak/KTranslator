use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_text_processing(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
) {
    ui.heading(i18n.tab_text_processing);
    ui.add_space(8.0);

    super::section_header(ui, i18n.txt_pre_trans);
    ui.add_space(4.0);
    ui.checkbox(&mut settings.smart_merge, i18n.smart_merge);
    ui.checkbox(&mut settings.txt_proc.enable_wordninja, i18n.txt_wordninja);

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    super::section_header(ui, i18n.txt_pre_trans);
    ui.label(egui::RichText::new(i18n.txt_proc_adv_desc).italics());
    ui.add_space(6.0);

    let tp = &mut settings.txt_proc;

    egui::Grid::new("txt_proc_grid")
        .num_columns(2)
        .spacing([20.0, 8.0])
        .show(ui, |ui| {
            ui.checkbox(&mut tp.remove_duplicates, i18n.clean_remove_dups);
            ui.checkbox(&mut tp.merge_broken_lines, i18n.clean_merge_broken);
            ui.end_row();

            ui.checkbox(&mut tp.merge_subtitle_fragments, i18n.clean_merge_fragments);
            ui.checkbox(&mut tp.remove_garbage, i18n.clean_remove_garbage);
            ui.end_row();

            ui.checkbox(&mut tp.recurring_suppression, i18n.clean_recurring);
            ui.checkbox(&mut tp.repeated_char_collapse, i18n.clean_repeat_char);
            ui.end_row();

            ui.checkbox(&mut tp.consonant_spam_filter, i18n.clean_consonant_spam);
            ui.checkbox(&mut tp.kana_spam_filter, i18n.clean_kana_spam);
            ui.end_row();

            ui.checkbox(&mut tp.punctuation_normalization, i18n.clean_punc_norm);
            ui.end_row();
        });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(format!("{}:", i18n.clean_min_len));
        ui.add(egui::Slider::new(&mut tp.min_text_length, 1..=10).suffix(" chars"));
    });

    if tp.remove_garbage {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(format!("{}:", i18n.clean_spec_ratio));
            ui.add(egui::Slider::new(
                &mut tp.special_char_ratio_limit,
                0.1..=1.0,
            ));
        });
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Language-Specific Processing Section ──
    super::section_header(ui, i18n.txt_lang_spec);
    ui.label(
        egui::RichText::new(
            "Advanced rules optimized for specific writing systems and linguistic nuances:",
        )
        .italics(),
    );
    ui.add_space(8.0);

    egui::Grid::new("lang_spec_grid").num_columns(2).spacing([20.0, 12.0]).show(ui, |ui| {
        // Japanese
        ui.label(egui::RichText::new(i18n.lang_japanese).strong());
        ui.vertical(|ui| {
            ui.checkbox(&mut tp.jp_merge_vertical, i18n.jp_merge_v);
            ui.checkbox(&mut tp.jp_kana_normalization, i18n.jp_kana_norm);
            ui.checkbox(&mut tp.jp_remove_furigana, i18n.jp_strip_furi);
            ui.checkbox(&mut tp.jp_preserve_honorifics, i18n.jp_honorifics);
        });
        ui.end_row();

        // Chinese
        ui.label(egui::RichText::new("Chinese:").strong());
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("cn_conv_sel")
                .selected_text(match tp.cn_conversion {
                    crate::infrastructure::settings::ChineseConversionMode::None => i18n.cn_no_conv,
                    crate::infrastructure::settings::ChineseConversionMode::SimplifiedToTraditional => i18n.cn_s2t,
                    crate::infrastructure::settings::ChineseConversionMode::TraditionalToSimplified => i18n.cn_t2s,
                })
                .show_ui(ui, |ui| {
                    use crate::infrastructure::settings::ChineseConversionMode::*;
                    ui.selectable_value(&mut tp.cn_conversion, None, i18n.cn_no_conv);
                    ui.selectable_value(&mut tp.cn_conversion, SimplifiedToTraditional, i18n.cn_s2t);
                    ui.selectable_value(&mut tp.cn_conversion, TraditionalToSimplified, i18n.cn_t2s);
                });
        });
        ui.end_row();

        // Thai
        ui.label(egui::RichText::new("Thai:").strong());
        ui.vertical(|ui| {
            egui::ComboBox::from_id_salt("th_seg_sel")
                .selected_text(match tp.th_segmentation {
                    crate::infrastructure::settings::ThaiSegmentationMode::Standard => i18n.th_std_split,
                    crate::infrastructure::settings::ThaiSegmentationMode::DictionaryAssisted => i18n.th_dict_break,
                    crate::infrastructure::settings::ThaiSegmentationMode::SyllableLevel => i18n.th_syllable,
                })
                .show_ui(ui, |ui| {
                    use crate::infrastructure::settings::ThaiSegmentationMode::*;
                    ui.selectable_value(&mut tp.th_segmentation, Standard, i18n.th_std_split);
                    ui.selectable_value(&mut tp.th_segmentation, DictionaryAssisted, i18n.th_dict_break);
                    ui.selectable_value(&mut tp.th_segmentation, SyllableLevel, i18n.th_syllable);
                });
            ui.add_space(4.0);
            ui.checkbox(&mut tp.th_zero_width_cleanup, i18n.th_zw_cleanup);
        });
        ui.end_row();

        // Arabic
        ui.label(egui::RichText::new(i18n.lang_arabic).strong());
        ui.checkbox(&mut tp.ar_rtl_correction, i18n.ar_rtl_fix);
        ui.end_row();
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

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

                ui.label(format!("{}:", i18n.prio));
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

    super::section_header(
        ui,
        &format!(
            "Custom Dictionary / {}",
            i18n.tab_text_processing
                .replace("Text Processing", "Glossary Engine")
                .replace("การประมวลผลข้อความ", "พจนานุกรม")
        ),
    );
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
