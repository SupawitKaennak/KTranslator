use super::PlatformServices;

/// Windows platform implementation using Win32 APIs.
pub struct WindowsPlatform;

impl PlatformServices for WindowsPlatform {
    fn find_window_by_title(&self, title: &str) -> Option<isize> {
        crate::infrastructure::win32::find_window(title)
    }

    fn boost_process_priority(&self) {
        crate::infrastructure::win32::boost_process_priority();
    }

    fn segment_thai(
        &self,
        text: &str,
        mode: crate::infrastructure::settings::ThaiSegmentationMode,
    ) -> String {
        crate::infrastructure::win32::segment_thai(text, mode)
    }
}
