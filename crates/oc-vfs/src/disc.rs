//! Game disc image management
//!
//! Handles mounting and managing PS3 game disc images (ISO files)

use crate::devices::bdvd::BdvdDevice;
use crate::formats::iso::{IsoReader, IsoVolume};
use crate::formats::sfo::Sfo;
use crate::VirtualFileSystem;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;
use std::io::Cursor;

/// Disc image format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscFormat {
    /// ISO 9660 image
    Iso,
    /// Folder structure (extracted disc)
    Folder,
}

/// Disc image manager
pub struct DiscManager {
    /// BDVD device
    bdvd: Arc<RwLock<BdvdDevice>>,
    /// Currently mounted disc
    current_disc: RwLock<Option<DiscInfo>>,
}

/// Information about a mounted disc
#[derive(Debug, Clone)]
pub struct DiscInfo {
    /// Path to the disc image or folder
    pub path: PathBuf,
    /// Disc format
    pub format: DiscFormat,
    /// Volume information (for ISO)
    pub volume: Option<IsoVolume>,
    /// Game title (from PARAM.SFO if available)
    pub title: Option<String>,
    /// Game ID (from directory structure)
    pub game_id: Option<String>,
}

impl DiscManager {
    /// Create a new disc manager
    pub fn new() -> Self {
        Self {
            bdvd: Arc::new(RwLock::new(BdvdDevice::new())),
            current_disc: RwLock::new(None),
        }
    }

    /// Mount a disc image
    pub fn mount_disc(&self, vfs: &VirtualFileSystem, disc_path: PathBuf) -> Result<(), String> {
        // Determine disc format
        let format = if disc_path.is_dir() {
            DiscFormat::Folder
        } else if disc_path.extension().and_then(|s| s.to_str()) == Some("iso") {
            DiscFormat::Iso
        } else {
            return Err(format!("Unsupported disc format: {:?}", disc_path));
        };

        // Parse ISO if needed
        let volume = if format == DiscFormat::Iso {
            let mut iso_reader = IsoReader::new(disc_path.clone());
            match iso_reader.open() {
                Ok(_) => iso_reader.volume().cloned(),
                Err(e) => {
                    tracing::warn!("Failed to parse ISO volume: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Extract game information
        let (title, game_id) = self.extract_game_info(&disc_path, format);

        // Mount the disc to BDVD device
        let mut bdvd = self.bdvd.write();
        bdvd.mount(disc_path.clone())?;

        // Update VFS mount
        vfs.mount("/dev_bdvd", disc_path.clone());

        // Store disc info
        let disc_info = DiscInfo {
            path: disc_path,
            format,
            volume,
            title,
            game_id,
        };

        *self.current_disc.write() = Some(disc_info.clone());

        tracing::info!(
            "Mounted disc: format={:?}, title={:?}, game_id={:?}",
            disc_info.format,
            disc_info.title,
            disc_info.game_id
        );

        Ok(())
    }

    /// Unmount the current disc
    pub fn unmount_disc(&self, vfs: &VirtualFileSystem) {
        let mut bdvd = self.bdvd.write();
        bdvd.unmount();

        vfs.unmount("/dev_bdvd");
        *self.current_disc.write() = None;

        tracing::info!("Disc unmounted");
    }

    /// Check if a disc is mounted
    pub fn is_disc_mounted(&self) -> bool {
        self.current_disc.read().is_some()
    }

    /// Get information about the current disc
    pub fn disc_info(&self) -> Option<DiscInfo> {
        self.current_disc.read().clone()
    }

    /// Extract game information from disc structure
    fn extract_game_info(&self, disc_path: &PathBuf, format: DiscFormat) -> (Option<String>, Option<String>) {
        // For folder format, look for PS3_GAME directory
        if format == DiscFormat::Folder {
            let ps3_game_path = disc_path.join("PS3_GAME");
            if ps3_game_path.exists() {
                // Try to find PARAM.SFO
                let param_sfo_path = ps3_game_path.join("PARAM.SFO");
                if param_sfo_path.exists() {
                    return self.parse_param_sfo_file(&param_sfo_path);
                }
            }
            // Fallback: extract game ID from path
            let game_id = disc_path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            return (None, game_id);
        }

        // For ISO format, read PARAM.SFO directly from the ISO
        if format == DiscFormat::Iso {
            let mut iso_reader = IsoReader::new(disc_path.clone());
            if iso_reader.open().is_ok() {
                // Try to read PARAM.SFO from ISO
                if let Ok(sfo_data) = iso_reader.read_file("/PS3_GAME/PARAM.SFO") {
                    return self.parse_param_sfo_data(&sfo_data);
                }
            }
        }

        (None, None)
    }

    /// Parse PARAM.SFO from file path
    fn parse_param_sfo_file(&self, path: &PathBuf) -> (Option<String>, Option<String>) {
        match std::fs::File::open(path) {
            Ok(file) => {
                let mut reader = std::io::BufReader::new(file);
                self.parse_param_sfo_reader(&mut reader)
            }
            Err(e) => {
                tracing::warn!("Failed to open PARAM.SFO: {}", e);
                (None, None)
            }
        }
    }

    /// Parse PARAM.SFO from raw data
    fn parse_param_sfo_data(&self, data: &[u8]) -> (Option<String>, Option<String>) {
        let mut cursor = Cursor::new(data);
        self.parse_param_sfo_reader(&mut cursor)
    }

    /// Parse PARAM.SFO from any reader
    fn parse_param_sfo_reader<R: std::io::Read + std::io::Seek>(&self, reader: &mut R) -> (Option<String>, Option<String>) {
        match Sfo::parse(reader) {
            Ok(sfo) => {
                let title = sfo.title().map(|s| s.to_string());
                let game_id = sfo.title_id().map(|s| s.to_string());
                tracing::debug!("Parsed PARAM.SFO: title={:?}, game_id={:?}", title, game_id);
                (title, game_id)
            }
            Err(e) => {
                tracing::warn!("Failed to parse PARAM.SFO: {}", e);
                (None, None)
            }
        }
    }

    /// Verify disc integrity (basic check)
    pub fn verify_disc(&self) -> Result<bool, String> {
        let disc_info = self.current_disc.read();
        let disc_info = disc_info.as_ref().ok_or("No disc mounted")?;

        match disc_info.format {
            DiscFormat::Folder => {
                // Check for PS3_GAME directory
                let ps3_game_path = disc_info.path.join("PS3_GAME");
                Ok(ps3_game_path.exists())
            }
            DiscFormat::Iso => {
                // Check if ISO file exists and has valid volume
                Ok(disc_info.path.exists() && disc_info.volume.is_some())
            }
        }
    }
}

impl Default for DiscManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::sfo::SfoBuilder;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_disc_manager_creation() {
        let manager = DiscManager::new();
        assert!(!manager.is_disc_mounted());
        assert!(manager.disc_info().is_none());
    }

    #[test]
    fn test_disc_format_detection() {
        // This would require actual test files, so we just test the structure
        let manager = DiscManager::new();
        assert!(!manager.is_disc_mounted());
    }

    #[test]
    fn test_extract_game_info_from_folder() {
        let manager = DiscManager::new();

        // Create a temporary directory structure
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let disc_path = temp_dir.path().to_path_buf();
        let ps3_game_path = disc_path.join("PS3_GAME");
        std::fs::create_dir_all(&ps3_game_path).expect("Failed to create PS3_GAME dir");

        // Create a PARAM.SFO file
        let sfo_data = SfoBuilder::new()
            .title("Test Game Title")
            .title_id("BLES12345")
            .category("DG")
            .generate();

        let param_sfo_path = ps3_game_path.join("PARAM.SFO");
        let mut file = std::fs::File::create(&param_sfo_path).expect("Failed to create PARAM.SFO");
        file.write_all(&sfo_data).expect("Failed to write PARAM.SFO");

        // Extract game info
        let (title, game_id) = manager.extract_game_info(&disc_path, DiscFormat::Folder);

        assert_eq!(title, Some("Test Game Title".to_string()));
        assert_eq!(game_id, Some("BLES12345".to_string()));
    }

    #[test]
    fn test_extract_game_info_missing_sfo() {
        let manager = DiscManager::new();

        // Create a temporary directory without PARAM.SFO
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let disc_path = temp_dir.path().to_path_buf();

        // Extract game info - should fall back to path-based extraction
        let (title, game_id) = manager.extract_game_info(&disc_path, DiscFormat::Folder);

        assert!(title.is_none());
        // game_id will be the temp dir name
        assert!(game_id.is_some());
    }

    #[test]
    fn test_parse_param_sfo_reader() {
        let manager = DiscManager::new();

        // Create SFO data
        let sfo_data = SfoBuilder::new()
            .title("Another Game")
            .title_id("NPUB00001")
            .version("01.00")
            .generate();

        // Parse using the helper
        let (title, game_id) = manager.parse_param_sfo_data(&sfo_data);

        assert_eq!(title, Some("Another Game".to_string()));
        assert_eq!(game_id, Some("NPUB00001".to_string()));
    }
}
