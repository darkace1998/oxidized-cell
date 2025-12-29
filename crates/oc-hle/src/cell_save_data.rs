//! cellSaveData HLE - Save Data Management
//!
//! This module provides HLE implementations for PS3 save data operations.

use std::collections::HashMap;
use tracing::{debug, trace};

/// VFS backend reference placeholder
/// In a real implementation, this would hold a reference to oc-vfs
type VfsBackend = Option<()>;

/// Encryption key type (128-bit AES key)
type EncryptionKey = [u8; 16];

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
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct SaveDataEntry {
    /// Directory name
    dir_name: String,
    /// Directory stat
    dir_stat: CellSaveDataDirStat,
    /// Files
    files: Vec<String>,
    /// Icon data (PNG)
    icon_data: Option<Vec<u8>>,
    /// Auto-save enabled
    auto_save: bool,
    /// Auto-save interval in seconds (0 = disabled)
    auto_save_interval: u32,
    /// Last auto-save timestamp
    last_auto_save: u64,
}

impl Default for SaveDataEntry {
    fn default() -> Self {
        Self {
            dir_name: String::new(),
            dir_stat: CellSaveDataDirStat::default(),
            files: Vec::new(),
            icon_data: None,
            auto_save: false,
            auto_save_interval: 0,
            last_auto_save: 0,
        }
    }
}

// ============================================================================
// Icon and Metadata Types
// ============================================================================

/// Save icon type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveIconType {
    /// PNG icon
    #[default]
    Png = 0,
    /// Animated PNG
    Apng = 1,
}

/// Save icon information
#[derive(Debug, Clone, Default)]
pub struct SaveIconInfo {
    /// Icon type
    pub icon_type: SaveIconType,
    /// Icon file name
    pub file_name: String,
    /// Icon data size
    pub size: u32,
    /// Icon width
    pub width: u32,
    /// Icon height
    pub height: u32,
}

/// Save metadata
#[derive(Debug, Clone, Default)]
pub struct SaveMetadata {
    /// Title
    pub title: String,
    /// Subtitle
    pub subtitle: String,
    /// Detail/description
    pub detail: String,
    /// User parameter (game-specific)
    pub user_param: u32,
    /// Parental level
    pub parental_level: u32,
    /// Creation time (UNIX timestamp)
    pub created_at: u64,
    /// Modified time (UNIX timestamp)
    pub modified_at: u64,
}

/// Auto-save configuration
#[derive(Debug, Clone)]
pub struct AutoSaveConfig {
    /// Auto-save enabled
    pub enabled: bool,
    /// Interval in seconds (0 = manual only)
    pub interval_secs: u32,
    /// Show notification when auto-saving
    pub show_notification: bool,
    /// Directory name for auto-save
    pub dir_name: String,
}

impl Default for AutoSaveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: 0,
            show_notification: true,
            dir_name: String::new(),
        }
    }
}

/// Save data manager
pub struct SaveDataManager {
    /// Save data entries
    entries: HashMap<String, SaveDataEntry>,
    /// Base path for save data
    base_path: String,
    /// VFS backend (for file operations)
    vfs_backend: VfsBackend,
    /// Encryption enabled
    encryption_enabled: bool,
    /// Default encryption key (per-user)
    encryption_key: EncryptionKey,
    /// Auto-save configuration
    auto_save_config: AutoSaveConfig,
}

impl SaveDataManager {
    /// Create a new save data manager
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            base_path: "/dev_hdd0/savedata".to_string(),
            vfs_backend: None,
            encryption_enabled: true,
            encryption_key: [0u8; 16], // Default key, should be user-specific
            auto_save_config: AutoSaveConfig::default(),
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

        let mut entry = SaveDataEntry::default();
        entry.dir_name = dir_name.to_string();

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

    // ========================================================================
    // VFS Backend Integration
    // ========================================================================

    /// Connect to VFS backend
    /// 
    /// This would integrate with oc-vfs for actual file system operations.
    /// For now, this is a stub implementation.
    pub fn connect_vfs_backend(&mut self, _backend: VfsBackend) -> i32 {
        debug!("SaveDataManager::connect_vfs_backend");
        
        // In a real implementation:
        // 1. Store the VFS backend reference
        // 2. Verify VFS is properly initialized
        // 3. Set up save data mount points
        
        self.vfs_backend = None; // Would store actual backend
        
        0 // CELL_OK
    }

    /// Read file from save directory (through VFS)
    pub fn read_file(&self, dir_name: &str, file_name: &str) -> Result<Vec<u8>, i32> {
        if !self.directory_exists(dir_name) {
            return Err(CELL_SAVEDATA_ERROR_NODATA);
        }
        
        debug!("SaveDataManager::read_file: {}/{}", dir_name, file_name);
        
        // In a real implementation, this would:
        // 1. Construct full path through VFS
        // 2. Read file through VFS backend
        // 3. Decrypt if encrypted
        
        // For HLE, return empty data
        Ok(Vec::new())
    }

    /// Write file to save directory (through VFS)
    pub fn write_file(&mut self, dir_name: &str, file_name: &str, data: &[u8]) -> i32 {
        // Ensure directory exists
        if !self.directory_exists(dir_name) {
            let result = self.create_directory(dir_name);
            if result != 0 {
                return result;
            }
        }
        
        debug!("SaveDataManager::write_file: {}/{}, {} bytes", dir_name, file_name, data.len());
        
        // In a real implementation, this would:
        // 1. Construct full path through VFS
        // 2. Encrypt data if needed
        // 3. Write file through VFS backend
        // 4. Update directory stat
        
        // Add file to tracking
        let _ = self.add_file(dir_name, file_name);
        
        0 // CELL_OK
    }

    /// Delete file from save directory (through VFS)
    pub fn delete_file(&mut self, dir_name: &str, file_name: &str) -> i32 {
        if !self.directory_exists(dir_name) {
            return CELL_SAVEDATA_ERROR_NODATA;
        }
        
        debug!("SaveDataManager::delete_file: {}/{}", dir_name, file_name);
        
        // In a real implementation, this would:
        // 1. Construct full path through VFS
        // 2. Delete file through VFS backend
        // 3. Update directory stat
        
        // Remove from tracking
        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.files.retain(|f| f != file_name);
        }
        
        0 // CELL_OK
    }

    // ========================================================================
    // Encryption/Decryption
    // ========================================================================

    /// Enable or disable encryption
    pub fn set_encryption_enabled(&mut self, enabled: bool) {
        debug!("SaveDataManager::set_encryption_enabled: {}", enabled);
        self.encryption_enabled = enabled;
    }

    /// Check if encryption is enabled
    pub fn is_encryption_enabled(&self) -> bool {
        self.encryption_enabled
    }

    /// Set encryption key
    pub fn set_encryption_key(&mut self, key: &[u8]) -> i32 {
        if key.len() != 16 {
            return CELL_SAVEDATA_ERROR_PARAM;
        }
        
        debug!("SaveDataManager::set_encryption_key: key length={}", key.len());
        self.encryption_key.copy_from_slice(key);
        
        0 // CELL_OK
    }

    /// Encrypt save data
    /// 
    /// Uses AES-128 encryption for save data protection.
    /// In a real implementation, this would use proper AES encryption.
    pub fn encrypt_data(&self, data: &[u8]) -> Vec<u8> {
        if !self.encryption_enabled {
            return data.to_vec();
        }
        
        trace!("SaveDataManager::encrypt_data: {} bytes", data.len());
        
        // For HLE, we simulate encryption with a simple XOR
        // Real implementation would use AES-128-CBC or similar
        let mut encrypted = data.to_vec();
        for (i, byte) in encrypted.iter_mut().enumerate() {
            *byte ^= self.encryption_key[i % 16];
        }
        
        encrypted
    }

    /// Decrypt save data
    /// 
    /// Decrypts AES-128 encrypted save data.
    /// In a real implementation, this would use proper AES decryption.
    pub fn decrypt_data(&self, data: &[u8]) -> Vec<u8> {
        if !self.encryption_enabled {
            return data.to_vec();
        }
        
        trace!("SaveDataManager::decrypt_data: {} bytes", data.len());
        
        // For HLE, encryption is symmetric XOR, so decrypt is the same
        self.encrypt_data(data)
    }

    /// Get encryption key
    pub fn get_encryption_key(&self) -> &EncryptionKey {
        &self.encryption_key
    }

    // ========================================================================
    // Auto-Save Support
    // ========================================================================

    /// Configure auto-save
    pub fn configure_auto_save(&mut self, config: AutoSaveConfig) -> i32 {
        debug!(
            "SaveDataManager::configure_auto_save: enabled={}, interval={}s, dir={}",
            config.enabled, config.interval_secs, config.dir_name
        );

        self.auto_save_config = config;

        0 // CELL_OK
    }

    /// Get auto-save configuration
    pub fn get_auto_save_config(&self) -> &AutoSaveConfig {
        &self.auto_save_config
    }

    /// Enable auto-save for a directory
    pub fn enable_auto_save(&mut self, dir_name: &str, interval_secs: u32) -> i32 {
        if !self.directory_exists(dir_name) {
            return CELL_SAVEDATA_ERROR_NODATA;
        }

        debug!(
            "SaveDataManager::enable_auto_save: dir={}, interval={}s",
            dir_name, interval_secs
        );

        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.auto_save = true;
            entry.auto_save_interval = interval_secs;
        }

        self.auto_save_config.enabled = true;
        self.auto_save_config.dir_name = dir_name.to_string();
        self.auto_save_config.interval_secs = interval_secs;

        0 // CELL_OK
    }

    /// Disable auto-save for a directory
    pub fn disable_auto_save(&mut self, dir_name: &str) -> i32 {
        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.auto_save = false;
            entry.auto_save_interval = 0;
            debug!("SaveDataManager::disable_auto_save: dir={}", dir_name);
        }

        if self.auto_save_config.dir_name == dir_name {
            self.auto_save_config.enabled = false;
        }

        0 // CELL_OK
    }

    /// Check if auto-save is enabled for a directory
    pub fn is_auto_save_enabled(&self, dir_name: &str) -> bool {
        self.entries.get(dir_name)
            .map(|e| e.auto_save)
            .unwrap_or(false)
    }

    /// Trigger auto-save check (called periodically)
    /// Returns the directory name if auto-save should be triggered
    pub fn check_auto_save(&mut self) -> Option<String> {
        if !self.auto_save_config.enabled {
            return None;
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let dir_name = self.auto_save_config.dir_name.clone();
        if let Some(entry) = self.entries.get_mut(&dir_name) {
            if entry.auto_save && entry.auto_save_interval > 0 {
                let elapsed = current_time.saturating_sub(entry.last_auto_save);
                if elapsed >= entry.auto_save_interval as u64 {
                    entry.last_auto_save = current_time;
                    return Some(dir_name);
                }
            }
        }

        None
    }

    /// Update last auto-save timestamp
    pub fn update_auto_save_timestamp(&mut self, dir_name: &str) -> i32 {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.last_auto_save = current_time;
            0 // CELL_OK
        } else {
            CELL_SAVEDATA_ERROR_NODATA
        }
    }

    // ========================================================================
    // Icon and Metadata Handling
    // ========================================================================

    /// Set save icon data
    pub fn set_icon(&mut self, dir_name: &str, icon_data: Vec<u8>) -> i32 {
        if !self.directory_exists(dir_name) {
            return CELL_SAVEDATA_ERROR_NODATA;
        }

        debug!(
            "SaveDataManager::set_icon: dir={}, size={} bytes",
            dir_name, icon_data.len()
        );

        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.icon_data = Some(icon_data);
        }

        0 // CELL_OK
    }

    /// Get save icon data
    pub fn get_icon(&self, dir_name: &str) -> Option<&[u8]> {
        self.entries.get(dir_name)
            .and_then(|e| e.icon_data.as_ref())
            .map(|v| v.as_slice())
    }

    /// Check if save has icon
    pub fn has_icon(&self, dir_name: &str) -> bool {
        self.entries.get(dir_name)
            .and_then(|e| e.icon_data.as_ref())
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    /// Remove save icon
    pub fn remove_icon(&mut self, dir_name: &str) -> i32 {
        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.icon_data = None;
            debug!("SaveDataManager::remove_icon: dir={}", dir_name);
            0 // CELL_OK
        } else {
            CELL_SAVEDATA_ERROR_NODATA
        }
    }

    /// Set save metadata (title, subtitle, detail)
    pub fn set_metadata(&mut self, dir_name: &str, metadata: &SaveMetadata) -> i32 {
        if !self.directory_exists(dir_name) {
            return CELL_SAVEDATA_ERROR_NODATA;
        }

        debug!(
            "SaveDataManager::set_metadata: dir={}, title={}",
            dir_name, metadata.title
        );

        if let Some(entry) = self.entries.get_mut(dir_name) {
            // Copy title
            let title_bytes = metadata.title.as_bytes();
            let title_len = title_bytes.len().min(127);
            entry.dir_stat.title[..title_len].copy_from_slice(&title_bytes[..title_len]);

            // Copy subtitle
            let subtitle_bytes = metadata.subtitle.as_bytes();
            let subtitle_len = subtitle_bytes.len().min(127);
            entry.dir_stat.subtitle[..subtitle_len].copy_from_slice(&subtitle_bytes[..subtitle_len]);

            // Copy detail
            let detail_bytes = metadata.detail.as_bytes();
            let detail_len = detail_bytes.len().min(1023);
            entry.dir_stat.detail[..detail_len].copy_from_slice(&detail_bytes[..detail_len]);

            // Update modified time
            entry.dir_stat.mtime = metadata.modified_at;
        }

        0 // CELL_OK
    }

    /// Get save metadata
    pub fn get_metadata(&self, dir_name: &str) -> Option<SaveMetadata> {
        let entry = self.entries.get(dir_name)?;

        // Extract title (find null terminator or use full array)
        let title = extract_string_from_bytes(&entry.dir_stat.title);
        let subtitle = extract_string_from_bytes(&entry.dir_stat.subtitle);
        let detail = extract_string_from_bytes(&entry.dir_stat.detail);

        Some(SaveMetadata {
            title,
            subtitle,
            detail,
            user_param: 0,
            parental_level: 0,
            created_at: entry.dir_stat.mtime,
            modified_at: entry.dir_stat.mtime,
        })
    }

    /// Get save data size in KB
    pub fn get_save_size_kb(&self, dir_name: &str) -> Option<u64> {
        self.entries.get(dir_name).map(|e| e.dir_stat.file_size_kb)
    }

    /// Set save data size in KB
    pub fn set_save_size_kb(&mut self, dir_name: &str, size_kb: u64) -> i32 {
        if let Some(entry) = self.entries.get_mut(dir_name) {
            entry.dir_stat.file_size_kb = size_kb;
            0 // CELL_OK
        } else {
            CELL_SAVEDATA_ERROR_NODATA
        }
    }
}

/// Helper function to extract string from null-terminated byte array
fn extract_string_from_bytes(bytes: &[u8]) -> String {
    let null_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..null_pos]).to_string()
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

    // Get save data list from global manager
    let _directories = crate::context::get_hle_context().save_data.list_directories();
    
    // Note: Calling callbacks and handling file operations requires
    // memory subsystem integration to read callback addresses and invoke them

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

    // Access global manager for save operations
    // Note: Actual save operations require VFS and memory integration
    let _base_path = crate::context::get_hle_context().save_data.get_base_path();

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

    // Note: Deletion through global manager requires reading directory name
    // from memory and invoking callbacks

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

    // Access global manager for fixed save data operations
    let _directory_count = crate::context::get_hle_context().save_data.directory_count();

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

    // Access global manager for fixed save data operations
    let _base_path = crate::context::get_hle_context().save_data.get_base_path();

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

    // ========================================================================
    // VFS Backend Tests
    // ========================================================================

    #[test]
    fn test_save_data_manager_vfs_connection() {
        let mut manager = SaveDataManager::new();
        assert_eq!(manager.connect_vfs_backend(None), 0);
    }

    #[test]
    fn test_save_data_manager_file_operations() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        // Write file
        let data = b"test data";
        assert_eq!(manager.write_file("SAVE0001", "DATA.BIN", data), 0);
        
        // Read file
        let result = manager.read_file("SAVE0001", "DATA.BIN");
        assert!(result.is_ok());
        
        // Delete file
        assert_eq!(manager.delete_file("SAVE0001", "DATA.BIN"), 0);
    }

    #[test]
    fn test_save_data_manager_file_operations_errors() {
        let mut manager = SaveDataManager::new();
        
        // Read from non-existent directory
        assert!(manager.read_file("NONEXISTENT", "DATA.BIN").is_err());
        
        // Delete from non-existent directory
        assert!(manager.delete_file("NONEXISTENT", "DATA.BIN") != 0);
    }

    // ========================================================================
    // Encryption Tests
    // ========================================================================

    #[test]
    fn test_save_data_manager_encryption_enabled() {
        let mut manager = SaveDataManager::new();
        
        // Encryption enabled by default
        assert!(manager.is_encryption_enabled());
        
        // Disable encryption
        manager.set_encryption_enabled(false);
        assert!(!manager.is_encryption_enabled());
        
        // Re-enable encryption
        manager.set_encryption_enabled(true);
        assert!(manager.is_encryption_enabled());
    }

    #[test]
    fn test_save_data_manager_encryption_key() {
        let mut manager = SaveDataManager::new();
        
        let key = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                   0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];
        
        assert_eq!(manager.set_encryption_key(&key), 0);
        assert_eq!(manager.get_encryption_key(), &key);
    }

    #[test]
    fn test_save_data_manager_encryption_key_invalid() {
        let mut manager = SaveDataManager::new();
        
        // Too short
        let short_key = [0x01, 0x02, 0x03];
        assert!(manager.set_encryption_key(&short_key) != 0);
        
        // Too long
        let long_key = [0u8; 32];
        assert!(manager.set_encryption_key(&long_key) != 0);
    }

    #[test]
    fn test_save_data_manager_encrypt_decrypt() {
        let mut manager = SaveDataManager::new();
        
        let key = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                   0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];
        manager.set_encryption_key(&key);
        
        let original_data = b"Hello, save data encryption!";
        
        // Encrypt
        let encrypted = manager.encrypt_data(original_data);
        
        // Should be different from original
        assert_ne!(encrypted.as_slice(), original_data);
        
        // Decrypt
        let decrypted = manager.decrypt_data(&encrypted);
        
        // Should match original
        assert_eq!(decrypted.as_slice(), original_data);
    }

    #[test]
    fn test_save_data_manager_encrypt_disabled() {
        let mut manager = SaveDataManager::new();
        manager.set_encryption_enabled(false);
        
        let data = b"test data";
        
        // With encryption disabled, data should be unchanged
        let encrypted = manager.encrypt_data(data);
        assert_eq!(encrypted.as_slice(), data);
        
        let decrypted = manager.decrypt_data(data);
        assert_eq!(decrypted.as_slice(), data);
    }

    #[test]
    fn test_save_data_manager_encrypt_empty_data() {
        let manager = SaveDataManager::new();
        
        let empty_data: &[u8] = &[];
        let encrypted = manager.encrypt_data(empty_data);
        assert_eq!(encrypted.len(), 0);
        
        let decrypted = manager.decrypt_data(&encrypted);
        assert_eq!(decrypted.len(), 0);
    }

    // ========================================================================
    // Auto-Save Tests
    // ========================================================================

    #[test]
    fn test_save_data_manager_auto_save_config() {
        let mut manager = SaveDataManager::new();
        
        // Default config
        let config = manager.get_auto_save_config();
        assert!(!config.enabled);
        assert_eq!(config.interval_secs, 0);
        
        // Configure auto-save
        let new_config = AutoSaveConfig {
            enabled: true,
            interval_secs: 300,
            show_notification: true,
            dir_name: "AUTOSAVE".to_string(),
        };
        
        assert_eq!(manager.configure_auto_save(new_config), 0);
        
        let config = manager.get_auto_save_config();
        assert!(config.enabled);
        assert_eq!(config.interval_secs, 300);
        assert_eq!(config.dir_name, "AUTOSAVE");
    }

    #[test]
    fn test_save_data_manager_enable_disable_auto_save() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        // Not enabled initially
        assert!(!manager.is_auto_save_enabled("SAVE0001"));
        
        // Enable auto-save
        assert_eq!(manager.enable_auto_save("SAVE0001", 60), 0);
        assert!(manager.is_auto_save_enabled("SAVE0001"));
        
        // Disable auto-save
        assert_eq!(manager.disable_auto_save("SAVE0001"), 0);
        assert!(!manager.is_auto_save_enabled("SAVE0001"));
    }

    #[test]
    fn test_save_data_manager_auto_save_nonexistent() {
        let mut manager = SaveDataManager::new();
        
        // Enable on non-existent directory should fail
        assert!(manager.enable_auto_save("NONEXISTENT", 60) != 0);
    }

    #[test]
    fn test_save_data_manager_update_auto_save_timestamp() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        assert_eq!(manager.update_auto_save_timestamp("SAVE0001"), 0);
        
        // Non-existent should fail
        assert!(manager.update_auto_save_timestamp("NONEXISTENT") != 0);
    }

    // ========================================================================
    // Icon and Metadata Tests
    // ========================================================================

    #[test]
    fn test_save_data_manager_icon() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        // No icon initially
        assert!(!manager.has_icon("SAVE0001"));
        assert!(manager.get_icon("SAVE0001").is_none());
        
        // Set icon
        let icon_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
        assert_eq!(manager.set_icon("SAVE0001", icon_data.clone()), 0);
        
        assert!(manager.has_icon("SAVE0001"));
        let retrieved = manager.get_icon("SAVE0001");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), icon_data.as_slice());
        
        // Remove icon
        assert_eq!(manager.remove_icon("SAVE0001"), 0);
        assert!(!manager.has_icon("SAVE0001"));
    }

    #[test]
    fn test_save_data_manager_icon_nonexistent() {
        let mut manager = SaveDataManager::new();
        
        // Set on non-existent directory should fail
        assert!(manager.set_icon("NONEXISTENT", vec![0x00]) != 0);
        assert!(manager.remove_icon("NONEXISTENT") != 0);
    }

    #[test]
    fn test_save_data_manager_metadata() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        let metadata = SaveMetadata {
            title: "My Save".to_string(),
            subtitle: "Chapter 5".to_string(),
            detail: "Level 42, 99% complete".to_string(),
            user_param: 0,
            parental_level: 0,
            created_at: 1700000000,
            modified_at: 1700001000,
        };
        
        assert_eq!(manager.set_metadata("SAVE0001", &metadata), 0);
        
        let retrieved = manager.get_metadata("SAVE0001");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "My Save");
        assert_eq!(retrieved.subtitle, "Chapter 5");
        assert_eq!(retrieved.detail, "Level 42, 99% complete");
    }

    #[test]
    fn test_save_data_manager_metadata_nonexistent() {
        let mut manager = SaveDataManager::new();
        
        let metadata = SaveMetadata::default();
        assert!(manager.set_metadata("NONEXISTENT", &metadata) != 0);
        assert!(manager.get_metadata("NONEXISTENT").is_none());
    }

    #[test]
    fn test_save_data_manager_save_size() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        
        // Default size
        assert_eq!(manager.get_save_size_kb("SAVE0001"), Some(0));
        
        // Set size
        assert_eq!(manager.set_save_size_kb("SAVE0001", 1024), 0);
        assert_eq!(manager.get_save_size_kb("SAVE0001"), Some(1024));
        
        // Non-existent
        assert!(manager.get_save_size_kb("NONEXISTENT").is_none());
        assert!(manager.set_save_size_kb("NONEXISTENT", 100) != 0);
    }

    #[test]
    fn test_save_icon_type_enum() {
        assert_eq!(SaveIconType::Png as u32, 0);
        assert_eq!(SaveIconType::Apng as u32, 1);
    }
}
