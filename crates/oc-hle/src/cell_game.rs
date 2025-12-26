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

        // Note: Would Actually detect game type from mounted media in a full implementation.
        // Note: Would read game attributes from PARAM.SFO file. Requires VFS integration.

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

        // Note: Would Check if game data exists in a full implementation.
        // Note: Would Calculate actual content size in a full implementation.

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
}
