use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RealtimeStabilitySettings {
    pub stability_threshold_frames: u32, // Wait N identical text frames before translating (typewriter debounce)
    pub subtitle_persistence_ms: u64,    // Keep text on screen for N ms after source disappears
    pub context_window_size: u32,        // N previous segment translations passed as context
    pub fade_smoothing: bool,            // Apply crossfade/smoothing animations
}

impl Default for RealtimeStabilitySettings {
    fn default() -> Self {
        Self {
            stability_threshold_frames: 1, // Default 1 (translates immediately on first full-text grab, or 2 for games)
            subtitle_persistence_ms: 2500, // Hold subtitles for 2.5 seconds
            context_window_size: 2,        // 2 prior segments
            fade_smoothing: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformancePreset {
    Eco,
    Balanced,
    Performance,
    Ultra,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuBackend {
    Auto,
    Cpu,
    Cuda,
    DirectMl,
    TensorRt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSettings {
    pub preset: PerformancePreset,
    pub worker_threads: usize,
    pub gpu_backend: GpuBackend,
    pub parallel_ocr: bool,
    pub enable_batching: bool,
    pub memory_cleanup_interval_secs: u64,
    pub max_cache_entries: usize,
    pub vram_limit_mb: u32, // 0 = unlimited
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            preset: PerformancePreset::Balanced,
            worker_threads: 4,
            gpu_backend: GpuBackend::Auto,
            parallel_ocr: true,
            enable_batching: true,
            memory_cleanup_interval_secs: 300, // 5 minutes
            max_cache_entries: 5000,
            vram_limit_mb: 0,
        }
    }
}

impl PerformanceSettings {
    pub fn apply_preset(&mut self, preset: PerformancePreset) {
        self.preset = preset;
        match preset {
            PerformancePreset::Eco => {
                self.worker_threads = 2;
                self.parallel_ocr = false;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 60;
                self.max_cache_entries = 1000;
                self.vram_limit_mb = 1024;
            }
            PerformancePreset::Balanced => {
                self.worker_threads = 4;
                self.parallel_ocr = true;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 300;
                self.max_cache_entries = 5000;
                self.vram_limit_mb = 0;
            }
            PerformancePreset::Performance => {
                self.worker_threads = 8;
                self.parallel_ocr = true;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 600;
                self.max_cache_entries = 20000;
                self.vram_limit_mb = 0;
            }
            PerformancePreset::Ultra => {
                self.worker_threads = 16;
                self.parallel_ocr = true;
                self.enable_batching = true;
                self.memory_cleanup_interval_secs = 1200;
                self.max_cache_entries = 50000;
                self.vram_limit_mb = 0;
            }
            PerformancePreset::Custom => {
                // Keep values as-is to allow manual user fine-tuning
            }
        }
    }

    pub fn enforce_preset_locks(&mut self) {
        // Automatically restore locked preset values if preset is not Custom
        let current_preset = self.preset;
        if current_preset != PerformancePreset::Custom {
            self.apply_preset(current_preset);
        }
    }
}
