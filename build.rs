fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        // 1. Convert PNG to ICO if needed
        let png_path = "assets/icons/icon.png";
        let ico_path = "assets/icons/icon.ico";
        
        // Simple conversion using the image crate (which is already a dependency)
        // Note: Since build.rs runs before main compilation, we use a simple approach
        if let Ok(img) = image::open(png_path) {
            let _ = img.save(ico_path);
        }

        // 2. Compile Windows resources
        let mut res = winres::WindowsResource::new();
        res.set_icon(ico_path);
        res.compile().unwrap();
    }
}
