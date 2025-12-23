//! ISO 9660 file system support
//!
//! Basic ISO 9660 parser for reading Blu-ray disc images

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

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
pub struct IsoVolume {
    /// Volume identifier
    pub volume_id: String,
    /// System identifier
    pub system_id: String,
    /// Volume size in blocks
    pub volume_size: u32,
    /// Block size (typically 2048)
    pub block_size: u32,
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

        Ok(Self {
            volume_id,
            system_id,
            volume_size,
            block_size,
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
        };
        
        assert_eq!(volume.volume_size_bytes(), 100 * 2048);
    }
}
