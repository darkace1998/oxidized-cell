//! Game list view

use eframe::egui;
use std::path::PathBuf;

/// Game metadata
#[derive(Debug, Clone)]
pub struct GameInfo {
    pub title: String,
    pub path: PathBuf,
    pub id: String,
    pub version: String,
    pub region: String,
}

/// Display mode for game list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Grid,
    List,
}

/// Game list view state
pub struct GameListView {
    /// List of games
    games: Vec<GameInfo>,
    /// Search query
    search_query: String,
    /// Display mode
    display_mode: DisplayMode,
    /// Selected game index
    selected_game: Option<usize>,
}

impl GameListView {
    /// Create a new game list view
    pub fn new() -> Self {
        Self {
            games: Vec::new(),
            search_query: String::new(),
            display_mode: DisplayMode::Grid,
            selected_game: None,
        }
    }

    /// Add a game to the list
    pub fn add_game(&mut self, game: GameInfo) {
        self.games.push(game);
    }

    /// Get filtered games based on search query
    fn filtered_games(&self) -> Vec<(usize, &GameInfo)> {
        if self.search_query.is_empty() {
            self.games.iter().enumerate().collect()
        } else {
            let query = self.search_query.to_lowercase();
            self.games
                .iter()
                .enumerate()
                .filter(|(_, game)| {
                    game.title.to_lowercase().contains(&query)
                        || game.id.to_lowercase().contains(&query)
                })
                .collect()
        }
    }

    /// Show the game list view
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut game_to_launch = None;

        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search_query);

            ui.separator();

            if ui
                .selectable_label(self.display_mode == DisplayMode::Grid, "Grid")
                .clicked()
            {
                self.display_mode = DisplayMode::Grid;
            }
            if ui
                .selectable_label(self.display_mode == DisplayMode::List, "List")
                .clicked()
            {
                self.display_mode = DisplayMode::List;
            }

            ui.separator();
            ui.label(format!("{} games", self.games.len()));
        });

        ui.separator();

        // Game display
        // Collect the filtered games into a Vec to avoid borrow issues
        let filtered: Vec<(usize, GameInfo)> = self.filtered_games()
            .into_iter()
            .map(|(idx, game)| (idx, game.clone()))
            .collect();

        if filtered.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                if self.games.is_empty() {
                    ui.label("No games found.");
                    ui.label("Use File > Open Game to add games.");
                } else {
                    ui.label("No games match your search.");
                }
            });
        } else {
            let display_mode = self.display_mode;
            egui::ScrollArea::vertical().show(ui, |ui| {
                match display_mode {
                    DisplayMode::Grid => {
                        self.show_grid(ui, &filtered, &mut game_to_launch);
                    }
                    DisplayMode::List => {
                        self.show_list(ui, &filtered, &mut game_to_launch);
                    }
                }
            });
        }

        game_to_launch
    }

    /// Show games in grid mode
    fn show_grid(
        &mut self,
        ui: &mut egui::Ui,
        games: &[(usize, GameInfo)],
        game_to_launch: &mut Option<PathBuf>,
    ) {
        let item_spacing = ui.spacing().item_spacing;
        let available_width = ui.available_width();
        let card_width = 200.0;
        let card_height = 280.0;
        let columns = ((available_width + item_spacing.x) / (card_width + item_spacing.x))
            .floor()
            .max(1.0) as usize;

        egui::Grid::new("game_grid")
            .spacing([item_spacing.x, item_spacing.y])
            .show(ui, |ui| {
                for (i, (idx, game)) in games.iter().enumerate() {
                    if i % columns == 0 && i > 0 {
                        ui.end_row();
                    }

                    let selected = self.selected_game == Some(*idx);
                    self.show_game_card(ui, game, selected, card_width, card_height, game_to_launch);
                    
                    if ui.response().clicked() {
                        self.selected_game = Some(*idx);
                    }
                }
            });
    }

    /// Show games in list mode
    fn show_list(
        &mut self,
        ui: &mut egui::Ui,
        games: &[(usize, GameInfo)],
        game_to_launch: &mut Option<PathBuf>,
    ) {
        egui::Grid::new("game_list")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.strong("Title");
                ui.strong("ID");
                ui.strong("Version");
                ui.strong("Region");
                ui.strong("Actions");
                ui.end_row();

                for (idx, game) in games {
                    let selected = self.selected_game == Some(*idx);

                    if ui
                        .selectable_label(selected, &game.title)
                        .clicked()
                    {
                        self.selected_game = Some(*idx);
                    }
                    ui.label(&game.id);
                    ui.label(&game.version);
                    ui.label(&game.region);

                    if ui.button("Launch").clicked() {
                        *game_to_launch = Some(game.path.clone());
                    }

                    ui.end_row();
                }
            });
    }

    /// Show a game card (for grid mode)
    fn show_game_card(
        &self,
        ui: &mut egui::Ui,
        game: &GameInfo,
        selected: bool,
        width: f32,
        height: f32,
        game_to_launch: &mut Option<PathBuf>,
    ) {
        let frame = if selected {
            egui::Frame::none()
                .fill(ui.visuals().selection.bg_fill)
                .stroke(ui.visuals().selection.stroke)
                .rounding(4.0)
                .inner_margin(8.0)
        } else {
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .stroke(ui.visuals().window_stroke)
                .rounding(4.0)
                .inner_margin(8.0)
        };

        frame.show(ui, |ui| {
            ui.set_width(width);
            ui.set_height(height);

            ui.vertical_centered(|ui| {
                // Icon placeholder
                let icon_size = egui::vec2(width - 16.0, 180.0);
                let (rect, _response) = ui.allocate_exact_size(icon_size, egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    4.0,
                    ui.visuals().window_fill,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📀",
                    egui::FontId::proportional(64.0),
                    ui.visuals().text_color(),
                );

                ui.add_space(8.0);

                // Title
                ui.label(
                    egui::RichText::new(&game.title)
                        .strong()
                        .size(14.0),
                );

                // ID and Version
                ui.label(
                    egui::RichText::new(format!("{} v{}", game.id, game.version))
                        .size(11.0)
                        .color(ui.visuals().weak_text_color()),
                );

                ui.add_space(4.0);

                if ui.button("Launch").clicked() {
                    *game_to_launch = Some(game.path.clone());
                }
            });
        });
    }

    /// Get the currently selected game
    pub fn selected_game(&self) -> Option<&GameInfo> {
        self.selected_game.and_then(|idx| self.games.get(idx))
    }
}

impl Default for GameListView {
    fn default() -> Self {
        Self::new()
    }
}
