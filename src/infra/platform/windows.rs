use super::PlatformServices;

/// Windows platform implementation using Win32 APIs.
pub struct WindowsPlatform;

impl PlatformServices for WindowsPlatform {
    fn find_window_by_title(&self, title: &str) -> Option<isize> {
        crate::infra::win32::find_window(title)
    }

    fn apply_overlay_transparency(&self, window_handle: isize) {
        crate::infra::win32::apply_overlay_attributes(window_handle);
    }

    fn boost_process_priority(&self) {
        crate::infra::win32::boost_process_priority();
    }
}
