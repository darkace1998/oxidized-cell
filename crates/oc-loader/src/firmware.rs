//! PS3 Firmware file support
//!
//! This module provides parsing and handling of PS3 firmware files.

use oc_core::error::LoaderError;
use tracing::{debug, info, warn};

/// PUP (PlayStation Update Package) file magic
pub const PUP_MAGIC: [u8; 8] = [0x53, 0x43, 0x45, 0x55, 0x46, 0x00, 0x00, 0x00]; // "SCEUF\0\0\0"

/// PUP file header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PupHeader {
    pub magic: [u8; 8],
    pub package_version: u64,
    pub image_version: u64,
    pub file_count: u64,
    pub header_length: u64,
    pub data_length: u64,
}

/// PUP file entry
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PupFileEntry {
    pub entry_id: u64,
    pub data_offset: u64,
    pub data_length: u64,
    pub padding: u64,
}

/// PUP hash entry
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PupHashEntry {
    pub entry_id: u64,
    pub hash: [u8; 20],
    pub padding: [u8; 4],
}

/// Known PUP entry IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum PupEntryId {
    /// Update version information
    UpdateVersion = 0x100,
    /// LV0 (bootloader)
    Lv0 = 0x200,
    /// LV1 (hypervisor)
    Lv1 = 0x201,
    /// LV2 (kernel/lv2_kernel.self)
    Lv2Kernel = 0x202,
    /// VSH modules
    Vsh = 0x300,
    /// Core OS files
    CoreOs = 0x301,
    /// Package metadata
    PkgMetadata = 0x400,
    /// Unknown entry
    Unknown = 0xFFFF,
}

impl From<u64> for PupEntryId {
    fn from(value: u64) -> Self {
        match value {
            0x100 => Self::UpdateVersion,
            0x200 => Self::Lv0,
            0x201 => Self::Lv1,
            0x202 => Self::Lv2Kernel,
            0x300 => Self::Vsh,
            0x301 => Self::CoreOs,
            0x400 => Self::PkgMetadata,
            _ => Self::Unknown,
        }
    }
}

/// Firmware version information
#[derive(Debug, Clone)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u16,
    pub build: u32,
}

impl FirmwareVersion {
    /// Parse from a version string (e.g., "04.90")
    pub fn parse(version_str: &str) -> Option<Self> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() >= 2 {
            let major = parts[0].parse().ok()?;
            let minor = parts[1].parse().ok()?;
            Some(Self {
                major,
                minor,
                patch: 0,
                build: 0,
            })
        } else {
            None
        }
    }

    /// Parse from a u64 version number
    pub fn from_u64(version: u64) -> Self {
        Self {
            major: ((version >> 56) & 0xFF) as u8,
            minor: ((version >> 48) & 0xFF) as u8,
            patch: ((version >> 32) & 0xFFFF) as u16,
            build: (version & 0xFFFFFFFF) as u32,
        }
    }

    /// Convert to display string
    pub fn to_string(&self) -> String {
        format!("{}.{:02}", self.major, self.minor)
    }
}

/// Firmware status for detecting and reporting firmware state
#[derive(Debug, Clone)]
pub struct FirmwareStatus {
    /// Whether firmware is installed
    pub installed: bool,
    /// Installed firmware version (if any)
    pub version: Option<FirmwareVersion>,
    /// Path to installed firmware
    pub path: Option<std::path::PathBuf>,
    /// List of missing components
    pub missing_components: Vec<String>,
    /// List of available components
    pub available_components: Vec<String>,
}

impl FirmwareStatus {
    /// Create a new empty firmware status
    pub fn new() -> Self {
        Self {
            installed: false,
            version: None,
            path: None,
            missing_components: Vec::new(),
            available_components: Vec::new(),
        }
    }

    /// Check firmware status at the given path
    pub fn check(firmware_path: &std::path::Path) -> Self {
        use std::fs;
        
        let mut status = Self::new();
        
        if !firmware_path.exists() {
            status.missing_components.push("Firmware directory not found".to_string());
            return status;
        }
        
        status.path = Some(firmware_path.to_path_buf());
        
        // Check for version.txt
        let version_file = firmware_path.join("version.txt");
        if let Ok(version_str) = fs::read_to_string(&version_file) {
            status.version = FirmwareVersion::parse(version_str.trim());
            status.available_components.push("version.txt".to_string());
        } else {
            status.missing_components.push("version.txt".to_string());
        }
        
        // Check for essential components
        let essential_files = [
            ("vsh/module/lv2_kernel.self", "LV2 Kernel"),
            ("vsh/module/vsh.self", "VSH Module"),
            ("sys/external", "External Libraries"),
        ];
        
        for (path, name) in &essential_files {
            let full_path = firmware_path.join(path);
            if full_path.exists() {
                status.available_components.push(name.to_string());
            } else {
                status.missing_components.push(name.to_string());
            }
        }
        
        // Firmware is considered installed if we have version and LV2 kernel
        status.installed = status.version.is_some() && 
            status.available_components.contains(&"LV2 Kernel".to_string());
        
        status
    }
    
    /// Get a user-friendly error message if firmware is not properly installed
    pub fn get_error_message(&self) -> Option<String> {
        if self.installed {
            return None;
        }
        
        let mut msg = String::from("PS3 firmware is not properly installed.\n\n");
        
        if self.missing_components.is_empty() {
            msg.push_str("The firmware directory exists but appears to be incomplete.\n");
        } else {
            msg.push_str("Missing components:\n");
            for component in &self.missing_components {
                msg.push_str(&format!("  - {}\n", component));
            }
        }
        
        msg.push_str("\nTo install firmware:\n");
        msg.push_str("1. Download the official PS3 firmware (PS3UPDAT.PUP) from playstation.com\n");
        msg.push_str("2. Place it in the 'firmware/' directory\n");
        msg.push_str("3. The emulator will automatically extract the necessary files\n\n");
        msg.push_str("Alternatively, you can extract the firmware manually and place it in 'dev_flash/'");
        
        Some(msg)
    }
}

impl Default for FirmwareStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Firmware file entry (extracted from PUP)
#[derive(Debug, Clone)]
pub struct FirmwareFile {
    pub id: PupEntryId,
    pub raw_id: u64,
    pub offset: u64,
    pub size: u64,
}

/// PUP file loader
pub struct PupLoader {
    /// Parsed entries
    entries: Vec<FirmwareFile>,
    /// Firmware version
    version: Option<FirmwareVersion>,
}

impl PupLoader {
    /// Create a new PUP loader
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            version: None,
        }
    }

    /// Check if data is a PUP file
    pub fn is_pup(data: &[u8]) -> bool {
        data.len() >= 8 && data[0..8] == PUP_MAGIC
    }

    /// Parse PUP header
    pub fn parse_header(data: &[u8]) -> Result<PupHeader, LoaderError> {
        if data.len() < 48 {
            return Err(LoaderError::InvalidPup("File too small".to_string()));
        }

        if !Self::is_pup(data) {
            return Err(LoaderError::InvalidPup("Invalid PUP magic".to_string()));
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&data[0..8]);

        let header = PupHeader {
            magic,
            package_version: u64::from_be_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15],
            ]),
            image_version: u64::from_be_bytes([
                data[16], data[17], data[18], data[19],
                data[20], data[21], data[22], data[23],
            ]),
            file_count: u64::from_be_bytes([
                data[24], data[25], data[26], data[27],
                data[28], data[29], data[30], data[31],
            ]),
            header_length: u64::from_be_bytes([
                data[32], data[33], data[34], data[35],
                data[36], data[37], data[38], data[39],
            ]),
            data_length: u64::from_be_bytes([
                data[40], data[41], data[42], data[43],
                data[44], data[45], data[46], data[47],
            ]),
        };

        info!(
            "PUP header: version=0x{:016x}, files={}, header_len=0x{:x}",
            header.image_version, header.file_count, header.header_length
        );

        Ok(header)
    }

    /// Parse PUP file entries
    pub fn parse(&mut self, data: &[u8]) -> Result<&[FirmwareFile], LoaderError> {
        let header = Self::parse_header(data)?;
        
        self.version = Some(FirmwareVersion::from_u64(header.image_version));
        self.entries.clear();

        // Parse file entries (after header)
        let entry_offset = 48usize;
        let entry_size = 32usize; // sizeof(PupFileEntry)

        for i in 0..header.file_count {
            let offset = entry_offset + (i as usize * entry_size);
            if data.len() < offset + entry_size {
                return Err(LoaderError::InvalidPup("Truncated file table".to_string()));
            }

            let entry_id = u64::from_be_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            let data_offset = u64::from_be_bytes([
                data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11],
                data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15],
            ]);
            let data_length = u64::from_be_bytes([
                data[offset + 16], data[offset + 17], data[offset + 18], data[offset + 19],
                data[offset + 20], data[offset + 21], data[offset + 22], data[offset + 23],
            ]);

            debug!(
                "PUP entry {}: id=0x{:x}, offset=0x{:x}, size=0x{:x}",
                i, entry_id, data_offset, data_length
            );

            self.entries.push(FirmwareFile {
                id: PupEntryId::from(entry_id),
                raw_id: entry_id,
                offset: data_offset,
                size: data_length,
            });
        }

        Ok(&self.entries)
    }

    /// Get firmware version
    pub fn version(&self) -> Option<&FirmwareVersion> {
        self.version.as_ref()
    }

    /// Get all entries
    pub fn entries(&self) -> &[FirmwareFile] {
        &self.entries
    }

    /// Get entry by ID
    pub fn get_entry(&self, id: PupEntryId) -> Option<&FirmwareFile> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Extract a file from the PUP
    pub fn extract(&self, data: &[u8], entry: &FirmwareFile) -> Result<Vec<u8>, LoaderError> {
        let start = entry.offset as usize;
        let end = start + entry.size as usize;

        if data.len() < end {
            return Err(LoaderError::InvalidPup("Entry extends beyond file".to_string()));
        }

        Ok(data[start..end].to_vec())
    }

    /// Install firmware from a PUP file to a target directory
    ///
    /// This extracts the necessary files from the firmware update package
    /// and installs them to the specified directory (usually dev_flash/).
    pub fn install(
        &mut self,
        pup_data: &[u8],
        target_dir: &std::path::Path,
    ) -> Result<FirmwareVersion, LoaderError> {
        use std::fs;
        use std::io::Write;

        info!("Installing firmware to: {}", target_dir.display());

        // Parse the PUP file
        self.parse(pup_data)?;

        let version = self.version.clone()
            .ok_or_else(|| LoaderError::InvalidPup("Could not determine firmware version".to_string()))?;

        info!("Firmware version: {}", version.to_string());

        // Create target directory structure
        let dirs = [
            "vsh/module",
            "sys/external",
            "sys/internal",
        ];

        for dir in &dirs {
            let dir_path = target_dir.join(dir);
            fs::create_dir_all(&dir_path).map_err(|e| {
                LoaderError::InvalidPup(format!("Failed to create directory {}: {}", dir, e))
            })?;
        }

        // Extract important entries
        for entry in &self.entries {
            let extracted = self.extract(pup_data, entry)?;
            
            // Determine target path based on entry type
            let target_path = match entry.id {
                PupEntryId::Lv2Kernel => Some(target_dir.join("vsh/module/lv2_kernel.self")),
                PupEntryId::Vsh => Some(target_dir.join("vsh/module/vsh.self")),
                _ => None,
            };

            if let Some(path) = target_path {
                debug!("Extracting {:?} to {}", entry.id, path.display());
                
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        LoaderError::InvalidPup(format!("Failed to create parent dir: {}", e))
                    })?;
                }

                let mut file = fs::File::create(&path).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to create file {}: {}", path.display(), e))
                })?;

                file.write_all(&extracted).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to write file {}: {}", path.display(), e))
                })?;
            }
        }

        // Write version info
        let version_file = target_dir.join("version.txt");
        fs::write(&version_file, version.to_string()).map_err(|e| {
            LoaderError::InvalidPup(format!("Failed to write version file: {}", e))
        })?;

        info!("Firmware {} installed successfully", version.to_string());
        Ok(version)
    }
    
    /// Auto-install firmware from a PUP file if found in common locations
    ///
    /// This looks for PS3UPDAT.PUP in common locations and automatically
    /// extracts it to the target directory.
    pub fn auto_install(target_dir: &std::path::Path) -> Result<Option<FirmwareVersion>, LoaderError> {
        use std::fs;
        
        // Check if firmware is already installed
        let status = FirmwareStatus::check(target_dir);
        if status.installed {
            info!("Firmware already installed: version {}", 
                status.version.as_ref().map(|v| v.to_string()).unwrap_or_default());
            return Ok(status.version);
        }
        
        // Look for PUP files in common locations
        let pup_search_paths = [
            "firmware/PS3UPDAT.PUP",
            "firmware/ps3updat.pup",
            "PS3UPDAT.PUP",
            "ps3updat.pup",
            "../firmware/PS3UPDAT.PUP",
        ];
        
        for pup_path in &pup_search_paths {
            let path = std::path::Path::new(pup_path);
            if path.exists() {
                info!("Found firmware update file: {}", pup_path);
                
                // Read the PUP file
                let pup_data = fs::read(path).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to read PUP file {}: {}", pup_path, e))
                })?;
                
                // Verify it's a valid PUP
                if !Self::is_pup(&pup_data) {
                    warn!("File {} is not a valid PUP file, skipping", pup_path);
                    continue;
                }
                
                // Create target directory if it doesn't exist
                fs::create_dir_all(target_dir).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to create firmware directory: {}", e))
                })?;
                
                // Install the firmware
                let mut loader = Self::new();
                let version = loader.install(&pup_data, target_dir)?;
                
                info!("Automatically installed firmware version {}", version.to_string());
                return Ok(Some(version));
            }
        }
        
        // No PUP file found
        debug!("No firmware update file found in common locations");
        Ok(None)
    }
    
    /// Get a detailed error message for missing firmware
    pub fn get_missing_firmware_error() -> String {
        let mut msg = String::from(
            "PS3 firmware is required to run encrypted games (SELF/EBOOT.BIN files).\n\n"
        );
        
        msg.push_str("The emulator cannot decrypt PS3 executables without the official firmware.\n\n");
        
        msg.push_str("To install firmware:\n");
        msg.push_str("─────────────────────\n");
        msg.push_str("1. Download the official PS3 firmware from:\n");
        msg.push_str("   https://www.playstation.com/en-us/support/hardware/ps3/system-software/\n\n");
        msg.push_str("2. Place the downloaded file (PS3UPDAT.PUP) in one of these locations:\n");
        msg.push_str("   • firmware/PS3UPDAT.PUP (recommended)\n");
        msg.push_str("   • ./PS3UPDAT.PUP (current directory)\n\n");
        msg.push_str("3. Restart the emulator - firmware will be extracted automatically\n\n");
        
        msg.push_str("Alternative:\n");
        msg.push_str("────────────\n");
        msg.push_str("• Use decrypted game files (EBOOT.ELF instead of EBOOT.BIN)\n");
        msg.push_str("• These can be created with PS3 decryption tools\n");
        
        msg
    }
}

impl Default for PupLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pup_magic() {
        assert_eq!(PUP_MAGIC, [0x53, 0x43, 0x45, 0x55, 0x46, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_is_pup() {
        let pup_data = [0x53, 0x43, 0x45, 0x55, 0x46, 0x00, 0x00, 0x00, 0x00];
        assert!(PupLoader::is_pup(&pup_data));

        let not_pup = [0x00, 0x00, 0x00, 0x00];
        assert!(!PupLoader::is_pup(&not_pup));
    }

    #[test]
    fn test_firmware_version() {
        let version = FirmwareVersion::parse("04.90");
        assert!(version.is_some());
        let v = version.unwrap();
        assert_eq!(v.major, 4);
        assert_eq!(v.minor, 90);
    }

    #[test]
    fn test_entry_id_conversion() {
        assert_eq!(PupEntryId::from(0x100), PupEntryId::UpdateVersion);
        assert_eq!(PupEntryId::from(0x202), PupEntryId::Lv2Kernel);
        assert_eq!(PupEntryId::from(0x9999), PupEntryId::Unknown);
    }
}
