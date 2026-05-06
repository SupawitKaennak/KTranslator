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
            "../../assets/NotoSansThai.ttf"
        ))),
    );

    // 2. Windows system fonts loaded at runtime
    //    We try each path; missing fonts are silently skipped.
    let system_fonts: &[(&str, &str)] = &[
        // CJK — Chinese (Simplified), Japanese, Korean
        ("msyh",    r"C:\Windows\Fonts\msyh.ttc"),     // Microsoft YaHei  (zh)
        ("msyh",    r"C:\Windows\Fonts\msyhbd.ttc"),
        ("msgoth",  r"C:\Windows\Fonts\msgothic.ttc"),  // MS Gothic         (ja)
        ("malgun",  r"C:\Windows\Fonts\malgun.ttf"),    // Malgun Gothic     (ko)
        ("malgunbd",r"C:\Windows\Fonts\malgunbd.ttf"),
        // Arabic, Hebrew, and wide Latin coverage
        ("arial",   r"C:\Windows\Fonts\arial.ttf"),
        ("tahoma",  r"C:\Windows\Fonts\tahoma.ttf"),
        // Devanagari (Hindi, Nepali, Marathi) + other South-Asian scripts
        ("nirmala", r"C:\Windows\Fonts\Nirmala.ttf"),
        ("nirmalab",r"C:\Windows\Fonts\NirmalaB.ttf"),
        ("mangal",  r"C:\Windows\Fonts\mangal.ttf"),
        // Fallback Unicode catch-all (Office installs)
        ("arialuni",r"C:\Windows\Fonts\ARIALUNI.TTF"),
    ];

    let mut loaded: Vec<String> = Vec::new();
    for (key, path) in system_fonts {
        if loaded.contains(&key.to_string()) {
            continue; // skip duplicate keys
        }
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                (*key).to_owned(),
                Arc::new(egui::FontData::from_owned(data)),
            );
            loaded.push(key.to_string());
        }
    }

    // 3. Register all fonts as fallbacks (Thai first, then system fonts)
    let fallback_order = {
        let mut v = vec!["noto_sans_thai".to_owned()];
        v.extend(loaded.iter().cloned());
        v
    };

    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().extend(fallback_order.clone());
    fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().extend(fallback_order);

    ctx.set_fonts(fonts);
}
