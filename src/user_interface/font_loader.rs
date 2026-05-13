use eframe::egui;
use std::sync::Arc;

/// Configures and loads all necessary fonts for multi-language support.
/// Includes an embedded Thai font and attempts to load common Windows 
/// system fonts for CJK and other scripts.
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 1. Embedded Thai font (always available)
    fonts.font_data.insert(
        "noto_sans_thai".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/NotoSansThai.ttf"
        ))),
    );

    // 2. Load additional fonts from assets/fonts (Runtime) or Windows system fonts
    //    We prioritize our local Noto Sans fonts if they exist.
    let mut loaded: Vec<String> = Vec::new();
    
    let local_fonts = &[
        ("noto_jp", "assets/fonts/NotoSansJP-Regular.otf"),
        ("noto_sc", "assets/fonts/NotoSansSC-Regular.otf"),
        ("noto_tc", "assets/fonts/NotoSansTC-Regular.otf"),
        ("noto_kr", "assets/fonts/NotoSansKR-Regular.otf"),
        ("noto_arabic", "assets/fonts/NotoSansArabic-Regular.ttf"),
        ("noto_devanagari", "assets/fonts/NotoSansDevanagari-Regular.ttf"),
        ("noto_latin", "assets/fonts/NotoSans-Regular.ttf"),
    ];

    for (key, path) in local_fonts {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                (*key).to_owned(),
                Arc::new(egui::FontData::from_owned(data)),
            );
            loaded.push(key.to_string());
        }
    }

    // 3. Fallback to Windows system fonts if any local fonts are missing
    let system_fonts: &[(&str, &str)] = &[
        ("msyh",    r"C:\Windows\Fonts\msyh.ttc"),     // Simplified Chinese
        ("msgoth",  r"C:\Windows\Fonts\msgothic.ttc"),  // Japanese
        ("malgun",  r"C:\Windows\Fonts\malgun.ttf"),    // Korean
        ("arial",   r"C:\Windows\Fonts\arial.ttf"),     // Arabic/Latin fallback
        ("nirmala", r"C:\Windows\Fonts\Nirmala.ttf"),   // Devanagari
    ];

    for (key, path) in system_fonts {
        // Only load if we don't have a better version already
        if !loaded.iter().any(|l| l.contains(key)) {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert(
                    (*key).to_owned(),
                    Arc::new(egui::FontData::from_owned(data)),
                );
                loaded.push(key.to_string());
            }
        }
    }

    // 4. Register all fonts as fallbacks (Thai first, then everything else)
    let fallback_order = {
        let mut v = vec!["noto_sans_thai".to_owned()];
        v.extend(loaded.iter().cloned());
        v
    };

    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().extend(fallback_order.clone());
    fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().extend(fallback_order);

    ctx.set_fonts(fonts);
}
