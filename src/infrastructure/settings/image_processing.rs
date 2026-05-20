use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MorphologyOp {
    None,
    Dilation,
    Erosion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageProcessingSettings {
    pub grayscale: bool,
    pub invert: bool,
    pub contrast: f32,   // 0.0 - 3.0 (default 1.0)
    pub brightness: i32, // -255 - 255 (default 0)
    pub gamma: f32,      // 0.1 - 5.0 (default 1.0)
    pub binarize: bool,
    pub binary_threshold: u8, // 0 - 255 (default 127)
    pub adaptive_threshold: bool,
    pub denoise: bool,
    pub sharpen: bool,
    pub morphology: MorphologyOp,
    pub resize_scale: f32, // 0.5 - 4.0 (default 1.0)
    pub deskew: bool,
    pub anti_alias_removal: bool,
}

impl Default for ImageProcessingSettings {
    fn default() -> Self {
        Self {
            grayscale: false,
            invert: false,
            contrast: 1.0,
            brightness: 0,
            gamma: 1.0,
            binarize: false,
            binary_threshold: 127,
            adaptive_threshold: false,
            denoise: false,
            sharpen: false,
            morphology: MorphologyOp::None,
            resize_scale: 1.0,
            deskew: false,
            anti_alias_removal: false,
        }
    }
}
