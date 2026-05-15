//! Luna-style live frame: drag / resize on screen in real time (no fullscreen editor).
//!
//! Uses a hollow Win32 window region so the center is click-through while the border
//! receives mouse events. Translation overlay stays a separate passthrough layer.

use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::core::model::AppModel;
use crate::core::types::Rect;
use crate::core::worker::SlotRuntimeState;
use crate::infrastructure::platform::PlatformServices;
use crate::infrastructure::settings::Settings;

const BORDER: f32 = 8.0;
const HANDLE: f32 = 12.0;
const MIN_SIDE: f32 = 48.0;
const TITLE_H: f32 = 20.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Zone {
    Move,
    N,
    S,
    E,
    W,
    Nw,
    Ne,
    Sw,
    Se,
}

fn snap_logical(px: f32, ppp: f32) -> f32 {
    (px / ppp).round()
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
    if p.y < full.top() + TITLE_H && p.x > full.left() + HANDLE && p.x < full.right() - HANDLE {
        return Some(Zone::Move);
    }
    if p.y < full.top() + BORDER {
        return Some(Zone::N);
    }
    if p.y > full.bottom() - BORDER {
        return Some(Zone::S);
    }
    if p.x < full.left() + BORDER {
        return Some(Zone::W);
    }
    if p.x > full.right() - BORDER {
        return Some(Zone::E);
    }
    None
}

fn apply_drag(zone: Zone, origin: Rect, dx: f32, dy: f32) -> Rect {
    let mut x = origin.x;
    let mut y = origin.y;
    let mut w = origin.w;
    let mut h = origin.h;
    match zone {
        Zone::Move => {
            x += dx;
            y += dy;
        }
        Zone::E => w = (origin.w + dx).max(MIN_SIDE),
        Zone::W => {
            let nw = (origin.w - dx).max(MIN_SIDE);
            x = origin.x + origin.w - nw;
            w = nw;
        }
        Zone::S => h = (origin.h + dy).max(MIN_SIDE),
        Zone::N => {
            let nh = (origin.h - dy).max(MIN_SIDE);
            y = origin.y + origin.h - nh;
            h = nh;
        }
        Zone::Se => {
            w = (origin.w + dx).max(MIN_SIDE);
            h = (origin.h + dy).max(MIN_SIDE);
        }
        Zone::Sw => {
            let nw = (origin.w - dx).max(MIN_SIDE);
            x = origin.x + origin.w - nw;
            w = nw;
            h = (origin.h + dy).max(MIN_SIDE);
        }
        Zone::Ne => {
            w = (origin.w + dx).max(MIN_SIDE);
            let nh = (origin.h - dy).max(MIN_SIDE);
            y = origin.y + origin.h - nh;
            h = nh;
        }
        Zone::Nw => {
            let nw = (origin.w - dx).max(MIN_SIDE);
            x = origin.x + origin.w - nw;
            w = nw;
            let nh = (origin.h - dy).max(MIN_SIDE);
            y = origin.y + origin.h - nh;
            h = nh;
        }
    }
    Rect {
        x: x.round(),
        y: y.round(),
        w: w.round(),
        h: h.round(),
    }
}

fn cursor_for(zone: Zone) -> egui::CursorIcon {
    match zone {
        Zone::Move => egui::CursorIcon::Grab,
        Zone::N | Zone::S => egui::CursorIcon::ResizeVertical,
        Zone::E | Zone::W => egui::CursorIcon::ResizeHorizontal,
        Zone::Nw | Zone::Se => egui::CursorIcon::ResizeNwSe,
        Zone::Ne | Zone::Sw => egui::CursorIcon::ResizeNeSw,
    }
}

fn repaint_slot_views(ctx: &egui::Context, slot_idx: usize) {
    ctx.request_repaint();
    ctx.request_repaint_of(egui::ViewportId::from_hash_of(format!("frame_overlay_{slot_idx}")));
    ctx.request_repaint_of(egui::ViewportId::from_hash_of(format!("frame_live_{slot_idx}")));
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

    let rect = {
        let m = model_arc.lock();
        if slot_idx >= m.slots.len() {
            return;
        }
        let slot = &m.slots[slot_idx];
        if !slot.show_frame {
            return;
        }
        slot.rect
    };
    let Some(r) = rect else { return };

    let title = format!("Frame Live {}", slot_idx + 1);
    let model_inner = model_arc.clone();
    let hwnd_cache = runtime.frame_live_hwnd.clone();
    let hide_capture = settings.hide_from_capture;
    let platform_svc = platform.clone();

    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of(format!("frame_live_{slot_idx}")),
        egui::ViewportBuilder::default()
            .with_title(&title)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_active(true)
            .with_mouse_passthrough(false)
            .with_inner_size(egui::vec2(
                snap_logical(r.w, ppp),
                snap_logical(r.h, ppp),
            ))
            .with_position(egui::pos2(
                snap_logical(r.x, ppp),
                snap_logical(r.y, ppp),
            )),
        move |ctx, class| {
            if matches!(class, egui::ViewportClass::Embedded) {
                return;
            }

            let full = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new(("live_frame", slot_idx)),
            ));

            let accent = egui::Color32::from_rgb(0, 255, 128);
            painter.rect_stroke(full, 0.0, egui::Stroke::new(2.0, accent), egui::StrokeKind::Inside);

            let title_rect = egui::Rect::from_min_max(
                full.min,
                egui::pos2(full.max.x, full.min.y + TITLE_H),
            );
            painter.rect_filled(
                title_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 180, 90, 200),
            );
            let galley = ctx.fonts(|f| {
                f.layout_no_wrap(
                    format!("Region {}", slot_idx + 1),
                    egui::FontId::proportional(12.0),
                    egui::Color32::WHITE,
                )
            });
            painter.galley(
                title_rect.center() - galley.size() / 2.0,
                galley,
                egui::Color32::WHITE,
            );

            for c in [full.left_top(), full.right_top(), full.left_bottom(), full.right_bottom()] {
                let hr = egui::Rect::from_center_size(c, egui::vec2(HANDLE, HANDLE));
                painter.rect_filled(hr, 2.0, egui::Color32::from_rgb(0, 220, 120));
                painter.rect_stroke(
                    hr,
                    2.0,
                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
            }

            let pointer = ctx.input(|i| i.pointer.latest_pos());
            let origin_id = egui::Id::new(("live_frame_origin", slot_idx));
            let zone_id = egui::Id::new(("live_frame_zone", slot_idx));
            let start_id = egui::Id::new(("live_frame_start", slot_idx));

            if let Some(p) = pointer {
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    if let Some(z) = hit_zone(full, p) {
                        if let Some(rect) = model_inner
                            .lock()
                            .slots
                            .get(slot_idx)
                            .and_then(|s| s.rect)
                        {
                            ctx.data_mut(|d| {
                                d.insert_temp(zone_id, z);
                                d.insert_temp(origin_id, rect);
                                d.insert_temp(start_id, p);
                            });
                        }
                    }
                }

                if ctx.input(|i| i.pointer.primary_down()) {
                    if let (Some(z), Some(origin), Some(start)) = (
                        ctx.data(|d| d.get_temp::<Zone>(zone_id)),
                        ctx.data(|d| d.get_temp::<Rect>(origin_id)),
                        ctx.data(|d| d.get_temp::<egui::Pos2>(start_id)),
                    ) {
                        let delta = p - start;
                        let next = apply_drag(z, origin, delta.x * ppp, delta.y * ppp);
                        let mut m = model_inner.lock();
                        if slot_idx < m.slots.len() {
                            m.slots[slot_idx].rect = Some(next);
                        }
                        repaint_slot_views(ctx, slot_idx);
                    }
                } else if ctx.input(|i| i.pointer.primary_released()) {
                    ctx.data_mut(|d| {
                        d.remove::<Zone>(zone_id);
                        d.remove::<Rect>(origin_id);
                        d.remove::<egui::Pos2>(start_id);
                    });
                }

                if let Some(z) = ctx
                    .data(|d| d.get_temp::<Zone>(zone_id))
                    .or_else(|| hit_zone(full, p))
                {
                    ctx.set_cursor_icon(cursor_for(z));
                }
            }

            if let Some(raw) = platform_svc.find_window_by_title(&title) {
                let w_px = (full.width() * ppp).round() as i32;
                let h_px = (full.height() * ppp).round() as i32;
                let border_px = (BORDER * ppp).round().max(4.0) as i32;
                crate::infrastructure::win32::set_hollow_window_region(raw, w_px, h_px, border_px);

                let cached = hwnd_cache.load(std::sync::atomic::Ordering::Relaxed);
                if raw != cached {
                    crate::infrastructure::win32::apply_overlay_attributes(raw, hide_capture);
                    hwnd_cache.store(raw, std::sync::atomic::Ordering::Relaxed);
                } else if hide_capture {
                    crate::infrastructure::win32::set_hide_from_capture(raw, true);
                }
            }
        },
    );
}
