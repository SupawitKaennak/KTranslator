use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_image_processing(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
    captured_frame: Option<&crate::core::ports::FrameRgba>,
) {
    ui.heading(i18n.tab_image_processing);
    ui.add_space(8.0);

    let img_proc = &mut settings.img_proc;

    // --- LIVE PREVIEW SECTION ---
    super::section_header(ui, "Live Preview Processed Image");
    ui.label("Real-time preview of filters applied before OCR engine extraction:");
    ui.add_space(4.0);

    // Let's handle vector allocation cleanly to satisfy strict compiler references:
    let dummy_buffer;
    let raw_pixels: &[u8];
    let w;
    let h;

    if let Some(frame) = captured_frame {
        raw_pixels = &frame.data;
        w = frame.width;
        h = frame.height;
        if frame.width > 0 {
            ui.label(
                egui::RichText::new(format!("Using live captured frame ({}x{})", w, h))
                    .color(egui::Color32::LIGHT_GREEN),
            );
        }
    } else {
        let fw = 400;
        let fh = 80;
        let mut sample = vec![240u8; (fw * fh * 4) as usize];
        for y in 20..60 {
            for x in 40..360 {
                if (x / 15) % 2 == 0 && (y / 5) % 2 == 0 {
                    let idx = ((y * fw + x) * 4) as usize;
                    sample[idx] = 40;
                    sample[idx + 1] = 40;
                    sample[idx + 2] = 40;
                    sample[idx + 3] = 255;
                }
            }
        }
        dummy_buffer = sample;
        raw_pixels = &dummy_buffer;
        w = fw;
        h = fh;
        ui.label(
            egui::RichText::new(
                "Using placeholder sample text (capture screen to view live frame)",
            )
            .color(egui::Color32::LIGHT_YELLOW),
        );
    }
    ui.add_space(4.0);

    // Apply high-performance processing pipeline
    let (processed_data, pw, ph) =
        crate::core::usecases::image_processing_usecase::process_image_for_ocr(
            raw_pixels, w, h, img_proc,
        );

    // Render Preview Texture on GUI
    let color_img =
        egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &processed_data);
    let handle = ctx.load_texture("img_proc_preview", color_img, egui::TextureOptions::NEAREST);

    ui.add(egui::Image::new(&handle).max_width(ui.available_width().min(pw as f32)));
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // --- CONTROLS SECTION ---
    egui::Grid::new("img_proc_grid")
        .num_columns(2)
        .spacing([20.0, 10.0])
        .show(ui, |ui| {
            ui.label("Grayscale:");
            ui.checkbox(&mut img_proc.grayscale, "Convert to Monochrome");
            ui.end_row();

            ui.label("Invert Colors:");
            ui.checkbox(&mut img_proc.invert, "Negative Mapping (White on Black)");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_binarize));
            ui.horizontal(|ui| {
                ui.checkbox(&mut img_proc.binarize, "Enable");
                if img_proc.binarize {
                    ui.add_space(10.0);
                    ui.add(
                        egui::Slider::new(&mut img_proc.binary_threshold, 0..=255).text("Level"),
                    );
                }
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.img_adaptive));
            ui.checkbox(
                &mut img_proc.adaptive_threshold,
                "Local Box-filter Mean (Best for gradients)",
            );
            ui.end_row();

            ui.label(format!("{}:", i18n.img_contrast));
            ui.add(egui::Slider::new(&mut img_proc.contrast, 0.0..=3.0));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_brightness));
            ui.add(egui::Slider::new(&mut img_proc.brightness, -255..=255));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_gamma));
            ui.add(egui::Slider::new(&mut img_proc.gamma, 0.1..=5.0));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_sharpen));
            ui.checkbox(&mut img_proc.sharpen, "3x3 Spatial Edge Boost");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_denoise));
            ui.checkbox(&mut img_proc.denoise, "Box Smoothing Filter");
            ui.end_row();

            ui.label(format!("{}:", i18n.img_morphology));
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut img_proc.morphology,
                    crate::infrastructure::settings::MorphologyOp::None,
                    "None",
                );
                ui.radio_value(
                    &mut img_proc.morphology,
                    crate::infrastructure::settings::MorphologyOp::Dilation,
                    "Dilation (Thick)",
                );
                ui.radio_value(
                    &mut img_proc.morphology,
                    crate::infrastructure::settings::MorphologyOp::Erosion,
                    "Erosion (Thin)",
                );
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.img_resize));
            ui.add(egui::Slider::new(&mut img_proc.resize_scale, 0.5..=4.0).suffix("x"));
            ui.end_row();

            ui.label(format!("{}:", i18n.img_antialias));
            ui.checkbox(
                &mut img_proc.anti_alias_removal,
                "Quantize Boundary Smoothing",
            );
            ui.end_row();

            ui.label(format!("{}:", i18n.img_deskew));
            ui.checkbox(&mut img_proc.deskew, "Auto Alignment Correction");
            ui.end_row();
        });
}
