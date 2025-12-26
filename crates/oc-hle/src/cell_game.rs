//! cellGame HLE - Game Data Management
//!
//! This module provides HLE implementations for PS3 game data access,
//! including disc content, digital content, and game directories.

use std::collections::HashMap;
use tracing::{debug, trace};

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

        // Calculate content size (simulated)
        self.content_size.size_kb = 1024 * 1024; // 1 GB
        self.content_size.sys_size_kb = 100 * 1024; // 100 MB

        // TODO: Check if game data exists
        // TODO: Calculate actual content size

        0 // CELL_OK
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
    if data_type < 1 || data_type > 3 {
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
/// * `path_addr` - Address to write path
///
/// # Returns
/// * 0 on success
pub fn cell_game_get_local_web_content_path(_path_addr: u32) -> i32 {
    debug!("cellGameGetLocalWebContentPath()");

    // Get directory name from global game manager
    let _dir_name = crate::context::get_hle_context().game.get_dir_name();

    // Note: Writing path to memory requires memory subsystem integration
    // Path would be like "/dev_hdd0/game/GAME00000/USRDIR/web"

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
}
