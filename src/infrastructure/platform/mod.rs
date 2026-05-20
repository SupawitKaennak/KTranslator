// ---------------------------------------------------------------------------
// Platform abstraction layer
//
// Provides a trait `PlatformServices` that encapsulates all OS-specific
// operations. This allows the core application to remain platform-agnostic
// while each platform provides its own implementation.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform as NativePlatform;

// Future: Linux and macOS implementations
// #[cfg(target_os = "linux")]
// mod linux_impl;
// #[cfg(target_os = "linux")]
// pub use linux_impl::LinuxPlatform as NativePlatform;

/// Platform-specific operations abstracted behind a common trait.
pub trait PlatformServices: Send + Sync {
    /// Find a window by its exact title. Returns an OS-specific handle.
    fn find_window_by_title(&self, title: &str) -> Option<isize>;

    /// Boost the current process priority for better responsiveness during gaming.
    fn boost_process_priority(&self);

    /// Break Thai text into words with spaces for better rendering.
    fn segment_thai(
        &self,
        text: &str,
        mode: crate::infrastructure::settings::ThaiSegmentationMode,
    ) -> String;
}

/// Create the platform services implementation for the current OS.
pub fn create_platform() -> Box<dyn PlatformServices> {
    #[cfg(target_os = "windows")]
    {
        Box::new(NativePlatform)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Box::new(StubPlatform)
    }
}

/// Stub implementation for unsupported platforms — all operations are no-ops.
#[cfg(not(target_os = "windows"))]
struct StubPlatform;

#[cfg(not(target_os = "windows"))]
impl PlatformServices for StubPlatform {
    fn find_window_by_title(&self, _title: &str) -> Option<isize> {
        None
    }
    fn boost_process_priority(&self) {}
    fn segment_thai(
        &self,
        text: &str,
        _mode: crate::infrastructure::settings::ThaiSegmentationMode,
    ) -> String {
        text.to_string()
    }
}
