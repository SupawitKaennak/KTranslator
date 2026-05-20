//! Luna-style live frame: drag / resize on screen in real time (no fullscreen editor).
//!
//! Uses a hollow Win32 window region so the center is click-through while the border
//! receives mouse events. Translation overlay stays a separate passthrough layer.

use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::core::region_slot_state::AppModel;
use crate::core::region_slot_state::SlotRuntimeState;
use crate::core::types::{physical_px_to_logical_points, Rect};
use crate::infrastructure::platform::PlatformServices;
use crate::infrastructure::settings::Settings;

const BORDER: f32 = 4.0;
const HANDLE: f32 = 14.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Zone {
    Move,
    Nw,
    Ne,
    Sw,
    Se,
}

fn snap_logical(px: f32, ppp: f32) -> f32 {
    physical_px_to_logical_points(px, ppp)
}

fn hit_zone(full: egui::Rect, p: egui::Pos2) -> Option<Zone> {
    if !full.contains(p) {
        return None;
    }
    let corners = [
        (full.left_top(), Zone::Nw),
        (full.right_top(), Zone::Ne),
        (full.left_bottom(), Zone::Sw),
        (full.right_bottom(), Zone::Se),
    ];
    for (c, z) in corners {
        if p.distance(c) <= HANDLE {
            return Some(z);
        }
    }

    // Border areas (draggable)
    if p.y < full.top() + BORDER
        || p.y > full.bottom() - BORDER
        || p.x < full.left() + BORDER
        || p.x > full.right() - BORDER
    {
        return Some(Zone::Move);
    }

    None
}

fn cursor_for(zone: Zone) -> egui::CursorIcon {
    match zone {
        Zone::Move => egui::CursorIcon::Grab,
        Zone::Nw | Zone::Se => egui::CursorIcon::ResizeNwSe,
        Zone::Ne | Zone::Sw => egui::CursorIcon::ResizeNeSw,
    }
}

pub fn render_live_frame_viewport(
    ctx: &egui::Context,
    slot_idx: usize,
    model_arc: &Arc<Mutex<AppModel>>,
    runtime: &SlotRuntimeState,
    settings: &Settings,
    platform: &Arc<dyn PlatformServices>,
) {
    let ppp = ctx.native_pixels_per_point().unwrap_or(1.0);

    let viewport_id = egui::ViewportId::from_hash_of(format!("frame_live_{slot_idx}"));
    let (rect, should_show) = {
        let m = model_arc.lock();
        if slot_idx >= m.slots.len() {
            return;
        }
        let slot = &m.slots[slot_idx];
        (slot.rect, slot.show_frame)
    };

    let hwnd = runtime
        .frame_live_hwnd
        .load(std::sync::atomic::Ordering::Relaxed);
    if !should_show || rect.is_none() {
        if hwnd != 0 {
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Close);
            runtime
                .frame_live_hwnd
                .store(0, std::sync::atomic::Ordering::Relaxed);

            // Clear cached states so that when re-opened, it forces the correct initial position
            ctx.data_mut(|d| {
                d.remove::<bool>(egui::Id::new(("first_frame", slot_idx)));
                d.remove::<Rect>(egui::Id::new(("last_rect", slot_idx)));
                d.remove::<f64>(egui::Id::new(("ignore_until", slot_idx)));
            });
        }
        return;
    }
    let r = rect.unwrap().snap_to_pixels();

    let title = format!("Frame Live {}", slot_idx + 1);
    let model_inner = model_arc.clone();
    let hwnd_cache = runtime.frame_live_hwnd.clone();
    let hide_capture = settings.hide_from_capture;
    let platform_svc = platform.clone();

    ctx.show_viewport_immediate(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title(&title)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_active(true)
            .with_min_inner_size([150.0, 100.0])
            .with_mouse_passthrough(false),
        move |ctx, class| {
            crate::user_interface::font_loader_setup::setup_fonts(ctx);
            if matches!(class, egui::ViewportClass::Embedded) {
                return;
            }

            let full = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new(("live_frame", slot_idx)),
            ));

            let accent = egui::Color32::from_rgb(0, 255, 128);

            // Draw a subtle "glow" border (Inside only to avoid Win32 clipping)
            for i in 1..5 {
                let alpha = (100 / i) as u8; // Increased alpha from 60
                let stroke_w = (i as f32) * 4.0; // Increased step to 4.0 to cover 16px total glow
                painter.rect_stroke(
                    full,
                    0.0,
                    egui::Stroke::new(stroke_w, accent.gamma_multiply(alpha as f32 / 255.0)),
                    egui::StrokeKind::Inside,
                );
            }
            // Main solid border - matched to BORDER width for perfect visual feedback
            painter.rect_stroke(
                full,
                0.0,
                egui::Stroke::new(BORDER, accent),
                egui::StrokeKind::Inside,
            );

            // Use an almost-invisible but solid color for draggable areas (Alpha 1)
            // We use a faint version of the accent color to blend in perfectly.
            let invisible_drag_color = accent.gamma_multiply(1.0 / 255.0);

            // Fill draggable areas with invisible-solid color
            let top_border =
                egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, full.min.y + BORDER));
            let left_border =
                egui::Rect::from_min_max(full.min, egui::pos2(full.min.x + BORDER, full.max.y));
            let right_border =
                egui::Rect::from_min_max(egui::pos2(full.max.x - BORDER, full.min.y), full.max);
            let bottom_border =
                egui::Rect::from_min_max(egui::pos2(full.min.x, full.max.y - BORDER), full.max);
            painter.rect_filled(top_border, 0.0, invisible_drag_color);
            painter.rect_filled(left_border, 0.0, invisible_drag_color);
            painter.rect_filled(right_border, 0.0, invisible_drag_color);
            painter.rect_filled(bottom_border, 0.0, invisible_drag_color);

            let pointer = ctx.input(|i| i.pointer.latest_pos());
            if let Some(p) = pointer {
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    if let Some(z) = hit_zone(full, p) {
                        match z {
                            Zone::Move => ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag),
                            Zone::Nw => ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                                egui::ResizeDirection::NorthWest,
                            )),
                            Zone::Ne => ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                                egui::ResizeDirection::NorthEast,
                            )),
                            Zone::Sw => ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                                egui::ResizeDirection::SouthWest,
                            )),
                            Zone::Se => ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                                egui::ResizeDirection::SouthEast,
                            )),
                        }
                    }
                }

                if let Some(z) = hit_zone(full, p) {
                    ctx.set_cursor_icon(cursor_for(z));
                }
            }

            // Sync: integer physical pixels <-> window; logical coords rounded for HiDPI stability.
            let now = ctx.input(|i| i.time);
            let ignore_id = egui::Id::new(("ignore_until", slot_idx));
            let mut ignore_until: f64 = ctx.data(|d| d.get_temp(ignore_id).unwrap_or(0.0));

            let last_rect_id = egui::Id::new(("last_rect", slot_idx));

            let mut current_r = {
                let m = model_inner.lock();
                m.slots
                    .get(slot_idx)
                    .and_then(|s| s.rect)
                    .map(|rect| rect.snap_to_pixels())
                    .unwrap_or(r)
            };

            let last_rect: Rect = ctx.data(|d| d.get_temp(last_rect_id)).unwrap_or(current_r);

            let model_changed = (current_r.x - last_rect.x).abs() > 0.1
                || (current_r.y - last_rect.y).abs() > 0.1
                || (current_r.w - last_rect.w).abs() > 0.1
                || (current_r.h - last_rect.h).abs() > 0.1;

            let first_frame = ctx
                .data(|d| d.get_temp::<bool>(egui::Id::new(("first_frame", slot_idx))))
                .is_none();
            if first_frame {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new(("first_frame", slot_idx)), false));
            }

            if model_changed || first_frame {
                let target_pos = egui::pos2(
                    snap_logical(current_r.x, ppp),
                    snap_logical(current_r.y, ppp),
                );
                let target_size = egui::vec2(
                    snap_logical(current_r.w, ppp),
                    snap_logical(current_r.h, ppp),
                );

                let needs_set = ctx.input(|i| i.viewport().outer_rect).is_none_or(|or| {
                    (or.min.x - target_pos.x).abs() > 0.6
                        || (or.min.y - target_pos.y).abs() > 0.6
                        || (or.width() - target_size.x).abs() > 0.6
                        || (or.height() - target_size.y).abs() > 0.6
                });

                if needs_set {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(target_pos));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
                    ignore_until = now + 0.25;
                    ctx.data_mut(|d| d.insert_temp(ignore_id, ignore_until));
                }
            }

            if now > ignore_until {
                if let Some(outer_rect) = ctx.input(|i| i.viewport().outer_rect) {
                    let physical_rect = Rect {
                        x: (outer_rect.min.x * ppp).round(),
                        y: (outer_rect.min.y * ppp).round(),
                        w: (outer_rect.width() * ppp).round(),
                        h: (outer_rect.height() * ppp).round(),
                    }
                    .snap_to_pixels();

                    if (current_r.x - physical_rect.x).abs() > 0.1
                        || (current_r.y - physical_rect.y).abs() > 0.1
                        || (current_r.w - physical_rect.w).abs() > 0.1
                        || (current_r.h - physical_rect.h).abs() > 0.1
                    {
                        let mut m = model_inner.lock();
                        m.slots[slot_idx].rect = Some(physical_rect);
                        current_r = physical_rect;
                    }
                }
            }

            ctx.data_mut(|d| d.insert_temp(last_rect_id, current_r));

            if let Some(raw) = platform_svc.find_window_by_title(&title) {
                let w_px = (full.width() * ppp).round() as i32;
                let h_px = (full.height() * ppp).round() as i32;
                let b_px = (BORDER * ppp).round() as i32;

                // We use set_hollow_window_region for robust click-through
                crate::infrastructure::win32::set_hollow_window_region(
                    raw, w_px, h_px, b_px, b_px, b_px, b_px,
                );

                let cached = hwnd_cache.load(std::sync::atomic::Ordering::Relaxed);
                if raw != cached {
                    crate::infrastructure::win32::apply_overlay_attributes(raw, hide_capture);
                    hwnd_cache.store(raw, std::sync::atomic::Ordering::Relaxed);
                }

                let last_hide_id = egui::Id::new(("last_hide", slot_idx));
                let last_hide = ctx.data(|d| d.get_temp::<bool>(last_hide_id));
                if last_hide != Some(hide_capture) {
                    crate::infrastructure::win32::set_hide_from_capture(raw, hide_capture);
                    ctx.data_mut(|d| d.insert_temp(last_hide_id, hide_capture));
                }
            }
        },
    );
}
