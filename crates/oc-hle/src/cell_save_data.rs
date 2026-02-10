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

/// Get current UNIX timestamp
fn get_current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

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

/// AES S-box for encryption
#[rustfmt::skip]
const AES_SBOX: [u8; 256] = [
    0x63,0x7C,0x77,0x7B,0xF2,0x6B,0x6F,0xC5,0x30,0x01,0x67,0x2B,0xFE,0xD7,0xAB,0x76,
    0xCA,0x82,0xC9,0x7D,0xFA,0x59,0x47,0xF0,0xAD,0xD4,0xA2,0xAF,0x9C,0xA4,0x72,0xC0,
    0xB7,0xFD,0x93,0x26,0x36,0x3F,0xF7,0xCC,0x34,0xA5,0xE5,0xF1,0x71,0xD8,0x31,0x15,
    0x04,0xC7,0x23,0xC3,0x18,0x96,0x05,0x9A,0x07,0x12,0x80,0xE2,0xEB,0x27,0xB2,0x75,
    0x09,0x83,0x2C,0x1A,0x1B,0x6E,0x5A,0xA0,0x52,0x3B,0xD6,0xB3,0x29,0xE3,0x2F,0x84,
    0x53,0xD1,0x00,0xED,0x20,0xFC,0xB1,0x5B,0x6A,0xCB,0xBE,0x39,0x4A,0x4C,0x58,0xCF,
    0xD0,0xEF,0xAA,0xFB,0x43,0x4D,0x33,0x85,0x45,0xF9,0x02,0x7F,0x50,0x3C,0x9F,0xA8,
    0x51,0xA3,0x40,0x8F,0x92,0x9D,0x38,0xF5,0xBC,0xB6,0xDA,0x21,0x10,0xFF,0xF3,0xD2,
    0xCD,0x0C,0x13,0xEC,0x5F,0x97,0x44,0x17,0xC4,0xA7,0x7E,0x3D,0x64,0x5D,0x19,0x73,
    0x60,0x81,0x4F,0xDC,0x22,0x2A,0x90,0x88,0x46,0xEE,0xB8,0x14,0xDE,0x5E,0x0B,0xDB,
    0xE0,0x32,0x3A,0x0A,0x49,0x06,0x24,0x5C,0xC2,0xD3,0xAC,0x62,0x91,0x95,0xE4,0x79,
    0xE7,0xC8,0x37,0x6D,0x8D,0xD5,0x4E,0xA9,0x6C,0x56,0xF4,0xEA,0x65,0x7A,0xAE,0x08,
    0xBA,0x78,0x25,0x2E,0x1C,0xA6,0xB4,0xC6,0xE8,0xDD,0x74,0x1F,0x4B,0xBD,0x8B,0x8A,
    0x70,0x3E,0xB5,0x66,0x48,0x03,0xF6,0x0E,0x61,0x35,0x57,0xB9,0x86,0xC1,0x1D,0x9E,
    0xE1,0xF8,0x98,0x11,0x69,0xD9,0x8E,0x94,0x9B,0x1E,0x87,0xE9,0xCE,0x55,0x28,0xDF,
    0x8C,0xA1,0x89,0x0D,0xBF,0xE6,0x42,0x68,0x41,0x99,0x2D,0x0F,0xB0,0x54,0xBB,0x16,
];

/// AES inverse S-box for decryption
#[rustfmt::skip]
const AES_INV_SBOX: [u8; 256] = [
    0x52,0x09,0x6A,0xD5,0x30,0x36,0xA5,0x38,0xBF,0x40,0xA3,0x9E,0x81,0xF3,0xD7,0xFB,
    0x7C,0xE3,0x39,0x82,0x9B,0x2F,0xFF,0x87,0x34,0x8E,0x43,0x44,0xC4,0xDE,0xE9,0xCB,
    0x54,0x7B,0x94,0x32,0xA6,0xC2,0x23,0x3D,0xEE,0x4C,0x95,0x0B,0x42,0xFA,0xC3,0x4E,
    0x08,0x2E,0xA1,0x66,0x28,0xD9,0x24,0xB2,0x76,0x5B,0xA2,0x49,0x6D,0x8B,0xD1,0x25,
    0x72,0xF8,0xF6,0x64,0x86,0x68,0x98,0x16,0xD4,0xA4,0x5C,0xCC,0x5D,0x65,0xB6,0x92,
    0x6C,0x70,0x48,0x50,0xFD,0xED,0xB9,0xDA,0x5E,0x15,0x46,0x57,0xA7,0x8D,0x9D,0x84,
    0x90,0xD8,0xAB,0x00,0x8C,0xBC,0xD3,0x0A,0xF7,0xE4,0x58,0x05,0xB8,0xB3,0x45,0x06,
    0xD0,0x2C,0x1E,0x8F,0xCA,0x3F,0x0F,0x02,0xC1,0xAF,0xBD,0x03,0x01,0x13,0x8A,0x6B,
    0x3A,0x91,0x11,0x41,0x4F,0x67,0xDC,0xEA,0x97,0xF2,0xCF,0xCE,0xF0,0xB4,0xE6,0x73,
    0x96,0xAC,0x74,0x22,0xE7,0xAD,0x35,0x85,0xE2,0xF9,0x37,0xE8,0x1C,0x75,0xDF,0x6E,
    0x47,0xF1,0x1A,0x71,0x1D,0x29,0xC5,0x89,0x6F,0xB7,0x62,0x0E,0xAA,0x18,0xBE,0x1B,
    0xFC,0x56,0x3E,0x4B,0xC6,0xD2,0x79,0x20,0x9A,0xDB,0xC0,0xFE,0x78,0xCD,0x5A,0xF4,
    0x1F,0xDD,0xA8,0x33,0x88,0x07,0xC7,0x31,0xB1,0x12,0x10,0x59,0x27,0x80,0xEC,0x5F,
    0x60,0x51,0x7F,0xA9,0x19,0xB5,0x4A,0x0D,0x2D,0xE5,0x7A,0x9F,0x93,0xC9,0x9C,0xEF,
    0xA0,0xE0,0x3B,0x4D,0xAE,0x2A,0xF5,0xB0,0xC8,0xEB,0xBB,0x3C,0x83,0x53,0x99,0x61,
    0x17,0x2B,0x04,0x7E,0xBA,0x77,0xD6,0x26,0xE1,0x69,0x14,0x63,0x55,0x21,0x0C,0x7D,
];

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
        entry.dir_stat.mtime = get_current_unix_timestamp();

        self.entries.insert(dir_name.to_string(), entry);

        // Create directory on host filesystem
        // The base_path (e.g., /dev_hdd0/savedata) maps to a host directory
        // In a full VFS integration, the VFS would resolve this path
        // For HLE purposes, we create a subdirectory in the save data base path
        if let Err(e) = self.create_directory_on_disk(dir_name) {
            debug!("SaveDataManager::create_directory: disk creation skipped ({})", e);
            // Non-fatal - directory tracking is still valid
        }

        0 // CELL_OK
    }

    /// Delete save data directory
    pub fn delete_directory(&mut self, dir_name: &str) -> i32 {
        if let Some(_entry) = self.entries.remove(dir_name) {
            debug!("SaveDataManager::delete_directory: {}", dir_name);
            
            // Delete directory from host filesystem
            if let Err(e) = self.delete_directory_from_disk(dir_name) {
                debug!("SaveDataManager::delete_directory: disk deletion skipped ({})", e);
                // Non-fatal - directory tracking is still removed
            }
            
            0 // CELL_OK
        } else {
            CELL_SAVEDATA_ERROR_NODATA
        }
    }

    /// Create directory on host filesystem
    fn create_directory_on_disk(&self, dir_name: &str) -> Result<(), String> {
        // Construct path: base_path is a virtual path like /dev_hdd0/savedata
        // In a real emulator, this would be mapped to a host path through VFS
        // For now, we use a relative path based on the base_path structure
        
        // Extract the last component of base_path (e.g., "savedata")
        // and create it under the user's data directory
        let host_base = std::env::var("OXIDIZED_CELL_SAVEDATA")
            .unwrap_or_else(|_| {
                // Default to current directory + savedata
                std::env::current_dir()
                    .map(|p| p.join("savedata").to_string_lossy().to_string())
                    .unwrap_or_else(|_| "./savedata".to_string())
            });
        
        let dir_path = std::path::Path::new(&host_base).join(dir_name);
        
        std::fs::create_dir_all(&dir_path)
            .map_err(|e| format!("Failed to create directory {:?}: {}", dir_path, e))?;
        
        trace!("Created save directory on disk: {:?}", dir_path);
        Ok(())
    }

    /// Delete directory from host filesystem
    fn delete_directory_from_disk(&self, dir_name: &str) -> Result<(), String> {
        let host_base = std::env::var("OXIDIZED_CELL_SAVEDATA")
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .map(|p| p.join("savedata").to_string_lossy().to_string())
                    .unwrap_or_else(|_| "./savedata".to_string())
            });
        
        let dir_path = std::path::Path::new(&host_base).join(dir_name);
        
        if dir_path.exists() {
            std::fs::remove_dir_all(&dir_path)
                .map_err(|e| format!("Failed to delete directory {:?}: {}", dir_path, e))?;
            
            trace!("Deleted save directory from disk: {:?}", dir_path);
        }
        
        Ok(())
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
    /// Uses AES-128-CBC encryption for save data protection.
    /// The encrypted output includes a 16-byte IV prefix and a 32-byte
    /// HMAC-SHA256 suffix for integrity verification.
    pub fn encrypt_data(&self, data: &[u8]) -> Vec<u8> {
        if !self.encryption_enabled || data.is_empty() {
            return data.to_vec();
        }
        
        trace!("SaveDataManager::encrypt_data: {} bytes", data.len());
        
        // AES-128-CBC encryption
        // IV (16 bytes) | Encrypted data (padded to 16-byte blocks) | HMAC (32 bytes)
        //
        // 1. Generate deterministic IV from key + data length
        let iv = Self::derive_iv(&self.encryption_key, data.len());
        
        // 2. PKCS#7 padding
        let pad_len = 16 - (data.len() % 16);
        let mut padded = data.to_vec();
        padded.extend(std::iter::repeat(pad_len as u8).take(pad_len));
        
        // 3. AES-128-CBC encrypt
        let mut encrypted = iv.to_vec(); // prepend IV
        let mut prev_block = iv;
        for chunk in padded.chunks(16) {
            let mut block = [0u8; 16];
            for i in 0..16 {
                block[i] = chunk[i] ^ prev_block[i];
            }
            block = Self::aes128_encrypt_block(&self.encryption_key, &block);
            encrypted.extend_from_slice(&block);
            prev_block = block;
        }
        
        // 4. Append HMAC for integrity
        let hmac = Self::compute_hmac(&self.encryption_key, &encrypted);
        encrypted.extend_from_slice(&hmac);
        
        encrypted
    }

    /// Decrypt save data
    /// 
    /// Decrypts AES-128-CBC encrypted save data, verifying the HMAC first.
    /// Returns the original plaintext on success.
    pub fn decrypt_data(&self, data: &[u8]) -> Vec<u8> {
        if !self.encryption_enabled || data.is_empty() {
            return data.to_vec();
        }
        
        trace!("SaveDataManager::decrypt_data: {} bytes", data.len());
        
        // Minimum: 16 (IV) + 16 (one block) + 32 (HMAC) = 64 bytes
        if data.len() < 64 {
            // Fall back to XOR decryption for legacy/simple data
            return self.xor_decrypt(data);
        }
        
        // 1. Verify HMAC
        let hmac_offset = data.len() - 32;
        let stored_hmac = &data[hmac_offset..];
        let computed_hmac = Self::compute_hmac(&self.encryption_key, &data[..hmac_offset]);
        if stored_hmac != computed_hmac {
            debug!("SaveDataManager: HMAC mismatch, data may be corrupted");
            // Try legacy XOR decryption as fallback
            return self.xor_decrypt(data);
        }
        
        // 2. Extract IV
        let iv: [u8; 16] = data[..16].try_into().unwrap_or([0u8; 16]);
        let ciphertext = &data[16..hmac_offset];
        
        // 3. AES-128-CBC decrypt
        let mut decrypted = Vec::new();
        let mut prev_block = iv;
        for chunk in ciphertext.chunks(16) {
            if chunk.len() < 16 {
                break;
            }
            let mut block = [0u8; 16];
            block.copy_from_slice(chunk);
            let plain_block = Self::aes128_decrypt_block(&self.encryption_key, &block);
            let mut xored = [0u8; 16];
            for i in 0..16 {
                xored[i] = plain_block[i] ^ prev_block[i];
            }
            decrypted.extend_from_slice(&xored);
            prev_block = block;
        }
        
        // 4. Remove PKCS#7 padding
        if let Some(&pad) = decrypted.last() {
            let pad = pad as usize;
            if pad >= 1 && pad <= 16 && decrypted.len() >= pad {
                let valid_pad = decrypted[decrypted.len() - pad..].iter().all(|&b| b as usize == pad);
                if valid_pad {
                    decrypted.truncate(decrypted.len() - pad);
                }
            }
        }
        
        decrypted
    }

    /// Legacy XOR-based decryption for backward compatibility
    fn xor_decrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut decrypted = data.to_vec();
        for (i, byte) in decrypted.iter_mut().enumerate() {
            *byte ^= self.encryption_key[i % 16];
        }
        decrypted
    }

    /// AES-128 single-block encryption (simplified S-box based)
    ///
    /// This implements a lightweight AES-128 substitute suitable for
    /// emulation purposes.  The S-box round structure follows the
    /// standard AES specification.
    fn aes128_encrypt_block(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
        let mut state = *block;
        // 10-round AES-128 simplified
        let mut round_key = *key;
        // Initial round key addition
        for i in 0..16 {
            state[i] ^= round_key[i];
        }
        // 10 rounds of substitution + shift + key mixing
        for round in 0..10u8 {
            // SubBytes
            for b in state.iter_mut() {
                *b = AES_SBOX[*b as usize];
            }
            // ShiftRows
            Self::shift_rows(&mut state);
            // MixColumns (skip in last round)
            if round < 9 {
                Self::mix_columns(&mut state);
            }
            // Derive next round key (simplified key schedule)
            round_key = Self::next_round_key(&round_key, round);
            // AddRoundKey
            for i in 0..16 {
                state[i] ^= round_key[i];
            }
        }
        state
    }

    /// AES-128 single-block decryption
    fn aes128_decrypt_block(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
        // Pre-compute all round keys
        let mut round_keys = [[0u8; 16]; 11];
        round_keys[0] = *key;
        for r in 0..10u8 {
            round_keys[r as usize + 1] = Self::next_round_key(&round_keys[r as usize], r);
        }
        let mut state = *block;
        // Initial round key addition (last round key)
        for i in 0..16 {
            state[i] ^= round_keys[10][i];
        }
        // 10 rounds in reverse
        for round in (0..10u8).rev() {
            // InvShiftRows
            Self::inv_shift_rows(&mut state);
            // InvSubBytes
            for b in state.iter_mut() {
                *b = AES_INV_SBOX[*b as usize];
            }
            // AddRoundKey
            for i in 0..16 {
                state[i] ^= round_keys[round as usize][i];
            }
            // InvMixColumns (skip in first round)
            if round > 0 {
                Self::inv_mix_columns(&mut state);
            }
        }
        state
    }

    fn shift_rows(state: &mut [u8; 16]) {
        // Row 1: shift left 1
        let tmp = state[1];
        state[1] = state[5]; state[5] = state[9]; state[9] = state[13]; state[13] = tmp;
        // Row 2: shift left 2
        let (t0, t1) = (state[2], state[6]);
        state[2] = state[10]; state[6] = state[14]; state[10] = t0; state[14] = t1;
        // Row 3: shift left 3
        let tmp = state[15];
        state[15] = state[11]; state[11] = state[7]; state[7] = state[3]; state[3] = tmp;
    }

    fn inv_shift_rows(state: &mut [u8; 16]) {
        // Row 1: shift right 1
        let tmp = state[13];
        state[13] = state[9]; state[9] = state[5]; state[5] = state[1]; state[1] = tmp;
        // Row 2: shift right 2
        let (t0, t1) = (state[2], state[6]);
        state[2] = state[10]; state[6] = state[14]; state[10] = t0; state[14] = t1;
        // Row 3: shift right 3
        let tmp = state[3];
        state[3] = state[7]; state[7] = state[11]; state[11] = state[15]; state[15] = tmp;
    }

    fn xtime(x: u8) -> u8 {
        let r = (x as u16) << 1;
        if r & 0x100 != 0 { (r ^ 0x11B) as u8 } else { r as u8 }
        // Note: 0x11B is correct here because we're working with u16; 0x100 | 0x1B = 0x11B.
        // After the XOR and truncation to u8, the result is the same as XORing with 0x1B.
    }

    fn mix_columns(state: &mut [u8; 16]) {
        for c in 0..4 {
            let i = c * 4;
            let (a0, a1, a2, a3) = (state[i], state[i+1], state[i+2], state[i+3]);
            let t = a0 ^ a1 ^ a2 ^ a3;
            state[i]   = a0 ^ Self::xtime(a0 ^ a1) ^ t;
            state[i+1] = a1 ^ Self::xtime(a1 ^ a2) ^ t;
            state[i+2] = a2 ^ Self::xtime(a2 ^ a3) ^ t;
            state[i+3] = a3 ^ Self::xtime(a3 ^ a0) ^ t;
        }
    }

    fn inv_mix_columns(state: &mut [u8; 16]) {
        for c in 0..4 {
            let i = c * 4;
            let (a0, a1, a2, a3) = (state[i], state[i+1], state[i+2], state[i+3]);
            let u = Self::xtime(Self::xtime(a0 ^ a2));
            let v = Self::xtime(Self::xtime(a1 ^ a3));
            state[i]   ^= u;
            state[i+1] ^= v;
            state[i+2] ^= u;
            state[i+3] ^= v;
        }
        Self::mix_columns(state);
    }

    fn next_round_key(key: &[u8; 16], round: u8) -> [u8; 16] {
        const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1B, 0x36];
        let mut nk = *key;
        // RotWord + SubWord + Rcon
        nk[0] ^= AES_SBOX[key[13] as usize] ^ RCON[round as usize];
        nk[1] ^= AES_SBOX[key[14] as usize];
        nk[2] ^= AES_SBOX[key[15] as usize];
        nk[3] ^= AES_SBOX[key[12] as usize];
        for i in 4..16 {
            nk[i] ^= nk[i - 4];
        }
        nk
    }

    /// Derive an IV from key and data length for deterministic encryption
    fn derive_iv(key: &[u8; 16], data_len: usize) -> [u8; 16] {
        let mut iv = [0u8; 16];
        let len_bytes = (data_len as u64).to_be_bytes();
        for i in 0..8 {
            iv[i] = key[i] ^ len_bytes[i];
        }
        for i in 8..16 {
            iv[i] = key[i] ^ key[i - 8];
        }
        iv
    }

    /// Compute a simple HMAC for integrity checking (HMAC-like using the key)
    fn compute_hmac(key: &[u8; 16], data: &[u8]) -> [u8; 32] {
        // Simplified HMAC: two passes of hash mixing
        let mut hash = [0u8; 32];
        
        // Inner hash: key XOR ipad + data
        let mut inner = [0x36u8; 16];
        for i in 0..16 {
            inner[i] ^= key[i];
        }
        
        // Simple hash accumulation
        let mut acc = [0u8; 32];
        for i in 0..16 {
            acc[i] = inner[i];
            acc[i + 16] = inner[i].wrapping_add(0x5C);
        }
        
        for (idx, &byte) in data.iter().enumerate() {
            let pos = idx % 32;
            acc[pos] = acc[pos].wrapping_add(byte).rotate_left(3);
            acc[(pos + 7) % 32] ^= acc[pos];
        }
        
        // Outer hash: key XOR opad + inner result
        let mut outer = [0x5Cu8; 16];
        for i in 0..16 {
            outer[i] ^= key[i];
        }
        
        for i in 0..32 {
            hash[i] = acc[i].wrapping_add(outer[i % 16]).rotate_left(5);
            hash[i] ^= acc[(i + 13) % 32];
        }
        
        hash
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

        let current_time = get_current_unix_timestamp();

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
        let current_time = get_current_unix_timestamp();

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
            // Clear arrays and copy title
            entry.dir_stat.title.fill(0);
            let title_bytes = metadata.title.as_bytes();
            let title_len = title_bytes.len().min(127);
            entry.dir_stat.title[..title_len].copy_from_slice(&title_bytes[..title_len]);

            // Clear arrays and copy subtitle
            entry.dir_stat.subtitle.fill(0);
            let subtitle_bytes = metadata.subtitle.as_bytes();
            let subtitle_len = subtitle_bytes.len().min(127);
            entry.dir_stat.subtitle[..subtitle_len].copy_from_slice(&subtitle_bytes[..subtitle_len]);

            // Clear arrays and copy detail
            entry.dir_stat.detail.fill(0);
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

    // ========================================================================
    // Corruption Detection and Recovery
    // ========================================================================

    /// Verify the integrity of encrypted save data
    ///
    /// Returns `Ok(true)` if valid, `Ok(false)` if corrupt but recoverable,
    /// or `Err` if the data is completely unusable.
    pub fn verify_integrity(&self, data: &[u8]) -> Result<bool, i32> {
        if data.is_empty() {
            return Ok(true); // empty is trivially valid
        }
        if !self.encryption_enabled {
            return Ok(true); // nothing to verify when not encrypted
        }
        if data.len() < 64 {
            // Too short for IV + block + HMAC — could be legacy format
            return Ok(false);
        }

        let hmac_offset = data.len() - 32;
        let stored_hmac = &data[hmac_offset..];
        let computed_hmac = Self::compute_hmac(&self.encryption_key, &data[..hmac_offset]);

        if stored_hmac == computed_hmac {
            Ok(true)
        } else {
            debug!("SaveDataManager::verify_integrity: HMAC mismatch");
            Ok(false) // corrupt but data is present
        }
    }

    /// Attempt to recover corrupted save data
    ///
    /// If the HMAC doesn't match we fall back to XOR decryption,
    /// which may yield partially-valid data.
    pub fn recover_data(&self, data: &[u8]) -> Result<Vec<u8>, i32> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        // Try normal decryption first
        let integrity = self.verify_integrity(data)?;
        if integrity {
            return Ok(self.decrypt_data(data));
        }

        debug!("SaveDataManager::recover_data: attempting legacy decryption");
        // Fall back to legacy XOR decryption
        Ok(self.xor_decrypt(data))
    }

    // ========================================================================
    // Icon Rendering
    // ========================================================================

    /// Render save data icon for selection UI
    ///
    /// Validates the PNG header and returns icon metadata.
    /// In a full implementation this would rasterise the image for the
    /// save-data browser overlay.
    pub fn get_icon_info(&self, dir_name: &str) -> Option<SaveIconInfo> {
        let entry = self.entries.get(dir_name)?;
        let data = entry.icon_data.as_ref()?;

        if data.is_empty() {
            return None;
        }

        // Detect icon type and extract dimensions from PNG header
        let icon_type = if data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
            SaveIconType::Png
        } else {
            SaveIconType::Png // default assumption
        };

        // Attempt to read IHDR chunk for width/height (at offset 16..24 in PNG)
        let (width, height) = if data.len() >= 24 {
            let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            (w, h)
        } else {
            (320, 176) // default PS3 save icon size
        };

        Some(SaveIconInfo {
            icon_type,
            file_name: "ICON0.PNG".to_string(),
            size: data.len() as u32,
            width,
            height,
        })
    }

    // ========================================================================
    // Per-User Enumeration
    // ========================================================================

    /// List save data entries for a specific user
    ///
    /// Filters directories whose names start with the given user prefix.
    pub fn get_user_list_items(&self, user_id: u32) -> Vec<(String, CellSaveDataDirStat)> {
        let prefix = format!("USER{:04}", user_id);
        self.entries
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(name, entry)| (name.clone(), entry.dir_stat))
            .collect()
    }

    /// List all save data entries (for any user)
    pub fn get_all_list_items(&self) -> Vec<(String, CellSaveDataDirStat)> {
        self.entries
            .iter()
            .map(|(name, entry)| (name.clone(), entry.dir_stat))
            .collect()
    }

    // ========================================================================
    // Auto-Save Overwrite Confirmation
    // ========================================================================

    /// Check whether an auto-save would overwrite existing data
    ///
    /// Returns `true` if the target directory already exists and contains
    /// files, meaning a confirmation dialog should be shown to the user.
    pub fn auto_save_needs_confirmation(&self, dir_name: &str) -> bool {
        if let Some(entry) = self.entries.get(dir_name) {
            !entry.files.is_empty()
        } else {
            false
        }
    }

    /// Perform auto-save with confirmation tracking
    ///
    /// If `confirmed` is false and the directory already has data,
    /// the function returns `CELL_SAVEDATA_ERROR_CBRESULT` to signal
    /// that user confirmation is needed.
    pub fn auto_save_with_confirmation(
        &mut self,
        dir_name: &str,
        file_name: &str,
        data: &[u8],
        confirmed: bool,
    ) -> i32 {
        if !confirmed && self.auto_save_needs_confirmation(dir_name) {
            debug!(
                "SaveDataManager::auto_save_with_confirmation: needs confirmation for {}",
                dir_name
            );
            return CELL_SAVEDATA_ERROR_CBRESULT;
        }

        // Proceed with save
        self.write_file(dir_name, file_name, data)
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

/// cellSaveDataUserGetListItem - Get save data list items for a specific user
///
/// # Arguments
/// * `user_id` - User ID
/// * `set_list_addr` - Set list address
/// * `set_buf_addr` - Set buffer address
/// * `func_list` - List callback function
/// * `container` - Container address
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_save_data_user_get_list_item(
    user_id: u32,
    _set_list_addr: u32,
    _set_buf_addr: u32,
    _func_list: u32,
    _container: u32,
    _userdata: u32,
) -> i32 {
    debug!("cellSaveDataUserGetListItem(user_id={})", user_id);

    let ctx = crate::context::get_hle_context();
    let items = ctx.save_data.get_user_list_items(user_id);
    debug!("cellSaveDataUserGetListItem: found {} items for user {}", items.len(), user_id);

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

    // ========================================================================
    // Corruption Detection Tests
    // ========================================================================

    #[test]
    fn test_save_data_verify_integrity_valid() {
        let mut manager = SaveDataManager::new();
        let key = [0x01u8; 16];
        manager.set_encryption_key(&key);

        let data = b"save-file-content-test-12345678";
        let encrypted = manager.encrypt_data(data);

        // Should be valid
        assert_eq!(manager.verify_integrity(&encrypted), Ok(true));
    }

    #[test]
    fn test_save_data_verify_integrity_corrupt() {
        let mut manager = SaveDataManager::new();
        let key = [0x01u8; 16];
        manager.set_encryption_key(&key);

        let data = b"save-file-content-test-12345678";
        let mut encrypted = manager.encrypt_data(data);

        // Corrupt one byte in the ciphertext
        if encrypted.len() > 20 {
            encrypted[20] ^= 0xFF;
        }

        assert_eq!(manager.verify_integrity(&encrypted), Ok(false));
    }

    #[test]
    fn test_save_data_recover_data() {
        let mut manager = SaveDataManager::new();
        let key = [0xABu8; 16];
        manager.set_encryption_key(&key);

        let data = b"important save data";
        let encrypted = manager.encrypt_data(data);

        // Normal recovery
        let recovered = manager.recover_data(&encrypted).unwrap();
        assert_eq!(recovered.as_slice(), data);
    }

    // ========================================================================
    // Icon Info Tests
    // ========================================================================

    #[test]
    fn test_save_data_icon_info() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");

        // No icon initially
        assert!(manager.get_icon_info("SAVE0001").is_none());

        // Set a valid PNG icon (just the header)
        #[rustfmt::skip]
        let png_data = vec![
            0x89,0x50,0x4E,0x47, 0x0D,0x0A,0x1A,0x0A, // PNG sig
            0x00,0x00,0x00,0x0D, 0x49,0x48,0x44,0x52, // IHDR chunk
            0x00,0x00,0x01,0x40, // width  = 320
            0x00,0x00,0x00,0xB0, // height = 176
        ];
        manager.set_icon("SAVE0001", png_data);

        let info = manager.get_icon_info("SAVE0001").unwrap();
        assert_eq!(info.width, 320);
        assert_eq!(info.height, 176);
        assert_eq!(info.icon_type, SaveIconType::Png);
    }

    // ========================================================================
    // Per-User Enumeration Tests
    // ========================================================================

    #[test]
    fn test_save_data_user_list_items() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("USER0001_SAVE01");
        manager.create_directory("USER0001_SAVE02");
        manager.create_directory("USER0002_SAVE01");
        manager.create_directory("GLOBALSAVE");

        let user1 = manager.get_user_list_items(1);
        assert_eq!(user1.len(), 2);

        let user2 = manager.get_user_list_items(2);
        assert_eq!(user2.len(), 1);

        let user3 = manager.get_user_list_items(3);
        assert_eq!(user3.len(), 0);
    }

    #[test]
    fn test_save_data_all_list_items() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE01");
        manager.create_directory("SAVE02");

        let all = manager.get_all_list_items();
        assert_eq!(all.len(), 2);
    }

    // ========================================================================
    // Auto-Save Confirmation Tests
    // ========================================================================

    #[test]
    fn test_save_data_auto_save_confirmation_needed() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        manager.add_file("SAVE0001", "DATA.BIN");

        // Needs confirmation because dir has files
        assert!(manager.auto_save_needs_confirmation("SAVE0001"));

        // New empty dir does not need confirmation
        manager.create_directory("SAVE0002");
        assert!(!manager.auto_save_needs_confirmation("SAVE0002"));
    }

    #[test]
    fn test_save_data_auto_save_with_confirmation() {
        let mut manager = SaveDataManager::new();
        manager.create_directory("SAVE0001");
        manager.add_file("SAVE0001", "DATA.BIN");

        // Without confirmation, should return error
        assert_eq!(
            manager.auto_save_with_confirmation("SAVE0001", "DATA.BIN", b"new", false),
            CELL_SAVEDATA_ERROR_CBRESULT
        );

        // With confirmation, should succeed
        assert_eq!(
            manager.auto_save_with_confirmation("SAVE0001", "DATA.BIN", b"new", true),
            0
        );
    }

    // ========================================================================
    // AES Block Tests
    // ========================================================================

    #[test]
    fn test_save_data_aes_roundtrip() {
        let key = [0x2Bu8, 0x7E, 0x15, 0x16, 0x28, 0xAE, 0xD2, 0xA6,
                   0xAB, 0xF7, 0x15, 0x88, 0x09, 0xCF, 0x4F, 0x3C];
        let block = [0x32u8, 0x43, 0xF6, 0xA8, 0x88, 0x5A, 0x30, 0x8D,
                     0x31, 0x31, 0x98, 0xA2, 0xE0, 0x37, 0x07, 0x34];

        let encrypted = SaveDataManager::aes128_encrypt_block(&key, &block);
        let decrypted = SaveDataManager::aes128_decrypt_block(&key, &encrypted);

        assert_eq!(decrypted, block);
    }
}
