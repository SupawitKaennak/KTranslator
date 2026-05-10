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

                                let words: Vec<&str> = trans.split_whitespace().collect();
                                let has_spaces = words.len() > 1;
                                let lines_count = block_lines.len().max(1);

                                let is_thai = trans.chars().any(|c| (0x0E01..=0x0E7F).contains(&(c as u32)));
                                let chunks: Vec<String> = if has_spaces {
                                    let words_per_line = (words.len() as f32 / lines_count as f32).ceil() as usize;
                                    let mut c = Vec::new();
                                    for chunk in words.chunks(words_per_line.max(1)) {
                                        let separator = if is_thai { "\u{200B}" } else { " " };
                                        c.push(chunk.join(separator));
                                    }
                                    c
                                } else {
                                    let chars: Vec<char> = trans.chars().collect();
                                    let chars_per_line = (chars.len() as f32 / lines_count as f32).ceil() as usize;
                                    let mut c = Vec::new();
                                    for chunk in chars.chunks(chars_per_line.max(1)) {
                                        c.push(chunk.iter().collect::<String>());
                                    }
                                    c
                                };

                                if block_lines.len() > 1 {
                                    // --- 1. Unified Background Mode (for Merged Sentences) ---
                                    let mut union_rect: Option<egui::Rect> = None;
                                    for line in &block_lines {
                                        let r = egui::Rect::from_min_size(
                                            egui::pos2(line.x / ppp, line.y / ppp),
                                            egui::vec2(line.w / ppp, line.h / ppp)
                                        );
                                        union_rect = Some(union_rect.map_or(r, |acc| acc.union(r)));
                                    }

                                    if let Some(bg_rect) = union_rect {
                                        let padded_bg = bg_rect.expand(overlay_padding);
                                        painter.rect_filled(padded_bg, overlay_corner_radius, overlay_bg_color);
                                        last_bottom_y = last_bottom_y.max(padded_bg.max.y);

                                        // Render the whole joined text inside the union rect
                                        let raw_text = chunks.join("\n");
                                        let full_text = wrap_thai_text(&raw_text);
                                        let font_size = overlay_settings.overlay_font_size;
                                        let wrap_width = bg_rect.width().max(50.0);

                                        let galley = ctx.fonts(|f| {
                                            let mut job = egui::text::LayoutJob::simple(
                                                full_text,
                                                egui::FontId::proportional(font_size),
                                                overlay_text_color,
                                                wrap_width
                                            );
                                            job.wrap.break_anywhere = false;
                                            job.halign = match overlay_settings.overlay_text_align {
                                                crate::infra::settings::TextAlign::Left => egui::Align::Min,
                                                crate::infra::settings::TextAlign::Center => egui::Align::Center,
                                                crate::infra::settings::TextAlign::Right => egui::Align::Max,
                                            };
                                            f.layout_job(job)
                                        });

                                        let text_x = match overlay_settings.overlay_text_align {
                                            crate::infra::settings::TextAlign::Left => bg_rect.left() + overlay_padding,
                                            crate::infra::settings::TextAlign::Center => bg_rect.center().x,
                                            crate::infra::settings::TextAlign::Right => bg_rect.right() - overlay_padding,
                                        };

                                        let text_pos = egui::pos2(text_x, bg_rect.top() + overlay_padding);
                                        painter.galley(text_pos, galley, overlay_text_color);
                                    }
                                } else {
                                    // --- 2. Individual Strip Mode (for Single Lines / Non-merged) ---
                                    for (i, line) in block_lines.iter().enumerate() {
                                        let line_h_points = line.h / ppp;
                                        let font_size = overlay_settings.overlay_font_size.min(line_h_points * 1.2).max(8.0);
                                        let wrap_width = (line.w / ppp).max(30.0); 

                                        let chunk_text = chunks.get(i).cloned().unwrap_or_default();
                                        let full_text = wrap_thai_text(&chunk_text);
                                        
                                        let galley = ctx.fonts(|f| {
                                            let mut job = egui::text::LayoutJob::simple(
                                                full_text,
                                                egui::FontId::proportional(font_size),
                                                overlay_text_color,
                                                wrap_width
                                            );
                                            job.wrap.break_anywhere = false;
                                            job.halign = match overlay_settings.overlay_text_align {
                                                crate::infra::settings::TextAlign::Left => egui::Align::Min,
                                                crate::infra::settings::TextAlign::Center => egui::Align::Center,
                                                crate::infra::settings::TextAlign::Right => egui::Align::Max,
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
                                            let text_y = start_y + (bg_h - galley.size().y) / 2.0;
                                            
                                            let text_x = match overlay_settings.overlay_text_align {
                                                crate::infra::settings::TextAlign::Left => bg.left() + overlay_padding/2.0,
                                                crate::infra::settings::TextAlign::Center => bg.center().x,
                                                crate::infra::settings::TextAlign::Right => bg.right() - overlay_padding/2.0,
                                            };

                                            let text_pos = egui::pos2(text_x, text_y);
                                            painter.galley(text_pos, galley, overlay_text_color);
                                        }
                                    }
                                }

                                // If the translation generated more chunks than original lines
                                if chunks.len() > block_lines.len() {
                                    let last_line = block_lines.last().unwrap();
                                    let mut extra_y = last_bottom_y + 4.0;
                                    for extra_chunk in &chunks[block_lines.len()..] {
                                        let line_h_points = last_line.h / ppp;
                                        let font_size = overlay_settings.overlay_font_size.min(line_h_points * 1.2).max(8.0);
                                        let wrap_width = (max_text_w - (last_line.x / ppp) + full_rect.left()).max(100.0);
                                        let galley = ctx.fonts(|f| {
                                            f.layout(
                                                extra_chunk.clone(),
                                                egui::FontId::proportional(font_size),
                                                overlay_text_color,
                                                wrap_width,
                                            )
                                        });

                                        let bg_w = (last_line.w / ppp).max(galley.size().x + (overlay_padding * 2.0)).min(wrap_width + (overlay_padding * 2.0));
                                        let bg_h = galley.size().y + overlay_padding;
                                        let bg = egui::Rect::from_min_size(
                                            egui::pos2((last_line.x / ppp) - overlay_padding/2.0, extra_y),
                                            egui::vec2(bg_w + overlay_padding, bg_h),
                                        );
                                        painter.rect_filled(bg, overlay_corner_radius, overlay_bg_color);
                                        
                                        let text_pos = egui::pos2(last_line.x / ppp, extra_y + overlay_padding/2.0);
                                        painter.galley(text_pos, galley, overlay_text_color);
                                        extra_y += bg_h + 4.0;
                                        last_bottom_y = last_bottom_y.max(extra_y);
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
                                    let mut job = egui::text::LayoutJob::simple(
                                        line.to_string(),
                                        egui::FontId::proportional(font_size),
                                        overlay_text_color,
                                        wrap_width
                                    );
                                    job.halign = match overlay_settings.overlay_text_align {
                                        crate::infra::settings::TextAlign::Left => egui::Align::Min,
                                        crate::infra::settings::TextAlign::Center => egui::Align::Center,
                                        crate::infra::settings::TextAlign::Right => egui::Align::Max,
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

            // Platform attributes (transparency/color key/capture exclusion)
            let title_inner = format!("Frame Overlay {}", slot_idx + 1);
            if let Some(raw) = platform_svc.find_window_by_title(&title_inner) {
                let cached_hwnd = hwnd_cache.load(std::sync::atomic::Ordering::Relaxed);
                let current_hide = overlay_settings.hide_from_capture;
                let mut last_hide = runtime.last_capture_hide.lock();
                
                if raw != cached_hwnd || *last_hide != Some(current_hide) {
                    crate::infra::win32::apply_overlay_attributes(raw, current_hide);
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

/// A helper function to handle Thai word-wrapping and character integrity.
/// 
/// 1. Converts regular spaces in Thai text into Zero-Width Spaces (\u{200B}) 
///    to provide invisible breakpoints for egui's word-wrapper.
/// 2. Injects Word Joiners (\u{2060}) between adjacent Thai characters to 
///    ensure they stay together (preventing vowel/tone mark dropping).
fn wrap_thai_text(text: &str) -> String {
    if !text.chars().any(|c| c >= '\u{0E00}' && c <= '\u{0E7F}') {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len() * 2);
    let chars: Vec<char> = text.chars().collect();
    let has_spaces = text.contains(' ');
    
    for i in 0..chars.len() {
        let c = chars[i];
        out.push(c);
        
        if i + 1 < chars.len() {
            let next = chars[i+1];
            
            if next == ' ' || next == '\n' {
                continue;
            }

            // --- LINGUISTIC RULES ---
            
            // 1. Mandatory Glue: Marks that MUST follow a consonant
            let next_is_mark = "\u{0E30}\u{0E31}\u{0E32}\u{0E33}\u{0E34}\u{0E35}\u{0E36}\u{0E37}\u{0E38}\u{0E39}\u{0E45}\u{0E47}\u{0E48}\u{0E49}\u{0E4A}\u{0E4B}\u{0E4C}\u{0E4D}\u{0E4E}".contains(next);
            
            // 2. High-Confidence Break Points (ZWSP)
            let next_is_leading = "\u{0E40}\u{0E41}\u{0E42}\u{0E43}\u{0E44}".contains(next);
            let curr_is_ending = "\u{0E30}\u{0E32}\u{0E33}\u{0E45}\u{0E4C}\u{0E46}\u{0E2F}".contains(c);
            
            // 3. Cluster Glue: Consonant clusters and Silent H
            let is_cluster = (c == 'ห' && "\u{0E19}\u{0E0D}\u{0E21}\u{0E22}\u{0E23}\u{0E25}\u{0E27}".contains(next)) || 
                             ("\u{0E01}\u{0E02}\u{0E04}\u{0E15}\u{0E1B}\u{0E1C}\u{0E1E}\u{0E1F}".contains(c) && "\u{0E23}\u{0E25}\u{0E27}".contains(next));

            if next_is_mark || is_cluster {
                // GLUE: Don't break inside clusters or before marks
                out.push('\u{2060}');
            } else if next_is_leading || curr_is_ending {
                // PREFERRED BREAK: Good places to wrap
                out.push('\u{200B}');
            } else if !has_spaces && c != ' ' && c != '\n' {
                // FOR GOOGLE: Auto-break between consonant groups, but be conservative
                out.push('\u{200B}');
            } else if next != ' ' && next != '\n' && c != ' ' {
                // FOR AI: Respect word boundaries by glueing everything else
                out.push('\u{2060}');
            }
        }
    }
    out.replace(' ', "\u{200B}")
}