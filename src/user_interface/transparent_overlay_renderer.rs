use crate::core::region_slot_state::AppModel;
use crate::core::region_slot_state::SlotRuntimeState;
use crate::core::types::physical_px_to_logical_points;
use crate::infrastructure::platform::PlatformServices;
use crate::infrastructure::settings::Settings;
use eframe::egui;
use parking_lot::Mutex;
use std::sync::Arc;

/// Convert physical screen pixels to logical viewport coordinates (rounded).
fn snap_logical(px: f32, ppp: f32) -> f32 {
    physical_px_to_logical_points(px, ppp)
}

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

    let viewport_id = egui::ViewportId::from_hash_of(format!("frame_overlay_{}", slot_idx));
    let (rect, should_show) = {
        let m = model_arc.lock();
        if slot_idx >= m.slots.len() {
            return;
        }
        let slot = &m.slots[slot_idx];
        (slot.rect, slot.overlay_mode)
    };

    let hwnd = runtime
        .overlay_hwnd
        .load(std::sync::atomic::Ordering::Relaxed);
    if !should_show || rect.is_none() {
        if hwnd != 0 {
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Close);
            runtime
                .overlay_hwnd
                .store(0, std::sync::atomic::Ordering::Relaxed);
        }
        return;
    }
    let r = rect.unwrap().snap_to_pixels();

    let title = format!("Frame Overlay {}", slot_idx + 1);

    let model_arc_inner = model_arc.clone();
    let hwnd_cache = runtime.overlay_hwnd.clone();
    let overlay_settings = settings.clone();
    let platform_svc = platform.clone();
    let fade_alpha = runtime.overlay_fade_alpha;
    let fade_smoothing = overlay_settings.realtime.fade_smoothing;

    ctx.show_viewport_immediate(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title(&title)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_active(false)
            .with_min_inner_size([150.0, 100.0])
            .with_inner_size(egui::vec2(
                snap_logical(r.w, ppp),
                snap_logical(r.h, ppp),
            ))
            .with_position(egui::pos2(
                snap_logical(r.x, ppp),
                snap_logical(r.y, ppp),
            )),
        move |ctx, class| {
            crate::user_interface::font_loader_setup::setup_fonts(ctx);
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
                    let ocr_lines    = slot.last_ocr_lines.clone();
                    let trans_lines  = slot.last_trans_lines.clone();
                    let fallback_text = slot.last_translation.clone();
                    let yolo_bubbles = slot.last_yolo_bubbles.clone();
                    drop(m);

                    if show_overlay {
                        // --- Windows LWA_COLORKEY Avoidance & Anti-Aliasing Protection ---
                        // The OS uses pure black RGB(0,0,0) as the transparent window color key.
                        // Full black pixels are made transparent by Windows DWM.
                        // Font anti-aliasing blending can cause near-black pixels to round down to 0,0,0.
                        // Shift very dark colors to a safe minimum RGB(12,12,12) to ensure a perfectly solid overlay.
                        let mut bg_r = overlay_settings.overlay_bg_color[0];
                        let mut bg_g = overlay_settings.overlay_bg_color[1];
                        let mut bg_b = overlay_settings.overlay_bg_color[2];
                        if bg_r <= 8 && bg_g <= 8 && bg_b <= 8 {
                            bg_r = 12; bg_g = 12; bg_b = 12;
                        }

                        let mut txt_r = overlay_settings.overlay_text_color[0];
                        let mut txt_g = overlay_settings.overlay_text_color[1];
                        let mut txt_b = overlay_settings.overlay_text_color[2];
                        if txt_r <= 8 && txt_g <= 8 && txt_b <= 8 {
                            txt_r = 12; txt_g = 12; txt_b = 12;
                        }

                        let fade_mul = if fade_smoothing {
                            fade_alpha.clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        let bg_a = overlay_settings.overlay_bg_color[3] as f32 / 255.0 * fade_mul;
                        let overlay_bg_color = egui::Color32::from_rgba_premultiplied(
                            (bg_r as f32 * bg_a) as u8,
                            (bg_g as f32 * bg_a) as u8,
                            (bg_b as f32 * bg_a) as u8,
                            (overlay_settings.overlay_bg_color[3] as f32 * fade_mul) as u8,
                        );

                        let txt_a = overlay_settings.overlay_text_color[3] as f32 / 255.0 * fade_mul;
                        let overlay_text_color = egui::Color32::from_rgba_premultiplied(
                            (txt_r as f32 * txt_a) as u8,
                            (txt_g as f32 * txt_a) as u8,
                            (txt_b as f32 * txt_a) as u8,
                            (overlay_settings.overlay_text_color[3] as f32 * fade_mul) as u8,
                        );
                        let overlay_padding = overlay_settings.overlay_padding;
                        let overlay_corner_radius = overlay_settings.overlay_corner_radius;

                        let has_positions = !ocr_lines.is_empty();

                        if has_positions {
                            let mut last_bottom_y = full_rect.top();

                            let mut idx = 0;
                            while idx < ocr_lines.len() {
                                let mut block_lines = vec![ocr_lines[idx].clone()];
                                let trans = trans_lines.get(idx).map(|s| s.as_str()).unwrap_or("").trim().to_string();

                                let mut j = idx + 1;
                                while j < ocr_lines.len() {
                                    let next_trans = trans_lines.get(j).map(|s| s.as_str()).unwrap_or("").trim();
                                    if !next_trans.is_empty() {
                                        break; // next block starts
                                    }
                                    block_lines.push(ocr_lines[j].clone());
                                    j += 1;
                                }

                                if trans.is_empty() {
                                    idx = j;
                                    continue;
                                }

                                let chunks = crate::core::usecases::text_formatting_usecase::TextFormatter::create_chunks(&trans, block_lines.len());
                                let raw_text = chunks.join("\n");
                                let full_text = crate::core::usecases::text_formatting_usecase::TextFormatter::wrap_thai_text(&raw_text);

                                // Compute the tight bounding box of the OCR text characters
                                let mut text_union_rect: Option<egui::Rect> = None;
                                for line in &block_lines {
                                    let r = egui::Rect::from_min_size(
                                        egui::pos2(line.x / ppp, line.y / ppp),
                                        egui::vec2(line.w / ppp, line.h / ppp)
                                    );
                                    text_union_rect = Some(text_union_rect.map_or(r, |acc| acc.union(r)));
                                }

                                if let Some(base_rect) = text_union_rect {
                                    // 1. Try to find a matching YOLO/CRAFT bubble
                                    let mut matched_bubble_rect: Option<egui::Rect> = None;
                                    let center = base_rect.center();
                                    for bubble in &yolo_bubbles {
                                        let b_rect = egui::Rect::from_min_size(
                                            egui::pos2(bubble.x / ppp, bubble.y / ppp),
                                            egui::vec2(bubble.w / ppp, bubble.h / ppp)
                                        );
                                        // If the text center is inside the bubble, or there is significant overlap
                                        if b_rect.contains(center) || b_rect.intersects(base_rect) {
                                            matched_bubble_rect = Some(b_rect);
                                            break;
                                        }
                                    }

                                    // 2. Determine the final background rect
                                    let mut is_bubble_mode = false;
                                    let mut final_bg_rect = if let Some(b_rect) = matched_bubble_rect {
                                        is_bubble_mode = true;
                                        // Use the bubble exactly, but ensure it encompasses the text
                                        b_rect.union(base_rect)
                                    } else {
                                        // Fallback: OCR Full Page or no bubble detected
                                        if block_lines.len() > 1 {
                                            is_bubble_mode = true;
                                            // Add a bit more generous padding to simulate a bubble
                                            base_rect.expand(overlay_padding * 1.5)
                                        } else {
                                            // Single strip mode
                                            base_rect
                                        }
                                    };

                                    if is_bubble_mode {
                                        // --- Unified / Bubble Mode ---
                                        let bg_w = final_bg_rect.width().max(50.0);
                                        let font_size = overlay_settings.overlay_font_size;

                                        // Pre-layout to check height
                                        let galley = ctx.fonts(|f| {
                                            let mut job = egui::text::LayoutJob::simple(
                                                full_text.clone(),
                                                egui::FontId::proportional(font_size),
                                                overlay_text_color,
                                                bg_w - (overlay_padding * 2.0)
                                            );
                                            job.wrap.break_anywhere = false;
                                            job.halign = match overlay_settings.overlay_text_align {
                                                crate::infrastructure::settings::TextAlign::Left => egui::Align::Min,
                                                crate::infrastructure::settings::TextAlign::Center => egui::Align::Center,
                                                crate::infrastructure::settings::TextAlign::Right => egui::Align::Max,
                                            };
                                            f.layout_job(job)
                                        });

                                        // If text is taller than the bubble, expand the bubble downwards
                                        let required_h = galley.size().y + (overlay_padding * 2.0);
                                        if final_bg_rect.height() < required_h {
                                            final_bg_rect.set_bottom(final_bg_rect.top() + required_h);
                                        }

                                        // For generated bubbles (no YOLO), ensure minimum width based on text
                                        if matched_bubble_rect.is_none() && final_bg_rect.width() < galley.size().x + (overlay_padding * 2.0) {
                                            let diff = (galley.size().x + (overlay_padding * 2.0)) - final_bg_rect.width();
                                            final_bg_rect = final_bg_rect.expand2(egui::vec2(diff / 2.0, 0.0));
                                        }

                                        painter.rect_filled(final_bg_rect, overlay_corner_radius, overlay_bg_color);
                                        last_bottom_y = last_bottom_y.max(final_bg_rect.max.y);

                                        // PERFECT CENTERING
                                        let text_y = final_bg_rect.center().y - (galley.size().y / 2.0);
                                        let text_x = match overlay_settings.overlay_text_align {
                                            crate::infrastructure::settings::TextAlign::Left => final_bg_rect.left() + overlay_padding,
                                            crate::infrastructure::settings::TextAlign::Center => final_bg_rect.center().x,
                                            crate::infrastructure::settings::TextAlign::Right => final_bg_rect.right() - overlay_padding,
                                        };

                                        let text_pos = egui::pos2(text_x, text_y);
                                        painter.galley(text_pos, galley, overlay_text_color);

                                    } else {
                                        // --- Individual Strip Mode (Single Line, No Bubble) ---
                                        let line = &block_lines[0];
                                        let line_h_points = line.h / ppp;
                                        let font_size = overlay_settings.overlay_font_size.min(line_h_points * 1.2).max(8.0);
                                        let wrap_width = (line.w / ppp).max(30.0);

                                        let galley = ctx.fonts(|f| {
                                            let mut job = egui::text::LayoutJob::simple(
                                                full_text.clone(),
                                                egui::FontId::proportional(font_size),
                                                overlay_text_color,
                                                wrap_width
                                            );
                                            job.wrap.break_anywhere = false;
                                            job.halign = match overlay_settings.overlay_text_align {
                                                crate::infrastructure::settings::TextAlign::Left => egui::Align::Min,
                                                crate::infrastructure::settings::TextAlign::Center => egui::Align::Center,
                                                crate::infrastructure::settings::TextAlign::Right => egui::Align::Max,
                                            };
                                            f.layout_job(job)
                                        });

                                        let start_y = line.y / ppp;
                                        let bg_w = (line.w / ppp).max(galley.size().x + (overlay_padding * 2.0)).min(wrap_width + (overlay_padding * 2.0));
                                        let bg_h = (line.h / ppp).max(galley.size().y + overlay_padding);
                                        let bg = egui::Rect::from_min_size(
                                            egui::pos2((line.x / ppp) - overlay_padding/2.0, start_y - overlay_padding/4.0),
                                            egui::vec2(bg_w + overlay_padding, bg_h + overlay_padding/2.0),
                                        );

                                        last_bottom_y = last_bottom_y.max(bg.max.y);
                                        painter.rect_filled(bg, overlay_corner_radius, overlay_bg_color);

                                        if !galley.rows.is_empty() {
                                            // PERFECT CENTERING
                                            let text_y = bg.center().y - (galley.size().y / 2.0);
                                            let text_x = match overlay_settings.overlay_text_align {
                                                crate::infrastructure::settings::TextAlign::Left => bg.left() + overlay_padding/2.0,
                                                crate::infrastructure::settings::TextAlign::Center => bg.center().x,
                                                crate::infrastructure::settings::TextAlign::Right => bg.right() - overlay_padding/2.0,
                                            };

                                            let text_pos = egui::pos2(text_x, text_y);
                                            painter.galley(text_pos, galley, overlay_text_color);
                                        }
                                    }
                                }

                                idx = j;
                            }

                            // Extra lines
                            if trans_lines.len() > ocr_lines.len() {
                                let last = ocr_lines.last().unwrap();
                                let mut y = last_bottom_y + 4.0;
                                for extra in &trans_lines[ocr_lines.len()..] {
                                    if extra.trim().is_empty() { continue; }
                                    let wrap_width = (full_rect.width() - (last.x / ppp) + full_rect.left() - 8.0).max(100.0);
                                    let galley = ctx.fonts(|f| {
                                        f.layout(
                                            extra.clone(),
                                            egui::FontId::proportional(overlay_settings.overlay_font_size),
                                            overlay_text_color,
                                            wrap_width,
                                        )
                                    });
                                    let pos = egui::pos2(last.x / ppp, y);
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
                                    let mut job = egui::text::LayoutJob::simple(
                                        line.to_string(),
                                        egui::FontId::proportional(font_size),
                                        overlay_text_color,
                                        wrap_width
                                    );
                                    job.halign = match overlay_settings.overlay_text_align {
                                        crate::infrastructure::settings::TextAlign::Left => egui::Align::Min,
                                        crate::infrastructure::settings::TextAlign::Center => egui::Align::Center,
                                        crate::infrastructure::settings::TextAlign::Right => egui::Align::Max,
                                    };
                                    f.layout_job(job)
                                });
                                let x = full_rect.left() + 8.0;
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

                    if overlay_settings.show_yolo_debug_borders {
                        for bubble in &yolo_bubbles {
                            let rect = egui::Rect::from_min_size(
                                egui::pos2(bubble.x / ppp, bubble.y / ppp),
                                egui::vec2(bubble.w / ppp, bubble.h / ppp)
                            );
                            let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 0, 0));
                            painter.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Outside);

                            let text_pos = egui::pos2(rect.min.x + 2.0, rect.min.y + 2.0);
                            let galley = ctx.fonts(|f| {
                                f.layout(
                                    "Bubble".to_string(),
                                    egui::FontId::proportional(10.0),
                                    egui::Color32::from_rgb(255, 0, 0),
                                    100.0,
                                )
                            });
                            let bg_rect = egui::Rect::from_min_size(
                                text_pos - egui::vec2(2.0, 1.0),
                                galley.size() + egui::vec2(4.0, 2.0)
                            );
                            painter.rect_filled(bg_rect, 2.0, egui::Color32::from_black_alpha(180));
                            painter.galley(text_pos, galley, egui::Color32::from_rgb(255, 255, 0));
                        }
                    }

                    // No border drawing here, it's handled by live_frame.rs
                }
            }

            // Platform attributes (transparency/color key/capture exclusion)
            let title_inner = format!("Frame Overlay {}", slot_idx + 1);
            if let Some(raw) = platform_svc.find_window_by_title(&title_inner) {
                let cached_hwnd = hwnd_cache.load(std::sync::atomic::Ordering::Relaxed);
                let current_hide = overlay_settings.hide_from_capture;
                let mut last_hide = runtime.last_capture_hide.lock();

                if raw != cached_hwnd || *last_hide != Some(current_hide) {
                    crate::infrastructure::win32::apply_overlay_attributes(raw, current_hide);
                    hwnd_cache.store(raw, std::sync::atomic::Ordering::Relaxed);
                    *last_hide = Some(current_hide);
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
                if slot_idx >= m.slots.len() {
                    return;
                }
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
