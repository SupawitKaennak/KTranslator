//! Fullscreen region picker / editor (Luna-style, single viewport).
//!
//! One opaque fullscreen window with a screenshot — no transparent layered HWND,
//! no multi-helper jitter. Used for both "select new area" and "adjust existing".

use std::sync::Arc;

use egui::{self, Color32, Pos2, Sense, Stroke};
use parking_lot::Mutex;
use screenshots::Screen;

use crate::core::types::Rect;

const MIN_W_PX: f32 = 150.0;
const MIN_H_PX: f32 = 100.0;
const HANDLE_RADIUS: f32 = 9.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    Body,
    N,
    S,
    E,
    W,
    Ne,
    Nw,
    Se,
    Sw,
}

/// Result of the region overlay (one shot).
pub enum RegionOutcome {
    Cancelled,
    Done { slot: usize, rect: Rect },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionMode {
    /// Drag empty area to draw a new rectangle.
    Create,
    /// Move / resize an existing rectangle.
    Edit,
}

pub struct RegionOverlayState {
    pub slot_idx: usize,
    pub mode: RegionMode,
    pub texture: egui::TextureHandle,
    pub origin: (i32, i32),
    pub px: (u32, u32),
    /// Selection in **screen** pixels.
    pub rect: Option<Rect>,
    create_drag_start: Option<Pos2>,
    create_drag_current: Option<Pos2>,
    active_handle: Option<Handle>,
    drag_pointer_start: Option<Pos2>,
    drag_rect_origin: Option<Rect>,
    /// True while the user is dragging a handle or the box body (edit mode).
    edit_drag_active: bool,
}

impl RegionOverlayState {
    /// Open the area picker. If `existing` is set, shows handles to move/resize it.
    pub fn start(
        slot_idx: usize,
        display_id: u32,
        ctx: &egui::Context,
        existing: Option<Rect>,
    ) -> anyhow::Result<Self> {
        let mut s = Self::capture(slot_idx, display_id, ctx)?;
        if let Some(r) = existing {
            s.mode = RegionMode::Edit;
            s.rect = Some(r);
        } else {
            s.mode = RegionMode::Create;
        }
        Ok(s)
    }

    fn capture(slot_idx: usize, display_id: u32, ctx: &egui::Context) -> anyhow::Result<Self> {
        let screens = Screen::all()?;
        let screen = screens
            .iter()
            .find(|s| s.display_info.id == display_id)
            .or_else(|| screens.iter().find(|s| s.display_info.is_primary))
            .or_else(|| screens.first())
            .ok_or_else(|| anyhow::anyhow!("no display found"))?;
        let img = screen.capture()?;
        let w = img.width();
        let h = img.height();
        let rgba = img.into_raw();
        let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let tid = format!(
            "region_{}_{}",
            slot_idx,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let texture = ctx.load_texture(tid, color, Default::default());
        Ok(Self {
            slot_idx,
            mode: RegionMode::Create,
            texture,
            origin: (screen.display_info.x, screen.display_info.y),
            px: (w, h),
            rect: None,
            create_drag_start: None,
            create_drag_current: None,
            active_handle: None,
            drag_pointer_start: None,
            drag_rect_origin: None,
            edit_drag_active: false,
        })
    }

    fn screen_to_egui(&self, img_rect: &egui::Rect, r: Rect) -> egui::Rect {
        let w = self.px.0 as f32;
        let h = self.px.1 as f32;
        let nx1 = ((r.x - self.origin.0 as f32) / w).clamp(0.0, 1.0);
        let ny1 = ((r.y - self.origin.1 as f32) / h).clamp(0.0, 1.0);
        let nx2 = ((r.x + r.w - self.origin.0 as f32) / w).clamp(0.0, 1.0);
        let ny2 = ((r.y + r.h - self.origin.1 as f32) / h).clamp(0.0, 1.0);
        egui::Rect::from_min_max(
            egui::pos2(
                img_rect.min.x + nx1 * img_rect.width(),
                img_rect.min.y + ny1 * img_rect.height(),
            ),
            egui::pos2(
                img_rect.min.x + nx2 * img_rect.width(),
                img_rect.min.y + ny2 * img_rect.height(),
            ),
        )
    }

    fn pixel_to_screen(&self, img_rect: &egui::Rect, p: Pos2) -> (f32, f32) {
        let w = self.px.0 as f32;
        let h = self.px.1 as f32;
        let nx = ((p.x - img_rect.min.x) / img_rect.width()).clamp(0.0, 1.0);
        let ny = ((p.y - img_rect.min.y) / img_rect.height()).clamp(0.0, 1.0);
        (self.origin.0 as f32 + nx * w, self.origin.1 as f32 + ny * h)
    }

    fn try_finish_create(&self, img_rect: &egui::Rect, a: Pos2, b: Pos2) -> Option<Rect> {
        let (sx1, sy1) = self.pixel_to_screen(img_rect, a);
        let (sx2, sy2) = self.pixel_to_screen(img_rect, b);
        let x = sx1.min(sx2);
        let y = sy1.min(sy2);
        let w = (sx1 - sx2).abs();
        let h = (sy1 - sy2).abs();
        if w < MIN_W_PX || h < MIN_H_PX {
            return None;
        }
        Some(Rect { x, y, w, h })
    }

    fn hit_handle(egui_r: egui::Rect, p: Pos2) -> Option<Handle> {
        let corners = [
            (egui_r.left_top(), Handle::Nw),
            (egui_r.right_top(), Handle::Ne),
            (egui_r.left_bottom(), Handle::Sw),
            (egui_r.right_bottom(), Handle::Se),
        ];
        for (c, h) in corners {
            if p.distance(c) <= HANDLE_RADIUS {
                return Some(h);
            }
        }
        let edges = [
            (egui::pos2(egui_r.center().x, egui_r.top()), Handle::N),
            (egui::pos2(egui_r.center().x, egui_r.bottom()), Handle::S),
            (egui::pos2(egui_r.left(), egui_r.center().y), Handle::W),
            (egui::pos2(egui_r.right(), egui_r.center().y), Handle::E),
        ];
        for (c, h) in edges {
            if p.distance(c) <= HANDLE_RADIUS {
                return Some(h);
            }
        }
        if egui_r.contains(p) {
            return Some(Handle::Body);
        }
        None
    }

    fn apply_handle_drag(origin: Rect, handle: Handle, dx: f32, dy: f32) -> Rect {
        let mut x = origin.x;
        let mut y = origin.y;
        let mut w = origin.w;
        let mut h = origin.h;
        match handle {
            Handle::Body => {
                x += dx;
                y += dy;
            }
            Handle::E => w = (origin.w + dx).max(MIN_W_PX),
            Handle::W => {
                let nw = (origin.w - dx).max(MIN_W_PX);
                x = origin.x + origin.w - nw;
                w = nw;
            }
            Handle::S => h = (origin.h + dy).max(MIN_H_PX),
            Handle::N => {
                let nh = (origin.h - dy).max(MIN_H_PX);
                y = origin.y + origin.h - nh;
                h = nh;
            }
            Handle::Se => {
                w = (origin.w + dx).max(MIN_W_PX);
                h = (origin.h + dy).max(MIN_H_PX);
            }
            Handle::Sw => {
                let nw = (origin.w - dx).max(MIN_W_PX);
                x = origin.x + origin.w - nw;
                w = nw;
                h = (origin.h + dy).max(MIN_H_PX);
            }
            Handle::Ne => {
                w = (origin.w + dx).max(MIN_W_PX);
                let nh = (origin.h - dy).max(MIN_H_PX);
                y = origin.y + origin.h - nh;
                h = nh;
            }
            Handle::Nw => {
                let nw = (origin.w - dx).max(MIN_W_PX);
                x = origin.x + origin.w - nw;
                w = nw;
                let nh = (origin.h - dy).max(MIN_H_PX);
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
}

pub fn run_region_viewport(
    ctx: &egui::Context,
    state: Arc<Mutex<RegionOverlayState>>,
    outcome: Arc<Mutex<Option<RegionOutcome>>>,
    lang: crate::infrastructure::settings::UiLanguage,
) {
    let i18n = crate::user_interface::i18n::get_i18n(lang);
    let title = i18n.region_title;

    let state_clone = state.clone();
    let outcome_clone = outcome.clone();
    
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("screen_translator_region_overlay"),
        egui::ViewportBuilder::default()
            .with_title(title)
            .with_fullscreen(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop),
        move |ctx, class| {
            let i18n = crate::user_interface::i18n::get_i18n(lang);
            crate::user_interface::font_loader_setup::setup_fonts(ctx);
            if matches!(class, egui::ViewportClass::Embedded) {
                egui::Window::new(i18n.region).show(ctx, |ui| {
                    region_content(ui, &state_clone, &outcome_clone, i18n);
                });
            } else {
                egui::CentralPanel::default().show(ctx, |ui| {
                    region_content(ui, &state_clone, &outcome_clone, i18n);
                });
            }
        },
    );
}

fn region_content(
    ui: &mut egui::Ui,
    state: &Arc<Mutex<RegionOverlayState>>,
    outcome: &Arc<Mutex<Option<RegionOutcome>>>,
    i18n: &crate::user_interface::i18n::I18n,
) {
    // If outcome is set, we're waiting for the parent to destroy this viewport.
    // Keep painting the frozen screenshot to prevent the default white background from flashing.
    if outcome.lock().is_some() {
        let st = state.lock();
        let tex = st.texture.clone();
        let full_rect = ui.max_rect();
        let painter = ui.painter();
        painter.image(
            tex.id(),
            full_rect,
            egui::Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_filled(full_rect, 0.0, Color32::from_black_alpha(140));
        return;
    }

    let mut st = state.lock();
    let tex = st.texture.clone();
    let mode = st.mode;
    let _slot_idx = st.slot_idx;

    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        drop(st);
        *outcome.lock() = Some(RegionOutcome::Cancelled);
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        ui.ctx().request_repaint();
        return;
    }

    let full_rect = ui.max_rect();
    let painter = ui.painter();
    let pointer = ui.ctx().input(|i| i.pointer.latest_pos());

    painter.image(
        tex.id(),
        full_rect,
        egui::Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    painter.rect_filled(full_rect, 0.0, Color32::from_black_alpha(140));

    match mode {
        RegionMode::Create => run_create_mode(ui, &mut st, &full_rect, pointer, outcome, i18n),
        RegionMode::Edit => run_edit_mode(ui, &mut st, &full_rect, pointer, outcome, i18n),
    }
}

fn run_create_mode(
    ui: &mut egui::Ui,
    st: &mut RegionOverlayState,
    full_rect: &egui::Rect,
    pointer: Option<Pos2>,
    outcome: &Arc<Mutex<Option<RegionOutcome>>>,
    i18n: &crate::user_interface::i18n::I18n,
) {
    let painter = ui.painter();
    let response = ui.interact(*full_rect, ui.id().with("create"), Sense::click_and_drag());

    if response.drag_started() {
        if let Some(p) = pointer {
            st.create_drag_start = Some(p);
            st.create_drag_current = Some(p);
        }
    }
    if response.dragged() {
        if let Some(p) = pointer {
            st.create_drag_current = Some(p);
        }
    }

    if let (Some(start), Some(curr)) = (st.create_drag_start, st.create_drag_current) {
        let r = egui::Rect::from_two_pos(start, curr);
        painter.image(
            st.texture.id(),
            r,
            egui::Rect::from_min_max(
                egui::pos2(
                    (r.min.x - full_rect.min.x) / full_rect.width(),
                    (r.min.y - full_rect.min.y) / full_rect.height(),
                ),
                egui::pos2(
                    (r.max.x - full_rect.min.x) / full_rect.width(),
                    (r.max.y - full_rect.min.y) / full_rect.height(),
                ),
            ),
            Color32::WHITE,
        );
        draw_selection_chrome(painter, r, st, full_rect, start, curr, i18n);
    }

    if let Some(p) = pointer {
        draw_crosshair(painter, *full_rect, p);
    }

    if response.drag_stopped() {
        if let (Some(a), Some(b)) = (st.create_drag_start, st.create_drag_current) {
            if let Some(rect) = st.try_finish_create(full_rect, a, b) {
                let slot = st.slot_idx;
                *outcome.lock() = Some(RegionOutcome::Done {
                    slot,
                    rect: rect.snap_to_pixels(),
                });
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                ui.ctx().request_repaint();
                return;
            }
        }
        st.create_drag_start = None;
        st.create_drag_current = None;
    }
}

fn run_edit_mode(
    ui: &mut egui::Ui,
    st: &mut RegionOverlayState,
    full_rect: &egui::Rect,
    pointer: Option<Pos2>,
    outcome: &Arc<Mutex<Option<RegionOutcome>>>,
    i18n: &crate::user_interface::i18n::I18n,
) {
    let screen_rect = match st.rect {
        Some(r) => r,
        None => return,
    };
    let egui_r = st.screen_to_egui(full_rect, screen_rect);

    {
        let painter = ui.painter();
        painter.image(
            st.texture.id(),
            egui_r,
            egui::Rect::from_min_max(
                egui::pos2(
                    (egui_r.min.x - full_rect.min.x) / full_rect.width(),
                    (egui_r.min.y - full_rect.min.y) / full_rect.height(),
                ),
                egui::pos2(
                    (egui_r.max.x - full_rect.min.x) / full_rect.width(),
                    (egui_r.max.y - full_rect.min.y) / full_rect.height(),
                ),
            ),
            Color32::WHITE,
        );
        draw_handles(painter, egui_r);
        let stroke = Stroke::new(2.5, Color32::from_rgb(0, 255, 128));
        painter.rect_stroke(egui_r, 0.0, stroke, egui::StrokeKind::Outside);
        let label = format!(
            "{} × {} — {}",
            screen_rect.w.round() as i32,
            screen_rect.h.round() as i32,
            i18n.release_to_save
        );
        let galley =
            painter.layout_no_wrap(label, egui::FontId::proportional(14.0), Color32::WHITE);
        let label_pos = egui::pos2(
            egui_r.center().x - galley.size().x / 2.0,
            egui_r.max.y + 8.0,
        );
        let bg = egui::Rect::from_min_size(
            label_pos - egui::vec2(4.0, 2.0),
            galley.size() + egui::vec2(8.0, 4.0),
        );
        painter.rect_filled(bg, 4.0, Color32::from_black_alpha(200));
        painter.galley(label_pos, galley, Color32::WHITE);
    }

    let Some(p) = pointer else {
        return;
    };

    if ui.ctx().input(|i| i.pointer.primary_pressed()) {
        st.active_handle = RegionOverlayState::hit_handle(egui_r, p);
        if st.active_handle.is_some() {
            st.drag_pointer_start = Some(p);
            st.drag_rect_origin = st.rect;
            st.edit_drag_active = true;
        }
    }

    if ui.ctx().input(|i| i.pointer.primary_down()) {
        if let (Some(handle), Some(start_p), Some(origin)) =
            (st.active_handle, st.drag_pointer_start, st.drag_rect_origin)
        {
            let (sx0, sy0) = st.pixel_to_screen(full_rect, start_p);
            let (sx1, sy1) = st.pixel_to_screen(full_rect, p);
            let dx = sx1 - sx0;
            let dy = sy1 - sy0;
            st.rect = Some(RegionOverlayState::apply_handle_drag(
                origin, handle, dx, dy,
            ));
        }
    }

    if ui.ctx().input(|i| i.pointer.primary_released()) {
        if st.edit_drag_active {
            if let Some(rect) = st.rect.map(|r| r.snap_to_pixels()) {
                let slot = st.slot_idx;
                *outcome.lock() = Some(RegionOutcome::Done { slot, rect });
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                ui.ctx().request_repaint();
                return;
            }
            st.edit_drag_active = false;
        }
        st.active_handle = None;
        st.drag_pointer_start = None;
        st.drag_rect_origin = None;
    }

    if let Some(handle) = st.active_handle {
        ui.ctx().set_cursor_icon(cursor_for_handle(handle));
    } else if let Some(h) = RegionOverlayState::hit_handle(egui_r, p) {
        ui.ctx().set_cursor_icon(cursor_for_handle(h));
    }
}

fn cursor_for_handle(h: Handle) -> egui::CursorIcon {
    match h {
        Handle::Body => egui::CursorIcon::Grab,
        Handle::N | Handle::S => egui::CursorIcon::ResizeVertical,
        Handle::E | Handle::W => egui::CursorIcon::ResizeHorizontal,
        Handle::Nw | Handle::Se => egui::CursorIcon::ResizeNwSe,
        Handle::Ne | Handle::Sw => egui::CursorIcon::ResizeNeSw,
    }
}

fn draw_handles(painter: &egui::Painter, r: egui::Rect) {
    let fill = Color32::from_rgb(0, 220, 120);
    let stroke = Stroke::new(1.5, Color32::WHITE);
    for c in [
        r.left_top(),
        r.right_top(),
        r.left_bottom(),
        r.right_bottom(),
    ] {
        let hr =
            egui::Rect::from_center_size(c, egui::vec2(HANDLE_RADIUS * 2.0, HANDLE_RADIUS * 2.0));
        painter.rect_filled(hr, 3.0, fill);
        painter.rect_stroke(hr, 3.0, stroke, egui::StrokeKind::Outside);
    }
}

fn draw_selection_chrome(
    painter: &egui::Painter,
    r: egui::Rect,
    st: &RegionOverlayState,
    full_rect: &egui::Rect,
    start: Pos2,
    curr: Pos2,
    _i18n: &crate::user_interface::i18n::I18n,
) {
    painter.rect_stroke(
        r,
        0.0,
        Stroke::new(2.0, Color32::from_rgb(0, 255, 128)),
        egui::StrokeKind::Outside,
    );
    draw_handles(painter, r);
    let (sx1, sy1) = st.pixel_to_screen(full_rect, start);
    let (sx2, sy2) = st.pixel_to_screen(full_rect, curr);
    let w = (sx1 - sx2).abs() as i32;
    let h = (sy1 - sy2).abs() as i32;
    let label = format!("{w} x {h}");
    let galley = painter.layout_no_wrap(label, egui::FontId::proportional(14.0), Color32::WHITE);
    let label_pos = curr + egui::vec2(10.0, 10.0);
    let label_rect = egui::Rect::from_min_size(
        label_pos - egui::vec2(4.0, 2.0),
        galley.size() + egui::vec2(8.0, 4.0),
    );
    painter.rect_filled(label_rect, 4.0, Color32::from_black_alpha(200));
    painter.galley(label_pos, galley, Color32::WHITE);
}

fn draw_crosshair(painter: &egui::Painter, full_rect: egui::Rect, p: Pos2) {
    let stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 100));
    painter.line_segment(
        [
            egui::pos2(full_rect.left(), p.y),
            egui::pos2(full_rect.right(), p.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(p.x, full_rect.top()),
            egui::pos2(p.x, full_rect.bottom()),
        ],
        stroke,
    );
}
