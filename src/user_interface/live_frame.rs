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

    // Body area (draggable anywhere inside)
    Some(Zone::Move)
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
                d.remove::<Rect>(egui::Id::new(("last_physical_rect", slot_idx)));
                d.remove::<f64>(egui::Id::new(("ignore_until", slot_idx)));
                d.remove::<f64>(egui::Id::new(("debounce_rect", slot_idx)));
            });
        }
        return;
    }
    let r = rect.unwrap().snap_to_pixels();

    let title = format!("Frame Live {}", slot_idx + 1);
    let model_inner = model_arc.clone();
    let hwnd_cache = runtime.frame_live_hwnd.clone();
    let visual_rect = runtime.visual_rect.clone();
    let hide_capture = settings.hide_from_capture;
    let platform_svc = platform.clone();

    ctx.show_viewport_deferred(
        viewport_id,
        egui::ViewportBuilder::default()
            .with_title(&title)
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(false)
            .with_always_on_top()
            .with_active(true)
            .with_min_inner_size([150.0, 100.0])
            .with_inner_size(egui::vec2(
                snap_logical(r.w, ppp),
                snap_logical(r.h, ppp),
            ))
            .with_position(egui::pos2(
                snap_logical(r.x, ppp),
                snap_logical(r.y, ppp),
            ))
            .with_mouse_passthrough(false),
        move |ctx, class| {
            crate::user_interface::font_loader_setup::setup_fonts(ctx);
            if matches!(class, egui::ViewportClass::Embedded) {
                return;
            }

            let first_frame = ctx
                .data(|d| d.get_temp::<bool>(egui::Id::new(("first_frame", slot_idx))))
                .is_none();
            if first_frame {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new(("first_frame", slot_idx)), false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            }

            let full = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new(("live_frame", slot_idx)),
            ));

            let mut is_hovered = false;
            if let Some(raw) = platform_svc.find_window_by_title(&title) {
                #[cfg(windows)]
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, GetCursorPos};
                    use windows::Win32::Foundation::{RECT, POINT, HWND};
                    let hwnd = HWND(raw as *mut _);
                    let mut rect = RECT::default();
                    let mut pt = POINT::default();
                    if GetWindowRect(hwnd, &mut rect).is_ok() && GetCursorPos(&mut pt).is_ok() {
                        is_hovered = pt.x >= rect.left && pt.x <= rect.right && pt.y >= rect.top && pt.y <= rect.bottom;
                    }
                }
            }

            if is_hovered {
                // We use RGB(25, 25, 25) so LWA_COLORKEY does not make it fully transparent.
                // It is far enough from pure black to prevent rounding errors causing pixel holes.
                // The global window alpha will be lowered to create the glass effect, and it will catch clicks.
                painter.rect_filled(full, 0.0, egui::Color32::from_rgb(25, 25, 25));
            } else {
                // Pure black triggers LWA_COLORKEY in GDI, but for wgpu we must use alpha channel transparency.
                painter.rect_filled(full, 0.0, egui::Color32::TRANSPARENT);
            }

            // Keep polling so we can detect mouse entering/leaving
            ctx.request_repaint_after(std::time::Duration::from_millis(30));

            let accent = egui::Color32::from_rgb(0, 255, 128);

            // Main solid border - matched to BORDER width for perfect visual feedback
            painter.rect_stroke(
                full,
                0.0,
                egui::Stroke::new(BORDER, accent),
                egui::StrokeKind::Inside,
            );

            // Invisible hit-testing border so the user can easily grab corners/edges for resizing.
            // wgpu alpha compositing means fully transparent pixels pass mouse clicks to the game.
            // Alpha=1 is visually invisible but opaque enough for Windows to catch mouse events.
            painter.rect_stroke(
                full,
                0.0,
                egui::Stroke::new(HANDLE * 2.0, egui::Color32::from_rgba_premultiplied(1, 1, 1, 1)),
                egui::StrokeKind::Inside,
            );

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

            if model_changed {
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

            let last_physical_rect_id = egui::Id::new(("last_physical_rect", slot_idx));
            let debounce_id = egui::Id::new(("debounce_rect", slot_idx));

            if let Some(outer_rect) = ctx.input(|i| i.viewport().outer_rect) {
                let physical_rect = Rect {
                    x: (outer_rect.min.x * ppp).round(),
                    y: (outer_rect.min.y * ppp).round(),
                    w: (outer_rect.width() * ppp).round(),
                    h: (outer_rect.height() * ppp).round(),
                }
                .snap_to_pixels();

                let last_physical = ctx.data(|d| d.get_temp::<Rect>(last_physical_rect_id)).unwrap_or(physical_rect);
                ctx.data_mut(|d| d.insert_temp(last_physical_rect_id, physical_rect));

                // Continuously update the visual rect so the transparent overlay moves instantly with the window
                *visual_rect.lock() = Some(physical_rect);

                let is_moving = (last_physical.x - physical_rect.x).abs() > 0.1
                    || (last_physical.y - physical_rect.y).abs() > 0.1
                    || (last_physical.w - physical_rect.w).abs() > 0.1
                    || (last_physical.h - physical_rect.h).abs() > 0.1;

                if is_moving {
                    // User is dragging or resizing the window, extend debounce timer
                    ctx.data_mut(|d| d.insert_temp(debounce_id, now + 0.3));
                    // Wake up the overlay viewport so it tracks the movement instantly
                    ctx.request_repaint_of(egui::ViewportId::from_hash_of(format!("frame_overlay_{slot_idx}")));
                } else if now > ignore_until {
                    // Window is stable and we are past the initial spawn ignore timer.
                    let debounce_until = ctx.data(|d| d.get_temp::<f64>(debounce_id)).unwrap_or(0.0);
                    if now > debounce_until {
                        // Stable for > 300ms. Sync to model if it differs.
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
            }

            ctx.data_mut(|d| d.insert_temp(last_rect_id, current_r));

            if let Some(raw) = platform_svc.find_window_by_title(&title) {
                // Ensure the window region is solid so the entire frame catches mouse events.
                // This removes any hollow region previously applied.
                crate::infrastructure::win32::clear_window_region(raw);

                let cached = hwnd_cache.load(std::sync::atomic::Ordering::Relaxed);
                let current_hide = hide_capture;
                let last_hide_id = egui::Id::new(("last_hide", slot_idx));
                let last_hide = ctx.data(|d| d.get_temp::<bool>(last_hide_id));

                if raw != cached || last_hide != Some(current_hide) {
                    crate::infrastructure::win32::apply_overlay_attributes(raw, current_hide);
                    hwnd_cache.store(raw, std::sync::atomic::Ordering::Relaxed);
                    ctx.data_mut(|d| d.insert_temp(last_hide_id, current_hide));
                }

                let alpha = if is_hovered { 120 } else { 255 };
                let last_alpha_id = egui::Id::new(("last_alpha", slot_idx));
                let last_alpha = ctx.data(|d| d.get_temp::<u8>(last_alpha_id));
                if last_alpha != Some(alpha) {
                    crate::infrastructure::win32::set_window_alpha(raw, alpha);
                    ctx.data_mut(|d| d.insert_temp(last_alpha_id, alpha));
                }
            }
        },
    );
}
