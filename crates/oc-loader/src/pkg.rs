//! PSN Package (.pkg) file support
//!
//! This module provides parsing and extraction of PlayStation Store package files.

use oc_core::error::LoaderError;
use tracing::{debug, info, warn};

/// PKG file magic
pub const PKG_MAGIC: u32 = 0x7F504B47; // "\x7FPKG"

/// PKG file types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PkgType {
    /// PS3 game package
    Ps3Game = 0x01,
    /// PS3 DLC package
    Ps3Dlc = 0x02,
    /// PS3 theme package
    Ps3Theme = 0x03,
    /// PS3 avatar package
    Ps3Avatar = 0x04,
    /// PSP game package
    PspGame = 0x05,
    /// PSP DLC package
    PspDlc = 0x06,
    /// PSVita game package
    VitaGame = 0x07,
    /// Unknown type
    Unknown = 0xFF,
}

impl From<u32> for PkgType {
    fn from(value: u32) -> Self {
        match value {
            0x01 => Self::Ps3Game,
            0x02 => Self::Ps3Dlc,
            0x03 => Self::Ps3Theme,
            0x04 => Self::Ps3Avatar,
            0x05 => Self::PspGame,
            0x06 => Self::PspDlc,
            0x07 => Self::VitaGame,
            _ => Self::Unknown,
        }
    }
}

/// PKG content type flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PkgContentType(pub u32);

impl PkgContentType {
    /// Game execution file
    pub const GAME_EXEC: u32 = 0x04;
    /// Game data
    pub const GAME_DATA: u32 = 0x05;
    /// DLC
    pub const DLC: u32 = 0x06;
    /// Theme
    pub const THEME: u32 = 0x09;
    /// License file
    pub const LICENSE: u32 = 0x0E;
    /// Widget
    pub const WIDGET: u32 = 0x0F;
}

/// PKG file header
#[derive(Debug, Clone)]
pub struct PkgHeader {
    /// Magic number (0x7F504B47)
    pub magic: u32,
    /// Revision
    pub revision: u16,
    /// Type
    pub pkg_type: u16,
    /// Metadata offset
    pub metadata_offset: u32,
    /// Metadata count
    pub metadata_count: u32,
    /// Metadata size
    pub metadata_size: u32,
    /// Item count
    pub item_count: u32,
    /// Total size
    pub total_size: u64,
    /// Data offset
    pub data_offset: u64,
    /// Data size
    pub data_size: u64,
    /// Content ID
    pub content_id: String,
    /// Package digest (SHA-1)
    pub digest: [u8; 16],
    /// Package key (AES key for decryption)
    pub pkg_data_key: [u8; 16],
    /// Package IV
    pub pkg_data_iv: [u8; 16],
}

/// PKG metadata entry
#[derive(Debug, Clone)]
pub struct PkgMetadataEntry {
    pub id: u32,
    pub size: u32,
    pub data: Vec<u8>,
}

/// Known PKG metadata IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PkgMetadataId {
    /// DRM type
    DrmType = 0x01,
    /// Content type
    ContentType = 0x02,
    /// Package type
    PackageType = 0x03,
    /// Package size
    PackageSize = 0x04,
    /// Make package revision
    MakePackageRev = 0x05,
    /// Title ID
    TitleId = 0x06,
    /// QA digest
    QaDigest = 0x07,
    /// Unknown
    Unknown = 0xFF,
}

/// PKG file entry (item in the package)
#[derive(Debug, Clone)]
pub struct PkgFileEntry {
    /// File name offset
    pub name_offset: u32,
    /// File name size
    pub name_size: u32,
    /// Data offset
    pub data_offset: u64,
    /// Data size
    pub data_size: u64,
    /// File type/flags
    pub flags: u32,
    /// File name (extracted)
    pub name: String,
}

/// PKG loader with extraction support
pub struct PkgLoader {
    /// Package header
    header: Option<PkgHeader>,
    /// Metadata entries
    metadata: Vec<PkgMetadataEntry>,
    /// File entries
    files: Vec<PkgFileEntry>,
}

impl PkgLoader {
    /// Create a new PKG loader
    pub fn new() -> Self {
        Self {
            header: None,
            metadata: Vec::new(),
            files: Vec::new(),
        }
    }

    /// Check if data is a PKG file
    pub fn is_pkg(data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        magic == PKG_MAGIC
    }

    /// Parse PKG header
    pub fn parse_header(data: &[u8]) -> Result<PkgHeader, LoaderError> {
        if data.len() < 0xC0 {
            return Err(LoaderError::InvalidPkg("File too small".to_string()));
        }

        if !Self::is_pkg(data) {
            return Err(LoaderError::InvalidPkg("Invalid PKG magic".to_string()));
        }

        let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let revision = u16::from_be_bytes([data[4], data[5]]);
        let pkg_type = u16::from_be_bytes([data[6], data[7]]);
        let metadata_offset = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let metadata_count = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let metadata_size = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let item_count = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let total_size = u64::from_be_bytes([
            data[24], data[25], data[26], data[27],
            data[28], data[29], data[30], data[31],
        ]);
        let data_offset = u64::from_be_bytes([
            data[32], data[33], data[34], data[35],
            data[36], data[37], data[38], data[39],
        ]);
        let data_size = u64::from_be_bytes([
            data[40], data[41], data[42], data[43],
            data[44], data[45], data[46], data[47],
        ]);

        // Content ID is at offset 0x30, 36 bytes
        let content_id = String::from_utf8_lossy(&data[0x30..0x54])
            .trim_end_matches('\0')
            .to_string();

        // Digest at offset 0x60
        let mut digest = [0u8; 16];
        digest.copy_from_slice(&data[0x60..0x70]);

        // Data key at offset 0x70
        let mut pkg_data_key = [0u8; 16];
        pkg_data_key.copy_from_slice(&data[0x70..0x80]);

        // Data IV at offset 0x80
        let mut pkg_data_iv = [0u8; 16];
        pkg_data_iv.copy_from_slice(&data[0x80..0x90]);

        info!(
            "PKG header: type={}, content_id={}, items={}, size={}",
            pkg_type, content_id, item_count, total_size
        );

        Ok(PkgHeader {
            magic,
            revision,
            pkg_type,
            metadata_offset,
            metadata_count,
            metadata_size,
            item_count,
            total_size,
            data_offset,
            data_size,
            content_id,
            digest,
            pkg_data_key,
            pkg_data_iv,
        })
    }

    /// Parse PKG file
    pub fn parse(&mut self, data: &[u8]) -> Result<(), LoaderError> {
        let header = Self::parse_header(data)?;
        
        // Parse metadata
        self.metadata.clear();
        let mut offset = header.metadata_offset as usize;
        
        for _ in 0..header.metadata_count {
            if data.len() < offset + 8 {
                break;
            }

            let id = u32::from_be_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]);
            let size = u32::from_be_bytes([
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);

            let entry_data = if data.len() >= offset + 8 + size as usize {
                data[offset + 8..offset + 8 + size as usize].to_vec()
            } else {
                Vec::new()
            };

            debug!("PKG metadata: id=0x{:x}, size={}", id, size);

            self.metadata.push(PkgMetadataEntry {
                id,
                size,
                data: entry_data,
            });

            offset += 8 + ((size + 15) & !15) as usize; // Align to 16 bytes
        }

        // Parse file entries
        self.files.clear();
        let items_offset = header.data_offset as usize;
        let item_entry_size = 32usize;

        for i in 0..header.item_count {
            let entry_offset = items_offset + (i as usize * item_entry_size);
            if data.len() < entry_offset + item_entry_size {
                break;
            }

            let name_offset = u32::from_be_bytes([
                data[entry_offset],
                data[entry_offset + 1],
                data[entry_offset + 2],
                data[entry_offset + 3],
            ]);
            let name_size = u32::from_be_bytes([
                data[entry_offset + 4],
                data[entry_offset + 5],
                data[entry_offset + 6],
                data[entry_offset + 7],
            ]);
            let file_data_offset = u64::from_be_bytes([
                data[entry_offset + 8],
                data[entry_offset + 9],
                data[entry_offset + 10],
                data[entry_offset + 11],
                data[entry_offset + 12],
                data[entry_offset + 13],
                data[entry_offset + 14],
                data[entry_offset + 15],
            ]);
            let file_data_size = u64::from_be_bytes([
                data[entry_offset + 16],
                data[entry_offset + 17],
                data[entry_offset + 18],
                data[entry_offset + 19],
                data[entry_offset + 20],
                data[entry_offset + 21],
                data[entry_offset + 22],
                data[entry_offset + 23],
            ]);
            let flags = u32::from_be_bytes([
                data[entry_offset + 24],
                data[entry_offset + 25],
                data[entry_offset + 26],
                data[entry_offset + 27],
            ]);

            // Extract file name
            let name_start = (header.data_offset + name_offset as u64) as usize;
            let name = if data.len() >= name_start + name_size as usize {
                String::from_utf8_lossy(&data[name_start..name_start + name_size as usize])
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                format!("file_{}", i)
            };

            debug!("PKG file {}: {} (size={})", i, name, file_data_size);

            self.files.push(PkgFileEntry {
                name_offset,
                name_size,
                data_offset: file_data_offset,
                data_size: file_data_size,
                flags,
                name,
            });
        }

        self.header = Some(header);
        Ok(())
    }

    /// Get package header
    pub fn header(&self) -> Option<&PkgHeader> {
        self.header.as_ref()
    }

    /// Get content ID
    pub fn content_id(&self) -> Option<&str> {
        self.header.as_ref().map(|h| h.content_id.as_str())
    }

    /// Get file entries
    pub fn files(&self) -> &[PkgFileEntry] {
        &self.files
    }

    /// Get metadata entries
    pub fn metadata(&self) -> &[PkgMetadataEntry] {
        &self.metadata
    }

    /// Extract a file from the package (returns encrypted data without keys)
    pub fn extract_raw(&self, data: &[u8], entry: &PkgFileEntry) -> Result<Vec<u8>, LoaderError> {
        let header = self.header.as_ref()
            .ok_or_else(|| LoaderError::InvalidPkg("Header not parsed".to_string()))?;

        let start = (header.data_offset + entry.data_offset) as usize;
        let end = start + entry.data_size as usize;

        if data.len() < end {
            return Err(LoaderError::InvalidPkg("File extends beyond package".to_string()));
        }

        warn!("Extracting raw (encrypted) PKG data - decryption requires valid keys");
        Ok(data[start..end].to_vec())
    }

    /// Get metadata by ID
    pub fn get_metadata(&self, id: PkgMetadataId) -> Option<&PkgMetadataEntry> {
        self.metadata.iter().find(|e| e.id == id as u32)
    }

    /// Get title ID from metadata
    pub fn title_id(&self) -> Option<String> {
        self.get_metadata(PkgMetadataId::TitleId)
            .map(|e| String::from_utf8_lossy(&e.data).trim_end_matches('\0').to_string())
    }
}

impl Default for PkgLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkg_magic() {
        assert_eq!(PKG_MAGIC, 0x7F504B47);
    }

    #[test]
    fn test_is_pkg() {
        let pkg_data = [0x7F, 0x50, 0x4B, 0x47, 0x00, 0x00];
        assert!(PkgLoader::is_pkg(&pkg_data));

        let not_pkg = [0x00, 0x00, 0x00, 0x00];
        assert!(!PkgLoader::is_pkg(&not_pkg));
    }

    #[test]
    fn test_pkg_type_conversion() {
        assert_eq!(PkgType::from(0x01), PkgType::Ps3Game);
        assert_eq!(PkgType::from(0x02), PkgType::Ps3Dlc);
        assert_eq!(PkgType::from(0x99), PkgType::Unknown);
    }
}
