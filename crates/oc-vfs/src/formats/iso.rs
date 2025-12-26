//! ISO 9660 file system support
//!
//! Basic ISO 9660 parser for reading Blu-ray disc images

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::fs::File;
use std::io::BufReader;

/// ISO 9660 volume descriptor type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeDescriptorType {
    BootRecord = 0,
    Primary = 1,
    Supplementary = 2,
    VolumePartition = 3,
    Terminator = 255,
}

/// ISO 9660 directory entry
#[derive(Debug, Clone)]
pub struct IsoDirectoryEntry {
    /// File name
    pub name: String,
    /// File size in bytes
    pub size: u32,
    /// Starting logical block address
    pub lba: u32,
    /// Is directory
    pub is_directory: bool,
}

/// ISO 9660 volume
#[derive(Debug, Clone)]
pub struct IsoVolume {
    /// Volume identifier
    pub volume_id: String,
    /// System identifier
    pub system_id: String,
    /// Volume size in blocks
    pub volume_size: u32,
    /// Block size (typically 2048)
    pub block_size: u32,
    /// Root directory LBA
    pub root_dir_lba: u32,
    /// Root directory size
    pub root_dir_size: u32,
}

impl IsoVolume {
    /// Parse ISO 9660 volume from reader
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self, std::io::Error> {
        // Seek to volume descriptor at sector 16
        reader.seek(SeekFrom::Start(16 * 2048))?;

        let mut descriptor = [0u8; 2048];
        reader.read_exact(&mut descriptor)?;

        // Check for ISO 9660 magic
        if &descriptor[1..6] != b"CD001" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid ISO 9660 signature",
            ));
        }

        // Parse primary volume descriptor
        let volume_id = String::from_utf8_lossy(&descriptor[40..72])
            .trim()
            .to_string();
        
        let system_id = String::from_utf8_lossy(&descriptor[8..40])
            .trim()
            .to_string();

        let volume_size = u32::from_le_bytes([
            descriptor[80],
            descriptor[81],
            descriptor[82],
            descriptor[83],
        ]);

        let block_size = u16::from_le_bytes([
            descriptor[128],
            descriptor[129],
        ]) as u32;

        // Parse root directory record (at offset 156)
        let root_dir_lba = u32::from_le_bytes([
            descriptor[158],
            descriptor[159],
            descriptor[160],
            descriptor[161],
        ]);
        
        let root_dir_size = u32::from_le_bytes([
            descriptor[166],
            descriptor[167],
            descriptor[168],
            descriptor[169],
        ]);

        Ok(Self {
            volume_id,
            system_id,
            volume_size,
            block_size,
            root_dir_lba,
            root_dir_size,
        })
    }

    /// Get volume size in bytes
    pub fn volume_size_bytes(&self) -> u64 {
        self.volume_size as u64 * self.block_size as u64
    }
}

/// ISO 9660 reader
pub struct IsoReader {
    /// Path to ISO file
    pub path: PathBuf,
    /// Volume information
    pub volume: Option<IsoVolume>,
}

impl IsoReader {
    /// Create a new ISO reader
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            volume: None,
        }
    }

    /// Open and parse the ISO file
    pub fn open(&mut self) -> Result<(), std::io::Error> {
        let file = std::fs::File::open(&self.path)?;
        let mut reader = std::io::BufReader::new(file);
        
        let volume = IsoVolume::parse(&mut reader)?;
        self.volume = Some(volume);
        
        tracing::info!("Opened ISO: {:?}, Volume ID: {}", 
            self.path, 
            self.volume.as_ref().unwrap().volume_id
        );
        
        Ok(())
    }

    /// Get volume information
    pub fn volume(&self) -> Option<&IsoVolume> {
        self.volume.as_ref()
    }

    /// Read a file from the ISO by path
    /// Path should be like "/PS3_GAME/USRDIR/EBOOT.BIN"
    pub fn read_file(&self, file_path: &str) -> Result<Vec<u8>, std::io::Error> {
        let volume = self.volume.as_ref().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "ISO not opened")
        })?;

        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        // Normalize path
        let path_parts: Vec<&str> = file_path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if path_parts.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Empty file path",
            ));
        }

        // Start from root directory
        let mut current_lba = volume.root_dir_lba;
        let mut current_size = volume.root_dir_size;
        let block_size = volume.block_size;

        // Navigate through directories
        for (i, part) in path_parts.iter().enumerate() {
            let entries = self.read_directory_entries(&mut reader, current_lba, current_size, block_size)?;
            
            let target_name = part.to_uppercase();
            let entry = entries.iter().find(|e| {
                // ISO 9660 names may have version suffix ";1"
                let name = e.name.split(';').next().unwrap_or(&e.name).to_uppercase();
                name == target_name
            });

            match entry {
                Some(e) => {
                    if i == path_parts.len() - 1 {
                        // This is the file we're looking for
                        if e.is_directory {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("{} is a directory", part),
                            ));
                        }
                        // Read the file content
                        return self.read_file_content(&mut reader, e.lba, e.size, block_size);
                    } else {
                        // This is an intermediate directory
                        if !e.is_directory {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::NotFound,
                                format!("{} is not a directory", part),
                            ));
                        }
                        current_lba = e.lba;
                        current_size = e.size;
                    }
                }
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("'{}' not found in ISO", part),
                    ));
                }
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ))
    }

    /// Read directory entries from ISO
    fn read_directory_entries<R: Read + Seek>(
        &self,
        reader: &mut R,
        lba: u32,
        size: u32,
        block_size: u32,
    ) -> Result<Vec<IsoDirectoryEntry>, std::io::Error> {
        let mut entries = Vec::new();
        let mut bytes_read = 0u32;

        reader.seek(SeekFrom::Start(lba as u64 * block_size as u64))?;

        while bytes_read < size {
            let mut record_len = [0u8; 1];
            reader.read_exact(&mut record_len)?;
            bytes_read += 1;

            if record_len[0] == 0 {
                // End of sector padding, skip to next sector
                let remaining_in_sector = block_size - (bytes_read % block_size);
                if remaining_in_sector < block_size {
                    reader.seek(SeekFrom::Current(remaining_in_sector as i64 - 1))?;
                    bytes_read += remaining_in_sector - 1;
                }
                continue;
            }

            let record_size = record_len[0] as usize;
            let mut record = vec![0u8; record_size - 1];
            reader.read_exact(&mut record)?;
            bytes_read += record_size as u32 - 1;

            // Parse directory entry
            // Offset 1: Extended attribute record length
            // Offset 2-9: Location of extent (LBA) - little endian at 2-5
            let entry_lba = u32::from_le_bytes([record[1], record[2], record[3], record[4]]);
            
            // Offset 10-17: Data length - little endian at 10-13
            let entry_size = u32::from_le_bytes([record[9], record[10], record[11], record[12]]);
            
            // Offset 25: File flags
            let flags = record[24];
            let is_directory = (flags & 0x02) != 0;
            
            // Offset 32: File identifier length
            let name_len = record[31] as usize;
            
            // Offset 33+: File identifier
            if name_len > 0 && 32 + name_len <= record.len() {
                let name_bytes = &record[32..32 + name_len];
                let name = if name_len == 1 && name_bytes[0] == 0 {
                    ".".to_string()
                } else if name_len == 1 && name_bytes[0] == 1 {
                    "..".to_string()
                } else {
                    String::from_utf8_lossy(name_bytes)
                        .trim_end_matches(char::from(0))
                        .to_string()
                };

                // Skip "." and ".." entries
                if name != "." && name != ".." {
                    entries.push(IsoDirectoryEntry {
                        name,
                        size: entry_size,
                        lba: entry_lba,
                        is_directory,
                    });
                }
            }
        }

        Ok(entries)
    }

    /// Read file content from ISO
    fn read_file_content<R: Read + Seek>(
        &self,
        reader: &mut R,
        lba: u32,
        size: u32,
        block_size: u32,
    ) -> Result<Vec<u8>, std::io::Error> {
        reader.seek(SeekFrom::Start(lba as u64 * block_size as u64))?;
        let mut data = vec![0u8; size as usize];
        reader.read_exact(&mut data)?;
        Ok(data)
    }

    /// List files in a directory
    pub fn list_directory(&self, dir_path: &str) -> Result<Vec<IsoDirectoryEntry>, std::io::Error> {
        let volume = self.volume.as_ref().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "ISO not opened")
        })?;

        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        let path_parts: Vec<&str> = dir_path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current_lba = volume.root_dir_lba;
        let mut current_size = volume.root_dir_size;
        let block_size = volume.block_size;

        // Navigate to the target directory
        for part in path_parts {
            let entries = self.read_directory_entries(&mut reader, current_lba, current_size, block_size)?;
            
            let target_name = part.to_uppercase();
            let entry = entries.iter().find(|e| {
                let name = e.name.split(';').next().unwrap_or(&e.name).to_uppercase();
                name == target_name
            });

            match entry {
                Some(e) if e.is_directory => {
                    current_lba = e.lba;
                    current_size = e.size;
                }
                Some(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{} is not a directory", part),
                    ));
                }
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Directory '{}' not found", part),
                    ));
                }
            }
        }

        self.read_directory_entries(&mut reader, current_lba, current_size, block_size)
    }

    /// Check if a file exists in the ISO
    pub fn file_exists(&self, file_path: &str) -> bool {
        self.read_file(file_path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso_reader_creation() {
        let reader = IsoReader::new(PathBuf::from("/tmp/test.iso"));
        assert!(reader.volume.is_none());
    }

    #[test]
    fn test_volume_size_calculation() {
        let volume = IsoVolume {
            volume_id: "TEST".to_string(),
            system_id: "LINUX".to_string(),
            volume_size: 100,
            block_size: 2048,
            root_dir_lba: 0,
            root_dir_size: 0,
        };
        
        assert_eq!(volume.volume_size_bytes(), 100 * 2048);
    }
}
