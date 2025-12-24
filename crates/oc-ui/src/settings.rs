//! Settings UI

use eframe::egui;
use oc_core::config::*;

/// Settings panel
pub struct SettingsPanel {
    /// Current tab
    current_tab: SettingsTab,
}

/// Settings tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Cpu,
    Gpu,
    Audio,
    Input,
    Paths,
    Debug,
}

impl SettingsPanel {
    /// Create a new settings panel
    pub fn new() -> Self {
        Self {
            current_tab: SettingsTab::General,
        }
    }

    /// Show the settings panel
    pub fn show(&mut self, ui: &mut egui::Ui, config: &mut Config) -> bool {
        let mut should_save = false;

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.current_tab, SettingsTab::General, "General");
            ui.selectable_value(&mut self.current_tab, SettingsTab::Cpu, "CPU");
            ui.selectable_value(&mut self.current_tab, SettingsTab::Gpu, "GPU");
            ui.selectable_value(&mut self.current_tab, SettingsTab::Audio, "Audio");
            ui.selectable_value(&mut self.current_tab, SettingsTab::Input, "Input");
            ui.selectable_value(&mut self.current_tab, SettingsTab::Paths, "Paths");
            ui.selectable_value(&mut self.current_tab, SettingsTab::Debug, "Debug");
        });

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            match self.current_tab {
                SettingsTab::General => {
                    should_save |= self.show_general_settings(ui, &mut config.general);
                }
                SettingsTab::Cpu => {
                    should_save |= self.show_cpu_settings(ui, &mut config.cpu);
                }
                SettingsTab::Gpu => {
                    should_save |= self.show_gpu_settings(ui, &mut config.gpu);
                }
                SettingsTab::Audio => {
                    should_save |= self.show_audio_settings(ui, &mut config.audio);
                }
                SettingsTab::Input => {
                    should_save |= self.show_input_settings(ui, &mut config.input);
                }
                SettingsTab::Paths => {
                    should_save |= self.show_path_settings(ui, &mut config.paths);
                }
                SettingsTab::Debug => {
                    should_save |= self.show_debug_settings(ui, &mut config.debug);
                }
            }
        });

        should_save
    }

    fn show_general_settings(&self, ui: &mut egui::Ui, config: &mut GeneralConfig) -> bool {
        let mut changed = false;

        ui.heading("General Settings");
        ui.add_space(10.0);

        changed |= ui.checkbox(&mut config.start_paused, "Start Paused")
            .on_hover_text("Start emulation in paused state")
            .changed();

        changed |= ui.checkbox(&mut config.confirm_exit, "Confirm Exit")
            .on_hover_text("Show confirmation dialog when closing")
            .changed();

        changed |= ui.checkbox(&mut config.auto_save_state, "Auto Save State")
            .on_hover_text("Automatically save state on exit")
            .changed();

        changed
    }

    fn show_cpu_settings(&self, ui: &mut egui::Ui, config: &mut CpuConfig) -> bool {
        let mut changed = false;

        ui.heading("CPU Settings");
        ui.add_space(10.0);

        ui.label("PPU Decoder:");
        changed |= ui.radio_value(&mut config.ppu_decoder, PpuDecoder::Interpreter, "Interpreter")
            .on_hover_text("Slower but more compatible")
            .changed();
        changed |= ui.radio_value(&mut config.ppu_decoder, PpuDecoder::Recompiler, "Recompiler (JIT)")
            .on_hover_text("Faster but may have compatibility issues")
            .changed();

        ui.add_space(5.0);

        ui.label("SPU Decoder:");
        changed |= ui.radio_value(&mut config.spu_decoder, SpuDecoder::Interpreter, "Interpreter")
            .changed();
        changed |= ui.radio_value(&mut config.spu_decoder, SpuDecoder::Recompiler, "Recompiler (JIT)")
            .changed();

        ui.add_space(10.0);

        ui.label("Thread Configuration:");
        changed |= ui.add(
            egui::Slider::new(&mut config.ppu_threads, 1..=8)
                .text("PPU Threads")
        ).changed();

        changed |= ui.add(
            egui::Slider::new(&mut config.spu_threads, 1..=6)
                .text("SPU Threads")
        ).changed();

        ui.add_space(10.0);

        ui.label("Accuracy Options:");
        changed |= ui.checkbox(&mut config.accurate_dfma, "Accurate DFMA")
            .on_hover_text("Use accurate decimal floating-point multiply-add")
            .changed();

        changed |= ui.checkbox(&mut config.accurate_rsx_reservation, "Accurate RSX Reservation")
            .on_hover_text("Use accurate RSX memory reservation")
            .changed();

        changed |= ui.checkbox(&mut config.spu_loop_detection, "SPU Loop Detection")
            .on_hover_text("Detect and optimize SPU loops")
            .changed();

        changed
    }

    fn show_gpu_settings(&self, ui: &mut egui::Ui, config: &mut GpuConfig) -> bool {
        let mut changed = false;

        ui.heading("GPU Settings");
        ui.add_space(10.0);

        ui.label("Backend:");
        changed |= ui.radio_value(&mut config.backend, GpuBackend::Vulkan, "Vulkan")
            .changed();
        changed |= ui.radio_value(&mut config.backend, GpuBackend::Null, "Null (No rendering)")
            .changed();

        ui.add_space(10.0);

        changed |= ui.add(
            egui::Slider::new(&mut config.resolution_scale, 50..=400)
                .text("Resolution Scale (%)")
        ).changed();

        changed |= ui.add(
            egui::Slider::new(&mut config.anisotropic_filter, 0..=16)
                .text("Anisotropic Filter")
        ).changed();

        changed |= ui.checkbox(&mut config.vsync, "VSync")
            .on_hover_text("Synchronize with display refresh rate")
            .changed();

        changed |= ui.add(
            egui::Slider::new(&mut config.frame_limit, 0..=240)
                .text("Frame Limit (0 = unlimited)")
        ).changed();

        changed |= ui.checkbox(&mut config.shader_cache, "Shader Cache")
            .on_hover_text("Cache compiled shaders to disk")
            .changed();

        ui.add_space(10.0);

        ui.label("Write Buffers (Debug):");
        changed |= ui.checkbox(&mut config.write_color_buffers, "Write Color Buffers")
            .changed();
        changed |= ui.checkbox(&mut config.write_depth_buffer, "Write Depth Buffer")
            .changed();

        changed
    }

    fn show_audio_settings(&self, ui: &mut egui::Ui, config: &mut AudioConfig) -> bool {
        let mut changed = false;

        ui.heading("Audio Settings");
        ui.add_space(10.0);

        changed |= ui.checkbox(&mut config.enable, "Enable Audio")
            .changed();

        ui.label("Backend:");
        changed |= ui.radio_value(&mut config.backend, AudioBackend::Auto, "Auto")
            .changed();
        changed |= ui.radio_value(&mut config.backend, AudioBackend::Null, "Null (No audio)")
            .changed();

        ui.add_space(10.0);

        changed |= ui.add(
            egui::Slider::new(&mut config.volume, 0.0..=2.0)
                .text("Volume")
        ).changed();

        changed |= ui.add(
            egui::Slider::new(&mut config.buffer_duration_ms, 20..=500)
                .text("Buffer Duration (ms)")
        ).changed();

        changed |= ui.checkbox(&mut config.time_stretching, "Time Stretching")
            .on_hover_text("Adjust audio speed to match emulation speed")
            .changed();

        changed
    }

    fn show_input_settings(&self, ui: &mut egui::Ui, config: &mut InputConfig) -> bool {
        let mut changed = false;

        ui.heading("Input Settings");
        ui.add_space(10.0);

        ui.label("Controller Configuration:");
        ui.label("(Controller support coming soon)");

        ui.add_space(10.0);

        ui.label("Keyboard Mapping:");
        
        egui::Grid::new("keyboard_mapping")
            .num_columns(2)
            .spacing([40.0, 8.0])
            .show(ui, |ui| {
                ui.label("Cross:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.cross).changed();
                ui.end_row();

                ui.label("Circle:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.circle).changed();
                ui.end_row();

                ui.label("Square:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.square).changed();
                ui.end_row();

                ui.label("Triangle:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.triangle).changed();
                ui.end_row();

                ui.label("L1:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.l1).changed();
                ui.end_row();

                ui.label("L2:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.l2).changed();
                ui.end_row();

                ui.label("R1:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.r1).changed();
                ui.end_row();

                ui.label("R2:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.r2).changed();
                ui.end_row();

                ui.label("Start:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.start).changed();
                ui.end_row();

                ui.label("Select:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.select).changed();
                ui.end_row();

                ui.label("D-Pad Up:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.dpad_up).changed();
                ui.end_row();

                ui.label("D-Pad Down:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.dpad_down).changed();
                ui.end_row();

                ui.label("D-Pad Left:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.dpad_left).changed();
                ui.end_row();

                ui.label("D-Pad Right:");
                changed |= ui.text_edit_singleline(&mut config.keyboard_mapping.dpad_right).changed();
                ui.end_row();
            });

        changed
    }

    fn show_path_settings(&self, ui: &mut egui::Ui, config: &mut PathConfig) -> bool {
        let mut changed = false;

        ui.heading("Path Configuration");
        ui.add_space(10.0);

        ui.label("Game Directories:");

        changed |= self.show_path_field(ui, "Games:", &mut config.games);
        changed |= self.show_path_field(ui, "dev_hdd0:", &mut config.dev_hdd0);
        changed |= self.show_path_field(ui, "dev_hdd1:", &mut config.dev_hdd1);
        changed |= self.show_path_field(ui, "dev_flash:", &mut config.dev_flash);
        changed |= self.show_path_field(ui, "Save Data:", &mut config.save_data);
        changed |= self.show_path_field(ui, "Shader Cache:", &mut config.shader_cache);

        changed
    }

    fn show_path_field(&self, ui: &mut egui::Ui, label: &str, path: &mut std::path::PathBuf) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(label);
            let mut path_str = path.to_string_lossy().to_string();
            if ui.text_edit_singleline(&mut path_str).changed() {
                *path = std::path::PathBuf::from(path_str);
                changed = true;
            }
        });
        changed
    }

    fn show_debug_settings(&self, ui: &mut egui::Ui, config: &mut DebugConfig) -> bool {
        let mut changed = false;

        ui.heading("Debug Settings");
        ui.add_space(10.0);

        ui.label("Logging:");
        
        let log_levels = [
            (LogLevel::Off, "Off"),
            (LogLevel::Error, "Error"),
            (LogLevel::Warn, "Warn"),
            (LogLevel::Info, "Info"),
            (LogLevel::Debug, "Debug"),
            (LogLevel::Trace, "Trace"),
        ];

        for (level, name) in log_levels {
            changed |= ui.radio_value(&mut config.log_level, level, name).changed();
        }

        ui.add_space(10.0);

        changed |= ui.checkbox(&mut config.log_to_file, "Log to File")
            .changed();

        if config.log_to_file {
            changed |= self.show_path_field(ui, "Log Path:", &mut config.log_path);
        }

        ui.add_space(10.0);

        ui.label("Tracing:");
        changed |= ui.checkbox(&mut config.trace_ppu, "Trace PPU")
            .on_hover_text("Log all PPU instructions (very slow)")
            .changed();

        changed |= ui.checkbox(&mut config.trace_spu, "Trace SPU")
            .on_hover_text("Log all SPU instructions (very slow)")
            .changed();

        changed |= ui.checkbox(&mut config.trace_rsx, "Trace RSX")
            .on_hover_text("Log all RSX commands (very slow)")
            .changed();

        changed |= ui.checkbox(&mut config.dump_shaders, "Dump Shaders")
            .on_hover_text("Save shader source code to disk")
            .changed();

        changed
    }
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self::new()
    }
}
