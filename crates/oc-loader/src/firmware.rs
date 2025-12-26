//! PS3 Firmware file support
//!
//! This module provides parsing and handling of PS3 firmware files.

use oc_core::error::LoaderError;
use tracing::{debug, info};

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
