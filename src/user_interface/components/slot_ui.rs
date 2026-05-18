use eframe::egui;
use crate::core::{
    model::AppModel,
    types::{LanguageTag, Rect},
};
use crate::core::worker::SlotRuntimeState;
use crate::infrastructure::settings::UiLanguage;
use crate::user_interface::i18n::get_i18n;

pub struct SlotUiResponse {
    pub do_crop: bool,
    pub should_remove: bool,
}

pub const LANGUAGE_OPTIONS: &[(&str, &str)] = &[
    ("Thai (th)",                     "th"),
    ("English (en)",                  "en"),
    ("Japanese (ja)",                 "ja"),
    ("Korean (ko)",                   "ko"),
    ("Chinese Simplified (zh-Hans)",  "zh-Hans"),
    ("Chinese Traditional (zh-Hant)", "zh-Hant"),
    ("French (fr)",                   "fr"),
    ("German (de)",                   "de"),
    ("Spanish (es)",                  "es"),
    ("Italian (it)",                  "it"),
    ("Portuguese (pt)",               "pt"),
    ("Russian (ru)",                  "ru"),
    ("Ukrainian (uk)",                "uk"),
    ("Bulgarian (bg)",                "bg"),
    ("Serbian (sr)",                  "sr"),
    ("Croatian (hr)",                 "hr"),
    ("Swahili (sw)",                  "sw"),
    ("Afrikaans (af)",                "af"),
];

pub fn render_slot_item(
    ui: &mut egui::Ui,
    slot_idx: usize,
    model: &mut AppModel,
    runtime: &SlotRuntimeState,
    available_screens: &[(u32, String)],
    lang: UiLanguage,
) -> SlotUiResponse {
    let mut do_crop = false;
    let mut should_remove = false;

    let frame = egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(8.0)
        .inner_margin(10.0);

    frame.show(ui, |ui| {
        ui.set_min_width(500.0);
        
        // --- HEADER ROW ---
        let i18n = get_i18n(lang);

        ui.horizontal(|ui| {
            ui.heading(format!("{} {}", i18n.region, slot_idx + 1));
            
            let slot = &mut model.slots[slot_idx];
            
            ui.checkbox(&mut slot.enabled, i18n.active).on_hover_text(i18n.active_hover);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if slot_idx > 0 {
                    if ui.button("🗑").on_hover_text(i18n.delete_hover).clicked() {
                        should_remove = true;
                    }
                }
                
                if ui
                    .button(i18n.select_area)
                    .on_hover_text(i18n.select_area_hover)
                    .clicked()
                {
                    do_crop = true;
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // --- SETTINGS ROW (Screen & Refresh) ---
        ui.horizontal(|ui| {
            ui.label(format!("{}:", i18n.screen));
            let slot = &mut model.slots[slot_idx];

            egui::ComboBox::from_id_salt(format!("disp_sel_{}", slot_idx))
                .selected_text({
                    available_screens.iter()
                        .find(|(id, _)| *id == slot.display_id)
                        .map(|(_, name)| name.clone())
                        .unwrap_or_else(|| "Primary".to_string())
                })
                .show_ui(ui, |ui| {
                    for (id, name) in available_screens {
                        ui.selectable_value(&mut slot.display_id, *id, name);
                    }
                });

            ui.add_space(20.0);
            ui.label(format!("{}:", i18n.refresh));
            ui.add(egui::DragValue::new(&mut slot.refresh_ms).speed(10.0).suffix("ms"))
                .on_hover_text(i18n.refresh_hover);
        });

        ui.add_space(8.0);

        // --- TRANSLATION ROW (From/To) ---
        ui.horizontal(|ui| {
            let slot = &mut model.slots[slot_idx];

            ui.label(format!("{}:", i18n.from));
            
            // Define source language code: Default to "en" (English) if None
            let mut current_src = slot.source_lang.as_ref().map(|l| l.0.clone()).unwrap_or_else(|| "en".to_string());
            
            let mut src_changed = false;
            egui::ComboBox::from_id_salt(format!("src_{slot_idx}"))
                .selected_text(
                    LANGUAGE_OPTIONS.iter()
                        .find(|(_, code)| *code == current_src)
                        .map(|(name, _)| *name)
                        .unwrap_or("English (en)")
                )
                .show_ui(ui, |ui| {
                    // Render all language options (No Auto Detect)
                    for (name, code) in LANGUAGE_OPTIONS {
                        if ui.selectable_value(&mut current_src, code.to_string(), *name).clicked() {
                            src_changed = true;
                        }
                    }
                });

            if src_changed || slot.source_lang.is_none() {
                let old_src = slot.source_lang.clone();
                slot.source_lang = Some(LanguageTag(current_src));
                tracing::info!("Slot {} source language forced/changed: {:?} -> {:?}", slot_idx, old_src, slot.source_lang);
            }

            ui.add_space(10.0);
            ui.label(format!("{}:", i18n.to));
            
            let mut current_tgt = slot.target_lang.0.clone();
            let mut tgt_changed = false;
            egui::ComboBox::from_id_salt(format!("tgt_{slot_idx}"))
                .selected_text(
                    LANGUAGE_OPTIONS.iter()
                        .find(|(_, code)| *code == current_tgt)
                        .map(|(name, _)| *name)
                        .unwrap_or("Thai (th)")
                )
                .show_ui(ui, |ui| {
                    for (name, code) in LANGUAGE_OPTIONS {
                        if ui.selectable_value(&mut current_tgt, code.to_string(), *name).clicked() {
                            tgt_changed = true;
                        }
                    }
                });

            if tgt_changed {
                slot.target_lang = LanguageTag(current_tgt);
            }
        });

        ui.add_space(8.0);

        // --- VIEW OPTIONS ROW ---
        ui.horizontal(|ui| {
            let slot = &mut model.slots[slot_idx];

            ui.checkbox(&mut slot.show_frame, format!("{}", i18n.show_frame))
                .on_hover_text(i18n.show_frame_hover);
            ui.add_space(10.0);
            ui.checkbox(&mut slot.overlay_mode, format!("{}", i18n.overlay_mode)).on_hover_text(i18n.overlay_mode_hover);
            ui.add_space(20.0);
            
            let popup_btn_text = if slot.popup_open { 
                i18n.close_popup
            } else { 
                i18n.open_popup 
            };
            if ui.button(popup_btn_text).on_hover_text(i18n.open_popup_hover).clicked() {
                slot.popup_open = !slot.popup_open;
            }
        });

        ui.add_space(8.0);

        // --- ADVANCED / POSITION ROW ---
        egui::CollapsingHeader::new(format!("{}", i18n.manual_pos))
            .id_salt(format!("manual_adj_{slot_idx}"))
            .default_open(false)
            .show(ui, |ui| {
                let slot = &mut model.slots[slot_idx];

                if slot.rect.is_none() {
                    slot.rect = Some(Rect { x: 0.0, y: 0.0, w: 400.0, h: 200.0 });
                }
                if let Some(r) = slot.rect.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        ui.add(egui::DragValue::new(&mut r.x).speed(1.0));
                        ui.add_space(8.0);
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut r.y).speed(1.0));
                        ui.add_space(8.0);
                        ui.label("W:");
                        ui.add(egui::DragValue::new(&mut r.w).speed(1.0).range(150.0..=9999.0));
                        ui.add_space(8.0);
                        ui.label("H:");
                        ui.add(egui::DragValue::new(&mut r.h).speed(1.0).range(100.0..=9999.0));
                    });
                    let s = (*r).snap_to_pixels();
                    *r = s;
                }
            });

        // --- RESULTS AREA ---
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if runtime.processing {
                ui.add(egui::Spinner::new().size(12.0));
            }

            let status_text = if runtime.status.is_empty() { i18n.idle } else { &runtime.status };
            ui.label(egui::RichText::new(status_text).size(13.0).strong());
        });
    });

    SlotUiResponse { do_crop, should_remove }
}
