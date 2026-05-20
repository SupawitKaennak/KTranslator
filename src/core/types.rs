use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegionId(pub usize); // 0..5

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Whole physical pixels — avoids jitter between Win32, egui logical space, and the model.
    #[must_use]
    pub fn snap_to_pixels(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
            w: self.w.round().max(1.0),
            h: self.h.round().max(1.0),
        }
    }
}

/// Physical screen pixels → egui logical points, rounded (stable window placement on HiDPI).
#[inline]
pub fn physical_px_to_logical_points(px: f32, ppp: f32) -> f32 {
    (px / ppp).round()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageTag(pub String); // BCP-47-ish: "en", "th", "ja", ...

impl LanguageTag {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
