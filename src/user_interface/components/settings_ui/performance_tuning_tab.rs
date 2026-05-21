use crate::infrastructure::settings::Settings;
use eframe::egui;

pub fn render_tab_performance(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &crate::user_interface::i18n::I18n,
) {
    super::section_header(ui, i18n.tab_performance);
    ui.label(egui::RichText::new("Fine-tune thread execution, hardware acceleration, and cache footprints for maximal frame stability.").small().color(egui::Color32::GRAY));
    ui.add_space(12.0);

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
                egui::ComboBox::from_id_salt("gpu_backend_sel")
                    .selected_text(format!("{:?}", perf.gpu_backend))
                    .show_ui(ui, |ui| {
                        use crate::infrastructure::settings::GpuBackend;
                        ui.selectable_value(&mut perf.gpu_backend, GpuBackend::Auto, "Auto-Detect");
                        ui.selectable_value(&mut perf.gpu_backend, GpuBackend::Cpu, "CPU fallback");
                        ui.selectable_value(&mut perf.gpu_backend, GpuBackend::Cuda, "Nvidia CUDA");
                        ui.selectable_value(
                            &mut perf.gpu_backend,
                            GpuBackend::DirectMl,
                            "DirectML (Windows)",
                        );
                        ui.selectable_value(
                            &mut perf.gpu_backend,
                            GpuBackend::TensorRt,
                            "Nvidia TensorRT",
                        );
                    });
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.perf_parallel));
            ui.add_enabled_ui(is_custom, |ui| {
                ui.checkbox(&mut perf.parallel_ocr, "Scan multi-regions concurrently");
            });
            ui.end_row();

            ui.label(format!("{}:", i18n.perf_batching));
            ui.add_enabled_ui(is_custom, |ui| {
                ui.checkbox(
                    &mut perf.enable_batching,
                    "Batch short strings into single API requests",
                );
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

            ui.label(format!("{}:", i18n.perf_vram));
            ui.horizontal(|ui| {
                ui.add_enabled(
                    is_custom,
                    egui::Slider::new(&mut perf.vram_limit_mb, 0..=24576)
                        .step_by(512.0)
                        .text("MB"),
                );
                let tooltip_str = if perf.vram_limit_mb == 0 {
                    "Unlimited"
                } else {
                    "Hard cap"
                };
                ui.label(
                    egui::RichText::new(tooltip_str)
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.end_row();
        });
}
