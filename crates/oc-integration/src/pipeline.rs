//! Game Loading Pipeline
//!
//! This module provides game discovery, scanning, and initialization
//! functionality for the oxidized-cell PS3 emulator.

use oc_core::error::{EmulatorError, LoaderError};
use oc_core::Result;
use oc_hle::ModuleRegistry;
use oc_memory::MemoryManager;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Process Data Area (PDA) base address - early in main memory, after null page
const PDA_BASE_ADDRESS: u32 = 0x0001_0000;
/// Offset for process ID in PDA
const PDA_PROCESS_ID_OFFSET: u32 = 0;
/// Offset for thread ID in PDA
const PDA_THREAD_ID_OFFSET: u32 = 4;

/// Game information extracted from PARAM.SFO
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GameInfo {
    /// Game title
    pub title: String,
    /// Title ID (e.g., "BLUS00001")
    pub title_id: String,
    /// Game version
    pub version: String,
    /// Path to the game directory or EBOOT.BIN
    pub path: PathBuf,
    /// Category (e.g., "DG" for disc game, "HG" for HDD game)
    pub category: String,
    /// Parental level
    pub parental_level: u32,
    /// Resolution (e.g., 1=480p, 2=720p, 4=1080p)
    pub resolution: u32,
    /// Sound format
    pub sound_format: u32,
    /// Icon0 image data (PNG format)
    pub icon0_data: Option<Vec<u8>>,
    /// PIC1 background image data (PNG format)
    pub pic1_data: Option<Vec<u8>>,
}

/// Game scanner for discovering PS3 games
pub struct GameScanner {
    /// Search directories
    search_dirs: Vec<PathBuf>,
    /// Discovered games
    games: HashMap<String, GameInfo>,
    /// Cache file path
    cache_path: Option<PathBuf>,
}

impl GameScanner {
    /// Create a new game scanner
    pub fn new() -> Self {
        Self {
            search_dirs: Vec::new(),
            games: HashMap::new(),
            cache_path: None,
        }
    }

    /// Set cache file path for storing game database
    pub fn set_cache_path<P: AsRef<Path>>(&mut self, path: P) {
        self.cache_path = Some(path.as_ref().to_path_buf());
    }

    /// Load games from cache file
    pub fn load_cache(&mut self) -> Result<()> {
        let cache_path = match &self.cache_path {
            Some(p) => p,
            None => return Ok(()), // No cache configured
        };

        if !cache_path.exists() {
            debug!("Game cache file does not exist: {:?}", cache_path);
            return Ok(());
        }

        info!("Loading game cache from {:?}", cache_path);

        let file = File::open(cache_path).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to open cache file: {}",
                e
            )))
        })?;

        let reader = BufReader::new(file);
        let cached_games: Vec<GameInfo> = serde_json::from_reader(reader).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to parse cache file: {}",
                e
            )))
        })?;

        for game in cached_games {
            self.games.insert(game.title_id.clone(), game);
        }

        info!("Loaded {} games from cache", self.games.len());
        Ok(())
    }

    /// Save games to cache file
    pub fn save_cache(&self) -> Result<()> {
        let cache_path = match &self.cache_path {
            Some(p) => p,
            None => return Ok(()), // No cache configured
        };

        info!("Saving game cache to {:?}", cache_path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EmulatorError::Loader(LoaderError::InvalidElf(format!(
                    "Failed to create cache directory: {}",
                    e
                )))
            })?;
        }

        let file = File::create(cache_path).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to create cache file: {}",
                e
            )))
        })?;

        let games: Vec<&GameInfo> = self.games.values().collect();
        serde_json::to_writer_pretty(file, &games).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to write cache file: {}",
                e
            )))
        })?;

        info!("Saved {} games to cache", self.games.len());
        Ok(())
    }

    /// Add a directory to search for games
    pub fn add_search_directory<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref().to_path_buf();
        if !self.search_dirs.contains(&path) {
            self.search_dirs.push(path);
        }
    }

    /// Scan all search directories for games
    pub fn scan(&mut self) -> Result<Vec<GameInfo>> {
        info!("Scanning {} directories for games", self.search_dirs.len());
        self.games.clear();

        for dir in self.search_dirs.clone() {
            if let Err(e) = self.scan_directory(&dir) {
                warn!("Failed to scan directory {:?}: {}", dir, e);
            }
        }

        info!("Found {} games", self.games.len());
        Ok(self.games.values().cloned().collect())
    }

    /// Scan a single directory for games
    fn scan_directory(&mut self, dir: &Path) -> Result<()> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        debug!("Scanning directory: {:?}", dir);

        // Check if this directory is a PS3 game directory itself
        if self.is_game_directory(dir) {
            self.try_add_game(dir)?;
            return Ok(());
        }

        // Scan subdirectories
        let entries = fs::read_dir(dir).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to read directory: {}",
                e
            )))
        })?;

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    if self.is_game_directory(&path) {
                        self.try_add_game(&path)?;
                    } else {
                        // Recursively scan subdirectories (one level deep)
                        self.scan_subdirectory_for_games(&path)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Scan a subdirectory for game directories (one level deep)
    fn scan_subdirectory_for_games(&mut self, dir: &Path) -> Result<()> {
        if let Ok(sub_entries) = fs::read_dir(dir) {
            for sub_entry in sub_entries.flatten() {
                let sub_path = sub_entry.path();
                if sub_path.is_dir() && self.is_game_directory(&sub_path) {
                    self.try_add_game(&sub_path)?;
                }
            }
        }
        Ok(())
    }

    /// Try to extract game info and add it to the games map
    fn try_add_game(&mut self, dir: &Path) -> Result<()> {
        if let Some(game_info) = self.extract_game_info(dir)? {
            self.games.insert(game_info.title_id.clone(), game_info);
        }
        Ok(())
    }

    /// Check if a directory is a PS3 game directory
    fn is_game_directory(&self, dir: &Path) -> bool {
        // Check for PS3_GAME/PARAM.SFO structure (disc games)
        let param_sfo = dir.join("PS3_GAME").join("PARAM.SFO");
        if param_sfo.exists() {
            return true;
        }

        // Check for PARAM.SFO in current directory (HDD games)
        let param_sfo = dir.join("PARAM.SFO");
        if param_sfo.exists() {
            return true;
        }

        // Check for EBOOT.BIN (minimal check)
        let eboot = dir.join("PS3_GAME").join("USRDIR").join("EBOOT.BIN");
        if eboot.exists() {
            return true;
        }

        let eboot = dir.join("USRDIR").join("EBOOT.BIN");
        eboot.exists()
    }

    /// Extract game info from a game directory
    fn extract_game_info(&self, dir: &Path) -> Result<Option<GameInfo>> {
        // Find PARAM.SFO
        let param_sfo_path = if dir.join("PS3_GAME").join("PARAM.SFO").exists() {
            dir.join("PS3_GAME").join("PARAM.SFO")
        } else if dir.join("PARAM.SFO").exists() {
            dir.join("PARAM.SFO")
        } else {
            debug!("No PARAM.SFO found in {:?}", dir);
            return Ok(None);
        };

        // Parse PARAM.SFO
        let game_info = self.parse_param_sfo(&param_sfo_path, dir)?;
        Ok(Some(game_info))
    }

    /// Parse PARAM.SFO file and extract game information
    fn parse_param_sfo(&self, sfo_path: &Path, game_dir: &Path) -> Result<GameInfo> {
        debug!("Parsing PARAM.SFO: {:?}", sfo_path);

        let file = File::open(sfo_path).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to open PARAM.SFO: {}",
                e
            )))
        })?;
        let mut reader = BufReader::new(file);

        // Parse SFO header
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to read SFO magic: {}",
                e
            )))
        })?;

        if &magic != b"\x00PSF" {
            return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                "Invalid SFO magic".to_string(),
            )));
        }

        let mut header = [0u8; 16];
        reader.read_exact(&mut header).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to read SFO header: {}",
                e
            )))
        })?;

        let _version = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let key_table_start = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        let data_table_start = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
        let entries_count = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);

        let mut entries: HashMap<String, SfoValue> = HashMap::new();

        // Parse entries
        for i in 0..entries_count {
            let entry_offset = 20 + i * 16;
            reader
                .seek(std::io::SeekFrom::Start(entry_offset as u64))
                .map_err(|e| {
                    EmulatorError::Loader(LoaderError::InvalidElf(format!(
                        "Failed to seek to entry: {}",
                        e
                    )))
                })?;

            let mut entry_data = [0u8; 16];
            reader.read_exact(&mut entry_data).map_err(|e| {
                EmulatorError::Loader(LoaderError::InvalidElf(format!(
                    "Failed to read SFO entry: {}",
                    e
                )))
            })?;

            let key_offset = u16::from_le_bytes([entry_data[0], entry_data[1]]);
            let data_fmt = u16::from_le_bytes([entry_data[2], entry_data[3]]);
            let data_len =
                u32::from_le_bytes([entry_data[4], entry_data[5], entry_data[6], entry_data[7]]);
            let data_offset = u32::from_le_bytes([
                entry_data[12],
                entry_data[13],
                entry_data[14],
                entry_data[15],
            ]);

            // Read key
            reader
                .seek(std::io::SeekFrom::Start(
                    (key_table_start + key_offset as u32) as u64,
                ))
                .map_err(|e| {
                    EmulatorError::Loader(LoaderError::InvalidElf(format!(
                        "Failed to seek to key: {}",
                        e
                    )))
                })?;

            let mut key = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                if reader.read_exact(&mut byte).is_err() {
                    break;
                }
                if byte[0] == 0 {
                    break;
                }
                key.push(byte[0]);
            }
            let key = String::from_utf8_lossy(&key).to_string();

            // Read value
            reader
                .seek(std::io::SeekFrom::Start(
                    (data_table_start + data_offset) as u64,
                ))
                .map_err(|e| {
                    EmulatorError::Loader(LoaderError::InvalidElf(format!(
                        "Failed to seek to value: {}",
                        e
                    )))
                })?;

            let value = match data_fmt {
                0x0404 => {
                    let mut buf = [0u8; 4];
                    reader.read_exact(&mut buf).map_err(|e| {
                        EmulatorError::Loader(LoaderError::InvalidElf(format!(
                            "Failed to read integer value: {}",
                            e
                        )))
                    })?;
                    SfoValue::Integer(u32::from_le_bytes(buf))
                }
                0x0004 | 0x0204 => {
                    let mut buf = vec![0u8; data_len as usize];
                    reader.read_exact(&mut buf).map_err(|e| {
                        EmulatorError::Loader(LoaderError::InvalidElf(format!(
                            "Failed to read string value: {}",
                            e
                        )))
                    })?;
                    // Remove null terminator
                    while buf.last() == Some(&0) {
                        buf.pop();
                    }
                    SfoValue::String(String::from_utf8_lossy(&buf).to_string())
                }
                _ => continue,
            };

            entries.insert(key, value);
        }

        // Build GameInfo from parsed entries
        let title = entries
            .get("TITLE")
            .and_then(|v| v.as_string())
            .unwrap_or("Unknown Title")
            .to_string();
        let title_id = entries
            .get("TITLE_ID")
            .and_then(|v| v.as_string())
            .unwrap_or("UNKNOWN")
            .to_string();
        let version = entries
            .get("VERSION")
            .and_then(|v| v.as_string())
            .unwrap_or("01.00")
            .to_string();
        let category = entries
            .get("CATEGORY")
            .and_then(|v| v.as_string())
            .unwrap_or("DG")
            .to_string();
        let parental_level = entries
            .get("PARENTAL_LEVEL")
            .and_then(|v| v.as_integer())
            .unwrap_or(0);
        let resolution = entries
            .get("RESOLUTION")
            .and_then(|v| v.as_integer())
            .unwrap_or(0);
        let sound_format = entries
            .get("SOUND_FORMAT")
            .and_then(|v| v.as_integer())
            .unwrap_or(0);

        info!(
            "Parsed game: {} ({}) v{}",
            title, title_id, version
        );

        // Extract icon and background images
        let (icon0_data, pic1_data) = self.extract_images(game_dir);

        Ok(GameInfo {
            title,
            title_id,
            version,
            path: game_dir.to_path_buf(),
            category,
            parental_level,
            resolution,
            sound_format,
            icon0_data,
            pic1_data,
        })
    }

    /// Extract ICON0.PNG and PIC1.PNG from game directory
    fn extract_images(&self, game_dir: &Path) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let mut icon0_data = None;
        let mut pic1_data = None;

        // Try to find ICON0.PNG in multiple locations
        let icon0_paths = [
            game_dir.join("ICON0.PNG"),
            game_dir.join("PS3_GAME").join("ICON0.PNG"),
        ];

        for icon_path in &icon0_paths {
            if icon_path.exists() {
                if let Ok(data) = fs::read(icon_path) {
                    debug!("Loaded ICON0.PNG: {} bytes", data.len());
                    icon0_data = Some(data);
                    break;
                }
            }
        }

        // Try to find PIC1.PNG in multiple locations
        let pic1_paths = [
            game_dir.join("PIC1.PNG"),
            game_dir.join("PS3_GAME").join("PIC1.PNG"),
        ];

        for pic_path in &pic1_paths {
            if pic_path.exists() {
                if let Ok(data) = fs::read(pic_path) {
                    debug!("Loaded PIC1.PNG: {} bytes", data.len());
                    pic1_data = Some(data);
                    break;
                }
            }
        }

        (icon0_data, pic1_data)
    }

    /// Get discovered games
    pub fn games(&self) -> &HashMap<String, GameInfo> {
        &self.games
    }

    /// Get a game by title ID
    pub fn get_game(&self, title_id: &str) -> Option<&GameInfo> {
        self.games.get(title_id)
    }
}

impl Default for GameScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// SFO value type for internal parsing
#[derive(Debug, Clone)]
enum SfoValue {
    String(String),
    Integer(u32),
}

impl SfoValue {
    fn as_string(&self) -> Option<&str> {
        match self {
            SfoValue::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_integer(&self) -> Option<u32> {
        match self {
            SfoValue::Integer(v) => Some(*v),
            _ => None,
        }
    }
}

/// System module initialization state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    /// Module is not loaded
    Unloaded,
    /// Module is loaded but not started
    Loaded,
    /// Module is started and running
    Running,
    /// Module is stopped
    Stopped,
}

/// System module information
#[derive(Debug, Clone)]
pub struct SystemModule {
    /// Module name
    pub name: String,
    /// Module state
    pub state: ModuleState,
    /// Module ID
    pub id: u32,
    /// Module dependencies (names of other modules this module requires)
    pub dependencies: Vec<String>,
    /// Modules that depend on this module (reverse dependencies)
    pub dependents: Vec<String>,
    /// Start order priority (lower = starts first)
    pub start_priority: u32,
    /// Stop order priority (lower = stops first)
    pub stop_priority: u32,
}

/// Module dependency information for PRX modules
#[derive(Debug, Clone)]
pub struct ModuleDependency {
    /// Module name
    pub name: String,
    /// Required version (0 = any version)
    pub version: u32,
    /// Whether this dependency is optional
    pub optional: bool,
}

/// Module lifecycle event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleLifecycleEvent {
    /// Module is being loaded
    Loading,
    /// Module has been loaded
    Loaded,
    /// Module is being started
    Starting,
    /// Module has been started
    Started,
    /// Module is being stopped
    Stopping,
    /// Module has been stopped
    Stopped,
    /// Module is being unloaded
    Unloading,
    /// Module has been unloaded
    Unloaded,
}

/// Game loading pipeline that coordinates all aspects of game loading
pub struct GamePipeline {
    /// HLE module registry
    module_registry: ModuleRegistry,
    /// Loaded system modules
    system_modules: HashMap<String, SystemModule>,
    /// Memory manager reference
    memory: Arc<MemoryManager>,
    /// Game scanner
    scanner: GameScanner,
    /// Next module ID
    next_module_id: u32,
    /// Module dependency graph (module name -> list of dependencies)
    module_dependencies: HashMap<String, Vec<ModuleDependency>>,
    /// Next start priority counter
    next_start_priority: u32,
}

impl GamePipeline {
    /// Create a new game pipeline
    pub fn new(memory: Arc<MemoryManager>) -> Self {
        let mut pipeline = Self {
            module_registry: ModuleRegistry::new(),
            system_modules: HashMap::new(),
            memory,
            scanner: GameScanner::new(),
            next_module_id: 1,
            module_dependencies: HashMap::new(),
            next_start_priority: 1,
        };
        
        // Initialize known module dependencies
        pipeline.init_module_dependencies();
        
        pipeline
    }
    
    /// Initialize the known module dependency graph
    fn init_module_dependencies(&mut self) {
        // Define known module dependencies for PS3 system modules
        // Format: module_name -> list of dependencies
        
        // cellGcmSys depends on cellSysutil
        self.module_dependencies.insert("cellGcmSys".to_string(), vec![
            ModuleDependency { name: "cellSysutil".to_string(), version: 0, optional: false },
        ]);
        
        // cellSpurs depends on cellSysutil
        self.module_dependencies.insert("cellSpurs".to_string(), vec![
            ModuleDependency { name: "cellSysutil".to_string(), version: 0, optional: false },
        ]);
        
        // cellGame depends on cellSysutil and cellFs
        self.module_dependencies.insert("cellGame".to_string(), vec![
            ModuleDependency { name: "cellSysutil".to_string(), version: 0, optional: false },
            ModuleDependency { name: "cellFs".to_string(), version: 0, optional: false },
        ]);
        
        // cellSaveData depends on cellFs
        self.module_dependencies.insert("cellSaveData".to_string(), vec![
            ModuleDependency { name: "cellFs".to_string(), version: 0, optional: false },
        ]);
        
        // cellAudio has no required dependencies but optional cellSpurs
        self.module_dependencies.insert("cellAudio".to_string(), vec![
            ModuleDependency { name: "cellSpurs".to_string(), version: 0, optional: true },
        ]);
        
        // cellPad depends on cellSysutil
        self.module_dependencies.insert("cellPad".to_string(), vec![
            ModuleDependency { name: "cellSysutil".to_string(), version: 0, optional: false },
        ]);
        
        // Image decoders depend on cellFs
        for decoder in &["cellPngDec", "cellJpgDec", "cellGifDec"] {
            self.module_dependencies.insert(decoder.to_string(), vec![
                ModuleDependency { name: "cellFs".to_string(), version: 0, optional: false },
            ]);
        }
        
        // Media modules depend on cellSpurs (optional)
        for media in &["cellDmux", "cellVdec", "cellAdec", "cellVpost"] {
            self.module_dependencies.insert(media.to_string(), vec![
                ModuleDependency { name: "cellSpurs".to_string(), version: 0, optional: true },
            ]);
        }
        
        // Network modules depend on cellSysutil
        for net in &["cellNetCtl", "cellHttp", "cellSsl"] {
            self.module_dependencies.insert(net.to_string(), vec![
                ModuleDependency { name: "cellSysutil".to_string(), version: 0, optional: false },
            ]);
        }
        
        debug!("Initialized {} module dependency entries", self.module_dependencies.len());
    }

    /// Add a directory to scan for games
    pub fn add_game_directory<P: AsRef<Path>>(&mut self, path: P) {
        self.scanner.add_search_directory(path);
    }

    /// Scan for games in all configured directories
    pub fn scan_games(&mut self) -> Result<Vec<GameInfo>> {
        self.scanner.scan()
    }

    /// Get the game scanner
    pub fn scanner(&self) -> &GameScanner {
        &self.scanner
    }

    /// Get mutable reference to game scanner
    pub fn scanner_mut(&mut self) -> &mut GameScanner {
        &mut self.scanner
    }

    /// Initialize all required system modules before game start
    pub fn initialize_system_modules(&mut self) -> Result<()> {
        info!("Initializing system modules");

        // List of core modules that need to be initialized for most games
        let core_modules = [
            "cellSysutil",
            "cellGcmSys",
            "cellFs",
            "cellPad",
            "cellAudio",
            "cellSpurs",
            "cellGame",
        ];

        for module_name in &core_modules {
            self.load_module(module_name)?;
        }

        info!(
            "Initialized {} system modules",
            self.system_modules.len()
        );
        Ok(())
    }

    /// Load a specific system module
    pub fn load_module(&mut self, name: &str) -> Result<u32> {
        // Check if already loaded
        if let Some(module) = self.system_modules.get(name) {
            debug!("Module {} already loaded with ID {}", name, module.id);
            return Ok(module.id);
        }

        // Check if module exists in registry
        if self.module_registry.get_module(name).is_none() {
            return Err(EmulatorError::Loader(LoaderError::MissingModule(
                name.to_string(),
            )));
        }

        // Load dependencies first
        let dependencies = self.load_module_dependencies(name)?;

        // Assign module ID and create entry
        let module_id = self.next_module_id;
        self.next_module_id += 1;
        
        let start_priority = self.next_start_priority;
        self.next_start_priority += 1;

        let module = SystemModule {
            name: name.to_string(),
            state: ModuleState::Loaded,
            id: module_id,
            dependencies: dependencies.clone(),
            dependents: Vec::new(),
            start_priority,
            stop_priority: u32::MAX - start_priority, // Reverse order for stopping
        };

        self.system_modules.insert(name.to_string(), module);
        
        // Update dependents for each dependency
        for dep_name in &dependencies {
            if let Some(dep_module) = self.system_modules.get_mut(dep_name) {
                if !dep_module.dependents.contains(&name.to_string()) {
                    dep_module.dependents.push(name.to_string());
                }
            }
        }
        
        info!("Loaded system module: {} (ID: {}, deps: {:?})", name, module_id, dependencies);

        Ok(module_id)
    }
    
    /// Load dependencies for a module (returns list of loaded dependency names)
    fn load_module_dependencies(&mut self, name: &str) -> Result<Vec<String>> {
        let mut loaded_deps = Vec::new();
        
        // Get the dependencies for this module (if any)
        let deps = self.module_dependencies.get(name).cloned().unwrap_or_default();
        
        for dep in deps {
            // Skip if already loaded
            if self.system_modules.contains_key(&dep.name) {
                loaded_deps.push(dep.name.clone());
                continue;
            }
            
            // Try to load the dependency
            match self.load_module(&dep.name) {
                Ok(_) => {
                    loaded_deps.push(dep.name.clone());
                    debug!("Loaded dependency {} for module {}", dep.name, name);
                }
                Err(e) => {
                    if dep.optional {
                        debug!("Optional dependency {} for module {} not available: {}", dep.name, name, e);
                    } else {
                        return Err(EmulatorError::Loader(LoaderError::MissingModule(
                            format!("Failed to load required dependency {} for {}: {}", dep.name, name, e)
                        )));
                    }
                }
            }
        }
        
        Ok(loaded_deps)
    }
    
    /// Get dependencies for a module
    pub fn get_module_dependencies(&self, name: &str) -> Vec<String> {
        self.module_dependencies
            .get(name)
            .map(|deps| deps.iter().map(|d| d.name.clone()).collect())
            .unwrap_or_default()
    }
    
    /// Get dependents for a module (modules that depend on this one)
    pub fn get_module_dependents(&self, name: &str) -> Vec<String> {
        self.system_modules
            .get(name)
            .map(|m| m.dependents.clone())
            .unwrap_or_default()
    }
    
    /// Check if all dependencies of a module are satisfied
    pub fn are_dependencies_satisfied(&self, name: &str) -> bool {
        let deps = self.module_dependencies.get(name);
        if deps.is_none() {
            return true;
        }
        
        for dep in deps.unwrap() {
            if !dep.optional && !self.system_modules.contains_key(&dep.name) {
                return false;
            }
        }
        
        true
    }
    
    /// Check if a module can be safely unloaded (no dependents are running)
    pub fn can_unload_module(&self, name: &str) -> bool {
        if let Some(module) = self.system_modules.get(name) {
            for dependent_name in &module.dependents {
                if let Some(dependent) = self.system_modules.get(dependent_name) {
                    if dependent.state == ModuleState::Running {
                        return false;
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Start a loaded module
    /// 
    /// This will first ensure all dependencies are started, then start the module.
    pub fn start_module(&mut self, name: &str) -> Result<()> {
        // First, start all dependencies
        self.start_module_dependencies(name)?;
        
        if let Some(module) = self.system_modules.get_mut(name) {
            match module.state {
                ModuleState::Loaded | ModuleState::Stopped => {
                    module.state = ModuleState::Running;
                    info!("Started module: {} (lifecycle: {:?} -> {:?})", 
                          name, ModuleLifecycleEvent::Starting, ModuleLifecycleEvent::Started);
                    Ok(())
                }
                ModuleState::Running => {
                    debug!("Module {} is already running", name);
                    Ok(())
                }
                ModuleState::Unloaded => {
                    Err(EmulatorError::Loader(LoaderError::InvalidElf(format!(
                        "Module {} is unloaded and cannot be started",
                        name
                    ))))
                }
            }
        } else {
            Err(EmulatorError::Loader(LoaderError::MissingModule(
                name.to_string(),
            )))
        }
    }
    
    /// Start all dependencies for a module
    fn start_module_dependencies(&mut self, name: &str) -> Result<()> {
        // Get the module's dependencies
        let deps: Vec<String> = self.system_modules
            .get(name)
            .map(|m| m.dependencies.clone())
            .unwrap_or_default();
        
        for dep_name in deps {
            if let Some(dep_module) = self.system_modules.get(&dep_name) {
                if dep_module.state != ModuleState::Running {
                    // Recursively start dependency (this will start its dependencies too)
                    self.start_module(&dep_name)?;
                }
            }
        }
        
        Ok(())
    }

    /// Stop a running module
    /// 
    /// This will first stop all dependent modules, then stop this module.
    pub fn stop_module(&mut self, name: &str) -> Result<()> {
        // First, stop all dependents
        self.stop_module_dependents(name)?;
        
        if let Some(module) = self.system_modules.get_mut(name) {
            match module.state {
                ModuleState::Running => {
                    module.state = ModuleState::Stopped;
                    info!("Stopped module: {} (lifecycle: {:?} -> {:?})", 
                          name, ModuleLifecycleEvent::Stopping, ModuleLifecycleEvent::Stopped);
                    Ok(())
                }
                ModuleState::Stopped => {
                    debug!("Module {} is already stopped", name);
                    Ok(())
                }
                ModuleState::Loaded => {
                    debug!("Module {} was never started", name);
                    Ok(())
                }
                ModuleState::Unloaded => {
                    Err(EmulatorError::Loader(LoaderError::InvalidElf(format!(
                        "Module {} is unloaded",
                        name
                    ))))
                }
            }
        } else {
            Err(EmulatorError::Loader(LoaderError::MissingModule(
                name.to_string(),
            )))
        }
    }
    
    /// Stop all modules that depend on this module
    fn stop_module_dependents(&mut self, name: &str) -> Result<()> {
        // Get the module's dependents
        let dependents: Vec<String> = self.system_modules
            .get(name)
            .map(|m| m.dependents.clone())
            .unwrap_or_default();
        
        for dependent_name in dependents {
            if let Some(dep_module) = self.system_modules.get(&dependent_name) {
                if dep_module.state == ModuleState::Running {
                    // Recursively stop dependent (this will stop its dependents too)
                    self.stop_module(&dependent_name)?;
                }
            }
        }
        
        Ok(())
    }

    /// Unload a module
    /// 
    /// This will first stop the module if running, then remove it.
    pub fn unload_module(&mut self, name: &str) -> Result<()> {
        // Stop the module first if it's running
        if let Some(module) = self.system_modules.get(name) {
            if module.state == ModuleState::Running {
                self.stop_module(name)?;
            }
        }
        
        // Check if any dependents are still loaded
        if let Some(module) = self.system_modules.get(name) {
            for dependent_name in &module.dependents {
                if self.system_modules.contains_key(dependent_name) {
                    return Err(EmulatorError::Loader(LoaderError::InvalidElf(format!(
                        "Cannot unload module {}: module {} still depends on it",
                        name, dependent_name
                    ))));
                }
            }
        }
        
        // Remove from dependents lists
        let deps: Vec<String> = self.system_modules
            .get(name)
            .map(|m| m.dependencies.clone())
            .unwrap_or_default();
            
        for dep_name in &deps {
            if let Some(dep_module) = self.system_modules.get_mut(dep_name) {
                dep_module.dependents.retain(|n| n != name);
            }
        }
        
        if let Some(module) = self.system_modules.remove(name) {
            info!("Unloaded module: {} (ID: {}, lifecycle: {:?})", 
                  name, module.id, ModuleLifecycleEvent::Unloaded);
            Ok(())
        } else {
            Err(EmulatorError::Loader(LoaderError::MissingModule(
                name.to_string(),
            )))
        }
    }

    /// Start all loaded modules in dependency order
    pub fn start_all_modules(&mut self) -> Result<()> {
        // Sort modules by start priority (lower priority starts first)
        let mut module_names: Vec<(String, u32)> = self.system_modules
            .iter()
            .filter(|(_, m)| m.state == ModuleState::Loaded)
            .map(|(name, m)| (name.clone(), m.start_priority))
            .collect();
        module_names.sort_by_key(|(_, priority)| *priority);
        
        info!("Starting {} modules in dependency order", module_names.len());
        
        for (name, _) in module_names {
            self.start_module(&name)?;
        }
        Ok(())
    }
    
    /// Stop all running modules in reverse dependency order
    pub fn stop_all_modules(&mut self) -> Result<()> {
        // Sort modules by stop priority (lower priority stops first, which means higher start priority)
        let mut module_names: Vec<(String, u32)> = self.system_modules
            .iter()
            .filter(|(_, m)| m.state == ModuleState::Running)
            .map(|(name, m)| (name.clone(), m.stop_priority))
            .collect();
        module_names.sort_by_key(|(_, priority)| *priority);
        
        info!("Stopping {} modules in reverse dependency order", module_names.len());
        
        for (name, _) in module_names {
            self.stop_module(&name)?;
        }
        Ok(())
    }
    
    /// Restart a module (stop then start)
    pub fn restart_module(&mut self, name: &str) -> Result<()> {
        info!("Restarting module: {}", name);
        self.stop_module(name)?;
        self.start_module(name)?;
        Ok(())
    }
    
    /// Get the lifecycle state of a module
    pub fn get_module_state(&self, name: &str) -> Option<ModuleState> {
        self.system_modules.get(name).map(|m| m.state)
    }
    
    /// Check if a module is running
    pub fn is_module_running(&self, name: &str) -> bool {
        self.system_modules
            .get(name)
            .map(|m| m.state == ModuleState::Running)
            .unwrap_or(false)
    }

    /// Set up proper memory layout for games
    ///
    /// This initializes the PS3 memory layout with proper regions:
    /// - Main memory (256 MB at 0x00000000)
    /// - User memory (256 MB at 0x20000000)
    /// - RSX mapped memory (256 MB at 0x30000000)
    /// - RSX I/O registers (1 MB at 0x40000000)
    /// - RSX local memory (256 MB at 0xC0000000)
    /// - Stack area (256 MB at 0xD0000000)
    /// - SPU local storage (at 0xE0000000)
    pub fn setup_memory_layout(&self) -> Result<MemoryLayoutInfo> {
        info!("Setting up PS3 memory layout");

        // The memory manager already initializes these regions in its constructor
        // We just need to verify and return the layout info

        let layout = MemoryLayoutInfo {
            main_memory_base: 0x0000_0000,
            main_memory_size: 0x1000_0000, // 256 MB
            user_memory_base: 0x2000_0000,
            user_memory_size: 0x1000_0000, // 256 MB
            rsx_map_base: 0x3000_0000,
            rsx_map_size: 0x1000_0000, // 256 MB
            rsx_io_base: 0x4000_0000,
            rsx_io_size: 0x0010_0000, // 1 MB
            rsx_mem_base: 0xC000_0000,
            rsx_mem_size: 0x1000_0000, // 256 MB
            stack_base: 0xD000_0000,
            stack_size: 0x1000_0000, // 256 MB
            spu_base: 0xE000_0000,
            spu_ls_size: 0x0004_0000, // 256 KB per SPU
        };

        // Initialize process data area (PDA) at a known location within main memory
        // This is where PS3 system information is stored
        
        // Write some initial PDA values
        // Note: These are stub values - real values would come from the system
        self.memory.write_be32(PDA_BASE_ADDRESS + PDA_PROCESS_ID_OFFSET, 0)?; // Process ID placeholder
        self.memory.write_be32(PDA_BASE_ADDRESS + PDA_THREAD_ID_OFFSET, 0)?; // Thread ID placeholder

        debug!(
            "Memory layout configured: main=0x{:08x}-0x{:08x}, user=0x{:08x}-0x{:08x}",
            layout.main_memory_base,
            layout.main_memory_base + layout.main_memory_size,
            layout.user_memory_base,
            layout.user_memory_base + layout.user_memory_size
        );

        Ok(layout)
    }
    
    /// Set up stack for main thread
    ///
    /// Allocates and initializes the stack for the main PPU thread.
    /// The stack grows downward from high addresses to low addresses.
    pub fn setup_main_thread_stack(&self, stack_size: u32) -> Result<ThreadStackInfo> {
        info!("Setting up main thread stack (size: 0x{:x} bytes)", stack_size);
        
        // PS3 stack base is at 0xD0000000
        let stack_base = 0xD000_0000u32;
        
        // Stack top (highest address) is base + size
        let stack_top = stack_base.checked_add(stack_size)
            .ok_or_else(|| EmulatorError::Loader(LoaderError::InvalidElf(
                "Stack address overflow".to_string()
            )))?;
        
        // Initialize stack with guard pattern (0xDEADBEEF) for debugging
        let guard_pattern = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let mut stack_data = Vec::with_capacity(stack_size as usize);
        
        // Fill stack with guard pattern
        for _ in 0..(stack_size / 4) {
            stack_data.extend_from_slice(&guard_pattern);
        }
        
        // Write guard pattern to stack memory
        self.memory.write_bytes(stack_base, &stack_data)?;
        
        // The stack pointer (SP) starts at the top and grows downward
        // Leave some space at the top for the red zone (288 bytes as per PowerPC ABI)
        const RED_ZONE_SIZE: u32 = 288;
        let initial_sp = stack_top - RED_ZONE_SIZE;
        
        debug!(
            "Stack configured: base=0x{:08x}, top=0x{:08x}, initial_sp=0x{:08x}",
            stack_base, stack_top, initial_sp
        );
        
        Ok(ThreadStackInfo {
            stack_base,
            stack_size,
            stack_top,
            initial_sp,
        })
    }
    
    /// Configure TLS (Thread-Local Storage) areas
    ///
    /// Sets up thread-local storage for the main thread and reserves space
    /// for additional threads.
    pub fn configure_tls_areas(&self, main_tls_size: u32, max_threads: u32) -> Result<TlsLayoutInfo> {
        info!("Configuring TLS areas (main size: 0x{:x}, max threads: {})", main_tls_size, max_threads);
        
        // TLS base address on PS3 - placed in user memory region
        const TLS_BASE: u32 = 0x2800_0000;
        
        // Align TLS size to 4KB boundaries
        let aligned_tls_size = (main_tls_size + 0xFFF) & !0xFFF;
        
        // Calculate total TLS region size
        let total_tls_size = aligned_tls_size * max_threads;
        
        if total_tls_size > 0x1000_0000 {  // Sanity check: TLS shouldn't exceed 256MB
            return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                "TLS size too large".to_string()
            )));
        }
        
        // Initialize main thread TLS
        let main_tls_addr = TLS_BASE;
        let zeros = vec![0u8; aligned_tls_size as usize];
        self.memory.write_bytes(main_tls_addr, &zeros)?;
        
        // Set up TLS template pointer at a known offset
        // On PS3, R13 register typically points to TLS
        let tls_template_offset = 0x7000;  // Standard PS3 TLS offset
        let tls_pointer = main_tls_addr + tls_template_offset;
        
        debug!(
            "TLS configured: base=0x{:08x}, size_per_thread=0x{:x}, pointer=0x{:08x}",
            main_tls_addr, aligned_tls_size, tls_pointer
        );
        
        // Reserve space for additional threads
        let mut thread_tls_areas = Vec::new();
        for i in 0..max_threads {
            let thread_tls_addr = TLS_BASE + (i * aligned_tls_size);
            thread_tls_areas.push(TlsThreadArea {
                address: thread_tls_addr,
                size: aligned_tls_size,
                thread_index: i,
            });
        }
        
        Ok(TlsLayoutInfo {
            base_address: TLS_BASE,
            size_per_thread: aligned_tls_size,
            max_threads,
            main_tls_pointer: tls_pointer,
            thread_areas: thread_tls_areas,
        })
    }
    
    /// Initialize kernel objects
    ///
    /// Sets up essential kernel objects like mutexes, semaphores, and event queues
    /// that are needed for game execution.
    pub fn initialize_kernel_objects(&mut self) -> Result<KernelObjectsInfo> {
        info!("Initializing kernel objects");
        
        // Create initial kernel objects
        let mut kernel_info = KernelObjectsInfo {
            mutex_count: 0,
            semaphore_count: 0,
            event_queue_count: 0,
            cond_var_count: 0,
            rwlock_count: 0,
            initialized: true,
        };
        
        // Pre-allocate some system mutexes
        // On PS3, there are typically a few system-wide mutexes for internal use
        const SYSTEM_MUTEX_COUNT: u32 = 16;
        for i in 0..SYSTEM_MUTEX_COUNT {
            debug!("Pre-allocating system mutex {}", i);
            kernel_info.mutex_count += 1;
        }
        
        // Pre-allocate some system semaphores
        const SYSTEM_SEMAPHORE_COUNT: u32 = 8;
        for i in 0..SYSTEM_SEMAPHORE_COUNT {
            debug!("Pre-allocating system semaphore {}", i);
            kernel_info.semaphore_count += 1;
        }
        
        // Pre-allocate system event queues
        const SYSTEM_EVENT_QUEUE_COUNT: u32 = 4;
        for i in 0..SYSTEM_EVENT_QUEUE_COUNT {
            debug!("Pre-allocating system event queue {}", i);
            kernel_info.event_queue_count += 1;
        }
        
        info!(
            "Kernel objects initialized: mutexes={}, semaphores={}, event_queues={}",
            kernel_info.mutex_count, kernel_info.semaphore_count, kernel_info.event_queue_count
        );
        
        Ok(kernel_info)
    }

    /// Get the HLE module registry
    pub fn module_registry(&self) -> &ModuleRegistry {
        &self.module_registry
    }

    /// Get mutable reference to HLE module registry
    pub fn module_registry_mut(&mut self) -> &mut ModuleRegistry {
        &mut self.module_registry
    }

    /// Get loaded system modules
    pub fn system_modules(&self) -> &HashMap<String, SystemModule> {
        &self.system_modules
    }

    /// Get a system module by name
    pub fn get_system_module(&self, name: &str) -> Option<&SystemModule> {
        self.system_modules.get(name)
    }

    /// Call an HLE function by module name and NID
    pub fn call_hle_function(&self, module: &str, nid: u32, args: &[u64]) -> Result<i64> {
        if let Some(func) = self.module_registry.find_function(module, nid) {
            Ok(func(args))
        } else {
            warn!(
                "HLE function not found: module={}, nid=0x{:08x}",
                module, nid
            );
            Err(EmulatorError::Loader(LoaderError::MissingModule(format!(
                "{}:0x{:08x}",
                module, nid
            ))))
        }
    }
}

/// Memory layout information
#[derive(Debug, Clone)]
pub struct MemoryLayoutInfo {
    /// Main memory base address
    pub main_memory_base: u32,
    /// Main memory size
    pub main_memory_size: u32,
    /// User memory base address
    pub user_memory_base: u32,
    /// User memory size
    pub user_memory_size: u32,
    /// RSX mapped memory base
    pub rsx_map_base: u32,
    /// RSX mapped memory size
    pub rsx_map_size: u32,
    /// RSX I/O base
    pub rsx_io_base: u32,
    /// RSX I/O size
    pub rsx_io_size: u32,
    /// RSX local memory base
    pub rsx_mem_base: u32,
    /// RSX local memory size
    pub rsx_mem_size: u32,
    /// Stack base
    pub stack_base: u32,
    /// Stack size
    pub stack_size: u32,
    /// SPU base
    pub spu_base: u32,
    /// SPU local storage size
    pub spu_ls_size: u32,
}

/// Thread stack information
#[derive(Debug, Clone)]
pub struct ThreadStackInfo {
    /// Stack base address (lowest address)
    pub stack_base: u32,
    /// Stack size in bytes
    pub stack_size: u32,
    /// Stack top address (highest address)
    pub stack_top: u32,
    /// Initial stack pointer (SP register value)
    pub initial_sp: u32,
}

/// TLS layout information
#[derive(Debug, Clone)]
pub struct TlsLayoutInfo {
    /// TLS base address
    pub base_address: u32,
    /// Size allocated per thread
    pub size_per_thread: u32,
    /// Maximum number of threads
    pub max_threads: u32,
    /// Main thread TLS pointer (R13 register value)
    pub main_tls_pointer: u32,
    /// Individual thread TLS areas
    pub thread_areas: Vec<TlsThreadArea>,
}

/// TLS area for a single thread
#[derive(Debug, Clone)]
pub struct TlsThreadArea {
    /// TLS address for this thread
    pub address: u32,
    /// Size of this thread's TLS
    pub size: u32,
    /// Thread index
    pub thread_index: u32,
}

/// Kernel objects information
#[derive(Debug, Clone)]
pub struct KernelObjectsInfo {
    /// Number of mutexes allocated
    pub mutex_count: u32,
    /// Number of semaphores allocated
    pub semaphore_count: u32,
    /// Number of event queues allocated
    pub event_queue_count: u32,
    /// Number of condition variables allocated
    pub cond_var_count: u32,
    /// Number of reader-writer locks allocated
    pub rwlock_count: u32,
    /// Whether kernel objects have been initialized
    pub initialized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_scanner_creation() {
        let scanner = GameScanner::new();
        assert!(scanner.games().is_empty());
    }

    #[test]
    fn test_game_scanner_add_directory() {
        let mut scanner = GameScanner::new();
        scanner.add_search_directory("/tmp/games");
        scanner.add_search_directory("/tmp/games"); // Duplicate should be ignored
        assert_eq!(scanner.search_dirs.len(), 1);
    }

    #[test]
    fn test_game_info_default() {
        let info = GameInfo::default();
        assert!(info.title.is_empty());
        assert!(info.title_id.is_empty());
    }

    #[test]
    fn test_sfo_value() {
        let string_val = SfoValue::String("test".to_string());
        assert_eq!(string_val.as_string(), Some("test"));
        assert_eq!(string_val.as_integer(), None);

        let int_val = SfoValue::Integer(42);
        assert_eq!(int_val.as_string(), None);
        assert_eq!(int_val.as_integer(), Some(42));
    }

    #[test]
    fn test_memory_layout_info() {
        let layout = MemoryLayoutInfo {
            main_memory_base: 0x0000_0000,
            main_memory_size: 0x1000_0000,
            user_memory_base: 0x2000_0000,
            user_memory_size: 0x1000_0000,
            rsx_map_base: 0x3000_0000,
            rsx_map_size: 0x1000_0000,
            rsx_io_base: 0x4000_0000,
            rsx_io_size: 0x0010_0000,
            rsx_mem_base: 0xC000_0000,
            rsx_mem_size: 0x1000_0000,
            stack_base: 0xD000_0000,
            stack_size: 0x1000_0000,
            spu_base: 0xE000_0000,
            spu_ls_size: 0x0004_0000,
        };

        assert_eq!(layout.main_memory_size, 256 * 1024 * 1024);
        assert_eq!(layout.user_memory_size, 256 * 1024 * 1024);
    }

    #[test]
    fn test_module_state() {
        let module = SystemModule {
            name: "test".to_string(),
            state: ModuleState::Loaded,
            id: 1,
            dependencies: Vec::new(),
            dependents: Vec::new(),
            start_priority: 1,
            stop_priority: u32::MAX - 1,
        };
        assert_eq!(module.state, ModuleState::Loaded);
    }
    
    #[test]
    fn test_module_dependency() {
        let dep = ModuleDependency {
            name: "cellSysutil".to_string(),
            version: 0,
            optional: false,
        };
        assert_eq!(dep.name, "cellSysutil");
        assert!(!dep.optional);
    }
    
    #[test]
    fn test_module_lifecycle_event() {
        let event = ModuleLifecycleEvent::Started;
        assert_eq!(event, ModuleLifecycleEvent::Started);
    }

    #[test]
    fn test_game_pipeline_creation() {
        let memory = MemoryManager::new().unwrap();
        let pipeline = GamePipeline::new(memory);
        assert!(pipeline.system_modules().is_empty());
    }

    #[test]
    fn test_game_pipeline_load_module() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);

        let id = pipeline.load_module("cellSysutil").unwrap();
        assert_eq!(id, 1);

        // Loading same module again should return same ID
        let id2 = pipeline.load_module("cellSysutil").unwrap();
        assert_eq!(id2, 1);
    }

    #[test]
    fn test_game_pipeline_start_module() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);

        pipeline.load_module("cellSysutil").unwrap();
        pipeline.start_module("cellSysutil").unwrap();

        let module = pipeline.get_system_module("cellSysutil").unwrap();
        assert_eq!(module.state, ModuleState::Running);
    }

    #[test]
    fn test_game_pipeline_stop_module() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);

        pipeline.load_module("cellSysutil").unwrap();
        pipeline.start_module("cellSysutil").unwrap();
        pipeline.stop_module("cellSysutil").unwrap();

        let module = pipeline.get_system_module("cellSysutil").unwrap();
        assert_eq!(module.state, ModuleState::Stopped);
    }

    #[test]
    fn test_game_pipeline_initialize_system_modules() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);

        pipeline.initialize_system_modules().unwrap();
        assert!(!pipeline.system_modules().is_empty());

        // Check core modules are loaded
        assert!(pipeline.get_system_module("cellSysutil").is_some());
        assert!(pipeline.get_system_module("cellGcmSys").is_some());
        assert!(pipeline.get_system_module("cellFs").is_some());
    }

    #[test]
    fn test_game_pipeline_setup_memory_layout() {
        let memory = MemoryManager::new().unwrap();
        let pipeline = GamePipeline::new(memory);

        let layout = pipeline.setup_memory_layout().unwrap();
        assert_eq!(layout.main_memory_base, 0x0000_0000);
        assert_eq!(layout.user_memory_base, 0x2000_0000);
        assert_eq!(layout.stack_base, 0xD000_0000);
    }

    #[test]
    fn test_game_pipeline_call_hle_function() {
        let memory = MemoryManager::new().unwrap();
        let pipeline = GamePipeline::new(memory);

        // Test calling a registered function
        let result = pipeline.call_hle_function("cellGcmSys", 0x21AC3697, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
    
    #[test]
    fn test_module_dependencies() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        // Load cellGcmSys which depends on cellSysutil
        pipeline.load_module("cellGcmSys").unwrap();
        
        // cellSysutil should have been loaded as a dependency
        assert!(pipeline.get_system_module("cellSysutil").is_some());
        
        // cellGcmSys should have cellSysutil as a dependency
        let gcm = pipeline.get_system_module("cellGcmSys").unwrap();
        assert!(gcm.dependencies.contains(&"cellSysutil".to_string()));
        
        // cellSysutil should have cellGcmSys as a dependent
        let sysutil = pipeline.get_system_module("cellSysutil").unwrap();
        assert!(sysutil.dependents.contains(&"cellGcmSys".to_string()));
    }
    
    #[test]
    fn test_module_dependency_start_order() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        // Load cellGcmSys which depends on cellSysutil
        pipeline.load_module("cellGcmSys").unwrap();
        
        // Start cellGcmSys - should also start cellSysutil
        pipeline.start_module("cellGcmSys").unwrap();
        
        // Both should be running
        assert!(pipeline.is_module_running("cellSysutil"));
        assert!(pipeline.is_module_running("cellGcmSys"));
    }
    
    #[test]
    fn test_module_dependency_stop_order() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        // Load and start modules
        pipeline.load_module("cellGcmSys").unwrap();
        pipeline.start_all_modules().unwrap();
        
        // Stop cellSysutil - should also stop cellGcmSys (dependent)
        pipeline.stop_module("cellSysutil").unwrap();
        
        // Both should be stopped
        assert!(!pipeline.is_module_running("cellSysutil"));
        assert!(!pipeline.is_module_running("cellGcmSys"));
    }
    
    #[test]
    fn test_stop_all_modules() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        pipeline.initialize_system_modules().unwrap();
        pipeline.start_all_modules().unwrap();
        
        // All modules should be running
        assert!(pipeline.is_module_running("cellSysutil"));
        
        pipeline.stop_all_modules().unwrap();
        
        // All modules should be stopped
        for (_, module) in pipeline.system_modules() {
            assert!(module.state == ModuleState::Stopped || module.state == ModuleState::Loaded);
        }
    }
    
    #[test]
    fn test_restart_module() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        pipeline.load_module("cellSysutil").unwrap();
        pipeline.start_module("cellSysutil").unwrap();
        
        assert!(pipeline.is_module_running("cellSysutil"));
        
        pipeline.restart_module("cellSysutil").unwrap();
        
        assert!(pipeline.is_module_running("cellSysutil"));
    }
    
    #[test]
    fn test_unload_module_with_dependents() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        // Load cellGcmSys which depends on cellSysutil
        pipeline.load_module("cellGcmSys").unwrap();
        
        // Try to unload cellSysutil - should fail because cellGcmSys depends on it
        let result = pipeline.unload_module("cellSysutil");
        assert!(result.is_err());
        
        // First unload cellGcmSys
        pipeline.unload_module("cellGcmSys").unwrap();
        
        // Now unloading cellSysutil should work
        pipeline.unload_module("cellSysutil").unwrap();
    }
    
    #[test]
    fn test_get_module_dependencies() {
        let memory = MemoryManager::new().unwrap();
        let pipeline = GamePipeline::new(memory);
        
        let deps = pipeline.get_module_dependencies("cellGcmSys");
        assert!(deps.contains(&"cellSysutil".to_string()));
        
        let deps = pipeline.get_module_dependencies("cellGame");
        assert!(deps.contains(&"cellSysutil".to_string()));
        assert!(deps.contains(&"cellFs".to_string()));
    }
    
    #[test]
    fn test_are_dependencies_satisfied() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        // Before loading any modules, cellGcmSys deps are not satisfied
        assert!(!pipeline.are_dependencies_satisfied("cellGcmSys"));
        
        // Load cellSysutil
        pipeline.load_module("cellSysutil").unwrap();
        
        // Now cellGcmSys deps are satisfied
        assert!(pipeline.are_dependencies_satisfied("cellGcmSys"));
    }
    
    #[test]
    fn test_setup_main_thread_stack() {
        let memory = MemoryManager::new().unwrap();
        let pipeline = GamePipeline::new(memory);
        
        let stack_size = 0x100000;  // 1MB stack
        let stack_info = pipeline.setup_main_thread_stack(stack_size).unwrap();
        
        assert_eq!(stack_info.stack_base, 0xD0000000);
        assert_eq!(stack_info.stack_size, stack_size);
        assert_eq!(stack_info.stack_top, 0xD0000000 + stack_size);
        assert!(stack_info.initial_sp < stack_info.stack_top);
        assert!(stack_info.initial_sp > stack_info.stack_base);
    }
    
    #[test]
    fn test_configure_tls_areas() {
        let memory = MemoryManager::new().unwrap();
        let pipeline = GamePipeline::new(memory);
        
        let tls_size = 0x10000;  // 64KB per thread
        let max_threads = 8;
        let tls_info = pipeline.configure_tls_areas(tls_size, max_threads).unwrap();
        
        assert_eq!(tls_info.base_address, 0x28000000);  // In user memory region
        assert!(tls_info.size_per_thread >= tls_size);  // Should be aligned
        assert_eq!(tls_info.max_threads, max_threads);
        assert_eq!(tls_info.thread_areas.len(), max_threads as usize);
        
        // Check first thread area
        assert_eq!(tls_info.thread_areas[0].address, 0x28000000);
        assert_eq!(tls_info.thread_areas[0].thread_index, 0);
    }
    
    #[test]
    fn test_initialize_kernel_objects() {
        let memory = MemoryManager::new().unwrap();
        let mut pipeline = GamePipeline::new(memory);
        
        let kernel_info = pipeline.initialize_kernel_objects().unwrap();
        
        assert!(kernel_info.initialized);
        assert!(kernel_info.mutex_count > 0);
        assert!(kernel_info.semaphore_count > 0);
        assert!(kernel_info.event_queue_count > 0);
    }
}
