use crate::infrastructure::settings::Settings;
use eframe::egui;

fn get_brand_color(name: &str) -> egui::Color32 {
    let lower = name.to_ascii_lowercase();
    if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("rtx") || lower.contains("gtx") {
        egui::Color32::from_rgb(118, 205, 50) // Nvidia Green
    } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("ryzen") {
        egui::Color32::from_rgb(240, 80, 50) // AMD Red-Orange
    } else if lower.contains("intel") || lower.contains("core") || lower.contains("xeon") || lower.contains("arc") {
        egui::Color32::from_rgb(50, 150, 255) // Intel Blue
    } else {
        egui::Color32::from_rgb(200, 200, 200) // Generic Light Gray
    }
}

pub fn render_tab_performance(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
) {
    super::section_header(ui, i18n.tab_performance);
    ui.label(egui::RichText::new("Fine-tune thread execution, hardware acceleration, and cache footprints for maximal frame stability.").small().color(egui::Color32::GRAY));
    ui.add_space(8.0);

    // ── Detected Hardware Telemetry Box ──
    let hw = crate::infrastructure::hardware_detector::get_hardware_info();
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("CPU:").strong());
                let cpu_color = get_brand_color(&hw.cpu_name);
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({} Threads)",
                        hw.cpu_name, hw.cpu_threads
                    ))
                    .small()
                    .color(cpu_color),
                );
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("GPU:").strong());
                if let Some(gpu) = &hw.primary_gpu {
                    let vram_text = if gpu.vram_mb > 0 {
                        format!("{} MB VRAM", gpu.vram_mb)
                    } else {
                        "Shared Memory".to_string()
                    };
                    let gpu_color = get_brand_color(&gpu.name);
                    ui.label(
                        egui::RichText::new(format!("{} ({})", gpu.name, vram_text))
                            .small()
                            .color(gpu_color),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("None detected (Using CPU Fallback)")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                }
            });
        });

    ui.add_space(10.0);

    // Enforce default locks immediately
    settings.perf.enforce_preset_locks();

    // ── Presets Selector ──
    ui.label(egui::RichText::new("Power & Speed Preset").strong());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        use crate::infrastructure::settings::PerformancePreset;
        let mut curr_preset = settings.perf.preset;

        let presets = [
            (PerformancePreset::Eco, "Eco", "Minimal CPU & VRAM usage"),
            (
                PerformancePreset::Balanced,
                "Balanced",
                "Optimal auto-tuned resources",
            ),
            (
                PerformancePreset::Performance,
                "Performance",
                "High speed thread scheduling",
            ),
            (
                PerformancePreset::Ultra,
                "Ultra",
                "Maximal cores & memory limits",
            ),
            (
                PerformancePreset::Custom,
                "Custom",
                "Unlock manual fine-tuning overrides",
            ),
        ];

        for (p, label, tooltip) in presets {
            if ui
                .selectable_value(&mut curr_preset, p, label)
                .on_hover_text(tooltip)
                .clicked()
            {
                settings.perf.apply_preset(p);
            }
        }
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Detailed Controls (Locked unless Custom) ──
    let is_custom =
        settings.perf.preset == crate::infrastructure::settings::PerformancePreset::Custom;
    let perf = &mut settings.perf;

    egui::Grid::new("performance_tuning_grid")
        .num_columns(2)
        .spacing([20.0, 12.0])
        .show(ui, |ui| {
            ui.label(format!("{}:", i18n.perf_threads));
            ui.horizontal(|ui| {
                ui.add_enabled(
                    is_custom,
                    egui::Slider::new(&mut perf.worker_threads, 1..=32).text("Threads"),
                );
                ui.label(
                    egui::RichText::new("Concurrent pipelines")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.perf_gpu));
            ui.add_enabled_ui(is_custom, |ui| {
                use crate::infrastructure::settings::GpuBackend;

                let auto_label = if let Some(gpu) = &hw.primary_gpu {
                    format!("Auto ({})", gpu.name)
                } else {
                    "Auto (CPU)".to_string()
                };
                let sel_label = match perf.gpu_backend {
                    GpuBackend::Auto => auto_label.clone(),
                    GpuBackend::Cpu => "CPU Fallback (Force CPU)".to_string(),
                    GpuBackend::Cuda => "Nvidia CUDA".to_string(),
                    GpuBackend::DirectMl => "DirectML (Windows)".to_string(),
                    GpuBackend::TensorRt => "Nvidia TensorRT".to_string(),
                };

                egui::ComboBox::from_id_salt("gpu_backend_sel")
                    .width(260.0)
                    .selected_text(sel_label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut perf.gpu_backend,
                            GpuBackend::Auto,
                            format!("Auto-Detect [{}]", auto_label),
                        );
                        ui.selectable_value(
                            &mut perf.gpu_backend,
                            GpuBackend::Cpu,
                            "CPU fallback (Force CPU)",
                        );
                        if let Some(gpu) = &hw.primary_gpu {
                            ui.selectable_value(
                                &mut perf.gpu_backend,
                                GpuBackend::DirectMl,
                                format!("DirectML [{}]", gpu.name),
                            );
                            ui.selectable_value(
                                &mut perf.gpu_backend,
                                GpuBackend::Cuda,
                                format!("Nvidia CUDA [{}]", gpu.name),
                            );
                            ui.selectable_value(
                                &mut perf.gpu_backend,
                                GpuBackend::TensorRt,
                                format!("Nvidia TensorRT [{}]", gpu.name),
                            );
                        } else {
                            ui.selectable_value(
                                &mut perf.gpu_backend,
                                GpuBackend::DirectMl,
                                "DirectML (Windows)",
                            );
                            ui.selectable_value(
                                &mut perf.gpu_backend,
                                GpuBackend::Cuda,
                                "Nvidia CUDA",
                            );
                            ui.selectable_value(
                                &mut perf.gpu_backend,
                                GpuBackend::TensorRt,
                                "Nvidia TensorRT",
                            );
                        }
                    });
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.perf_parallel));
            ui.add_enabled_ui(is_custom, |ui| {
                ui.checkbox(&mut perf.parallel_ocr, "Scan multi-regions concurrently");
            });
            ui.end_row();

            ui.label("Batch Translation:");
            ui.add_enabled_ui(is_custom, |ui| {
                ui.checkbox(&mut perf.enable_batching, "Send all OCR lines as one API call (faster)");
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.perf_memory));
            ui.horizontal(|ui| {
                ui.add_enabled(
                    is_custom,
                    egui::Slider::new(&mut perf.memory_cleanup_interval_secs, 10..=3600)
                        .step_by(10.0)
                        .text("Seconds"),
                );
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.perf_cache));
            ui.horizontal(|ui| {
                ui.add_enabled(
                    is_custom,
                    egui::Slider::new(&mut perf.max_cache_entries, 500..=100000)
                        .step_by(500.0)
                        .text("Entries"),
                );
            });
            ui.end_row();


            ui.label("Translation Trigger Delay (Debounce):");
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut settings.realtime.debounce_timeout_ms, 50..=1000).step_by(50.0).text("ms"));
                ui.label(
                    egui::RichText::new("Wait time before translation (lower is faster, higher saves API calls)")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.end_row();

            ui.label("Frame Stability Threshold:");
            ui.horizontal(|ui| {
                ui.add_enabled(
                    is_custom,
                    egui::Slider::new(&mut settings.realtime.stability_threshold_frames, 1..=10)
                        .text("Frames"),
                );
                ui.label(
                    egui::RichText::new("Identical frames required before translating (higher = less jitter for fast-scrolling text)")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.end_row();

            ui.label("VRAM Limit:");
            ui.horizontal(|ui| {
                ui.add_enabled(
                    is_custom,
                    egui::Slider::new(&mut perf.vram_limit_mb, 0..=24576)
                        .step_by(512.0)
                        .text("MB"),
                );
                let tooltip_str = if perf.vram_limit_mb == 0 { "Unlimited" } else { "Hard cap applied to ONNX sessions" };
                ui.label(egui::RichText::new(tooltip_str).small().color(egui::Color32::GRAY));
            });
            ui.end_row();
        });
}
