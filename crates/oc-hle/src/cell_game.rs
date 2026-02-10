//! cellGame HLE - Game Data Management
//!
//! This module provides HLE implementations for PS3 game data access,
//! including disc content, digital content, and game directories.

use std::collections::HashMap;
use tracing::{debug, trace, warn};
use crate::memory::{write_be32, write_be64, write_string};

/// Game data type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellGameDataType {
    /// Game disc
    Disc = 1,
    /// HDD game
    Hdd = 2,
    /// Home (digital)
    Home = 3,
}

/// Game attribute flags
pub const CELL_GAME_ATTRIBUTE_PATCH: u32 = 1;
pub const CELL_GAME_ATTRIBUTE_APP_HOME: u32 = 2;
pub const CELL_GAME_ATTRIBUTE_DEBUG: u32 = 4;

/// Parameter IDs for PARAM.SFO
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellGameParamId {
    /// Title
    Title = 0,
    /// Title ID
    TitleId = 1,
    /// Version
    Version = 2,
    /// Parental level
    ParentalLevel = 3,
    /// Resolution
    Resolution = 4,
    /// Sound format
    SoundFormat = 5,
}

/// Game content size
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGameContentSize {
    /// HDD free size (KB)
    pub hdd_free_size: u64,
    /// Size required for game (KB)
    pub size_kb: u64,
    /// System size (KB)
    pub sys_size_kb: u64,
}

impl Default for CellGameContentSize {
    fn default() -> Self {
        Self {
            hdd_free_size: 100 * 1024 * 1024, // 100 GB
            size_kb: 0,
            sys_size_kb: 0,
        }
    }
}

/// Game set initial info
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGameSetInitParams {
    /// Title
    pub title: [u8; 128],
    /// Title ID
    pub title_id: [u8; 10],
    /// Version
    pub version: [u8; 6],
}

/// PARAM.SFO entry
#[derive(Debug, Clone)]
pub struct ParamSfoEntry {
    /// Parameter key name
    pub key: String,
    /// Parameter value (string or integer)
    pub value: ParamSfoValue,
}

/// PARAM.SFO value types
#[derive(Debug, Clone)]
pub enum ParamSfoValue {
    /// String value (UTF-8)
    String(String),
    /// Integer value (32-bit)
    Integer(i32),
}

/// Game installation state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameInstallState {
    /// Not installed
    #[default]
    NotInstalled = 0,
    /// Installation in progress
    Installing = 1,
    /// Installed
    Installed = 2,
    /// Installation failed
    Failed = 3,
}

/// Game installation info
#[derive(Debug, Clone, Default)]
pub struct GameInstallInfo {
    /// Installation state
    pub state: GameInstallState,
    /// Installation progress (0-100)
    pub progress: u32,
    /// Total size in KB
    pub total_size_kb: u64,
    /// Installed size in KB
    pub installed_size_kb: u64,
    /// Error code (if failed)
    pub error_code: i32,
}

/// Game update state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameUpdateState {
    /// No update available
    #[default]
    NoUpdate = 0,
    /// Update available
    Available = 1,
    /// Update downloading
    Downloading = 2,
    /// Update ready to install
    Ready = 3,
    /// Update installing
    Installing = 4,
    /// Update installed
    Installed = 5,
    /// Update failed
    Failed = 6,
}

/// Game update info
#[derive(Debug, Clone, Default)]
pub struct GameUpdateInfo {
    /// Update state
    pub state: GameUpdateState,
    /// Update version
    pub version: String,
    /// Update size in KB
    pub size_kb: u64,
    /// Download progress (0-100)
    pub download_progress: u32,
    /// Installation progress (0-100)
    pub install_progress: u32,
    /// Update URL
    pub url: String,
    /// Error code (if failed)
    pub error_code: i32,
}

// ============================================================================
// DLC (Downloadable Content) Handling
// ============================================================================

/// DLC state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DlcState {
    /// Not installed
    #[default]
    NotInstalled = 0,
    /// Installed
    Installed = 1,
    /// Downloading
    Downloading = 2,
    /// Installing
    Installing = 3,
    /// Failed
    Failed = 4,
}

/// DLC entry information
#[derive(Debug, Clone)]
pub struct DlcEntry {
    /// DLC content ID
    pub content_id: String,
    /// DLC name/title
    pub name: String,
    /// DLC description
    pub description: String,
    /// DLC version
    pub version: String,
    /// DLC size in KB
    pub size_kb: u64,
    /// DLC state
    pub state: DlcState,
    /// Install path
    pub install_path: String,
    /// License valid
    pub licensed: bool,
    /// Entitlement ID
    pub entitlement_id: u64,
}

impl Default for DlcEntry {
    fn default() -> Self {
        Self {
            content_id: String::new(),
            name: String::new(),
            description: String::new(),
            version: "01.00".to_string(),
            size_kb: 0,
            state: DlcState::NotInstalled,
            install_path: String::new(),
            licensed: false,
            entitlement_id: 0,
        }
    }
}

/// DLC manager state
#[derive(Debug, Clone, Default)]
pub struct DlcManagerState {
    /// Registered DLC entries
    pub entries: Vec<DlcEntry>,
    /// Currently downloading DLC content ID
    pub downloading_id: Option<String>,
    /// Download progress (0-100)
    pub download_progress: u32,
}

// ============================================================================
// Game Patch Handling
// ============================================================================

/// Game patch state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatchState {
    /// No patch detected
    #[default]
    None = 0,
    /// Patch detected and pending merge
    Detected = 1,
    /// Patch merge in progress
    Merging = 2,
    /// Patch merged successfully
    Applied = 3,
    /// Patch merge failed
    Failed = 4,
}

/// Information about a detected game patch
#[derive(Debug, Clone, Default)]
pub struct PatchInfo {
    /// Patch state
    pub state: PatchState,
    /// Patch version string (e.g. "01.02")
    pub version: String,
    /// Patch source path (where patch files reside)
    pub source_path: String,
    /// Patch size in KB
    pub size_kb: u64,
    /// Number of files in the patch
    pub file_count: u32,
    /// Number of files merged so far
    pub merged_count: u32,
    /// Error code (if failed)
    pub error_code: i32,
}

// ============================================================================
// Content Info for DLC Enumeration
// ============================================================================

/// Content info entry returned by cellGameGetContentInfoList
#[derive(Debug, Clone)]
pub struct ContentInfoEntry {
    /// Content ID string
    pub content_id: String,
    /// Content path on HDD
    pub path: String,
    /// Content size in KB
    pub size_kb: u64,
    /// Is installed
    pub installed: bool,
}

/// Game manager
pub struct GameManager {
    /// Current game data type
    game_type: CellGameDataType,
    /// Game attributes
    attributes: u32,
    /// Content size information
    content_size: CellGameContentSize,
    /// Game directory name
    dir_name: String,
    /// PARAM.SFO integer parameters
    param_int: HashMap<u32, i32>,
    /// PARAM.SFO string parameters
    param_string: HashMap<u32, String>,
    /// Initialization flag
    initialized: bool,
    /// Raw PARAM.SFO entries
    param_sfo_entries: Vec<ParamSfoEntry>,
    /// PARAM.SFO loaded flag
    param_sfo_loaded: bool,
    /// Game installation info
    install_info: GameInstallInfo,
    /// Content info path
    content_info_path: String,
    /// User directory path
    usrdir_path: String,
    /// Game update info
    update_info: GameUpdateInfo,
    /// DLC manager state
    dlc_state: DlcManagerState,
    /// Game patch info
    patch_info: PatchInfo,
}

impl GameManager {
    /// Create a new game manager
    pub fn new() -> Self {
        let mut manager = Self {
            game_type: CellGameDataType::Hdd,
            attributes: 0,
            content_size: CellGameContentSize::default(),
            dir_name: String::new(),
            param_int: HashMap::new(),
            param_string: HashMap::new(),
            initialized: false,
            param_sfo_entries: Vec::new(),
            param_sfo_loaded: false,
            install_info: GameInstallInfo::default(),
            content_info_path: String::new(),
            usrdir_path: String::new(),
            update_info: GameUpdateInfo::default(),
            dlc_state: DlcManagerState::default(),
            patch_info: PatchInfo::default(),
        };
        
        // Initialize default parameters
        manager.init_default_params();
        manager
    }

    /// Initialize default PARAM.SFO parameters
    fn init_default_params(&mut self) {
        // String parameters
        self.param_string.insert(
            CellGameParamId::Title as u32,
            "Test Game".to_string(),
        );
        self.param_string.insert(
            CellGameParamId::TitleId as u32,
            "BLUS00000".to_string(),
        );
        self.param_string.insert(
            CellGameParamId::Version as u32,
            "01.00".to_string(),
        );

        // Integer parameters
        self.param_int.insert(CellGameParamId::ParentalLevel as u32, 0);
        self.param_int.insert(CellGameParamId::Resolution as u32, 1080); // 1080p
        self.param_int.insert(CellGameParamId::SoundFormat as u32, 1); // LPCM 2.0
    }

    /// Boot check - detect game type and attributes
    pub fn boot_check(&mut self) -> i32 {
        debug!("GameManager::boot_check");
        
        // Detect game type (defaulting to HDD game)
        self.game_type = CellGameDataType::Hdd;
        self.attributes = 0;
        self.dir_name = "GAME00000".to_string();
        self.initialized = true;
        
        // Set up paths
        self.content_info_path = format!("/dev_hdd0/game/{}", self.dir_name);
        self.usrdir_path = format!("/dev_hdd0/game/{}/USRDIR", self.dir_name);

        0 // CELL_OK
    }

    /// Check game data
    pub fn data_check(&mut self, data_type: CellGameDataType, dir_name: &str) -> i32 {
        debug!("GameManager::data_check: type={:?}, dir={}", data_type, dir_name);
        
        self.game_type = data_type;
        self.dir_name = dir_name.to_string();

        // Check if game data exists
        let game_exists = self.game_data_exists(dir_name);
        debug!("GameManager::data_check: game_exists={}", game_exists);
        
        // Calculate actual content size from directory
        let (size_kb, sys_size_kb) = self.calculate_content_size(dir_name);
        self.content_size.size_kb = size_kb;
        self.content_size.sys_size_kb = sys_size_kb;
        
        debug!(
            "GameManager::data_check: size_kb={}, sys_size_kb={}",
            size_kb, sys_size_kb
        );

        0 // CELL_OK
    }
    
    /// Check if game data exists for the given directory name
    pub fn game_data_exists(&self, dir_name: &str) -> bool {
        // Check in HDD game directory
        let hdd_path = format!("/dev_hdd0/game/{}", dir_name);
        
        // Try to detect via standard path checking
        // In a real implementation, this would query the VFS
        let host_path = self.get_host_game_path(dir_name);
        if let Some(path) = host_path {
            let path_buf = std::path::Path::new(&path);
            if path_buf.exists() {
                return true;
            }
        }
        
        // Check via internal state - if we've already done boot_check, game exists
        if self.initialized && self.dir_name == dir_name {
            return true;
        }
        
        // For HLE compatibility, simulate that disc/HDD games exist
        debug!("GameManager::game_data_exists: checking {}", hdd_path);
        true // Default to exists for HLE compatibility
    }
    
    /// Calculate actual content size for a game directory
    /// 
    /// Returns (size_kb, sys_size_kb) tuple
    fn calculate_content_size(&self, dir_name: &str) -> (u64, u64) {
        // Try to get actual size from host filesystem
        if let Some(host_path) = self.get_host_game_path(dir_name) {
            let path = std::path::Path::new(&host_path);
            if path.exists() {
                let total_size = Self::calculate_directory_size(path);
                let size_kb = (total_size + 1023) / 1024; // Round up to KB
                
                // System size is typically ~10% of total or minimum 100MB
                let sys_size_kb = std::cmp::max(size_kb / 10, 100 * 1024);
                
                debug!(
                    "GameManager::calculate_content_size: host path {} = {} KB",
                    host_path, size_kb
                );
                return (size_kb, sys_size_kb);
            }
        }
        
        // Default simulated sizes if no host path or calculation fails
        let size_kb = 1024 * 1024; // 1 GB default
        let sys_size_kb = 100 * 1024; // 100 MB default
        
        (size_kb, sys_size_kb)
    }
    
    /// Calculate total size of a directory recursively
    fn calculate_directory_size(path: &std::path::Path) -> u64 {
        let mut total_size = 0u64;
        
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    if let Ok(metadata) = entry_path.metadata() {
                        total_size += metadata.len();
                    }
                } else if entry_path.is_dir() {
                    total_size += Self::calculate_directory_size(&entry_path);
                }
            }
        }
        
        total_size
    }
    
    /// Get host filesystem path for game directory
    /// 
    /// This resolves the virtual game path to a host path if available
    fn get_host_game_path(&self, dir_name: &str) -> Option<String> {
        // Check for environment variable override first
        if let Ok(base_path) = std::env::var("OXIDIZED_CELL_GAMES") {
            let path = format!("{}/{}", base_path, dir_name);
            return Some(path);
        }
        
        // Try HOME-based path on Unix or LOCALAPPDATA on Windows
        #[cfg(target_family = "unix")]
        if let Ok(home) = std::env::var("HOME") {
            let path = format!("{}/.local/share/oxidized-cell/games/{}", home, dir_name);
            return Some(path);
        }
        
        #[cfg(target_family = "windows")]
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            let path = format!("{}\\oxidized-cell\\games\\{}", appdata, dir_name);
            return Some(path);
        }
        
        None
    }

    /// Get game type
    pub fn get_game_type(&self) -> CellGameDataType {
        self.game_type
    }

    /// Get attributes
    pub fn get_attributes(&self) -> u32 {
        self.attributes
    }

    /// Get content size
    pub fn get_content_size(&self) -> CellGameContentSize {
        self.content_size
    }

    /// Get directory name
    pub fn get_dir_name(&self) -> &str {
        &self.dir_name
    }

    /// Get integer parameter
    pub fn get_param_int(&self, id: u32) -> Option<i32> {
        self.param_int.get(&id).copied()
    }

    /// Get string parameter
    pub fn get_param_string(&self, id: u32) -> Option<&str> {
        self.param_string.get(&id).map(|s| s.as_str())
    }

    /// Set integer parameter
    pub fn set_param_int(&mut self, id: u32, value: i32) {
        self.param_int.insert(id, value);
    }

    /// Set string parameter
    pub fn set_param_string(&mut self, id: u32, value: String) {
        self.param_string.insert(id, value);
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    // ========================================================================
    // PARAM.SFO Reading/Writing
    // ========================================================================

    /// Load PARAM.SFO from data
    /// 
    /// This parses raw PARAM.SFO binary data and populates the parameter maps.
    /// In a real implementation, this would be connected to oc-vfs.
    pub fn load_param_sfo(&mut self, data: &[u8]) -> i32 {
        debug!("GameManager::load_param_sfo: {} bytes", data.len());
        
        // PARAM.SFO header structure:
        // 0x00: Magic (0x00505346 = "\0PSF")
        // 0x04: Version
        // 0x08: Key table start
        // 0x0C: Data table start
        // 0x10: Table entries
        
        if data.len() < 20 {
            return 0x8002b101u32 as i32; // CELL_GAME_ERROR_PARAM
        }
        
        // Check magic
        if data[0..4] != [0x00, 0x50, 0x53, 0x46] {
            return 0x8002b101u32 as i32; // Invalid magic
        }
        
        // For now, we use default parameters
        // Real implementation would parse the binary format
        self.param_sfo_loaded = true;
        
        debug!("GameManager: PARAM.SFO loaded (using defaults for HLE)");
        
        0 // CELL_OK
    }

    /// Save PARAM.SFO to binary data
    /// 
    /// This generates PARAM.SFO binary data from the current parameters.
    pub fn save_param_sfo(&self) -> Result<Vec<u8>, i32> {
        debug!("GameManager::save_param_sfo");
        
        // Build PARAM.SFO binary format
        // For HLE, we generate a minimal valid PARAM.SFO
        
        let mut data = Vec::new();
        
        // Magic: "\0PSF"
        data.extend_from_slice(&[0x00, 0x50, 0x53, 0x46]);
        
        // Version (1.1)
        data.extend_from_slice(&[0x01, 0x01, 0x00, 0x00]);
        
        // Key table offset (placeholder)
        data.extend_from_slice(&[0x14, 0x00, 0x00, 0x00]);
        
        // Data table offset (placeholder)
        data.extend_from_slice(&[0x30, 0x00, 0x00, 0x00]);
        
        // Entry count
        let entry_count = (self.param_string.len() + self.param_int.len()) as u32;
        data.extend_from_slice(&entry_count.to_le_bytes());
        
        debug!("GameManager: Generated PARAM.SFO stub with {} entries", entry_count);
        
        Ok(data)
    }

    /// Get a PARAM.SFO entry by key name
    pub fn get_param_sfo_entry(&self, key: &str) -> Option<&ParamSfoEntry> {
        self.param_sfo_entries.iter().find(|e| e.key == key)
    }

    /// Add or update a PARAM.SFO entry
    pub fn set_param_sfo_entry(&mut self, key: &str, value: ParamSfoValue) {
        // Update existing entry or add new one
        if let Some(entry) = self.param_sfo_entries.iter_mut().find(|e| e.key == key) {
            entry.value = value;
        } else {
            self.param_sfo_entries.push(ParamSfoEntry {
                key: key.to_string(),
                value,
            });
        }
        
        // Also update the typed parameter maps
        match &self.param_sfo_entries.last().unwrap().value {
            ParamSfoValue::String(s) => {
                // Map common keys to param IDs
                let param_id = match key {
                    "TITLE" => Some(CellGameParamId::Title as u32),
                    "TITLE_ID" => Some(CellGameParamId::TitleId as u32),
                    "VERSION" => Some(CellGameParamId::Version as u32),
                    _ => None,
                };
                if let Some(id) = param_id {
                    self.param_string.insert(id, s.clone());
                }
            }
            ParamSfoValue::Integer(i) => {
                let param_id = match key {
                    "PARENTAL_LEVEL" => Some(CellGameParamId::ParentalLevel as u32),
                    "RESOLUTION" => Some(CellGameParamId::Resolution as u32),
                    "SOUND_FORMAT" => Some(CellGameParamId::SoundFormat as u32),
                    _ => None,
                };
                if let Some(id) = param_id {
                    self.param_int.insert(id, *i);
                }
            }
        }
    }

    /// Check if PARAM.SFO is loaded
    pub fn is_param_sfo_loaded(&self) -> bool {
        self.param_sfo_loaded
    }

    /// Get content info path
    pub fn get_content_info_path(&self) -> &str {
        &self.content_info_path
    }

    /// Get user directory path
    pub fn get_usrdir_path(&self) -> &str {
        &self.usrdir_path
    }

    // ========================================================================
    // Game Data Installation
    // ========================================================================

    /// Start game data installation
    pub fn start_installation(&mut self, source_path: &str, total_size_kb: u64) -> i32 {
        if self.install_info.state == GameInstallState::Installing {
            return 0x8002b104u32 as i32; // Already installing
        }
        
        debug!(
            "GameManager::start_installation: source={}, size={} KB",
            source_path, total_size_kb
        );
        
        self.install_info = GameInstallInfo {
            state: GameInstallState::Installing,
            progress: 0,
            total_size_kb,
            installed_size_kb: 0,
            error_code: 0,
        };
        
        0 // CELL_OK
    }

    /// Update installation progress
    pub fn update_installation_progress(&mut self, installed_kb: u64) -> i32 {
        if self.install_info.state != GameInstallState::Installing {
            return 0x8002b101u32 as i32; // Not installing
        }
        
        self.install_info.installed_size_kb = installed_kb;
        
        if self.install_info.total_size_kb > 0 {
            self.install_info.progress = 
                ((installed_kb * 100) / self.install_info.total_size_kb) as u32;
            self.install_info.progress = self.install_info.progress.min(100);
        }
        
        trace!(
            "GameManager: installation progress {}% ({}/{} KB)",
            self.install_info.progress,
            installed_kb,
            self.install_info.total_size_kb
        );
        
        0 // CELL_OK
    }

    /// Complete installation
    pub fn complete_installation(&mut self) -> i32 {
        if self.install_info.state != GameInstallState::Installing {
            return 0x8002b101u32 as i32; // Not installing
        }
        
        debug!("GameManager::complete_installation");
        
        self.install_info.state = GameInstallState::Installed;
        self.install_info.progress = 100;
        self.install_info.installed_size_kb = self.install_info.total_size_kb;
        
        0 // CELL_OK
    }

    /// Fail installation with error
    pub fn fail_installation(&mut self, error_code: i32) -> i32 {
        debug!("GameManager::fail_installation: error=0x{:08X}", error_code);
        
        self.install_info.state = GameInstallState::Failed;
        self.install_info.error_code = error_code;
        
        0 // CELL_OK
    }

    /// Get installation state
    pub fn get_install_state(&self) -> GameInstallState {
        self.install_info.state
    }

    /// Get installation progress (0-100)
    pub fn get_install_progress(&self) -> u32 {
        self.install_info.progress
    }

    /// Get installation info
    pub fn get_install_info(&self) -> &GameInstallInfo {
        &self.install_info
    }

    /// Check if game is installed
    pub fn is_installed(&self) -> bool {
        self.install_info.state == GameInstallState::Installed
    }

    /// Reset installation state
    pub fn reset_installation(&mut self) {
        self.install_info = GameInstallInfo::default();
    }

    // ========================================================================
    // Game Update Handling
    // ========================================================================

    /// Check for game updates
    pub fn check_for_updates(&mut self) -> i32 {
        debug!("GameManager::check_for_updates");
        
        // In a real implementation, this would:
        // 1. Connect to update server
        // 2. Check current game version against available versions
        // 3. Download update metadata
        
        // For HLE, we simulate no updates available by default
        self.update_info.state = GameUpdateState::NoUpdate;
        
        0 // CELL_OK
    }

    /// Start update download
    pub fn start_update_download(&mut self, url: &str, size_kb: u64, version: &str) -> i32 {
        if self.update_info.state == GameUpdateState::Downloading 
            || self.update_info.state == GameUpdateState::Installing {
            return 0x8002b105u32 as i32; // Already in progress
        }
        
        debug!(
            "GameManager::start_update_download: url={}, size={} KB, version={}",
            url, size_kb, version
        );
        
        self.update_info = GameUpdateInfo {
            state: GameUpdateState::Downloading,
            version: version.to_string(),
            size_kb,
            download_progress: 0,
            install_progress: 0,
            url: url.to_string(),
            error_code: 0,
        };
        
        0 // CELL_OK
    }

    /// Update download progress
    pub fn update_download_progress(&mut self, downloaded_kb: u64) -> i32 {
        if self.update_info.state != GameUpdateState::Downloading {
            return 0x8002b101u32 as i32; // Not downloading
        }
        
        if self.update_info.size_kb > 0 {
            self.update_info.download_progress = 
                ((downloaded_kb * 100) / self.update_info.size_kb) as u32;
            self.update_info.download_progress = self.update_info.download_progress.min(100);
        }
        
        trace!(
            "GameManager: download progress {}% ({}/{} KB)",
            self.update_info.download_progress,
            downloaded_kb,
            self.update_info.size_kb
        );
        
        0 // CELL_OK
    }

    /// Complete update download
    pub fn complete_update_download(&mut self) -> i32 {
        if self.update_info.state != GameUpdateState::Downloading {
            return 0x8002b101u32 as i32; // Not downloading
        }
        
        debug!("GameManager::complete_update_download");
        
        self.update_info.state = GameUpdateState::Ready;
        self.update_info.download_progress = 100;
        
        0 // CELL_OK
    }

    /// Start update installation
    pub fn start_update_installation(&mut self) -> i32 {
        if self.update_info.state != GameUpdateState::Ready {
            return 0x8002b101u32 as i32; // Update not ready
        }
        
        debug!("GameManager::start_update_installation: version={}", self.update_info.version);
        
        self.update_info.state = GameUpdateState::Installing;
        self.update_info.install_progress = 0;
        
        0 // CELL_OK
    }

    /// Update installation progress
    pub fn update_install_progress(&mut self, progress: u32) -> i32 {
        if self.update_info.state != GameUpdateState::Installing {
            return 0x8002b101u32 as i32; // Not installing
        }
        
        self.update_info.install_progress = progress.min(100);
        
        trace!(
            "GameManager: update installation progress {}%",
            self.update_info.install_progress
        );
        
        0 // CELL_OK
    }

    /// Complete update installation
    pub fn complete_update_installation(&mut self) -> i32 {
        if self.update_info.state != GameUpdateState::Installing {
            return 0x8002b101u32 as i32; // Not installing
        }
        
        debug!("GameManager::complete_update_installation: version={}", self.update_info.version);
        
        self.update_info.state = GameUpdateState::Installed;
        self.update_info.install_progress = 100;
        
        // Update game version
        self.param_string.insert(
            CellGameParamId::Version as u32,
            self.update_info.version.clone(),
        );
        
        0 // CELL_OK
    }

    /// Fail update with error
    pub fn fail_update(&mut self, error_code: i32) -> i32 {
        debug!("GameManager::fail_update: error=0x{:08X}", error_code);
        
        self.update_info.state = GameUpdateState::Failed;
        self.update_info.error_code = error_code;
        
        0 // CELL_OK
    }

    /// Get update state
    pub fn get_update_state(&self) -> GameUpdateState {
        self.update_info.state
    }

    /// Get update info
    pub fn get_update_info(&self) -> &GameUpdateInfo {
        &self.update_info
    }

    /// Check if update is available
    pub fn is_update_available(&self) -> bool {
        self.update_info.state == GameUpdateState::Available
    }

    /// Reset update state
    pub fn reset_update(&mut self) {
        self.update_info = GameUpdateInfo::default();
    }

    // ========================================================================
    // DLC (Downloadable Content) Handling
    // ========================================================================

    /// Register a DLC entry
    pub fn register_dlc(&mut self, content_id: &str, name: &str, size_kb: u64) -> i32 {
        // Check if already registered
        if self.dlc_state.entries.iter().any(|e| e.content_id == content_id) {
            return 0x8002b104u32 as i32; // Already registered
        }

        debug!(
            "GameManager::register_dlc: content_id={}, name={}, size={} KB",
            content_id, name, size_kb
        );

        let entry = DlcEntry {
            content_id: content_id.to_string(),
            name: name.to_string(),
            size_kb,
            ..DlcEntry::default()
        };

        self.dlc_state.entries.push(entry);

        0 // CELL_OK
    }

    /// Get DLC entry by content ID
    pub fn get_dlc(&self, content_id: &str) -> Option<&DlcEntry> {
        self.dlc_state.entries.iter().find(|e| e.content_id == content_id)
    }

    /// Get mutable DLC entry by content ID
    pub fn get_dlc_mut(&mut self, content_id: &str) -> Option<&mut DlcEntry> {
        self.dlc_state.entries.iter_mut().find(|e| e.content_id == content_id)
    }

    /// List all DLC entries
    pub fn list_dlc(&self) -> &[DlcEntry] {
        &self.dlc_state.entries
    }

    /// Get installed DLC entries
    pub fn list_installed_dlc(&self) -> Vec<&DlcEntry> {
        self.dlc_state.entries.iter()
            .filter(|e| e.state == DlcState::Installed)
            .collect()
    }

    /// Get DLC count
    pub fn dlc_count(&self) -> usize {
        self.dlc_state.entries.len()
    }

    /// Get installed DLC count
    pub fn installed_dlc_count(&self) -> usize {
        self.dlc_state.entries.iter()
            .filter(|e| e.state == DlcState::Installed)
            .count()
    }

    /// Check if DLC is installed
    pub fn is_dlc_installed(&self, content_id: &str) -> bool {
        self.get_dlc(content_id)
            .map(|e| e.state == DlcState::Installed)
            .unwrap_or(false)
    }

    /// Check if DLC is licensed (user owns it)
    pub fn is_dlc_licensed(&self, content_id: &str) -> bool {
        self.get_dlc(content_id)
            .map(|e| e.licensed)
            .unwrap_or(false)
    }

    /// Set DLC license status
    pub fn set_dlc_licensed(&mut self, content_id: &str, licensed: bool) -> i32 {
        if let Some(entry) = self.get_dlc_mut(content_id) {
            entry.licensed = licensed;
            debug!("GameManager::set_dlc_licensed: {} = {}", content_id, licensed);
            0 // CELL_OK
        } else {
            0x8002b101u32 as i32 // Not found
        }
    }

    /// Start DLC download
    pub fn start_dlc_download(&mut self, content_id: &str) -> i32 {
        if self.dlc_state.downloading_id.is_some() {
            return 0x8002b104u32 as i32; // Already downloading
        }

        let entry = match self.get_dlc_mut(content_id) {
            Some(e) => e,
            None => return 0x8002b101u32 as i32, // Not found
        };

        if entry.state == DlcState::Installed {
            return 0x8002b104u32 as i32; // Already installed
        }

        debug!("GameManager::start_dlc_download: {}", content_id);

        entry.state = DlcState::Downloading;
        self.dlc_state.downloading_id = Some(content_id.to_string());
        self.dlc_state.download_progress = 0;

        0 // CELL_OK
    }

    /// Update DLC download progress
    pub fn update_dlc_download_progress(&mut self, progress: u32) -> i32 {
        if self.dlc_state.downloading_id.is_none() {
            return 0x8002b101u32 as i32; // Not downloading
        }

        self.dlc_state.download_progress = progress.min(100);
        trace!("GameManager: DLC download progress {}%", self.dlc_state.download_progress);

        0 // CELL_OK
    }

    /// Complete DLC download and install
    pub fn complete_dlc_download(&mut self) -> i32 {
        let content_id = match self.dlc_state.downloading_id.take() {
            Some(id) => id,
            None => return 0x8002b101u32 as i32, // Not downloading
        };

        // Clone dir_name to avoid borrow issue
        let dir_name = self.dir_name.clone();
        let install_path = format!("/dev_hdd0/game/{}/USRDIR/{}", dir_name, content_id);

        let entry = match self.get_dlc_mut(&content_id) {
            Some(e) => e,
            None => return 0x8002b101u32 as i32, // Not found
        };

        debug!("GameManager::complete_dlc_download: {}", content_id);

        entry.state = DlcState::Installed;
        entry.install_path = install_path;
        self.dlc_state.download_progress = 100;

        0 // CELL_OK
    }

    /// Fail DLC download
    pub fn fail_dlc_download(&mut self) -> i32 {
        let content_id = match self.dlc_state.downloading_id.take() {
            Some(id) => id,
            None => return 0x8002b101u32 as i32, // Not downloading
        };

        if let Some(entry) = self.get_dlc_mut(&content_id) {
            entry.state = DlcState::Failed;
        }

        debug!("GameManager::fail_dlc_download: {}", content_id);

        0 // CELL_OK
    }

    /// Remove DLC
    pub fn remove_dlc(&mut self, content_id: &str) -> i32 {
        let entry = match self.get_dlc_mut(content_id) {
            Some(e) => e,
            None => return 0x8002b101u32 as i32, // Not found
        };

        if entry.state != DlcState::Installed {
            return 0x8002b101u32 as i32; // Not installed
        }

        debug!("GameManager::remove_dlc: {}", content_id);

        entry.state = DlcState::NotInstalled;
        entry.install_path.clear();

        0 // CELL_OK
    }

    /// Get DLC download progress
    pub fn get_dlc_download_progress(&self) -> u32 {
        self.dlc_state.download_progress
    }

    /// Check if DLC is downloading
    pub fn is_dlc_downloading(&self) -> bool {
        self.dlc_state.downloading_id.is_some()
    }

    /// Get currently downloading DLC content ID
    pub fn get_downloading_dlc_id(&self) -> Option<&str> {
        self.dlc_state.downloading_id.as_deref()
    }

    // ========================================================================
    // Game Patch Detection and Merge
    // ========================================================================

    /// Detect if a patch exists for the current game
    ///
    /// Checks the standard PS3 patch directory structure:
    /// `/dev_hdd0/game/<TITLE_ID>/USRDIR/` overlay files
    pub fn detect_patch(&mut self) -> i32 {
        if self.dir_name.is_empty() {
            return 0x8002b101u32 as i32; // CELL_GAME_ERROR_PARAM
        }

        let patch_path = format!("/dev_hdd0/game/{}_patch", self.dir_name);

        debug!("GameManager::detect_patch: checking {}", patch_path);

        // Try to find patch directory on host filesystem
        if let Some(host_path) = self.get_host_game_path(&format!("{}_patch", self.dir_name)) {
            let path = std::path::Path::new(&host_path);
            if path.exists() && path.is_dir() {
                // Count patch files
                let file_count = Self::count_files_recursive(path);
                let size = Self::calculate_directory_size(path);
                let size_kb = (size + 1023) / 1024;

                // Try to read patch version from PARAM.SFO
                let version = self.read_patch_version(path);

                self.patch_info = PatchInfo {
                    state: PatchState::Detected,
                    version,
                    source_path: patch_path.clone(),
                    size_kb,
                    file_count,
                    merged_count: 0,
                    error_code: 0,
                };

                // Set the PATCH attribute flag
                self.attributes |= CELL_GAME_ATTRIBUTE_PATCH;

                debug!(
                    "GameManager::detect_patch: found patch v{} ({} files, {} KB)",
                    self.patch_info.version, file_count, size_kb
                );

                return 0;
            }
        }

        // No patch found — this is not an error
        self.patch_info.state = PatchState::None;
        debug!("GameManager::detect_patch: no patch found");

        0 // CELL_OK
    }

    /// Apply a detected patch by merging files into the game directory
    ///
    /// In a real implementation this would copy/overlay each file from the
    /// patch directory into the base game directory.  For HLE we track the
    /// merge progress and update the version string.
    pub fn apply_patch(&mut self) -> i32 {
        if self.patch_info.state != PatchState::Detected {
            return 0x8002b101u32 as i32; // No patch to apply
        }

        debug!("GameManager::apply_patch: merging {} files", self.patch_info.file_count);

        self.patch_info.state = PatchState::Merging;

        // Simulate merging all files
        self.patch_info.merged_count = self.patch_info.file_count;
        self.patch_info.state = PatchState::Applied;

        // Update the game version to the patch version
        if !self.patch_info.version.is_empty() {
            self.param_string.insert(
                CellGameParamId::Version as u32,
                self.patch_info.version.clone(),
            );
        }

        debug!(
            "GameManager::apply_patch: patch applied, version now {}",
            self.patch_info.version
        );

        0 // CELL_OK
    }

    /// Get patch state
    pub fn get_patch_state(&self) -> PatchState {
        self.patch_info.state
    }

    /// Get patch info
    pub fn get_patch_info(&self) -> &PatchInfo {
        &self.patch_info
    }

    /// Check if a patch has been detected
    pub fn has_patch(&self) -> bool {
        self.patch_info.state != PatchState::None
    }

    /// Reset patch state
    pub fn reset_patch(&mut self) {
        self.patch_info = PatchInfo::default();
        self.attributes &= !CELL_GAME_ATTRIBUTE_PATCH;
    }

    /// Read patch version from a PARAM.SFO file in the patch directory
    fn read_patch_version(&self, patch_dir: &std::path::Path) -> String {
        let sfo_path = patch_dir.join("PARAM.SFO");
        if let Ok(data) = std::fs::read(&sfo_path) {
            // Minimal SFO parse: look for VERSION key's value
            // Full parsing is in load_param_sfo; here we just extract the version
            if data.len() > 20 && data[0..4] == [0x00, 0x50, 0x53, 0x46] {
                // For HLE we return a stub version; real impl would parse SFO
                return "01.01".to_string();
            }
        }
        // Default patch version
        "01.01".to_string()
    }

    /// Count files in a directory recursively
    fn count_files_recursive(path: &std::path::Path) -> u32 {
        let mut count = 0u32;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    count += 1;
                } else if p.is_dir() {
                    count += Self::count_files_recursive(&p);
                }
            }
        }
        count
    }

    // ========================================================================
    // Web Content Path
    // ========================================================================

    /// Get the local web content path for web-content titles
    ///
    /// Returns the path to the web content directory within the game's USRDIR.
    pub fn get_web_content_path(&self) -> String {
        if self.dir_name.is_empty() {
            return String::new();
        }
        format!("/dev_hdd0/game/{}/USRDIR/web", self.dir_name)
    }

    // ========================================================================
    // Content Info List (DLC Enumeration via HLE)
    // ========================================================================

    /// Get content info list for DLC enumeration
    ///
    /// Returns a list of all registered DLC content entries as
    /// `ContentInfoEntry` values suitable for the `cellGameGetContentInfoList` API.
    pub fn get_content_info_list(&self) -> Vec<ContentInfoEntry> {
        self.dlc_state.entries.iter().map(|dlc| {
            ContentInfoEntry {
                content_id: dlc.content_id.clone(),
                path: if dlc.install_path.is_empty() {
                    format!("/dev_hdd0/game/{}/USRDIR/dlc/{}", self.dir_name, dlc.content_id)
                } else {
                    dlc.install_path.clone()
                },
                size_kb: dlc.size_kb,
                installed: dlc.state == DlcState::Installed,
            }
        }).collect()
    }

    /// Get count of available content info entries
    pub fn get_content_info_count(&self) -> u32 {
        self.dlc_state.entries.len() as u32
    }
}

impl Default for GameManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellGameBootCheck - Check game boot status
///
/// # Arguments
/// * `type_addr` - Address to write game data type
/// * `attributes_addr` - Address to write attributes
/// * `size_addr` - Address to write content size structure
/// * `dirName_addr` - Address to write directory name
///
/// # Returns
/// * 0 on success
pub fn cell_game_boot_check(
    _type_addr: u32,
    _attributes_addr: u32,
    _size_addr: u32,
    _dir_name_addr: u32,
) -> i32 {
    debug!("cellGameBootCheck()");

    // Call through global game manager
    let result = crate::context::get_hle_context_mut().game.boot_check();
    if result != 0 {
        return result;
    }

    // Note: Writing to memory addresses requires memory subsystem integration
    // The values would be written to the provided addresses:
    // - game type to type_addr
    // - attributes to attributes_addr  
    // - content size to size_addr
    // - directory name to dir_name_addr

    0 // CELL_OK
}

/// cellGameDataCheck - Check game data
///
/// # Arguments
/// * `type` - Game data type
/// * `dirName` - Directory name
/// * `size_addr` - Address to write content size structure
///
/// # Returns
/// * 0 on success
pub fn cell_game_data_check(data_type: u32, _dir_name_addr: u32, _size_addr: u32) -> i32 {
    debug!("cellGameDataCheck(type={})", data_type);

    // Validate data type
    if !(1..=3).contains(&data_type) {
        return 0x8002b101u32 as i32; // CELL_GAME_ERROR_PARAM
    }

    // Convert data type
    let game_type = match data_type {
        1 => CellGameDataType::Disc,
        2 => CellGameDataType::Hdd,
        3 => CellGameDataType::Home,
        _ => return 0x8002b101u32 as i32,
    };

    // Check game data through global manager
    // Note: actual directory name reading requires memory access
    let result = crate::context::get_hle_context_mut().game.data_check(game_type, "GAME00000");
    if result != 0 {
        return result;
    }

    // Note: Writing content size to memory requires memory subsystem integration

    0 // CELL_OK
}

/// cellGameContentPermit - Set game content permissions
///
/// # Arguments
/// * `contentInfoPath_addr` - Address to write content info path
/// * `usrdirPath_addr` - Address to write user directory path
///
/// # Returns
/// * 0 on success
pub fn cell_game_content_permit(
    _content_info_path_addr: u32,
    _usrdir_path_addr: u32,
) -> i32 {
    debug!("cellGameContentPermit()");

    // Verify game manager is initialized
    if !crate::context::get_hle_context().game.is_initialized() {
        // Auto-initialize if not already done
        crate::context::get_hle_context_mut().game.boot_check();
    }

    // Note: Writing paths to memory requires memory subsystem integration
    // Content info path would be like "/dev_hdd0/game/GAME00000"
    // Usrdir path would be like "/dev_hdd0/game/GAME00000/USRDIR"

    0 // CELL_OK
}

/// cellGameContentErrorDialog - Show content error dialog
///
/// # Arguments
/// * `type` - Error type
/// * `errNeedSizeKB` - Required size in KB
/// * `dirName` - Directory name
///
/// # Returns
/// * 0 on success
pub fn cell_game_content_error_dialog(
    error_type: u32,
    err_need_size_kb: u64,
    _dir_name_addr: u32,
) -> i32 {
    debug!(
        "cellGameContentErrorDialog(type={}, needSize={} KB)",
        error_type, err_need_size_kb
    );

    // Note: Displaying dialog requires UI subsystem integration
    // For now, just log and return success

    0 // CELL_OK
}

/// cellGameGetParamInt - Get game parameter (integer)
///
/// # Arguments
/// * `id` - Parameter ID
/// * `value_addr` - Address to write value
///
/// # Returns
/// * 0 on success
pub fn cell_game_get_param_int(id: u32, _value_addr: u32) -> i32 {
    trace!("cellGameGetParamInt(id={})", id);

    // Get parameter from global game manager
    let _value = crate::context::get_hle_context().game.get_param_int(id);

    // Note: Writing value to memory requires memory subsystem integration

    0 // CELL_OK
}

/// cellGameGetParamString - Get game parameter (string)
///
/// # Arguments
/// * `id` - Parameter ID
/// * `buf_addr` - Buffer address
/// * `bufsize` - Buffer size
///
/// # Returns
/// * 0 on success
pub fn cell_game_get_param_string(id: u32, _buf_addr: u32, bufsize: u32) -> i32 {
    trace!("cellGameGetParamString(id={}, bufsize={})", id, bufsize);

    // Get parameter string from global game manager
    let _value = crate::context::get_hle_context().game.get_param_string(id);

    // Note: Writing string to memory requires memory subsystem integration

    0 // CELL_OK
}

/// cellGameGetLocalWebContentPath - Get local web content path
///
/// # Arguments
/// * `path_addr` - Address to write path (null-terminated string, max 1024 bytes)
///
/// # Returns
/// * 0 on success
pub fn cell_game_get_local_web_content_path(path_addr: u32) -> i32 {
    debug!("cellGameGetLocalWebContentPath(path_addr=0x{:08X})", path_addr);

    let ctx = crate::context::get_hle_context();
    let web_path = ctx.game.get_web_content_path();

    if web_path.is_empty() {
        warn!("cellGameGetLocalWebContentPath: game not initialized");
        return 0x8002b101u32 as i32; // CELL_GAME_ERROR_PARAM
    }

    if let Err(e) = write_string(path_addr, &web_path, 1024) {
        return e;
    }

    0 // CELL_OK
}

// ============================================================================
// DLC Functions
// ============================================================================

/// cellGameGetSizeKB - Get content size in KB
///
/// # Arguments
/// * `size_addr` - Address to write size
///
/// # Returns
/// * 0 on success
pub fn cell_game_get_size_kb(size_addr: u32) -> i32 {
    trace!("cellGameGetSizeKB(size_addr=0x{:08X})", size_addr);
    
    // Get content size from game manager
    let ctx = crate::context::get_hle_context();
    let content_size = ctx.game.get_content_size();
    
    // Write content size in KB to memory
    // Note: PS3 API uses u32 for size_kb, which supports up to 4TB in KB (4GB in bytes)
    // This is sufficient for all practical game content sizes
    if let Err(e) = write_be32(size_addr, content_size.size_kb as u32) {
        return e;
    }
    
    0 // CELL_OK
}

/// cellGameDiscCheck - Check disc content
///
/// # Returns
/// * 0 on success
pub fn cell_game_disc_check() -> i32 {
    debug!("cellGameDiscCheck()");
    
    // Verify game manager is initialized
    if !crate::context::get_hle_context().game.is_initialized() {
        crate::context::get_hle_context_mut().game.boot_check();
    }
    
    0 // CELL_OK
}

/// cellGameRegisterDiscChangeCallback - Register disc change callback
///
/// # Arguments
/// * `callback` - Callback function
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_game_register_disc_change_callback(_callback: u32, _userdata: u32) -> i32 {
    debug!("cellGameRegisterDiscChangeCallback()");
    0 // CELL_OK
}

/// cellGameUnregisterDiscChangeCallback - Unregister disc change callback
///
/// # Returns
/// * 0 on success
pub fn cell_game_unregister_disc_change_callback() -> i32 {
    debug!("cellGameUnregisterDiscChangeCallback()");
    0 // CELL_OK
}

/// cellGameGetDiscContentInfoUpdatePath - Get disc content info update path
///
/// # Arguments
/// * `path_addr` - Address to write path
///
/// # Returns
/// * 0 on success
pub fn cell_game_get_disc_content_info_update_path(_path_addr: u32) -> i32 {
    debug!("cellGameGetDiscContentInfoUpdatePath()");
    
    // Note: Path would be like "/dev_hdd0/game/GAME00000"
    
    0 // CELL_OK
}

/// cellNpDrmGetContentKey - Get content key for DRM-protected content
///
/// # Arguments
/// * `content_id_addr` - Address of content ID
/// * `key_addr` - Address to write key
///
/// # Returns
/// * 0 on success
pub fn cell_np_drm_get_content_key(_content_id_addr: u32, _key_addr: u32) -> i32 {
    debug!("cellNpDrmGetContentKey()");
    
    // For HLE, we return success without actual DRM processing
    0 // CELL_OK
}

/// cellNpDrmIsAvailable - Check if DRM content is available
///
/// # Arguments
/// * `content_id_addr` - Address of content ID
///
/// # Returns
/// * 0 if available, error otherwise
pub fn cell_np_drm_is_available(_content_id_addr: u32) -> i32 {
    trace!("cellNpDrmIsAvailable()");
    
    // For HLE, all content is considered available
    0 // CELL_OK
}

/// cellNpDrmIsAvailable2 - Check if DRM content is available (v2)
///
/// # Arguments
/// * `content_id_addr` - Address of content ID
///
/// # Returns
/// * 0 if available, error otherwise
pub fn cell_np_drm_is_available2(_content_id_addr: u32) -> i32 {
    trace!("cellNpDrmIsAvailable2()");
    
    // For HLE, all content is considered available
    0 // CELL_OK
}

/// cellGameContentGetPath - Get path to additional content
///
/// # Arguments
/// * `content_id_addr` - Address of content ID
/// * `path_addr` - Address to write path
///
/// # Returns
/// * 0 on success
pub fn cell_game_content_get_path(_content_id_addr: u32, _path_addr: u32) -> i32 {
    debug!("cellGameContentGetPath()");
    
    // Note: Path would be constructed from content ID
    
    0 // CELL_OK
}

/// cellGameDrmIsAvailable - Check if game DRM content is available
///
/// # Arguments
/// * `content_id_addr` - Address of content ID
///
/// # Returns
/// * 0 if available, error otherwise  
pub fn cell_game_drm_is_available(_content_id_addr: u32) -> i32 {
    trace!("cellGameDrmIsAvailable()");
    
    // For HLE, all content is considered available
    0 // CELL_OK
}

/// cellGameGetContentInfoList - Enumerate DLC / additional content
///
/// Writes up to `max_entries` content-info structures into guest memory.
///
/// Each entry is laid out as:
/// ```text
///   offset 0x00: content_id (128 bytes, null-terminated)
///   offset 0x80: path       (256 bytes, null-terminated)
///   offset 0x180: size_kb   (u64, big-endian)
///   offset 0x188: installed (u32, big-endian, 1 = installed)
/// ```
///
/// The total entry count is written to `count_addr`.
///
/// # Arguments
/// * `list_addr` - Address of output buffer for entries
/// * `max_entries` - Maximum number of entries to write
/// * `count_addr` - Address to write actual number of entries
///
/// # Returns
/// * 0 on success
pub fn cell_game_get_content_info_list(list_addr: u32, max_entries: u32, count_addr: u32) -> i32 {
    debug!(
        "cellGameGetContentInfoList(list=0x{:08X}, max={}, count=0x{:08X})",
        list_addr, max_entries, count_addr
    );

    let ctx = crate::context::get_hle_context();
    let entries = ctx.game.get_content_info_list();

    let write_count = std::cmp::min(entries.len() as u32, max_entries);

    const ENTRY_SIZE: u32 = 0x190; // 128 + 256 + 8 + 4 = 396 (round to 0x190 = 400)

    for i in 0..write_count {
        let entry = &entries[i as usize];
        let base = list_addr + i * ENTRY_SIZE;

        // Write content_id (128 bytes, null-terminated)
        if let Err(e) = write_string(base, &entry.content_id, 128) {
            return e;
        }
        // Write path (256 bytes, null-terminated)
        if let Err(e) = write_string(base + 0x80, &entry.path, 256) {
            return e;
        }
        // Write size_kb (u64)
        if let Err(e) = write_be64(base + 0x180, entry.size_kb) {
            return e;
        }
        // Write installed flag (u32)
        if let Err(e) = write_be32(base + 0x188, if entry.installed { 1 } else { 0 }) {
            return e;
        }
    }

    // Write actual count
    if let Err(e) = write_be32(count_addr, write_count) {
        return e;
    }

    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_manager() {
        let mut manager = GameManager::new();
        assert_eq!(manager.boot_check(), 0);
        assert!(manager.is_initialized());
        assert_eq!(manager.get_game_type(), CellGameDataType::Hdd);
    }

    #[test]
    fn test_game_manager_data_check() {
        let mut manager = GameManager::new();
        assert_eq!(manager.data_check(CellGameDataType::Disc, "GAME00001"), 0);
        assert_eq!(manager.get_game_type(), CellGameDataType::Disc);
        assert_eq!(manager.get_dir_name(), "GAME00001");
    }

    #[test]
    fn test_game_manager_params() {
        let manager = GameManager::new();
        
        // Test integer parameters
        let resolution = manager.get_param_int(CellGameParamId::Resolution as u32);
        assert_eq!(resolution, Some(1080));
        
        let parental = manager.get_param_int(CellGameParamId::ParentalLevel as u32);
        assert_eq!(parental, Some(0));
        
        // Test string parameters
        let title = manager.get_param_string(CellGameParamId::Title as u32);
        assert_eq!(title, Some("Test Game"));
        
        let title_id = manager.get_param_string(CellGameParamId::TitleId as u32);
        assert_eq!(title_id, Some("BLUS00000"));
        
        let version = manager.get_param_string(CellGameParamId::Version as u32);
        assert_eq!(version, Some("01.00"));
    }

    #[test]
    fn test_game_manager_param_mutation() {
        let mut manager = GameManager::new();
        
        // Set integer parameter
        manager.set_param_int(CellGameParamId::Resolution as u32, 720);
        assert_eq!(manager.get_param_int(CellGameParamId::Resolution as u32), Some(720));
        
        // Set string parameter
        manager.set_param_string(CellGameParamId::Title as u32, "New Game".to_string());
        assert_eq!(manager.get_param_string(CellGameParamId::Title as u32), Some("New Game"));
    }

    #[test]
    fn test_game_manager_content_size() {
        let mut manager = GameManager::new();
        manager.data_check(CellGameDataType::Hdd, "GAME00000");
        
        let size = manager.get_content_size();
        assert!(size.hdd_free_size > 0);
        assert!(size.size_kb > 0);
    }

    #[test]
    fn test_game_content_size_default() {
        let size = CellGameContentSize::default();
        assert!(size.hdd_free_size > 0);
    }

    #[test]
    fn test_game_boot_check() {
        let result = cell_game_boot_check(0x10000000, 0x10000100, 0x10000200, 0x10000300);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_game_data_check_validation() {
        // Valid types
        assert_eq!(cell_game_data_check(1, 0x10000000, 0x10000100), 0);
        assert_eq!(cell_game_data_check(2, 0x10000000, 0x10000100), 0);
        assert_eq!(cell_game_data_check(3, 0x10000000, 0x10000100), 0);
        
        // Invalid types
        assert!(cell_game_data_check(0, 0x10000000, 0x10000100) != 0);
        assert!(cell_game_data_check(4, 0x10000000, 0x10000100) != 0);
    }

    #[test]
    fn test_game_data_type() {
        assert_eq!(CellGameDataType::Disc as u32, 1);
        assert_eq!(CellGameDataType::Hdd as u32, 2);
        assert_eq!(CellGameDataType::Home as u32, 3);
    }

    #[test]
    fn test_game_param_ids() {
        assert_eq!(CellGameParamId::Title as u32, 0);
        assert_eq!(CellGameParamId::TitleId as u32, 1);
        assert_eq!(CellGameParamId::Version as u32, 2);
    }

    #[test]
    fn test_game_attributes() {
        assert_eq!(CELL_GAME_ATTRIBUTE_PATCH, 1);
        assert_eq!(CELL_GAME_ATTRIBUTE_APP_HOME, 2);
        assert_eq!(CELL_GAME_ATTRIBUTE_DEBUG, 4);
    }

    // ========================================================================
    // PARAM.SFO Tests
    // ========================================================================

    #[test]
    fn test_game_manager_param_sfo_load() {
        let mut manager = GameManager::new();
        
        // Initially not loaded
        assert!(!manager.is_param_sfo_loaded());
        
        // Create a minimal valid PARAM.SFO header
        let sfo_data = vec![
            0x00, 0x50, 0x53, 0x46, // Magic "\0PSF"
            0x01, 0x01, 0x00, 0x00, // Version 1.1
            0x14, 0x00, 0x00, 0x00, // Key table offset
            0x30, 0x00, 0x00, 0x00, // Data table offset
            0x00, 0x00, 0x00, 0x00, // Entry count
        ];
        
        assert_eq!(manager.load_param_sfo(&sfo_data), 0);
        assert!(manager.is_param_sfo_loaded());
    }

    #[test]
    fn test_game_manager_param_sfo_invalid() {
        let mut manager = GameManager::new();
        
        // Too short
        assert!(manager.load_param_sfo(&[0, 1, 2]) != 0);
        
        // Wrong magic
        let bad_magic = vec![0x01, 0x02, 0x03, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(manager.load_param_sfo(&bad_magic) != 0);
    }

    #[test]
    fn test_game_manager_param_sfo_save() {
        let manager = GameManager::new();
        
        let result = manager.save_param_sfo();
        assert!(result.is_ok());
        
        let data = result.unwrap();
        // Check magic
        assert_eq!(&data[0..4], &[0x00, 0x50, 0x53, 0x46]);
    }

    #[test]
    fn test_game_manager_param_sfo_entries() {
        let mut manager = GameManager::new();
        
        // Add entry
        manager.set_param_sfo_entry("TITLE", ParamSfoValue::String("My Game".to_string()));
        manager.set_param_sfo_entry("RESOLUTION", ParamSfoValue::Integer(720));
        
        // Verify it was added
        let entry = manager.get_param_sfo_entry("TITLE");
        assert!(entry.is_some());
        
        // Verify it updated the param maps
        assert_eq!(manager.get_param_string(CellGameParamId::Title as u32), Some("My Game"));
    }

    #[test]
    fn test_game_manager_paths() {
        let mut manager = GameManager::new();
        manager.boot_check();
        
        assert!(!manager.get_content_info_path().is_empty());
        assert!(!manager.get_usrdir_path().is_empty());
        assert!(manager.get_usrdir_path().contains("USRDIR"));
    }

    // ========================================================================
    // Game Installation Tests
    // ========================================================================

    #[test]
    fn test_game_manager_installation_lifecycle() {
        let mut manager = GameManager::new();
        
        // Not installed initially
        assert_eq!(manager.get_install_state(), GameInstallState::NotInstalled);
        assert!(!manager.is_installed());
        
        // Start installation
        assert_eq!(manager.start_installation("/dev_bdvd/PS3_GAME", 1024 * 1024), 0);
        assert_eq!(manager.get_install_state(), GameInstallState::Installing);
        
        // Update progress
        assert_eq!(manager.update_installation_progress(512 * 1024), 0);
        assert_eq!(manager.get_install_progress(), 50);
        
        // Complete installation
        assert_eq!(manager.complete_installation(), 0);
        assert_eq!(manager.get_install_state(), GameInstallState::Installed);
        assert!(manager.is_installed());
        assert_eq!(manager.get_install_progress(), 100);
    }

    #[test]
    fn test_game_manager_installation_failure() {
        let mut manager = GameManager::new();
        
        manager.start_installation("/dev_bdvd/PS3_GAME", 1024);
        manager.fail_installation(0x80010001u32 as i32);
        
        assert_eq!(manager.get_install_state(), GameInstallState::Failed);
        assert_eq!(manager.get_install_info().error_code, 0x80010001u32 as i32);
    }

    #[test]
    fn test_game_manager_installation_double_start() {
        let mut manager = GameManager::new();
        
        // First start succeeds
        assert_eq!(manager.start_installation("/path1", 1024), 0);
        
        // Second start fails
        assert!(manager.start_installation("/path2", 2048) != 0);
    }

    #[test]
    fn test_game_manager_installation_reset() {
        let mut manager = GameManager::new();
        
        manager.start_installation("/path", 1024);
        manager.complete_installation();
        
        manager.reset_installation();
        
        assert_eq!(manager.get_install_state(), GameInstallState::NotInstalled);
        assert_eq!(manager.get_install_progress(), 0);
    }

    #[test]
    fn test_game_install_state_enum() {
        assert_eq!(GameInstallState::NotInstalled as u32, 0);
        assert_eq!(GameInstallState::Installing as u32, 1);
        assert_eq!(GameInstallState::Installed as u32, 2);
        assert_eq!(GameInstallState::Failed as u32, 3);
    }

    // ========================================================================
    // Game Update Tests
    // ========================================================================

    #[test]
    fn test_game_manager_update_check() {
        let mut manager = GameManager::new();
        
        assert_eq!(manager.check_for_updates(), 0);
        assert_eq!(manager.get_update_state(), GameUpdateState::NoUpdate);
    }

    #[test]
    fn test_game_manager_update_download_lifecycle() {
        let mut manager = GameManager::new();
        
        // Not downloading initially
        assert_eq!(manager.get_update_state(), GameUpdateState::NoUpdate);
        
        // Start download
        assert_eq!(
            manager.start_update_download("http://example.com/update.pkg", 100 * 1024, "02.00"),
            0
        );
        assert_eq!(manager.get_update_state(), GameUpdateState::Downloading);
        
        // Update progress
        assert_eq!(manager.update_download_progress(50 * 1024), 0);
        assert_eq!(manager.get_update_info().download_progress, 50);
        
        // Complete download
        assert_eq!(manager.complete_update_download(), 0);
        assert_eq!(manager.get_update_state(), GameUpdateState::Ready);
    }

    #[test]
    fn test_game_manager_update_installation() {
        let mut manager = GameManager::new();
        manager.start_update_download("http://example.com/update.pkg", 1024, "02.00");
        manager.complete_update_download();
        
        // Start installation
        assert_eq!(manager.start_update_installation(), 0);
        assert_eq!(manager.get_update_state(), GameUpdateState::Installing);
        
        // Update progress
        assert_eq!(manager.update_install_progress(75), 0);
        assert_eq!(manager.get_update_info().install_progress, 75);
        
        // Complete installation
        assert_eq!(manager.complete_update_installation(), 0);
        assert_eq!(manager.get_update_state(), GameUpdateState::Installed);
        
        // Version should be updated
        assert_eq!(manager.get_param_string(CellGameParamId::Version as u32), Some("02.00"));
    }

    #[test]
    fn test_game_manager_update_failure() {
        let mut manager = GameManager::new();
        manager.start_update_download("http://example.com/update.pkg", 1024, "02.00");
        
        manager.fail_update(0x80010001u32 as i32);
        
        assert_eq!(manager.get_update_state(), GameUpdateState::Failed);
        assert_eq!(manager.get_update_info().error_code, 0x80010001u32 as i32);
    }

    #[test]
    fn test_game_manager_update_double_start() {
        let mut manager = GameManager::new();
        
        // First start succeeds
        assert_eq!(manager.start_update_download("http://example.com/update1.pkg", 1024, "02.00"), 0);
        
        // Second start fails
        assert!(manager.start_update_download("http://example.com/update2.pkg", 2048, "03.00") != 0);
    }

    #[test]
    fn test_game_manager_update_reset() {
        let mut manager = GameManager::new();
        
        manager.start_update_download("http://example.com/update.pkg", 1024, "02.00");
        manager.complete_update_download();
        
        manager.reset_update();
        
        assert_eq!(manager.get_update_state(), GameUpdateState::NoUpdate);
        assert_eq!(manager.get_update_info().version, "");
    }

    #[test]
    fn test_game_manager_update_available() {
        let mut manager = GameManager::new();
        
        assert!(!manager.is_update_available());
        
        manager.update_info.state = GameUpdateState::Available;
        assert!(manager.is_update_available());
    }

    #[test]
    fn test_game_update_state_enum() {
        assert_eq!(GameUpdateState::NoUpdate as u32, 0);
        assert_eq!(GameUpdateState::Available as u32, 1);
        assert_eq!(GameUpdateState::Downloading as u32, 2);
        assert_eq!(GameUpdateState::Ready as u32, 3);
        assert_eq!(GameUpdateState::Installing as u32, 4);
        assert_eq!(GameUpdateState::Installed as u32, 5);
        assert_eq!(GameUpdateState::Failed as u32, 6);
    }

    // ========================================================================
    // DLC Tests
    // ========================================================================

    #[test]
    fn test_game_manager_dlc_register() {
        let mut manager = GameManager::new();
        
        // No DLC initially
        assert_eq!(manager.dlc_count(), 0);
        
        // Register DLC
        assert_eq!(manager.register_dlc("DLC001", "Extra Levels Pack", 102400), 0);
        assert_eq!(manager.register_dlc("DLC002", "Costume Pack", 51200), 0);
        
        assert_eq!(manager.dlc_count(), 2);
        
        // Double registration should fail
        assert!(manager.register_dlc("DLC001", "Duplicate", 1024) != 0);
    }

    #[test]
    fn test_game_manager_dlc_get() {
        let mut manager = GameManager::new();
        manager.register_dlc("DLC001", "Test DLC", 1024);
        
        let dlc = manager.get_dlc("DLC001");
        assert!(dlc.is_some());
        let dlc = dlc.unwrap();
        assert_eq!(dlc.content_id, "DLC001");
        assert_eq!(dlc.name, "Test DLC");
        assert_eq!(dlc.size_kb, 1024);
        assert_eq!(dlc.state, DlcState::NotInstalled);
        
        // Non-existent DLC
        assert!(manager.get_dlc("NONEXISTENT").is_none());
    }

    #[test]
    fn test_game_manager_dlc_download_lifecycle() {
        let mut manager = GameManager::new();
        manager.boot_check();
        manager.register_dlc("DLC001", "Test DLC", 102400);
        
        // Not downloading initially
        assert!(!manager.is_dlc_downloading());
        assert!(!manager.is_dlc_installed("DLC001"));
        
        // Start download
        assert_eq!(manager.start_dlc_download("DLC001"), 0);
        assert!(manager.is_dlc_downloading());
        assert_eq!(manager.get_downloading_dlc_id(), Some("DLC001"));
        
        // Update progress
        assert_eq!(manager.update_dlc_download_progress(50), 0);
        assert_eq!(manager.get_dlc_download_progress(), 50);
        
        // Complete download
        assert_eq!(manager.complete_dlc_download(), 0);
        assert!(!manager.is_dlc_downloading());
        assert!(manager.is_dlc_installed("DLC001"));
        assert_eq!(manager.installed_dlc_count(), 1);
        
        // DLC install path should be set
        let dlc = manager.get_dlc("DLC001").unwrap();
        assert!(!dlc.install_path.is_empty());
    }

    #[test]
    fn test_game_manager_dlc_download_failure() {
        let mut manager = GameManager::new();
        manager.register_dlc("DLC001", "Test DLC", 1024);
        
        manager.start_dlc_download("DLC001");
        assert_eq!(manager.fail_dlc_download(), 0);
        
        let dlc = manager.get_dlc("DLC001").unwrap();
        assert_eq!(dlc.state, DlcState::Failed);
    }

    #[test]
    fn test_game_manager_dlc_remove() {
        let mut manager = GameManager::new();
        manager.boot_check();
        manager.register_dlc("DLC001", "Test DLC", 1024);
        manager.start_dlc_download("DLC001");
        manager.complete_dlc_download();
        
        // Remove DLC
        assert_eq!(manager.remove_dlc("DLC001"), 0);
        assert!(!manager.is_dlc_installed("DLC001"));
        
        // Removing uninstalled DLC should fail
        assert!(manager.remove_dlc("DLC001") != 0);
    }

    #[test]
    fn test_game_manager_dlc_licensing() {
        let mut manager = GameManager::new();
        manager.register_dlc("DLC001", "Test DLC", 1024);
        
        // Not licensed by default
        assert!(!manager.is_dlc_licensed("DLC001"));
        
        // Set license
        assert_eq!(manager.set_dlc_licensed("DLC001", true), 0);
        assert!(manager.is_dlc_licensed("DLC001"));
        
        // Revoke license
        assert_eq!(manager.set_dlc_licensed("DLC001", false), 0);
        assert!(!manager.is_dlc_licensed("DLC001"));
    }

    #[test]
    fn test_game_manager_dlc_list_installed() {
        let mut manager = GameManager::new();
        manager.boot_check();
        manager.register_dlc("DLC001", "DLC 1", 1024);
        manager.register_dlc("DLC002", "DLC 2", 2048);
        manager.register_dlc("DLC003", "DLC 3", 3072);
        
        // Install some DLCs
        manager.start_dlc_download("DLC001");
        manager.complete_dlc_download();
        
        manager.start_dlc_download("DLC003");
        manager.complete_dlc_download();
        
        let installed = manager.list_installed_dlc();
        assert_eq!(installed.len(), 2);
        assert_eq!(manager.installed_dlc_count(), 2);
    }

    #[test]
    fn test_game_manager_dlc_double_download() {
        let mut manager = GameManager::new();
        manager.register_dlc("DLC001", "DLC 1", 1024);
        manager.register_dlc("DLC002", "DLC 2", 2048);
        
        // Start first download
        assert_eq!(manager.start_dlc_download("DLC001"), 0);
        
        // Second download should fail
        assert!(manager.start_dlc_download("DLC002") != 0);
    }

    #[test]
    fn test_dlc_state_enum() {
        assert_eq!(DlcState::NotInstalled as u32, 0);
        assert_eq!(DlcState::Installed as u32, 1);
        assert_eq!(DlcState::Downloading as u32, 2);
        assert_eq!(DlcState::Installing as u32, 3);
        assert_eq!(DlcState::Failed as u32, 4);
    }

    // ========================================================================
    // Game Data Existence and Content Size Tests
    // ========================================================================

    #[test]
    fn test_game_data_exists_default() {
        let manager = GameManager::new();
        
        // Should return true for HLE compatibility (games always "exist")
        assert!(manager.game_data_exists("GAME00000"));
        assert!(manager.game_data_exists("NONEXISTENT"));
    }

    #[test]
    fn test_game_data_exists_after_boot_check() {
        let mut manager = GameManager::new();
        manager.boot_check();
        
        // After boot check, the initialized game dir should exist
        assert!(manager.game_data_exists("GAME00000"));
    }

    #[test]
    fn test_calculate_content_size_default() {
        let manager = GameManager::new();
        
        // Without a real path, should return default sizes
        let (size_kb, sys_size_kb) = manager.calculate_content_size("NONEXISTENT");
        
        // Default is 1 GB / 100 MB
        assert_eq!(size_kb, 1024 * 1024);
        assert_eq!(sys_size_kb, 100 * 1024);
    }

    #[test]
    fn test_calculate_directory_size_empty() {
        // Create a temp directory for testing
        let temp_dir = std::env::temp_dir().join("oxidized_cell_test_empty");
        let _ = std::fs::create_dir_all(&temp_dir);
        
        // Empty directory should have 0 size
        let size = GameManager::calculate_directory_size(&temp_dir);
        assert_eq!(size, 0);
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_calculate_directory_size_with_files() {
        // Create a temp directory with some test files
        let temp_dir = std::env::temp_dir().join("oxidized_cell_test_files");
        let _ = std::fs::create_dir_all(&temp_dir);
        
        // Create a test file with known size
        let test_file = temp_dir.join("test.bin");
        let test_data = vec![0u8; 1024]; // 1 KB
        let _ = std::fs::write(&test_file, &test_data);
        
        // Directory should have at least 1024 bytes
        let size = GameManager::calculate_directory_size(&temp_dir);
        assert!(size >= 1024);
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_get_host_game_path_with_env_var() {
        // Set environment variable
        std::env::set_var("OXIDIZED_CELL_GAMES", "/test/games");
        
        let manager = GameManager::new();
        let path = manager.get_host_game_path("GAME00000");
        
        assert!(path.is_some());
        assert_eq!(path.unwrap(), "/test/games/GAME00000");
        
        // Cleanup
        std::env::remove_var("OXIDIZED_CELL_GAMES");
    }

    #[test]
    fn test_data_check_calculates_size() {
        let mut manager = GameManager::new();
        
        // Initial size should be default
        let initial_size = manager.get_content_size();
        
        // After data_check, sizes should be set
        manager.data_check(CellGameDataType::Hdd, "TESTGAME");
        
        let new_size = manager.get_content_size();
        assert!(new_size.size_kb > 0);
        assert!(new_size.sys_size_kb > 0);
    }

    // ========================================================================
    // Patch Detection and Merge Tests
    // ========================================================================

    #[test]
    fn test_patch_state_enum() {
        assert_eq!(PatchState::None as u32, 0);
        assert_eq!(PatchState::Detected as u32, 1);
        assert_eq!(PatchState::Merging as u32, 2);
        assert_eq!(PatchState::Applied as u32, 3);
        assert_eq!(PatchState::Failed as u32, 4);
    }

    #[test]
    fn test_patch_info_default() {
        let info = PatchInfo::default();
        assert_eq!(info.state, PatchState::None);
        assert!(info.version.is_empty());
        assert_eq!(info.size_kb, 0);
        assert_eq!(info.file_count, 0);
    }

    #[test]
    fn test_detect_patch_no_dir_name() {
        let mut manager = GameManager::new();
        // dir_name is empty → should return error
        let result = manager.detect_patch();
        assert_ne!(result, 0);
    }

    #[test]
    fn test_detect_patch_no_patch_directory() {
        let mut manager = GameManager::new();
        manager.boot_check();

        // No patch directory exists on disk → state should be None
        let result = manager.detect_patch();
        assert_eq!(result, 0);
        assert_eq!(manager.get_patch_state(), PatchState::None);
        assert!(!manager.has_patch());
    }

    #[test]
    fn test_detect_and_apply_patch_with_real_dir() {
        let mut manager = GameManager::new();
        manager.boot_check();

        // Create a temporary patch directory
        let temp_dir = std::env::temp_dir().join("oxidized_cell_test_patch");
        let patch_dir = temp_dir.join("GAME00000_patch");
        let _ = std::fs::create_dir_all(&patch_dir);

        // Add a fake patch file
        let _ = std::fs::write(patch_dir.join("EBOOT.BIN"), b"patched_data");

        // Point env var to our temp dir
        std::env::set_var("OXIDIZED_CELL_GAMES", temp_dir.to_str().unwrap());

        assert_eq!(manager.detect_patch(), 0);
        assert_eq!(manager.get_patch_state(), PatchState::Detected);
        assert!(manager.has_patch());
        assert!(manager.get_attributes() & CELL_GAME_ATTRIBUTE_PATCH != 0);
        assert_eq!(manager.get_patch_info().file_count, 1);
        assert!(manager.get_patch_info().size_kb > 0);

        // Apply the patch
        assert_eq!(manager.apply_patch(), 0);
        assert_eq!(manager.get_patch_state(), PatchState::Applied);
        assert_eq!(manager.get_patch_info().merged_count, 1);

        // Reset
        manager.reset_patch();
        assert_eq!(manager.get_patch_state(), PatchState::None);
        assert!(manager.get_attributes() & CELL_GAME_ATTRIBUTE_PATCH == 0);

        // Cleanup
        std::env::remove_var("OXIDIZED_CELL_GAMES");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_apply_patch_without_detection() {
        let mut manager = GameManager::new();
        manager.boot_check();

        // Should fail: no patch detected
        assert_ne!(manager.apply_patch(), 0);
    }

    // ========================================================================
    // Web Content Path Tests
    // ========================================================================

    #[test]
    fn test_web_content_path_empty_before_init() {
        let manager = GameManager::new();
        assert!(manager.get_web_content_path().is_empty());
    }

    #[test]
    fn test_web_content_path_after_boot() {
        let mut manager = GameManager::new();
        manager.boot_check();

        let path = manager.get_web_content_path();
        assert_eq!(path, "/dev_hdd0/game/GAME00000/USRDIR/web");
    }

    #[test]
    fn test_web_content_path_after_data_check() {
        let mut manager = GameManager::new();
        manager.data_check(CellGameDataType::Hdd, "WEBGAME01");

        let path = manager.get_web_content_path();
        assert_eq!(path, "/dev_hdd0/game/WEBGAME01/USRDIR/web");
    }

    // ========================================================================
    // Content Info List (DLC Enumeration) Tests
    // ========================================================================

    #[test]
    fn test_content_info_list_empty() {
        let manager = GameManager::new();
        let list = manager.get_content_info_list();
        assert!(list.is_empty());
        assert_eq!(manager.get_content_info_count(), 0);
    }

    #[test]
    fn test_content_info_list_with_dlc() {
        let mut manager = GameManager::new();
        manager.boot_check();

        manager.register_dlc("DLC001", "Extra Maps", 51200);
        manager.register_dlc("DLC002", "Costume Pack", 10240);

        // Install one
        manager.start_dlc_download("DLC001");
        manager.complete_dlc_download();

        let list = manager.get_content_info_list();
        assert_eq!(list.len(), 2);
        assert_eq!(manager.get_content_info_count(), 2);

        // Check first entry (installed)
        assert_eq!(list[0].content_id, "DLC001");
        assert_eq!(list[0].size_kb, 51200);
        assert!(list[0].installed);
        assert!(!list[0].path.is_empty());

        // Check second entry (not installed)
        assert_eq!(list[1].content_id, "DLC002");
        assert_eq!(list[1].size_kb, 10240);
        assert!(!list[1].installed);
        assert!(list[1].path.contains("DLC002"));
    }

    #[test]
    fn test_content_info_entry_path_default() {
        let mut manager = GameManager::new();
        manager.boot_check();
        manager.register_dlc("MY_DLC", "Test", 100);

        let list = manager.get_content_info_list();
        assert_eq!(
            list[0].path,
            "/dev_hdd0/game/GAME00000/USRDIR/dlc/MY_DLC"
        );
    }
}
