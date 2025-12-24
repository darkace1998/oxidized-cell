//! Main application

use eframe::egui;
use oc_core::config::Config;
use std::path::PathBuf;

use crate::debugger::DebuggerView;
use crate::game_list::{GameInfo, GameListView};
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
    /// Current theme
    theme: Theme,
    /// Game list view
    game_list: GameListView,
    /// Debugger view
    debugger: DebuggerView,
    /// Settings panel
    settings_panel: SettingsPanel,
    /// Emulation state
    emulation_state: EmulationState,
    /// FPS counter
    fps: f32,
    /// Frame time (ms)
    frame_time: f32,
}

/// Application views
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    GameList,
    Emulation,
    Debugger,
}

/// Emulation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmulationState {
    Stopped,
    Running,
    Paused,
}

impl OxidizedCellApp {
    /// Create a new application
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = Config::load().unwrap_or_default();
        
        let theme = Theme::default();
        theme.apply(&cc.egui_ctx);
        
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
            theme,
            game_list,
            debugger: DebuggerView::new(),
            settings_panel: SettingsPanel::new(),
            emulation_state: EmulationState::Stopped,
            fps: 0.0,
            frame_time: 0.0,
        }
    }
}

impl eframe::App for OxidizedCellApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update FPS (mock for now)
        self.fps = ctx.input(|i| 1.0 / i.stable_dt.max(0.001));
        self.frame_time = 1000.0 / self.fps.max(1.0);
        
        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Game...").clicked() {
                        if let Some(path) = Self::open_game_dialog() {
                            let game_info = Self::create_game_info_from_path(&path);
                            self.game_list.add_game(game_info);
                            tracing::info!("Added game: {}", path.display());
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
                    let can_start = self.emulation_state == EmulationState::Stopped 
                        || self.emulation_state == EmulationState::Paused;
                    let can_pause = self.emulation_state == EmulationState::Running;
                    let can_stop = self.emulation_state != EmulationState::Stopped;
                    
                    if ui.add_enabled(can_start, egui::Button::new("Start")).clicked() {
                        self.emulation_state = EmulationState::Running;
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_pause, egui::Button::new("Pause")).clicked() {
                        self.emulation_state = EmulationState::Paused;
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_stop, egui::Button::new("Stop")).clicked() {
                        self.emulation_state = EmulationState::Stopped;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Reset").clicked() {
                        self.emulation_state = EmulationState::Stopped;
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
                    if ui.checkbox(&mut self.show_performance, "Performance Overlay").clicked() {
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
                let state_text = match self.emulation_state {
                    EmulationState::Stopped => "⏹ Stopped",
                    EmulationState::Running => "▶ Running",
                    EmulationState::Paused => "⏸ Paused",
                };
                ui.label(state_text);
                
                ui.separator();
                
                // FPS
                if self.emulation_state == EmulationState::Running {
                    ui.label(format!("FPS: {:.1}", 60.0)); // Mock FPS
                } else {
                    ui.label("FPS: --");
                }
                
                // Selected game info
                if let Some(game) = self.game_list.selected_game() {
                    ui.separator();
                    ui.label(format!("Selected: {}", game.title));
                }
            });
        });
        
        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_view {
                View::GameList => {
                    if let Some(game_path) = self.game_list.show(ui) {
                        // Launch game
                        tracing::info!("Launching game: {:?}", game_path);
                        self.current_view = View::Emulation;
                        self.emulation_state = EmulationState::Running;
                    }
                }
                View::Emulation => {
                    ui.vertical_centered(|ui| {
                        ui.heading("Emulation View");
                        ui.add_space(20.0);
                        
                        // Game display area
                        let available_size = ui.available_size();
                        let aspect_ratio = 16.0 / 9.0;
                        let (width, height) = if available_size.x / available_size.y > aspect_ratio {
                            (available_size.y * aspect_ratio, available_size.y)
                        } else {
                            (available_size.x, available_size.x / aspect_ratio)
                        };
                        
                        let (rect, _response) = ui.allocate_exact_size(
                            egui::vec2(width, height),
                            egui::Sense::hover()
                        );
                        
                        // Draw placeholder
                        ui.painter().rect_filled(
                            rect,
                            4.0,
                            egui::Color32::from_gray(20),
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Game Display\n(RSX output would render here)",
                            egui::FontId::proportional(24.0),
                            ui.visuals().text_color(),
                        );
                    });
                }
                View::Debugger => {
                    self.debugger.show(ui);
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
                    ui.label(format!("FPS: {:.1}", self.fps));
                    ui.label(format!("Frame Time: {:.2}ms", self.frame_time));
                    ui.label(format!("UI FPS: {:.1}", 1.0 / ctx.input(|i| i.stable_dt).max(0.001)));
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
                                tracing::error!("Failed to save config: {}", e);
                            } else {
                                tracing::info!("Configuration saved");
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
