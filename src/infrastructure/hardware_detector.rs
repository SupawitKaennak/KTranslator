use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedGpu {
    pub name: String,
    pub vram_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemHardwareInfo {
    pub cpu_name: String,
    pub cpu_threads: usize,
    pub gpus: Vec<DetectedGpu>,
    pub primary_gpu: Option<DetectedGpu>,
}

pub static HARDWARE_INFO: OnceLock<SystemHardwareInfo> = OnceLock::new();

pub fn get_hardware_info() -> &'static SystemHardwareInfo {
    HARDWARE_INFO.get_or_init(detect_hardware)
}

pub fn detect_hardware() -> SystemHardwareInfo {
    let cpu_name = detect_cpu_name();
    let cpu_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let gpus = detect_gpus();
    let primary_gpu = gpus.iter().max_by_key(|g| g.vram_mb).cloned();

    SystemHardwareInfo {
        cpu_name,
        cpu_threads,
        gpus,
        primary_gpu,
    }
}

#[cfg(target_os = "windows")]
fn detect_cpu_name() -> String {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ,
    };

    unsafe {
        let subkey = windows::core::w!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0");
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey, None, KEY_READ, &mut hkey).is_ok() {
            let val_name = windows::core::w!("ProcessorNameString");
            let mut buf = [0u16; 256];
            let mut size = (buf.len() * std::mem::size_of::<u16>()) as u32;
            let mut dtype = REG_SZ;

            let res = RegQueryValueExW(
                hkey,
                val_name,
                None,
                Some(&mut dtype),
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut size),
            );
            let _ = RegCloseKey(hkey);

            if res.is_ok() && size > 2 {
                let len = ((size as usize) / 2).saturating_sub(1);
                let name = String::from_utf16_lossy(&buf[..len]);
                let trimmed = name.trim().to_string();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
        }
    }

    std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "Generic CPU".to_string())
}

#[cfg(not(target_os = "windows"))]
fn detect_cpu_name() -> String {
    "Generic CPU".to_string()
}

#[cfg(target_os = "windows")]
fn detect_gpus() -> Vec<DetectedGpu> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD,
        REG_QWORD, REG_SZ,
    };

    let mut gpus = Vec::new();

    // Enumerate display adapter keys 0000..0009 under display adapter class GUID
    for idx in 0..10 {
        let subkey_str = format!(
            "SYSTEM\\CurrentControlSet\\Control\\Class\\{{4d36e968-e325-11ce-bfc1-08002be10318}}\\{:04}\0",
            idx
        );
        let subkey_utf16: Vec<u16> = subkey_str.encode_utf16().collect();
        let subkey = windows::core::PCWSTR(subkey_utf16.as_ptr());

        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey, None, KEY_READ, &mut hkey).is_ok() {
                // Read DriverDesc
                let desc_val = windows::core::w!("DriverDesc");
                let mut buf = [0u16; 256];
                let mut size = (buf.len() * std::mem::size_of::<u16>()) as u32;
                let mut dtype = REG_SZ;

                let name = if RegQueryValueExW(
                    hkey,
                    desc_val,
                    None,
                    Some(&mut dtype),
                    Some(buf.as_mut_ptr() as *mut u8),
                    Some(&mut size),
                )
                .is_ok()
                    && size > 2
                {
                    let len = ((size as usize) / 2).saturating_sub(1);
                    String::from_utf16_lossy(&buf[..len]).trim().to_string()
                } else {
                    String::new()
                };

                // Read HardwareInformation.qwMemorySize (QWORD, 64-bit) or HardwareInformation.MemorySize (DWORD, 32-bit)
                let mut vram_bytes: u64 = 0;
                let qw_mem_val = windows::core::w!("HardwareInformation.qwMemorySize");
                let mut qw_buf = 0u64;
                let mut qw_size = std::mem::size_of::<u64>() as u32;
                let mut qw_dtype = REG_QWORD;

                if RegQueryValueExW(
                    hkey,
                    qw_mem_val,
                    None,
                    Some(&mut qw_dtype),
                    Some(&mut qw_buf as *mut u64 as *mut u8),
                    Some(&mut qw_size),
                )
                .is_ok()
                {
                    vram_bytes = qw_buf;
                } else {
                    let dw_mem_val = windows::core::w!("HardwareInformation.MemorySize");
                    let mut dw_buf = 0u32;
                    let mut dw_size = std::mem::size_of::<u32>() as u32;
                    let mut dw_dtype = REG_DWORD;
                    if RegQueryValueExW(
                        hkey,
                        dw_mem_val,
                        None,
                        Some(&mut dw_dtype),
                        Some(&mut dw_buf as *mut u32 as *mut u8),
                        Some(&mut dw_size),
                    )
                    .is_ok()
                    {
                        vram_bytes = dw_buf as u64;
                    }
                }

                let _ = RegCloseKey(hkey);

                if !name.is_empty() && !name.contains("Basic Display") {
                    let vram_mb = vram_bytes / (1024 * 1024);
                    if !gpus.iter().any(|g: &DetectedGpu| g.name == name) {
                        gpus.push(DetectedGpu { name, vram_mb });
                    }
                }
            }
        }
    }

    gpus
}

#[cfg(not(target_os = "windows"))]
fn detect_gpus() -> Vec<DetectedGpu> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hardware() {
        let info = detect_hardware();
        assert!(!info.cpu_name.is_empty(), "CPU name should not be empty");
        assert!(info.cpu_threads >= 1, "CPU threads should be >= 1");
        println!("Detected CPU: {} ({} Threads)", info.cpu_name, info.cpu_threads);
        if let Some(gpu) = &info.primary_gpu {
            println!("Detected GPU: {} ({} MB VRAM)", gpu.name, gpu.vram_mb);
        }
    }
}
