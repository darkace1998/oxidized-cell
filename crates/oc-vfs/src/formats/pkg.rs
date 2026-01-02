//! PKG file format
//!
//! Parser for PS3 package (.pkg) files

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// PKG file type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgType {
    /// PS3 game package
    Game = 0x01,
    /// PS3 update package
    Update = 0x02,
    /// PS3 DLC package
    Dlc = 0x03,
    /// Other/unknown
    Other = 0xFF,
}

impl From<u32> for PkgType {
    fn from(value: u32) -> Self {
        match value {
            0x01 => PkgType::Game,
            0x02 => PkgType::Update,
            0x03 => PkgType::Dlc,
            _ => PkgType::Other,
        }
    }
}

/// PKG metadata entry types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgMetadataType {
    /// DRM type
    DrmType = 0x01,
    /// Content type
    ContentType = 0x02,
    /// Package type
    PackageType = 0x03,
    /// Title ID
    TitleId = 0x0E,
    /// QA digest
    QaDigest = 0x10,
    /// Content ID
    ContentId = 0x10000000,
    /// Unknown
    Unknown = 0xFF,
}

impl From<u32> for PkgMetadataType {
    fn from(value: u32) -> Self {
        match value {
            0x01 => PkgMetadataType::DrmType,
            0x02 => PkgMetadataType::ContentType,
            0x03 => PkgMetadataType::PackageType,
            0x0E => PkgMetadataType::TitleId,
            0x10 => PkgMetadataType::QaDigest,
            _ => PkgMetadataType::Unknown,
        }
    }
}

/// PKG file entry in the file table
#[derive(Debug, Clone)]
pub struct PkgFileEntry {
    /// File name offset in the name table
    pub name_offset: u32,
    /// File name
    pub name: String,
    /// Data offset in PKG
    pub data_offset: u64,
    /// Data size
    pub data_size: u64,
    /// Flags
    pub flags: u32,
    /// Is directory
    pub is_directory: bool,
}

/// PKG file header
#[derive(Debug, Clone)]
pub struct PkgHeader {
    /// Package type
    pub pkg_type: PkgType,
    /// Package version
    pub version: u32,
    /// Package size
    pub pkg_size: u64,
    /// Data offset
    pub data_offset: u64,
    /// Data size
    pub data_size: u64,
    /// Title ID
    pub title_id: String,
    /// Content ID
    pub content_id: String,
    /// File count
    pub file_count: u32,
    /// File table offset
    pub file_table_offset: u64,
    /// File table size
    pub file_table_size: u64,
}

impl PkgHeader {
    /// Parse PKG header from reader
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self, std::io::Error> {
        reader.seek(SeekFrom::Start(0))?;

        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        if &magic != b"\x7FPKG" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid PKG magic",
            ));
        }

        // Read header fields (PKG structure is big-endian)
        let mut header_data = [0u8; 256];
        reader.seek(SeekFrom::Start(0))?;
        reader.read_exact(&mut header_data)?;

        let pkg_type = u32::from_be_bytes([
            header_data[4],
            header_data[5],
            header_data[6],
            header_data[7],
        ]).into();

        let version = u32::from_be_bytes([
            header_data[8],
            header_data[9],
            header_data[10],
            header_data[11],
        ]);

        let pkg_size = u64::from_be_bytes([
            header_data[16], header_data[17], header_data[18], header_data[19],
            header_data[20], header_data[21], header_data[22], header_data[23],
        ]);

        let data_offset = u64::from_be_bytes([
            header_data[24], header_data[25], header_data[26], header_data[27],
            header_data[28], header_data[29], header_data[30], header_data[31],
        ]);

        let data_size = u64::from_be_bytes([
            header_data[32], header_data[33], header_data[34], header_data[35],
            header_data[36], header_data[37], header_data[38], header_data[39],
        ]);

        // Content ID at offset 48 (36 bytes null-terminated string)
        let content_id_bytes = &header_data[48..84];
        let content_id = String::from_utf8_lossy(content_id_bytes)
            .trim_end_matches('\0')
            .to_string();

        // Parse title ID from Content ID (format: XX00000-TITLE_ID_00000)
        // Title ID is typically embedded in the content ID after the first dash
        let title_id = Self::extract_title_id_from_content_id(&content_id);

        // File table info - located at different offsets depending on PKG revision
        // For PKG revision 1 (most common):
        // File count at offset 40
        let file_count = u32::from_be_bytes([
            header_data[40], header_data[41], header_data[42], header_data[43],
        ]);

        // File table offset - typically after header, we'll calculate based on data offset
        let file_table_offset = data_offset;
        let file_table_size = 32 * file_count as u64; // Each entry is 32 bytes

        Ok(Self {
            pkg_type,
            version,
            pkg_size,
            data_offset,
            data_size,
            title_id,
            content_id,
            file_count,
            file_table_offset,
            file_table_size,
        })
    }

    /// Extract title ID from content ID string
    fn extract_title_id_from_content_id(content_id: &str) -> String {
        // Content ID format: XX00000-TITLE_ID_00000
        // Examples: UP0001-NPUB00000_00-0000000000000000
        //           EP0001-BLES00000_00-0000000000000000
        if let Some(dash_pos) = content_id.find('-') {
            if dash_pos + 10 <= content_id.len() {
                // Title ID is 9 characters after the first dash
                let title_part = &content_id[dash_pos + 1..];
                if let Some(underscore_pos) = title_part.find('_') {
                    return title_part[..underscore_pos].to_string();
                }
                // If no underscore, take first 9 chars
                if title_part.len() >= 9 {
                    return title_part[..9].to_string();
                }
            }
        }
        String::new()
    }
}

/// PKG file reader
pub struct PkgReader {
    /// Path to PKG file
    pub path: PathBuf,
    /// Header information
    pub header: Option<PkgHeader>,
    /// File entries
    pub files: Vec<PkgFileEntry>,
}

impl PkgReader {
    /// Create a new PKG reader
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            header: None,
            files: Vec::new(),
        }
    }

    /// Open and parse the PKG file
    pub fn open(&mut self) -> Result<(), std::io::Error> {
        let file = std::fs::File::open(&self.path)?;
        let mut reader = std::io::BufReader::new(file);

        let header = PkgHeader::parse(&mut reader)?;

        tracing::info!(
            "Opened PKG: {:?}, Type: {:?}, Content ID: {}, Title ID: {}",
            self.path,
            header.pkg_type,
            header.content_id,
            header.title_id
        );

        // Parse file table if available
        self.files = self.parse_file_table(&mut reader, &header)?;

        self.header = Some(header);

        Ok(())
    }

    /// Parse the file table from the PKG
    fn parse_file_table<R: Read + Seek>(
        &self,
        reader: &mut R,
        header: &PkgHeader,
    ) -> Result<Vec<PkgFileEntry>, std::io::Error> {
        let mut entries = Vec::new();

        if header.file_count == 0 {
            return Ok(entries);
        }

        // Seek to file table
        reader.seek(SeekFrom::Start(header.file_table_offset))?;

        // Read file entries (each entry is 32 bytes in PKG format)
        for _ in 0..header.file_count {
            let mut entry_data = [0u8; 32];
            if reader.read_exact(&mut entry_data).is_err() {
                break;
            }

            let name_offset = u32::from_be_bytes([
                entry_data[0], entry_data[1], entry_data[2], entry_data[3],
            ]);

            let name_size = u32::from_be_bytes([
                entry_data[4], entry_data[5], entry_data[6], entry_data[7],
            ]);

            let data_offset = u64::from_be_bytes([
                entry_data[8], entry_data[9], entry_data[10], entry_data[11],
                entry_data[12], entry_data[13], entry_data[14], entry_data[15],
            ]);

            let data_size = u64::from_be_bytes([
                entry_data[16], entry_data[17], entry_data[18], entry_data[19],
                entry_data[20], entry_data[21], entry_data[22], entry_data[23],
            ]);

            let flags = u32::from_be_bytes([
                entry_data[24], entry_data[25], entry_data[26], entry_data[27],
            ]);

            let is_directory = (flags & 0x04) != 0;

            // Read file name
            let name = if name_size > 0 && name_size < 1024 {
                let current_pos = reader.stream_position()?;
                reader.seek(SeekFrom::Start(header.file_table_offset + name_offset as u64))?;
                let mut name_bytes = vec![0u8; name_size as usize];
                reader.read_exact(&mut name_bytes)?;
                reader.seek(SeekFrom::Start(current_pos))?;
                String::from_utf8_lossy(&name_bytes)
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                format!("file_{}", entries.len())
            };

            entries.push(PkgFileEntry {
                name_offset,
                name,
                data_offset,
                data_size,
                flags,
                is_directory,
            });
        }

        Ok(entries)
    }

    /// Get header information
    pub fn header(&self) -> Option<&PkgHeader> {
        self.header.as_ref()
    }

    /// Get title ID
    pub fn title_id(&self) -> Option<&str> {
        self.header.as_ref().map(|h| h.title_id.as_str())
    }

    /// Get content ID
    pub fn content_id(&self) -> Option<&str> {
        self.header.as_ref().map(|h| h.content_id.as_str())
    }

    /// Get file entries
    pub fn files(&self) -> &[PkgFileEntry] {
        &self.files
    }

    /// Read a file from the PKG by name
    pub fn read_file(&self, file_name: &str) -> Result<Vec<u8>, std::io::Error> {
        let entry = self.files.iter().find(|e| e.name == file_name).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File '{}' not found in PKG", file_name),
            )
        })?;

        if entry.is_directory {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("'{}' is a directory", file_name),
            ));
        }

        let file = std::fs::File::open(&self.path)?;
        let mut reader = std::io::BufReader::new(file);

        reader.seek(SeekFrom::Start(entry.data_offset))?;
        let mut data = vec![0u8; entry.data_size as usize];
        reader.read_exact(&mut data)?;

        // Note: Real PKG files are encrypted. This returns raw data which
        // may need decryption. For unencrypted/decrypted PKGs, this works directly.

        Ok(data)
    }

    /// Extract PKG contents to a directory
    pub fn extract_to(&self, output_dir: &PathBuf) -> Result<u32, std::io::Error> {
        let _header = self.header.as_ref().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "PKG not opened",
            )
        })?;

        // Create output directory
        std::fs::create_dir_all(output_dir)?;

        let file = std::fs::File::open(&self.path)?;
        let mut reader = std::io::BufReader::new(file);

        let mut extracted_count = 0u32;

        for entry in &self.files {
            let output_path = output_dir.join(&entry.name);

            if entry.is_directory {
                // Create directory
                std::fs::create_dir_all(&output_path)?;
                tracing::debug!("Created directory: {:?}", output_path);
            } else {
                // Create parent directories if needed
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                // Read and write file data
                reader.seek(SeekFrom::Start(entry.data_offset))?;
                let mut data = vec![0u8; entry.data_size as usize];
                if let Err(e) = reader.read_exact(&mut data) {
                    tracing::warn!("Failed to read file {}: {}", entry.name, e);
                    continue;
                }

                // Note: Real PKG files are encrypted with AES. For encrypted PKGs,
                // you would need to decrypt the data here using the appropriate keys.
                // This implementation handles unencrypted/already-decrypted PKGs.

                let mut output_file = std::fs::File::create(&output_path)?;
                output_file.write_all(&data)?;

                tracing::debug!("Extracted: {} ({} bytes)", entry.name, entry.data_size);
                extracted_count += 1;
            }
        }

        tracing::info!(
            "Extracted {} files from PKG to {:?}",
            extracted_count,
            output_dir
        );

        Ok(extracted_count)
    }

    /// List all files in the PKG
    pub fn list_files(&self) -> Vec<&str> {
        self.files.iter().map(|e| e.name.as_str()).collect()
    }

    /// Get PKG info summary
    pub fn info_summary(&self) -> String {
        match &self.header {
            Some(h) => {
                format!(
                    "PKG: {:?}\n  Type: {:?}\n  Version: {}\n  Content ID: {}\n  Title ID: {}\n  Files: {}\n  Size: {} bytes",
                    self.path,
                    h.pkg_type,
                    h.version,
                    h.content_id,
                    h.title_id,
                    h.file_count,
                    h.pkg_size
                )
            }
            None => format!("PKG not opened: {:?}", self.path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_pkg_reader_creation() {
        let reader = PkgReader::new(PathBuf::from("/tmp/test.pkg"));
        assert!(reader.header.is_none());
        assert!(reader.files.is_empty());
    }

    #[test]
    fn test_pkg_type_conversion() {
        assert_eq!(PkgType::from(0x01), PkgType::Game);
        assert_eq!(PkgType::from(0x02), PkgType::Update);
        assert_eq!(PkgType::from(0x03), PkgType::Dlc);
        assert_eq!(PkgType::from(0xFF), PkgType::Other);
    }

    #[test]
    fn test_extract_title_id_from_content_id() {
        // Standard PS3 content ID format
        assert_eq!(
            PkgHeader::extract_title_id_from_content_id("UP0001-NPUB00000_00-0000000000000000"),
            "NPUB00000"
        );
        assert_eq!(
            PkgHeader::extract_title_id_from_content_id("EP0001-BLES12345_00-GAMECONTENT00001"),
            "BLES12345"
        );
        assert_eq!(
            PkgHeader::extract_title_id_from_content_id("JP0000-BLJM60000_00-TESTPACKAGE00000"),
            "BLJM60000"
        );

        // Edge cases
        assert_eq!(
            PkgHeader::extract_title_id_from_content_id(""),
            ""
        );
        assert_eq!(
            PkgHeader::extract_title_id_from_content_id("NODASH"),
            ""
        );
    }

    #[test]
    fn test_pkg_header_parse_valid() {
        // Create a minimal valid PKG header
        let mut pkg_data = vec![0u8; 256];
        
        // Magic: "\x7FPKG"
        pkg_data[0] = 0x7F;
        pkg_data[1] = b'P';
        pkg_data[2] = b'K';
        pkg_data[3] = b'G';
        
        // Type: Game (0x01)
        pkg_data[7] = 0x01;
        
        // Version: 1
        pkg_data[11] = 0x01;
        
        // Content ID at offset 48: "UP0001-NPUB31337_00-0000000000000000"
        let content_id = b"UP0001-NPUB31337_00-0000000000000000";
        for (i, b) in content_id.iter().enumerate() {
            if i + 48 < pkg_data.len() {
                pkg_data[48 + i] = *b;
            }
        }
        
        let mut cursor = Cursor::new(&pkg_data);
        let header = PkgHeader::parse(&mut cursor).expect("Failed to parse PKG header");
        
        assert_eq!(header.pkg_type, PkgType::Game);
        assert_eq!(header.version, 1);
        assert_eq!(header.content_id, "UP0001-NPUB31337_00-0000000000000000");
        assert_eq!(header.title_id, "NPUB31337");
    }

    #[test]
    fn test_pkg_header_parse_invalid_magic() {
        let pkg_data = vec![0u8; 256]; // No magic
        let mut cursor = Cursor::new(&pkg_data);
        
        let result = PkgHeader::parse(&mut cursor);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid PKG magic"));
    }

    #[test]
    fn test_pkg_info_summary() {
        let reader = PkgReader::new(PathBuf::from("/tmp/test.pkg"));
        let summary = reader.info_summary();
        assert!(summary.contains("PKG not opened"));
    }

    #[test]
    fn test_pkg_file_entry_struct() {
        let entry = PkgFileEntry {
            name_offset: 0,
            name: "USRDIR/EBOOT.BIN".to_string(),
            data_offset: 1024,
            data_size: 4096,
            flags: 0,
            is_directory: false,
        };
        
        assert_eq!(entry.name, "USRDIR/EBOOT.BIN");
        assert_eq!(entry.data_offset, 1024);
        assert_eq!(entry.data_size, 4096);
        assert!(!entry.is_directory);
    }

    #[test]
    fn test_pkg_file_entry_directory() {
        let entry = PkgFileEntry {
            name_offset: 0,
            name: "USRDIR".to_string(),
            data_offset: 0,
            data_size: 0,
            flags: 0x04, // Directory flag
            is_directory: true,
        };
        
        assert!(entry.is_directory);
    }
}
