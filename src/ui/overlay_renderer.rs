use eframe::egui;
use parking_lot::Mutex;
use std::sync::Arc;
use crate::core::model::AppModel;
use crate::infra::settings::Settings;
use crate::infra::platform::PlatformServices;
use crate::core::worker::SlotRuntimeState;

/// Renders the transparent overlay window for a specific translation region.
pub fn render_overlay_viewport(
    ctx: &egui::Context,
    slot_idx: usize,
    model_arc: &Arc<Mutex<AppModel>>,
    runtime: &SlotRuntimeState,
    settings: &Settings,
    platform: &Arc<dyn PlatformServices>,
) {
    let ppp = ctx.native_pixels_per_point().unwrap_or(1.0);
    
    // Get the rect for this slot
    let rect = {
        let m = model_arc.lock();
        if slot_idx >= m.slots.len() { return; }
        m.slots[slot_idx].rect
    };
    
    let Some(r) = rect else { return; };

    let title = format!("Frame Overlay {}", slot_idx + 1);
    let viewport_id = egui::ViewportId::from_hash_of(format!("frame_overlay_{}", slot_idx));
    
    let model_arc_inner = model_arc.clone();
    let hwnd_cache = runtime.overlay_hwnd.clone();
    let overlay_settings = settings.clone();
    let platform_svc = platform.clone();

    ctx.show_viewport_immediate(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title(&title)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_active(false)
            .with_inner_size(egui::vec2(r.w / ppp, r.h / ppp))
            .with_position(egui::pos2(r.x / ppp, r.y / ppp)),
        move |ctx, class| {
            if matches!(class, egui::ViewportClass::Embedded) {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Frame Viewer (Embedded)");
                });
                return;
            }

            let painter = ctx.layer_painter(egui::LayerId::background());
            let full_rect = ctx.screen_rect();

            {
                let m = model_arc_inner.lock();
                if slot_idx < m.slots.len() {
                    let slot = &m.slots[slot_idx];
                    let show_overlay = slot.overlay_mode && !slot.last_translation.is_empty();
                    let show_border  = slot.show_frame;
                    let ocr_lines    = slot.last_ocr_lines.clone();
                    let trans_lines  = slot.last_trans_lines.clone();
                    let fallback_text = slot.last_translation.clone();
                    drop(m);

                    if show_overlay {
                        let overlay_bg_color = egui::Color32::from_rgba_unmultiplied(
                            overlay_settings.overlay_bg_color[0],
                            overlay_settings.overlay_bg_color[1],
                            overlay_settings.overlay_bg_color[2],
                            overlay_settings.overlay_bg_color[3],
                        );
                        let overlay_text_color = egui::Color32::from_rgba_unmultiplied(
                            overlay_settings.overlay_text_color[0],
                            overlay_settings.overlay_text_color[1],
                            overlay_settings.overlay_text_color[2],
                            overlay_settings.overlay_text_color[3],
                        );
                        let overlay_padding = overlay_settings.overlay_padding;
                        let overlay_corner_radius = overlay_settings.overlay_corner_radius;

                        let has_positions = !ocr_lines.is_empty();

                        if has_positions {
                            let max_text_w = full_rect.width() - 8.0;
                            let mut last_bottom_y = full_rect.top();

                            for (idx, ocr_line) in ocr_lines.iter().enumerate() {
                                let trans = trans_lines.get(idx).map(|s| s.as_str()).unwrap_or("");
                                if trans.trim().is_empty() { continue; }

                                let line_h_points = ocr_line.h / ppp;
                                let font_size = overlay_settings.overlay_font_size.min(line_h_points * 1.2).max(8.0);
                                let wrap_width = (max_text_w - (ocr_line.x as f32 / ppp) + full_rect.left()).max(100.0);
                                
                                let galley = ctx.fonts(|f| {
                                    f.layout(
                                        trans.to_string(),
                                        egui::FontId::proportional(font_size),
                                        overlay_text_color,
                                        wrap_width,
                                    )
                                });

                                let start_y = ocr_line.y / ppp;
                                let bg_w = (ocr_line.w / ppp).max(galley.size().x + (overlay_padding * 2.0)).min(wrap_width + (overlay_padding * 2.0));
                                let bg_h = (ocr_line.h / ppp).max(galley.size().y + overlay_padding);
                                let bg = egui::Rect::from_min_size(
                                    egui::pos2((ocr_line.x / ppp) - overlay_padding/2.0, start_y - overlay_padding/4.0),
                                    egui::vec2(bg_w + overlay_padding, bg_h + overlay_padding/2.0),
                                );
                                
                                last_bottom_y = bg.max.y;
                                painter.rect_filled(bg, overlay_corner_radius, overlay_bg_color);
                                
                                let text_y = start_y + (bg_h - galley.size().y) / 2.0;
                                let text_pos = egui::pos2(ocr_line.x / ppp, text_y);
                                painter.galley(text_pos, galley, overlay_text_color);
                            }

                            // Extra lines
                            if trans_lines.len() > ocr_lines.len() {
                                let last = ocr_lines.last().unwrap();
                                let mut y = last_bottom_y + 4.0;
                                for extra in &trans_lines[ocr_lines.len()..] {
                                    if extra.trim().is_empty() { continue; }
                                    let wrap_width = (full_rect.width() - (last.x as f32 / ppp) + full_rect.left() - 8.0).max(100.0);
                                    let galley = ctx.fonts(|f| {
                                        f.layout(
                                            extra.clone(),
                                            egui::FontId::proportional(overlay_settings.overlay_font_size),
                                            overlay_text_color,
                                            wrap_width,
                                        )
                                    });
                                    let pos = egui::pos2(last.x as f32 / ppp, y);
                                    let bg = egui::Rect::from_min_size(
                                        pos - egui::vec2(overlay_padding, overlay_padding/2.0),
                                        galley.size() + egui::vec2(overlay_padding*2.0, overlay_padding),
                                    );
                                    painter.rect_filled(bg, overlay_corner_radius, overlay_bg_color);
                                    let line_h = galley.size().y;
                                    painter.galley(pos, galley, overlay_text_color);
                                    y += line_h + 4.0;
                                }
                            }
                        } else {
                            // Fallback
                            let font_size = overlay_settings.overlay_font_size;
                            let mut y = full_rect.top() + 8.0;
                            for line in fallback_text.lines() {
                                if line.trim().is_empty() { continue; }
                                let wrap_width = full_rect.width() - 16.0;
                                let galley = ctx.fonts(|f| {
                                    f.layout(
                                        line.to_string(),
                                        egui::FontId::proportional(font_size),
                                        overlay_text_color,
                                        wrap_width,
                                    )
                                });
                                let x = (full_rect.center().x - galley.size().x / 2.0).clamp(full_rect.left() + 4.0, full_rect.right() - 4.0);
                                let pos = egui::pos2(x, y);
                                let bg = egui::Rect::from_min_size(
                                    pos - egui::vec2(overlay_padding, overlay_padding/2.0),
                                    galley.size() + egui::vec2(overlay_padding*2.0, overlay_padding),
                                );
                                painter.rect_filled(bg, overlay_corner_radius, overlay_bg_color);
                                let line_h = galley.size().y;
                                painter.galley(pos, galley, overlay_text_color);
                                y += line_h + 4.0;
                            }
                        }
                    }

                    if show_border {
                        let stroke = egui::Stroke::new(2.5, egui::Color32::from_rgb(0, 255, 128));
                        painter.rect_stroke(full_rect, 0.0, stroke, egui::StrokeKind::Inside);
                    }
                }
            }

            // Drag reposition
            ctx.input(|i| {
                if i.pointer.primary_down() {
                    let delta = i.pointer.delta();
                    if delta != egui::Vec2::ZERO {
                        let mut m = model_arc_inner.lock();
                        if slot_idx < m.slots.len() {
                            if let Some(rect) = m.slots[slot_idx].rect.as_mut() {
                                rect.x += delta.x * ppp;
                                rect.y += delta.y * ppp;
                            }
                        }
                    }
                }
            });

            // Platform attributes (transparency/color key)
            let title_inner = format!("Frame Overlay {}", slot_idx + 1);
            if let Some(raw) = platform_svc.find_window_by_title(&title_inner) {
                let cached = hwnd_cache.load(std::sync::atomic::Ordering::Relaxed);
                if raw != cached {
                    platform_svc.apply_overlay_transparency(raw);
                    hwnd_cache.store(raw, std::sync::atomic::Ordering::Relaxed);
                }
            }
        },
    );
}

/// Renders a simple text-based popup window for a translation region.
pub fn render_popup_viewport(
    ctx: &egui::Context,
    slot_idx: usize,
    model_arc: &Arc<Mutex<AppModel>>,
) {
    let title = format!("Region {} (Popup)", slot_idx + 1);
    let viewport_id = egui::ViewportId::from_hash_of(format!("popup_{}", slot_idx));
    let model_arc_inner = model_arc.clone();

    ctx.show_viewport_immediate(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([400.0, 200.0])
            .with_always_on_top(),
        move |ctx, class| {
            let (last_ocr_text, last_trans_lines) = {
                let m = model_arc_inner.lock();
                if slot_idx >= m.slots.len() { return; }
                let slot = &m.slots[slot_idx];
                (slot.last_ocr_text.clone(), slot.last_trans_lines.clone())
            };

            if ctx.input(|i| i.viewport().close_requested()) {
                let mut m = model_arc_inner.lock();
                if slot_idx < m.slots.len() {
                    m.slots[slot_idx].popup_open = false;
                }
            }

            let show_content = |ui: &mut egui::Ui| {
                if !last_ocr_text.is_empty() {
                    ui.label("OCR:");
                    ui.monospace(&last_ocr_text);
                }
                ui.separator();
                ui.label("Translation:");
                if last_trans_lines.is_empty() {
                    ui.monospace("(waiting...)");
                } else {
                    for line in &last_trans_lines {
                        ui.label(line);
                    }
                }
            };

            if matches!(class, egui::ViewportClass::Embedded) {
                egui::Window::new(&title).show(ctx, show_content);
            } else {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, show_content);
                });
            }
        },
    );
}
