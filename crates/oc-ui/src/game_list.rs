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
    /// Game category for filtering (e.g., "Action", "RPG", "Sports")
    pub category: String,
    /// Icon data (PNG format)
    pub icon_data: Option<Vec<u8>>,
    /// Last played timestamp (Unix timestamp)
    pub last_played: Option<u64>,
}

/// Display mode for game list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Grid,
    List,
}

/// Filter by game category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryFilter {
    All,
    Action,
    Adventure,
    RPG,
    Sports,
    Racing,
    Shooter,
    Strategy,
    Simulation,
    Puzzle,
    Fighting,
    Other,
}

impl CategoryFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            CategoryFilter::All => "All",
            CategoryFilter::Action => "Action",
            CategoryFilter::Adventure => "Adventure",
            CategoryFilter::RPG => "RPG",
            CategoryFilter::Sports => "Sports",
            CategoryFilter::Racing => "Racing",
            CategoryFilter::Shooter => "Shooter",
            CategoryFilter::Strategy => "Strategy",
            CategoryFilter::Simulation => "Simulation",
            CategoryFilter::Puzzle => "Puzzle",
            CategoryFilter::Fighting => "Fighting",
            CategoryFilter::Other => "Other",
        }
    }
    
    pub fn all() -> Vec<CategoryFilter> {
        vec![
            CategoryFilter::All,
            CategoryFilter::Action,
            CategoryFilter::Adventure,
            CategoryFilter::RPG,
            CategoryFilter::Sports,
            CategoryFilter::Racing,
            CategoryFilter::Shooter,
            CategoryFilter::Strategy,
            CategoryFilter::Simulation,
            CategoryFilter::Puzzle,
            CategoryFilter::Fighting,
            CategoryFilter::Other,
        ]
    }
}

/// Sort order for games
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    TitleAZ,
    TitleZA,
    RecentlyPlayed,
    MostPlayed,
}

impl SortOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortOrder::TitleAZ => "Title (A-Z)",
            SortOrder::TitleZA => "Title (Z-A)",
            SortOrder::RecentlyPlayed => "Recently Played",
            SortOrder::MostPlayed => "Most Played",
        }
    }
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
    /// Category filter
    category_filter: CategoryFilter,
    /// Sort order
    sort_order: SortOrder,
    /// Recent games (up to 10 most recently played)
    recent_games: Vec<String>, // Store game IDs
    /// Loaded icon textures (game_id -> texture_handle)
    icon_textures: std::collections::HashMap<String, egui::TextureHandle>,
}

impl GameListView {
    /// Create a new game list view
    pub fn new() -> Self {
        Self {
            games: Vec::new(),
            search_query: String::new(),
            display_mode: DisplayMode::Grid,
            selected_game: None,
            category_filter: CategoryFilter::All,
            sort_order: SortOrder::TitleAZ,
            recent_games: Vec::new(),
            icon_textures: std::collections::HashMap::new(),
        }
    }

    /// Add a game to the list
    pub fn add_game(&mut self, game: GameInfo) {
        self.games.push(game);
    }
    
    /// Set category filter
    pub fn set_category_filter(&mut self, category: CategoryFilter) {
        self.category_filter = category;
    }
    
    /// Set sort order
    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
    }
    
    /// Mark a game as recently played
    pub fn mark_as_played(&mut self, game_id: &str) {
        // Remove if already in list
        self.recent_games.retain(|id| id != game_id);
        
        // Add to front
        self.recent_games.insert(0, game_id.to_string());
        
        // Keep only last 10
        if self.recent_games.len() > 10 {
            self.recent_games.truncate(10);
        }
        
        // Update last_played timestamp for the game
        if let Some(game) = self.games.iter_mut().find(|g| g.id == game_id) {
            game.last_played = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
        }
    }
    
    /// Get recent games list
    pub fn recent_games(&self) -> Vec<&GameInfo> {
        self.recent_games
            .iter()
            .filter_map(|id| self.games.iter().find(|g| &g.id == id))
            .collect()
    }
    
    /// Load game icon texture from data
    fn load_icon_texture(&mut self, ctx: &egui::Context, game_id: &str, icon_data: &[u8]) -> Option<egui::TextureHandle> {
        // Try to decode PNG
        if let Ok(image) = image::load_from_memory(icon_data) {
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                size,
                pixels.as_slice(),
            );
            
            let texture = ctx.load_texture(
                game_id,
                color_image,
                egui::TextureOptions::LINEAR
            );
            
            Some(texture)
        } else {
            None
        }
    }

    /// Get filtered and sorted games based on search query, category, and sort order
    fn filtered_games(&self) -> Vec<(usize, &GameInfo)> {
        let mut games: Vec<(usize, &GameInfo)> = self.games
            .iter()
            .enumerate()
            .filter(|(_, game)| {
                // Apply search filter
                let search_match = if self.search_query.is_empty() {
                    true
                } else {
                    let query = self.search_query.to_lowercase();
                    game.title.to_lowercase().contains(&query)
                        || game.id.to_lowercase().contains(&query)
                };
                
                // Apply category filter
                let category_match = match self.category_filter {
                    CategoryFilter::All => true,
                    _ => game.category.to_lowercase() == self.category_filter.as_str().to_lowercase(),
                };
                
                search_match && category_match
            })
            .collect();
        
        // Apply sorting
        match self.sort_order {
            SortOrder::TitleAZ => {
                games.sort_by(|a, b| a.1.title.to_lowercase().cmp(&b.1.title.to_lowercase()));
            }
            SortOrder::TitleZA => {
                games.sort_by(|a, b| b.1.title.to_lowercase().cmp(&a.1.title.to_lowercase()));
            }
            SortOrder::RecentlyPlayed => {
                games.sort_by(|a, b| {
                    b.1.last_played.unwrap_or(0).cmp(&a.1.last_played.unwrap_or(0))
                });
            }
            SortOrder::MostPlayed => {
                // For now, same as recently played
                // In future, we'd track play count
                games.sort_by(|a, b| {
                    b.1.last_played.unwrap_or(0).cmp(&a.1.last_played.unwrap_or(0))
                });
            }
        }
        
        games
    }

    /// Show the game list view
    pub fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut game_to_launch = None;

        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search_query);

            ui.separator();
            
            // Category filter
            ui.label("Category:");
            egui::ComboBox::from_id_salt("category_filter")
                .selected_text(self.category_filter.as_str())
                .show_ui(ui, |ui| {
                    for category in CategoryFilter::all() {
                        ui.selectable_value(&mut self.category_filter, category, category.as_str());
                    }
                });
            
            ui.separator();
            
            // Sort order
            ui.label("Sort:");
            egui::ComboBox::from_id_salt("sort_order")
                .selected_text(self.sort_order.as_str())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort_order, SortOrder::TitleAZ, SortOrder::TitleAZ.as_str());
                    ui.selectable_value(&mut self.sort_order, SortOrder::TitleZA, SortOrder::TitleZA.as_str());
                    ui.selectable_value(&mut self.sort_order, SortOrder::RecentlyPlayed, SortOrder::RecentlyPlayed.as_str());
                    ui.selectable_value(&mut self.sort_order, SortOrder::MostPlayed, SortOrder::MostPlayed.as_str());
                });

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
        
        // Show recent games section if we have any
        if !self.recent_games.is_empty() && self.search_query.is_empty() && self.category_filter == CategoryFilter::All {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Recently Played").strong().size(16.0));
                ui.separator();
                
                ui.horizontal(|ui| {
                    // Collect recent games to avoid borrow issues
                    let recent: Vec<GameInfo> = self.recent_games()
                        .into_iter()
                        .cloned()
                        .collect();
                    
                    for game in recent.iter().take(5) {
                        let frame = egui::Frame::none()
                            .fill(ui.visuals().faint_bg_color)
                            .stroke(ui.visuals().window_stroke)
                            .rounding(4.0)
                            .inner_margin(4.0);
                        
                        frame.show(ui, |ui| {
                            ui.set_width(120.0);
                            ui.vertical_centered(|ui| {
                                // Icon or placeholder
                                let icon_size = egui::vec2(100.0, 100.0);
                                let (rect, response) = ui.allocate_exact_size(icon_size, egui::Sense::click());
                                
                                // Try to load texture if we have icon data
                                if let Some(icon_data) = &game.icon_data {
                                    if !self.icon_textures.contains_key(&game.id) {
                                        if let Some(texture) = self.load_icon_texture(ctx, &game.id, icon_data) {
                                            self.icon_textures.insert(game.id.clone(), texture);
                                        }
                                    }
                                    
                                    if let Some(texture) = self.icon_textures.get(&game.id) {
                                        ui.painter().image(
                                            texture.id(),
                                            rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        // Fallback to icon
                                        ui.painter().rect_filled(rect, 4.0, ui.visuals().window_fill);
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "📀",
                                            egui::FontId::proportional(32.0),
                                            ui.visuals().text_color(),
                                        );
                                    }
                                } else {
                                    ui.painter().rect_filled(rect, 4.0, ui.visuals().window_fill);
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "📀",
                                        egui::FontId::proportional(32.0),
                                        ui.visuals().text_color(),
                                    );
                                }
                                
                                if response.clicked() {
                                    game_to_launch = Some(game.path.clone());
                                }
                                
                                ui.label(egui::RichText::new(&game.title)
                                    .size(11.0)
                                    .color(ui.visuals().text_color()));
                            });
                        });
                    }
                });
            });
            
            ui.add_space(8.0);
            ui.separator();
        }

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
                        self.show_grid(ctx, ui, &filtered, &mut game_to_launch);
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
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        games: &[(usize, GameInfo)],
        game_to_launch: &mut Option<PathBuf>,
    ) {
        let item_spacing = ui.spacing().item_spacing;
        let available_width = ui.available_width();
        let card_width = 200.0;
        let card_height = 300.0;
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
                    let clicked = self.show_game_card(ctx, ui, game, selected, card_width, card_height, game_to_launch);
                    
                    if clicked {
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
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        game: &GameInfo,
        selected: bool,
        width: f32,
        height: f32,
        game_to_launch: &mut Option<PathBuf>,
    ) -> bool {
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

        let mut clicked = false;
        
        let response = frame.show(ui, |ui| {
            ui.set_width(width);
            ui.set_height(height);

            ui.vertical_centered(|ui| {
                // Icon
                let icon_size = egui::vec2(width - 16.0, 180.0);
                let (rect, response) = ui.allocate_exact_size(icon_size, egui::Sense::click());
                
                // Try to load texture if we have icon data
                if let Some(icon_data) = &game.icon_data {
                    if !self.icon_textures.contains_key(&game.id) {
                        if let Some(texture) = self.load_icon_texture(ctx, &game.id, icon_data) {
                            self.icon_textures.insert(game.id.clone(), texture);
                        }
                    }
                    
                    if let Some(texture) = self.icon_textures.get(&game.id) {
                        ui.painter().image(
                            texture.id(),
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    } else {
                        // Fallback to placeholder
                        ui.painter().rect_filled(rect, 4.0, ui.visuals().window_fill);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "📀",
                            egui::FontId::proportional(64.0),
                            ui.visuals().text_color(),
                        );
                    }
                } else {
                    // Icon placeholder
                    ui.painter().rect_filled(rect, 4.0, ui.visuals().window_fill);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "📀",
                        egui::FontId::proportional(64.0),
                        ui.visuals().text_color(),
                    );
                }
                
                if response.clicked() {
                    clicked = true;
                }

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
                
                // Category
                if !game.category.is_empty() {
                    ui.label(
                        egui::RichText::new(&game.category)
                            .size(10.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                }

                ui.add_space(4.0);

                if ui.button("Launch").clicked() {
                    *game_to_launch = Some(game.path.clone());
                }
            });
        });
        
        // Check if the frame itself was clicked
        if response.response.clicked() {
            clicked = true;
        }
        
        clicked
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
