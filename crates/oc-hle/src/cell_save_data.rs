//! cellSaveData HLE - Save Data Management
//!
//! This module provides HLE implementations for PS3 save data operations.

use std::collections::HashMap;
use tracing::{debug, trace};

/// Maximum directory name length
pub const CELL_SAVEDATA_DIRNAME_SIZE: usize = 32;

/// Maximum file name length
pub const CELL_SAVEDATA_FILENAME_SIZE: usize = 13;

/// Maximum list item count
pub const CELL_SAVEDATA_LISTITEM_MAX: usize = 2048;

/// Save data version
pub const CELL_SAVEDATA_VERSION_CURRENT: u32 = 0;

/// Save data error codes
pub const CELL_SAVEDATA_ERROR_CBRESULT: i32 = 0x8002b401u32 as i32;
pub const CELL_SAVEDATA_ERROR_ACCESS_ERROR: i32 = 0x8002b402u32 as i32;
pub const CELL_SAVEDATA_ERROR_INTERNAL: i32 = 0x8002b403u32 as i32;
pub const CELL_SAVEDATA_ERROR_PARAM: i32 = 0x8002b404u32 as i32;
pub const CELL_SAVEDATA_ERROR_NOSPACE: i32 = 0x8002b405u32 as i32;
pub const CELL_SAVEDATA_ERROR_BROKEN: i32 = 0x8002b406u32 as i32;
pub const CELL_SAVEDATA_ERROR_NODATA: i32 = 0x8002b410u32 as i32;

/// Save data operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveDataOperation {
    Load,
    Save,
    Delete,
}

/// Save data list item
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellSaveDataListItem {
    /// Directory name
    pub dir_name: [u8; CELL_SAVEDATA_DIRNAME_SIZE],
    /// List parameter address
    pub list_param: u32,
}

impl Default for CellSaveDataListItem {
    fn default() -> Self {
        Self {
            dir_name: [0; CELL_SAVEDATA_DIRNAME_SIZE],
            list_param: 0,
        }
    }
}

/// Save data directory stat
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellSaveDataDirStat {
    /// Title
    pub title: [u8; 128],
    /// Subtitle
    pub subtitle: [u8; 128],
    /// Detail
    pub detail: [u8; 1024],
    /// Icon file name
    pub icon_file_name: [u8; CELL_SAVEDATA_FILENAME_SIZE],
    /// Icon buffer size
    pub icon_buf_size: u32,
    /// Modified time
    pub mtime: u64,
    /// File size (KB)
    pub file_size_kb: u64,
}

impl Default for CellSaveDataDirStat {
    fn default() -> Self {
        Self {
            title: [0; 128],
            subtitle: [0; 128],
            detail: [0; 1024],
            icon_file_name: [0; CELL_SAVEDATA_FILENAME_SIZE],
            icon_buf_size: 0,
            mtime: 0,
            file_size_kb: 0,
        }
    }
}

/// Save data file stat
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellSaveDataFileStat {
    /// File type
    pub file_type: u32,
    /// File size
    pub file_size: u64,
    /// Modified time
    pub mtime: u64,
    /// File name
    pub file_name: [u8; CELL_SAVEDATA_FILENAME_SIZE],
}

/// Save data set
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellSaveDataSetBuf {
    /// Directory name
    pub dir_name: u32,
    /// New data
    pub new_data: u32,
}

/// Save data entry
#[derive(Debug, Clone)]
struct SaveDataEntry {
    /// Directory name
    dir_name: String,
    /// Directory stat
    dir_stat: CellSaveDataDirStat,
    /// Files
    files: Vec<String>,
}

/// Save data manager
pub struct SaveDataManager {
    /// Save data entries
    entries: HashMap<String, SaveDataEntry>,
    /// Base path for save data
    base_path: String,
}

impl SaveDataManager {
    /// Create a new save data manager
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            base_path: "/dev_hdd0/savedata".to_string(),
        }
    }

    /// List save data directories
    pub fn list_directories(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Create save data directory
    pub fn create_directory(&mut self, dir_name: &str) -> i32 {
        if dir_name.is_empty() || dir_name.len() > CELL_SAVEDATA_DIRNAME_SIZE {
            return CELL_SAVEDATA_ERROR_PARAM;
        }

        debug!("SaveDataManager::create_directory: {}", dir_name);

        let entry = SaveDataEntry {
            dir_name: dir_name.to_string(),
            dir_stat: CellSaveDataDirStat::default(),
            files: Vec::new(),
        };

        self.entries.insert(dir_name.to_string(), entry);

        // TODO: Create directory in VFS

        0 // CELL_OK
    }

    /// Delete save data directory
    pub fn delete_directory(&mut self, dir_name: &str) -> i32 {
        if let Some(_entry) = self.entries.remove(dir_name) {
            debug!("SaveDataManager::delete_directory: {}", dir_name);
            // TODO: Delete directory from VFS
            0 // CELL_OK
        } else {
            CELL_SAVEDATA_ERROR_NODATA
        }
    }

    /// Check if directory exists
    pub fn directory_exists(&self, dir_name: &str) -> bool {
        self.entries.contains_key(dir_name)
    }

    /// Get directory stat
    pub fn get_dir_stat(&self, dir_name: &str) -> Option<CellSaveDataDirStat> {
        self.entries.get(dir_name).map(|e| e.dir_stat)
    }

    /// Update directory stat
    pub fn update_dir_stat(&mut self, dir_name: &str, stat: CellSaveDataDirStat) -> i32 {
        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.dir_stat = stat;
            debug!("SaveDataManager::update_dir_stat: {}", dir_name);
            0 // CELL_OK
        } else {
            CELL_SAVEDATA_ERROR_NODATA
        }
    }

    /// Add file to directory
    pub fn add_file(&mut self, dir_name: &str, file_name: &str) -> i32 {
        if file_name.is_empty() || file_name.len() > CELL_SAVEDATA_FILENAME_SIZE {
            return CELL_SAVEDATA_ERROR_PARAM;
        }

        if let Some(entry) = self.entries.get_mut(dir_name) {
            if !entry.files.contains(&file_name.to_string()) {
                entry.files.push(file_name.to_string());
                debug!("SaveDataManager::add_file: {}/{}", dir_name, file_name);
            }
            0 // CELL_OK
        } else {
            CELL_SAVEDATA_ERROR_NODATA
        }
    }

    /// Get files in directory
    pub fn get_files(&self, dir_name: &str) -> Option<Vec<String>> {
        self.entries.get(dir_name).map(|e| e.files.clone())
    }

    /// Get directory count
    pub fn directory_count(&self) -> usize {
        self.entries.len()
    }

    /// Set base path
    pub fn set_base_path(&mut self, path: String) {
        self.base_path = path;
    }

    /// Get base path
    pub fn get_base_path(&self) -> &str {
        &self.base_path
    }
}

impl Default for SaveDataManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellSaveDataListLoad2 - Load save data list
///
/// # Arguments
/// * `version` - Version
/// * `setList` - Set list address
/// * `setBuf` - Set buffer address
/// * `funcList` - List callback function
/// * `funcStat` - Status callback function
/// * `funcFile` - File callback function
/// * `container` - Container address
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_save_data_list_load2(
    version: u32,
    _set_list_addr: u32,
    _set_buf_addr: u32,
    _func_list: u32,
    _func_stat: u32,
    _func_file: u32,
    _container: u32,
    _userdata: u32,
) -> i32 {
    debug!("cellSaveDataListLoad2(version={})", version);

    // Validate version
    if version != CELL_SAVEDATA_VERSION_CURRENT {
        return CELL_SAVEDATA_ERROR_PARAM;
    }

    // TODO: Load save data list from VFS through global manager
    // TODO: Call list callback with results
    // TODO: Handle file operations

    0 // CELL_OK
}

/// cellSaveDataListSave2 - Save data list
///
/// # Arguments
/// * `version` - Version
/// * `setList` - Set list address
/// * `setBuf` - Set buffer address
/// * `funcList` - List callback function
/// * `funcFixed` - Fixed callback function
/// * `funcFile` - File callback function
/// * `container` - Container address
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_save_data_list_save2(
    version: u32,
    _set_list_addr: u32,
    _set_buf_addr: u32,
    _func_list: u32,
    _func_fixed: u32,
    _func_file: u32,
    _container: u32,
    _userdata: u32,
) -> i32 {
    debug!("cellSaveDataListSave2(version={})", version);

    // Validate version
    if version != CELL_SAVEDATA_VERSION_CURRENT {
        return CELL_SAVEDATA_ERROR_PARAM;
    }

    // TODO: Save data list to VFS through global manager
    // TODO: Call callbacks with progress
    // TODO: Handle file operations

    0 // CELL_OK
}

/// cellSaveDataDelete2 - Delete save data
///
/// # Arguments
/// * `version` - Version
/// * `setList` - Set list address
/// * `setBuf` - Set buffer address
/// * `funcList` - List callback function
/// * `funcDone` - Done callback function
/// * `container` - Container address
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_save_data_delete2(
    version: u32,
    _set_list_addr: u32,
    _set_buf_addr: u32,
    _func_list: u32,
    _func_done: u32,
    _container: u32,
    _userdata: u32,
) -> i32 {
    debug!("cellSaveDataDelete2(version={})", version);

    // Validate version
    if version != CELL_SAVEDATA_VERSION_CURRENT {
        return CELL_SAVEDATA_ERROR_PARAM;
    }

    // TODO: Delete save data from VFS through global manager
    // TODO: Call callbacks

    0 // CELL_OK
}

/// cellSaveDataFixedLoad2 - Load fixed save data
///
/// # Arguments
/// * `version` - Version
/// * `setList` - Set list address
/// * `setBuf` - Set buffer address
/// * `funcFixed` - Fixed callback function
/// * `funcStat` - Status callback function
/// * `funcFile` - File callback function
/// * `container` - Container address
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_save_data_fixed_load2(
    version: u32,
    _set_list_addr: u32,
    _set_buf_addr: u32,
    _func_fixed: u32,
    _func_stat: u32,
    _func_file: u32,
    _container: u32,
    _userdata: u32,
) -> i32 {
    debug!("cellSaveDataFixedLoad2(version={})", version);

    // Validate version
    if version != CELL_SAVEDATA_VERSION_CURRENT {
        return CELL_SAVEDATA_ERROR_PARAM;
    }

    // TODO: Load fixed save data through global manager
    // TODO: Call callbacks

    0 // CELL_OK
}

/// cellSaveDataFixedSave2 - Save fixed save data
///
/// # Arguments
/// * `version` - Version
/// * `setList` - Set list address
/// * `setBuf` - Set buffer address
/// * `funcFixed` - Fixed callback function
/// * `funcStat` - Status callback function
/// * `funcFile` - File callback function
/// * `container` - Container address
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_save_data_fixed_save2(
    version: u32,
    _set_list_addr: u32,
    _set_buf_addr: u32,
    _func_fixed: u32,
    _func_stat: u32,
    _func_file: u32,
    _container: u32,
    _userdata: u32,
) -> i32 {
    debug!("cellSaveDataFixedSave2(version={})", version);

    // Validate version
    if version != CELL_SAVEDATA_VERSION_CURRENT {
        return CELL_SAVEDATA_ERROR_PARAM;
    }

    // TODO: Save fixed save data through global manager
    // TODO: Call callbacks

    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_data_manager() {
        let mut manager = SaveDataManager::new();
        
        // Create directory
        assert_eq!(manager.create_directory("SAVE0001"), 0);
        assert_eq!(manager.directory_count(), 1);
        assert!(manager.directory_exists("SAVE0001"));
        
        // Delete directory
        assert_eq!(manager.delete_directory("SAVE0001"), 0);
        assert_eq!(manager.directory_count(), 0);
        assert!(!manager.directory_exists("SAVE0001"));
    }

    #[test]
    fn test_save_data_manager_files() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        // Add files
        assert_eq!(manager.add_file("SAVE0001", "DATA.BIN"), 0);
        assert_eq!(manager.add_file("SAVE0001", "ICON0.PNG"), 0);
        
        let files = manager.get_files("SAVE0001");
        assert!(files.is_some());
        let files = files.unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"DATA.BIN".to_string()));
        assert!(files.contains(&"ICON0.PNG".to_string()));
    }

    #[test]
    fn test_save_data_manager_validation() {
        let mut manager = SaveDataManager::new();
        
        // Empty directory name
        assert!(manager.create_directory("") != 0);
        
        // Too long directory name
        let long_name = "A".repeat(CELL_SAVEDATA_DIRNAME_SIZE + 1);
        assert!(manager.create_directory(&long_name) != 0);
        
        // Delete non-existent directory
        assert!(manager.delete_directory("NONEXISTENT") != 0);
    }

    #[test]
    fn test_save_data_manager_dir_stat() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        // Get default stat
        let stat = manager.get_dir_stat("SAVE0001");
        assert!(stat.is_some());
        
        // Update stat
        let mut new_stat = CellSaveDataDirStat::default();
        new_stat.file_size_kb = 1024;
        assert_eq!(manager.update_dir_stat("SAVE0001", new_stat), 0);
        
        // Verify update
        let stat = manager.get_dir_stat("SAVE0001").unwrap();
        assert_eq!(stat.file_size_kb, 1024);
    }

    #[test]
    fn test_save_data_manager_list() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        manager.create_directory("SAVE0002");
        manager.create_directory("SAVE0003");
        
        let dirs = manager.list_directories();
        assert_eq!(dirs.len(), 3);
    }

    #[test]
    fn test_save_data_manager_base_path() {
        let mut manager = SaveDataManager::new();
        assert_eq!(manager.get_base_path(), "/dev_hdd0/savedata");
        
        manager.set_base_path("/custom/path".to_string());
        assert_eq!(manager.get_base_path(), "/custom/path");
    }

    #[test]
    fn test_save_data_constants() {
        assert_eq!(CELL_SAVEDATA_DIRNAME_SIZE, 32);
        assert_eq!(CELL_SAVEDATA_FILENAME_SIZE, 13);
        assert_eq!(CELL_SAVEDATA_VERSION_CURRENT, 0);
    }

    #[test]
    fn test_save_data_list_load() {
        let result = cell_save_data_list_load2(0, 0, 0, 0, 0, 0, 0, 0);
        assert_eq!(result, 0);
        
        // Invalid version
        let result = cell_save_data_list_load2(999, 0, 0, 0, 0, 0, 0, 0);
        assert!(result != 0);
    }

    #[test]
    fn test_save_data_list_save() {
        let result = cell_save_data_list_save2(0, 0, 0, 0, 0, 0, 0, 0);
        assert_eq!(result, 0);
        
        // Invalid version
        let result = cell_save_data_list_save2(999, 0, 0, 0, 0, 0, 0, 0);
        assert!(result != 0);
    }

    #[test]
    fn test_save_data_delete() {
        let result = cell_save_data_delete2(0, 0, 0, 0, 0, 0, 0);
        assert_eq!(result, 0);
        
        // Invalid version
        let result = cell_save_data_delete2(999, 0, 0, 0, 0, 0, 0);
        assert!(result != 0);
    }

    #[test]
    fn test_save_data_error_codes() {
        assert_eq!(CELL_SAVEDATA_ERROR_CBRESULT, 0x8002b401u32 as i32);
        assert_eq!(CELL_SAVEDATA_ERROR_NODATA, 0x8002b410u32 as i32);
    }
}
