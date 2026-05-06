use crate::infra::settings::UiLanguage;

pub struct I18n {
    pub settings: &'static str,
    pub translation: &'static str,
    pub ocr: &'static str,
    pub language: &'static str,
    pub provider: &'static str,
    pub model: &'static str,
    pub api_key: &'static str,
    pub appearance: &'static str,
    pub dark_mode: &'static str,
    pub overlay: &'static str,
    pub font_size: &'static str,
    pub padding: &'static str,
    pub corner_radius: &'static str,
    pub bg_color: &'static str,
    pub text_color: &'static str,
    pub start_capture: &'static str,
    pub stop_capture: &'static str,
    pub clear_results: &'static str,
    pub ui_language: &'static str,
    pub auto_detect: &'static str,
    pub system_default: &'static str,
    pub allow_capture: &'static str,
    pub add_region: &'static str,
    
    // Slot UI
    pub region: &'static str,
    pub active: &'static str,
    pub select_area: &'static str,
    pub screen: &'static str,
    pub refresh: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    pub show_frame: &'static str,
    pub overlay_mode: &'static str,
    pub open_popup: &'static str,
    pub manual_pos: &'static str,
    pub idle: &'static str,
    pub busy: &'static str,
}

const EN: I18n = I18n {
    settings: "Settings",
    translation: "Translation",
    ocr: "OCR",
    language: "Language",
    provider: "Provider",
    model: "Model",
    api_key: "API Key",
    appearance: "Appearance",
    dark_mode: "Dark Mode",
    overlay: "Overlay",
    font_size: "Font Size",
    padding: "Padding",
    corner_radius: "Corner Radius",
    bg_color: "Background Color",
    text_color: "Text Color",
    start_capture: "Start Translate",
    stop_capture: "Stop Translate",
    clear_results: "Clear Results",
    ui_language: "UI Language",
    auto_detect: "Auto Detect",
    system_default: "System Default",
    allow_capture: "Screenshot Mode (Allow Snip)",
    add_region: "Add Region",
    
    region: "Region",
    active: "Active",
    select_area: "Select Area",
    screen: "Screen",
    refresh: "Refresh",
    from: "From",
    to: "To",
    show_frame: "Show Frame Box",
    overlay_mode: "Overlay Mode",
    open_popup: "Open Popup",
    manual_pos: "Manual Position Adjustment",
    idle: "Idle",
    busy: "Processing...",
};

const TH: I18n = I18n {
    settings: "ตั้งค่า",
    translation: "การแปล",
    ocr: "OCR (การอ่านข้อความ)",
    language: "ภาษา",
    provider: "ผู้ให้บริการ AI",
    model: "รุ่น Model",
    api_key: "รหัส API Key",
    appearance: "รูปลักษณ์",
    dark_mode: "โหมดมืด",
    overlay: "การแสดงผลทับหน้าจอ",
    font_size: "ขนาดตัวอักษร",
    padding: "ระยะขอบ",
    corner_radius: "ความโค้งมน",
    bg_color: "สีพื้นหลัง",
    text_color: "สีตัวอักษร",
    start_capture: "เริ่มแปล",
    stop_capture: "หยุดแปล",
    clear_results: "ล้างหน้าจอ",
    ui_language: "ภาษาของเมนู",
    auto_detect: "ตรวจจับอัตโนมัติ",
    system_default: "ตามระบบเครื่อง",
    allow_capture: "โหมดแคปจอ (ปิดการล่องหน)",
    add_region: "เพิ่มพื้นที่",

    region: "พื้นที่",
    active: "ทำงานอยู่",
    select_area: "เลือกพื้นที่",
    screen: "หน้าจอ",
    refresh: "ความถี่",
    from: "จาก",
    to: "เป็น",
    show_frame: "แสดงกรอบ",
    overlay_mode: "โหมดทับหน้าจอ",
    open_popup: "เปิดหน้าต่างแยก",
    manual_pos: "ปรับตำแหน่งเอง",
    idle: "รอทำงาน",
    busy: "กำลังประมวลผล...",
};

pub fn get_i18n(lang: UiLanguage) -> &'static I18n {
    match lang {
        UiLanguage::Thai => &TH,
        UiLanguage::English => &EN,
        UiLanguage::System => {
            let locale = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
            if locale.starts_with("th") {
                &TH
            } else {
                &EN
            }
        }
    }
}
