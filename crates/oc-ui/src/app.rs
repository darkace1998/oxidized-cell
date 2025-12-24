//! Main application

use eframe::egui;
use oc_core::config::Config;
use oc_integration::{EmulatorRunner, RunnerState};
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::debugger::DebuggerView;
use crate::game_list::{GameInfo, GameListView};
use crate::log_viewer::{LogViewer, LogLevel};
use crate::memory_viewer::MemoryViewer;
use crate::settings::SettingsPanel;
use crate::themes::Theme;

/// Main application state
pub struct OxidizedCellApp {
    /// Configuration
    config: Config,
    /// Current view
    current_view: View,
    /// Show settings window
    show_settings: bool,
    /// Show about window
    show_about: bool,
    /// Show performance overlay
    show_performance: bool,
    /// Show log viewer window
    show_log_viewer: bool,
    /// Show memory viewer window
    show_memory_viewer: bool,
    /// Current theme
    theme: Theme,
    /// Game list view
    game_list: GameListView,
    /// Debugger view
    debugger: DebuggerView,
    /// Settings panel
    settings_panel: SettingsPanel,
    /// Log viewer panel
    log_viewer: LogViewer,
    /// Memory viewer panel
    memory_viewer: MemoryViewer,
    /// Emulator runner (wrapped in Arc<RwLock> for thread safety)
    emulator: Option<Arc<RwLock<EmulatorRunner>>>,
    /// Currently loaded game path
    loaded_game_path: Option<PathBuf>,
    /// FPS counter
    fps: f32,
    /// Frame time (ms)
    frame_time: f32,
    /// Emulator FPS (from runner)
    emulator_fps: f64,
    /// Error message to display
    error_message: Option<String>,
}

/// Application views
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    GameList,
    Emulation,
    Debugger,
    LogViewer,
    MemoryViewer,
}

impl OxidizedCellApp {
    /// Create a new application
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = Config::load().unwrap_or_default();
        
        let theme = Theme::default();
        theme.apply(&cc.egui_ctx);
        
        // Create log viewer and log initial message
        let log_viewer = LogViewer::new();
        log_viewer.log(LogLevel::Info, "oc-ui", "oxidized-cell UI initialized");
        
        // Add some sample games for demonstration
        let mut game_list = GameListView::new();
        game_list.add_game(GameInfo {
            title: "Sample Game 1".to_string(),
            path: "/path/to/game1.elf".into(),
            id: "BLUS00001".to_string(),
            version: "1.00".to_string(),
            region: "US".to_string(),
        });
        game_list.add_game(GameInfo {
            title: "Sample Game 2".to_string(),
            path: "/path/to/game2.elf".into(),
            id: "BLES00002".to_string(),
            version: "1.01".to_string(),
            region: "EU".to_string(),
        });
        
        Self {
            config,
            current_view: View::GameList,
            show_settings: false,
            show_about: false,
            show_performance: false,
            show_log_viewer: false,
            show_memory_viewer: false,
            theme,
            game_list,
            debugger: DebuggerView::new(),
            settings_panel: SettingsPanel::new(),
            log_viewer,
            memory_viewer: MemoryViewer::new(),
            emulator: None,
            loaded_game_path: None,
            fps: 0.0,
            frame_time: 0.0,
            emulator_fps: 0.0,
            error_message: None,
        }
    }

    /// Get the current emulation state from the runner
    fn emulation_state(&self) -> RunnerState {
        self.emulator
            .as_ref()
            .map(|e| e.read().state())
            .unwrap_or(RunnerState::Stopped)
    }

    /// Initialize the emulator runner
    fn init_emulator(&mut self) {
        if self.emulator.is_some() {
            return;
        }

        self.log_viewer.log(LogLevel::Info, "oc-ui", "Initializing emulator runner...");
        
        match EmulatorRunner::new(self.config.clone()) {
            Ok(runner) => {
                let runner = Arc::new(RwLock::new(runner));
                
                // Connect memory viewer to the emulator's memory
                self.memory_viewer.connect(Arc::clone(runner.read().memory()));
                
                self.emulator = Some(runner);
                self.log_viewer.log(LogLevel::Info, "oc-ui", "Emulator runner initialized successfully");
            }
            Err(e) => {
                let msg = format!("Failed to initialize emulator: {}", e);
                self.log_viewer.log(LogLevel::Error, "oc-ui", &msg);
                self.error_message = Some(msg);
            }
        }
    }

    /// Launch a game from the given path
    fn launch_game(&mut self, game_path: PathBuf) {
        self.log_viewer.log(LogLevel::Info, "oc-ui", &format!("Launching game: {:?}", game_path));
        
        // Initialize emulator if not already done
        self.init_emulator();
        
        if let Some(ref emulator) = self.emulator {
            // Load the game
            match emulator.read().load_game(&game_path) {
                Ok(loaded_game) => {
                    self.log_viewer.log(
                        LogLevel::Info, 
                        "oc-ui", 
                        &format!("Game loaded: entry=0x{:x}, base=0x{:08x}", 
                            loaded_game.entry_point, 
                            loaded_game.base_addr)
                    );
                    
                    // Start the emulator
                    if let Err(e) = emulator.write().start() {
                        let msg = format!("Failed to start emulator: {}", e);
                        self.log_viewer.log(LogLevel::Error, "oc-ui", &msg);
                        self.error_message = Some(msg);
                    } else {
                        self.loaded_game_path = Some(game_path);
                        self.current_view = View::Emulation;
                        self.log_viewer.log(LogLevel::Info, "oc-ui", "Emulator started");
                    }
                }
                Err(e) => {
                    let msg = format!("Failed to load game: {}", e);
                    self.log_viewer.log(LogLevel::Error, "oc-ui", &msg);
                    self.error_message = Some(msg);
                }
            }
        }
    }

    /// Start/Resume emulation
    fn start_emulation(&mut self) {
        if let Some(ref emulator) = self.emulator {
            let state = emulator.read().state();
            let result = match state {
                RunnerState::Paused => emulator.write().resume(),
                RunnerState::Stopped => emulator.write().start(),
                RunnerState::Running => Ok(()),
            };
            
            if let Err(e) = result {
                let msg = format!("Failed to start emulation: {}", e);
                self.log_viewer.log(LogLevel::Error, "oc-ui", &msg);
            } else {
                self.log_viewer.log(LogLevel::Info, "oc-ui", "Emulation started/resumed");
            }
        }
    }

    /// Pause emulation
    fn pause_emulation(&mut self) {
        if let Some(ref emulator) = self.emulator {
            if let Err(e) = emulator.write().pause() {
                let msg = format!("Failed to pause emulation: {}", e);
                self.log_viewer.log(LogLevel::Error, "oc-ui", &msg);
            } else {
                self.log_viewer.log(LogLevel::Info, "oc-ui", "Emulation paused");
            }
        }
    }

    /// Stop emulation
    fn stop_emulation(&mut self) {
        if let Some(ref emulator) = self.emulator {
            if let Err(e) = emulator.write().stop() {
                let msg = format!("Failed to stop emulation: {}", e);
                self.log_viewer.log(LogLevel::Error, "oc-ui", &msg);
            } else {
                self.log_viewer.log(LogLevel::Info, "oc-ui", "Emulation stopped");
                self.loaded_game_path = None;
            }
        }
    }

    /// Run one emulator frame (called when running)
    fn run_emulator_frame(&mut self) {
        if let Some(ref emulator) = self.emulator {
            if emulator.read().state() == RunnerState::Running {
                if let Err(e) = emulator.write().run_frame() {
                    let msg = format!("Emulator frame error: {}", e);
                    self.log_viewer.log(LogLevel::Error, "oc-ui", &msg);
                }
                self.emulator_fps = emulator.read().fps();
            }
        }
    }
}

impl eframe::App for OxidizedCellApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update FPS
        self.fps = ctx.input(|i| 1.0 / i.stable_dt.max(0.001));
        self.frame_time = 1000.0 / self.fps.max(1.0);

        // Run emulator frame if running
        self.run_emulator_frame();

        // Get current emulation state
        let emulation_state = self.emulation_state();
        
        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Game...").clicked() {
                        if let Some(path) = Self::open_game_dialog() {
                            let game_info = Self::create_game_info_from_path(&path);
                            self.game_list.add_game(game_info);
                            self.log_viewer.log(LogLevel::Info, "oc-ui", &format!("Added game: {}", path.display()));
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        if !self.config.general.confirm_exit
                            || self.show_exit_confirmation(ui)
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
                
                ui.menu_button("Emulation", |ui| {
                    let can_start = emulation_state == RunnerState::Stopped 
                        || emulation_state == RunnerState::Paused;
                    let can_pause = emulation_state == RunnerState::Running;
                    let can_stop = emulation_state != RunnerState::Stopped;
                    
                    if ui.add_enabled(can_start, egui::Button::new("Start")).clicked() {
                        self.start_emulation();
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_pause, egui::Button::new("Pause")).clicked() {
                        self.pause_emulation();
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_stop, egui::Button::new("Stop")).clicked() {
                        self.stop_emulation();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Reset").clicked() {
                        self.stop_emulation();
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("View", |ui| {
                    if ui.selectable_label(
                        self.current_view == View::GameList,
                        "Game List"
                    ).clicked() {
                        self.current_view = View::GameList;
                        ui.close_menu();
                    }
                    if ui.selectable_label(
                        self.current_view == View::Emulation,
                        "Emulation"
                    ).clicked() {
                        self.current_view = View::Emulation;
                        ui.close_menu();
                    }
                    if ui.selectable_label(
                        self.current_view == View::Debugger,
                        "Debugger"
                    ).clicked() {
                        self.current_view = View::Debugger;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.selectable_label(
                        self.current_view == View::LogViewer,
                        "Log Viewer"
                    ).clicked() {
                        self.current_view = View::LogViewer;
                        ui.close_menu();
                    }
                    if ui.selectable_label(
                        self.current_view == View::MemoryViewer,
                        "Memory Viewer"
                    ).clicked() {
                        // Initialize emulator if needed for memory viewer
                        self.init_emulator();
                        self.current_view = View::MemoryViewer;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.checkbox(&mut self.show_performance, "Performance Overlay").clicked() {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.show_log_viewer, "Log Window").clicked() {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.show_memory_viewer, "Memory Window").clicked() {
                        // Initialize emulator if needed
                        if self.show_memory_viewer {
                            self.init_emulator();
                        }
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("Settings", |ui| {
                    if ui.button("Configuration...").clicked() {
                        self.show_settings = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.label("Theme:");
                    for theme in Theme::all() {
                        if ui.selectable_label(self.theme == *theme, theme.name()).clicked() {
                            self.theme = *theme;
                            self.theme.apply(ctx);
                            ui.close_menu();
                        }
                    }
                });
                
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });
        
        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Emulation state
                let state_text = match emulation_state {
                    RunnerState::Stopped => "⏹ Stopped",
                    RunnerState::Running => "▶ Running",
                    RunnerState::Paused => "⏸ Paused",
                };
                ui.label(state_text);
                
                ui.separator();
                
                // FPS
                if emulation_state == RunnerState::Running {
                    ui.label(format!("FPS: {:.1}", self.emulator_fps));
                } else {
                    ui.label("FPS: --");
                }

                // Thread counts
                if let Some(ref emulator) = self.emulator {
                    let runner = emulator.read();
                    ui.separator();
                    ui.label(format!("PPU: {} | SPU: {}", runner.ppu_thread_count(), runner.spu_thread_count()));
                }
                
                // Selected game info
                if let Some(game) = self.game_list.selected_game() {
                    ui.separator();
                    ui.label(format!("Selected: {}", game.title));
                }
                
                // Loaded game info
                if let Some(ref path) = self.loaded_game_path {
                    ui.separator();
                    ui.label(format!("Loaded: {}", path.file_name().unwrap_or_default().to_string_lossy()));
                }
            });
        });
        
        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_view {
                View::GameList => {
                    if let Some(game_path) = self.game_list.show(ui) {
                        // Launch game using the emulator runner
                        self.launch_game(game_path);
                    }
                }
                View::Emulation => {
                    self.show_emulation_view(ui, emulation_state);
                }
                View::Debugger => {
                    self.debugger.show(ui);
                }
                View::LogViewer => {
                    ui.heading("Log Viewer");
                    ui.separator();
                    self.log_viewer.show(ui);
                }
                View::MemoryViewer => {
                    ui.heading("Memory Viewer");
                    ui.separator();
                    self.memory_viewer.show(ui);
                }
            }
        });
        
        // Performance overlay
        if self.show_performance {
            egui::Window::new("Performance")
                .default_pos([10.0, 40.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("UI FPS: {:.1}", self.fps));
                    ui.label(format!("Frame Time: {:.2}ms", self.frame_time));
                    if emulation_state == RunnerState::Running {
                        ui.label(format!("Emulator FPS: {:.1}", self.emulator_fps));
                    }
                    if let Some(ref emulator) = self.emulator {
                        let runner = emulator.read();
                        ui.separator();
                        ui.label(format!("Frame Count: {}", runner.frame_count()));
                        ui.label(format!("Total Cycles: {}", runner.total_cycles()));
                    }
                });
        }

        // Log viewer window (floating)
        if self.show_log_viewer {
            egui::Window::new("Logs")
                .open(&mut self.show_log_viewer)
                .default_size([600.0, 400.0])
                .show(ctx, |ui| {
                    self.log_viewer.show(ui);
                });
        }

        // Memory viewer window (floating)
        if self.show_memory_viewer {
            egui::Window::new("Memory")
                .open(&mut self.show_memory_viewer)
                .default_size([700.0, 500.0])
                .show(ctx, |ui| {
                    self.memory_viewer.show(ui);
                });
        }
        
        // Settings window
        if self.show_settings {
            let mut close_requested = false;
            egui::Window::new("Settings")
                .open(&mut self.show_settings)
                .default_width(600.0)
                .default_height(500.0)
                .show(ctx, |ui| {
                    if self.settings_panel.show(ui, &mut self.config) {
                        // Auto-save on change
                        let _ = self.config.save();
                    }
                    
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            if let Err(e) = self.config.save() {
                                self.log_viewer.log(LogLevel::Error, "oc-ui", &format!("Failed to save config: {}", e));
                            } else {
                                self.log_viewer.log(LogLevel::Info, "oc-ui", "Configuration saved");
                            }
                        }
                        if ui.button("Close").clicked() {
                            close_requested = true;
                        }
                    });
                });
            if close_requested {
                self.show_settings = false;
            }
        }
        
        // About window
        if self.show_about {
            egui::Window::new("About")
                .open(&mut self.show_about)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("oxidized-cell");
                        ui.label("PlayStation 3 Emulator");
                        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.label("A Rust/C++ hybrid PS3 emulator");
                        ui.label("implementing full system emulation.");
                        ui.add_space(10.0);
                        ui.label("Licensed under GPL-3.0");
                        ui.add_space(5.0);
                        ui.hyperlink_to(
                            "GitHub Repository",
                            "https://github.com/darkace1998/oxidized-cell"
                        );
                    });
                });
        }

        // Error dialog
        if let Some(ref error) = self.error_message.clone() {
            let mut show_error = true;
            egui::Window::new("Error")
                .open(&mut show_error)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.colored_label(egui::Color32::RED, "⚠ Error");
                    ui.separator();
                    ui.label(error);
                    ui.separator();
                    if ui.button("OK").clicked() {
                        self.error_message = None;
                    }
                });
            if !show_error {
                self.error_message = None;
            }
        }

        // Request repaint if emulator is running
        if emulation_state == RunnerState::Running {
            ctx.request_repaint();
        }
    }
    
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // Save config on app exit
        let _ = self.config.save();
    }
}

impl OxidizedCellApp {
    fn show_exit_confirmation(&self, _ui: &mut egui::Ui) -> bool {
        // For now just return true, in a real implementation
        // this would show a modal dialog
        true
    }

    /// Show the emulation view with RSX output area
    fn show_emulation_view(&mut self, ui: &mut egui::Ui, emulation_state: RunnerState) {
        ui.vertical_centered(|ui| {
            // Control buttons
            ui.horizontal(|ui| {
                let can_start = emulation_state == RunnerState::Stopped 
                    || emulation_state == RunnerState::Paused;
                let can_pause = emulation_state == RunnerState::Running;
                let can_stop = emulation_state != RunnerState::Stopped;

                if ui.add_enabled(can_start, egui::Button::new("▶ Start")).clicked() {
                    self.start_emulation();
                }
                if ui.add_enabled(can_pause, egui::Button::new("⏸ Pause")).clicked() {
                    self.pause_emulation();
                }
                if ui.add_enabled(can_stop, egui::Button::new("⏹ Stop")).clicked() {
                    self.stop_emulation();
                }

                ui.separator();

                // State indicator
                let state_color = match emulation_state {
                    RunnerState::Stopped => egui::Color32::GRAY,
                    RunnerState::Running => egui::Color32::GREEN,
                    RunnerState::Paused => egui::Color32::YELLOW,
                };
                ui.colored_label(state_color, format!("● {:?}", emulation_state));
            });

            ui.add_space(10.0);
            
            // Game display area
            let available_size = ui.available_size();
            let aspect_ratio = 16.0 / 9.0;
            let (width, height) = if available_size.x / available_size.y > aspect_ratio {
                ((available_size.y - 20.0) * aspect_ratio, available_size.y - 20.0)
            } else {
                (available_size.x - 20.0, (available_size.x - 20.0) / aspect_ratio)
            };
            
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(width, height),
                egui::Sense::hover()
            );
            
            // Draw RSX output area (placeholder with state information)
            ui.painter().rect_filled(
                rect,
                4.0,
                egui::Color32::from_gray(20),
            );

            // Draw frame border
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(2.0, egui::Color32::from_gray(60)),
            );

            // Display status text based on emulation state
            let display_text = match emulation_state {
                RunnerState::Stopped => {
                    if self.loaded_game_path.is_some() {
                        "Game Stopped\nPress Start to resume"
                    } else {
                        "No Game Loaded\nSelect a game from the Game List"
                    }
                }
                RunnerState::Running => {
                    "RSX Output\n(Rendering connected to RSX backend)"
                }
                RunnerState::Paused => {
                    "Paused\nPress Start to resume"
                }
            };

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                display_text,
                egui::FontId::proportional(18.0),
                ui.visuals().text_color(),
            );

            // Show emulator stats when running or paused
            if emulation_state != RunnerState::Stopped {
                if let Some(ref emulator) = self.emulator {
                    let runner = emulator.read();
                    let stats_text = format!(
                        "Frame: {} | Cycles: {} | FPS: {:.1}",
                        runner.frame_count(),
                        runner.total_cycles(),
                        self.emulator_fps
                    );
                    
                    ui.painter().text(
                        egui::pos2(rect.center().x, rect.max.y - 20.0),
                        egui::Align2::CENTER_CENTER,
                        stats_text,
                        egui::FontId::monospace(12.0),
                        egui::Color32::LIGHT_GRAY,
                    );
                }
            }
        });
    }

    /// Open a file dialog to select a game file
    fn open_game_dialog() -> Option<PathBuf> {
        let file = rfd::FileDialog::new()
            .set_title("Open PS3 Game")
            .add_filter("PS3 Executables", &["elf", "self", "bin", "iso"])
            .add_filter("All Files", &["*"])
            .pick_file();
        
        file
    }

    /// Create GameInfo from a file path
    fn create_game_info_from_path(path: &PathBuf) -> GameInfo {
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Game")
            .to_string();
        
        // Try to extract game ID from path (e.g., BLUS12345)
        let id = path
            .to_str()
            .and_then(|s| {
                // Look for PS3 game ID pattern
                let patterns = ["BLUS", "BLES", "BLJM", "BCUS", "BCES", "BCJS"];
                for pattern in patterns {
                    if let Some(pos) = s.find(pattern) {
                        let id_part = &s[pos..];
                        if id_part.len() >= 9 {
                            return Some(id_part[..9].to_string());
                        }
                    }
                }
                None
            })
            .unwrap_or_else(|| "UNKNOWN".to_string());

        // Determine region from ID prefix
        let region = match id.get(0..2) {
            Some("BL") => "US",  // BLUS
            Some("BE") => "EU",  // BLES
            Some("BJ") | Some("BC") => match id.get(2..3) {
                Some("U") => "US",
                Some("E") => "EU", 
                Some("J") => "JP",
                _ => "Unknown",
            },
            _ => "Unknown",
        }.to_string();

        GameInfo {
            title: file_name,
            path: path.clone(),
            id,
            version: "1.00".to_string(),
            region,
        }
    }
}

/// Run the application
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "oxidized-cell",
        options,
        Box::new(|cc| Ok(Box::new(OxidizedCellApp::new(cc)))),
    )
}
