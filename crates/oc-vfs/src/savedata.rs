//! Save data management
//!
//! Handles PS3 save data creation, deletion, and management

use crate::VirtualFileSystem;
use std::path::PathBuf;
use parking_lot::RwLock;
use std::collections::HashMap;

/// Save data type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveDataType {
    /// Normal save data
    Normal,
    /// Trophy data
    Trophy,
    /// System data
    System,
}

/// Save data information
#[derive(Debug, Clone)]
pub struct SaveDataInfo {
    /// Save data directory name (e.g., "BLES00000-SAVEDATA01")
    pub dir_name: String,
    /// Game title
    pub title: String,
    /// Game ID
    pub game_id: String,
    /// Save data type
    pub save_type: SaveDataType,
    /// Directory path
    pub path: PathBuf,
    /// Total size in bytes
    pub size: u64,
    /// Last modified timestamp
    pub modified: Option<std::time::SystemTime>,
}

/// Save data manager
pub struct SaveDataManager {
    /// Cached save data entries
    saves: RwLock<HashMap<String, SaveDataInfo>>,
}

impl SaveDataManager {
    /// Create a new save data manager
    pub fn new() -> Self {
        Self {
            saves: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new save data directory
    pub fn create_save(
        &self,
        vfs: &VirtualFileSystem,
        game_id: &str,
        title: &str,
        save_name: &str,
    ) -> Result<SaveDataInfo, String> {
        // Construct save directory name
        let dir_name = format!("{}-{}", game_id, save_name);
        let virtual_path = format!("/dev_hdd0/savedata/{}", dir_name);

        // Resolve to host path
        let host_path = vfs
            .resolve(&virtual_path)
            .ok_or("Failed to resolve save data path")?;

        // Create directory
        std::fs::create_dir_all(&host_path)
            .map_err(|e| format!("Failed to create save directory: {}", e))?;

        // Create PARAM.SFO
        let param_sfo_path = host_path.join("PARAM.SFO");
        self.create_param_sfo(&param_sfo_path, game_id, title)?;

        let save_info = SaveDataInfo {
            dir_name: dir_name.clone(),
            title: title.to_string(),
            game_id: game_id.to_string(),
            save_type: SaveDataType::Normal,
            path: host_path.clone(),
            size: 0,
            modified: std::fs::metadata(&host_path)
                .ok()
                .and_then(|m| m.modified().ok()),
        };

        // Cache the save info
        self.saves.write().insert(dir_name, save_info.clone());

        tracing::info!("Created save data: {:?}", host_path);

        Ok(save_info)
    }

    /// Delete save data
    pub fn delete_save(&self, vfs: &VirtualFileSystem, dir_name: &str) -> Result<(), String> {
        let virtual_path = format!("/dev_hdd0/savedata/{}", dir_name);

        // Resolve to host path
        let host_path = vfs
            .resolve(&virtual_path)
            .ok_or("Failed to resolve save data path")?;

        // Delete directory
        std::fs::remove_dir_all(&host_path)
            .map_err(|e| format!("Failed to delete save directory: {}", e))?;

        // Remove from cache
        self.saves.write().remove(dir_name);

        tracing::info!("Deleted save data: {:?}", host_path);

        Ok(())
    }

    /// List all save data for a game
    pub fn list_saves(&self, vfs: &VirtualFileSystem, game_id: &str) -> Vec<SaveDataInfo> {
        let virtual_path = "/dev_hdd0/savedata";

        // Resolve to host path
        let host_path = match vfs.resolve(virtual_path) {
            Some(path) => path,
            None => return Vec::new(),
        };

        let mut saves = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&host_path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let dir_name = entry.file_name().to_string_lossy().to_string();

                        // Check if this save belongs to the game
                        if dir_name.starts_with(game_id) {
                            if let Ok(save_info) = self.load_save_info(&entry.path(), &dir_name) {
                                saves.push(save_info);
                            }
                        }
                    }
                }
            }
        }

        saves
    }

    /// Get save data information
    pub fn get_save_info(&self, dir_name: &str) -> Option<SaveDataInfo> {
        self.saves.read().get(dir_name).cloned()
    }

    /// Load save data information from directory
    fn load_save_info(&self, path: &PathBuf, dir_name: &str) -> Result<SaveDataInfo, String> {
        // Try to parse PARAM.SFO
        let param_sfo_path = path.join("PARAM.SFO");
        let (title, game_id) = if param_sfo_path.exists() {
            self.parse_param_sfo(&param_sfo_path)?
        } else {
            // Extract from directory name
            let parts: Vec<&str> = dir_name.split('-').collect();
            let game_id = parts.first().unwrap_or(&"UNKNOWN").to_string();
            (format!("Save Data"), game_id)
        };

        // Calculate directory size
        let size = self.calculate_dir_size(path);

        let modified = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok());

        Ok(SaveDataInfo {
            dir_name: dir_name.to_string(),
            title,
            game_id,
            save_type: SaveDataType::Normal,
            path: path.clone(),
            size,
            modified,
        })
    }

    /// Create PARAM.SFO file
    fn create_param_sfo(&self, path: &PathBuf, game_id: &str, title: &str) -> Result<(), String> {
        // TODO: Implement proper PARAM.SFO format generation
        // PARAM.SFO format specification:
        // - Header: Magic (0x00505346), Version, Key table offset, Data table offset
        // - Index table: entries for each parameter (key offset, data type, data length, etc.)
        // - Key table: null-terminated strings for parameter names
        // - Data table: actual parameter values
        // 
        // Required parameters for save data:
        // - CATEGORY: "SD" (save data)
        // - TITLE: Game title
        // - TITLE_ID: Game ID
        // - SAVEDATA_DIRECTORY: Directory name
        // - DETAIL: Save description (optional)
        // 
        // For now, we create a minimal placeholder file that marks the directory
        // as save data. Real games would expect a properly formatted PARAM.SFO.
        
        let placeholder = format!("PARAM.SFO placeholder\ngame_id={}\ntitle={}\n", game_id, title);
        std::fs::write(path, placeholder)
            .map_err(|e| format!("Failed to create PARAM.SFO: {}", e))?;

        tracing::debug!("Created PARAM.SFO placeholder for {} ({})", title, game_id);
        tracing::warn!("PARAM.SFO is a placeholder - implement proper format for production use");

        Ok(())
    }

    /// Parse PARAM.SFO file
    fn parse_param_sfo(&self, _path: &PathBuf) -> Result<(String, String), String> {
        // TODO: Implement actual PARAM.SFO parsing using ParamSfo struct
        // For now, return placeholder values
        Ok((String::from("Unknown Title"), String::from("UNKNOWN00")))
    }

    /// Calculate directory size recursively
    fn calculate_dir_size(&self, path: &PathBuf) -> u64 {
        let mut size = 0;

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        size += metadata.len();
                    } else if metadata.is_dir() {
                        size += self.calculate_dir_size(&entry.path());
                    }
                }
            }
        }

        size
    }

    /// Initialize save data directory structure
    pub fn init_savedata_directory(&self, vfs: &VirtualFileSystem) -> Result<(), String> {
        let virtual_path = "/dev_hdd0/savedata";

        // Resolve to host path
        let host_path = vfs
            .resolve(virtual_path)
            .ok_or("Failed to resolve savedata path")?;

        // Create directory
        std::fs::create_dir_all(&host_path)
            .map_err(|e| format!("Failed to create savedata directory: {}", e))?;

        tracing::info!("Initialized savedata directory: {:?}", host_path);

        Ok(())
    }
}

impl Default for SaveDataManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_data_manager_creation() {
        let manager = SaveDataManager::new();
        assert!(manager.saves.read().is_empty());
    }

    #[test]
    fn test_save_data_info_structure() {
        let info = SaveDataInfo {
            dir_name: "BLES00000-SAVE01".to_string(),
            title: "Test Game".to_string(),
            game_id: "BLES00000".to_string(),
            save_type: SaveDataType::Normal,
            path: PathBuf::from("/tmp/savedata/BLES00000-SAVE01"),
            size: 1024,
            modified: None,
        };

        assert_eq!(info.game_id, "BLES00000");
        assert_eq!(info.save_type, SaveDataType::Normal);
    }
}
