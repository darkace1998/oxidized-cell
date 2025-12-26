//! SELF file loader

use oc_core::error::LoaderError;
use crate::crypto::{CryptoEngine, KeyType};
use tracing::{debug, info, warn};

/// SELF file magic
pub const SELF_MAGIC: [u8; 4] = [0x53, 0x43, 0x45, 0x00]; // "SCE\0"

/// SELF file header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SelfHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub key_type: u16,
    pub header_type: u16,
    pub metadata_offset: u32,
    pub header_len: u64,
    pub data_len: u64,
}

/// SELF application info
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AppInfo {
    pub auth_id: u64,
    pub vendor_id: u32,
    pub self_type: u32,
    pub version: u64,
    pub padding: u64,
}

/// SELF metadata info
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MetadataInfo {
    pub key_pad: [u8; 16],
    pub iv_pad: [u8; 16],
}

/// SELF metadata header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MetadataHeader {
    pub signature_input_length: u64,
    pub unknown1: u32,
    pub section_count: u32,
    pub key_count: u32,
    pub optional_header_size: u32,
    pub unknown2: u64,
    pub unknown3: u64,
}

/// SELF metadata section header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MetadataSectionHeader {
    pub data_offset: u64,
    pub data_size: u64,
    pub section_type: u32,
    pub section_index: u32,
    pub hashed: u32,
    pub sha1_index: u32,
    pub encrypted: u32,
    pub key_index: u32,
    pub iv_index: u32,
    pub compressed: u32,
}

/// SELF loader with decryption support
pub struct SelfLoader {
    crypto: CryptoEngine,
}

impl SelfLoader {
    /// Create a new SELF loader
    pub fn new() -> Self {
        Self {
            crypto: CryptoEngine::new(),
        }
    }

    /// Create a new SELF loader with firmware keys loaded
    pub fn with_firmware(firmware_path: &str) -> Result<Self, LoaderError> {
        let mut crypto = CryptoEngine::new();
        crypto.load_firmware_keys(firmware_path)?;
        Ok(Self { crypto })
    }

    /// Create a new SELF loader with a keys file
    pub fn with_keys_file(keys_path: &str) -> Result<Self, LoaderError> {
        let mut crypto = CryptoEngine::new();
        crypto.load_keys_file(keys_path)?;
        Ok(Self { crypto })
    }

    /// Get a reference to the crypto engine
    pub fn crypto(&self) -> &CryptoEngine {
        &self.crypto
    }

    /// Get a mutable reference to the crypto engine
    pub fn crypto_mut(&mut self) -> &mut CryptoEngine {
        &mut self.crypto
    }

    /// Check if decryption keys are available
    pub fn has_keys(&self) -> bool {
        self.crypto.has_firmware_keys()
    }

    /// Check if data is a SELF file
    pub fn is_self(data: &[u8]) -> bool {
        data.len() >= 4 && data[0..4] == SELF_MAGIC
    }

    /// Parse SELF header
    pub fn parse_header(data: &[u8]) -> Result<SelfHeader, LoaderError> {
        if data.len() < 32 {
            return Err(LoaderError::InvalidSelf("File too small".to_string()));
        }

        if !Self::is_self(data) {
            return Err(LoaderError::InvalidSelf("Invalid SELF magic".to_string()));
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[0..4]);

        let header = SelfHeader {
            magic,
            version: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            key_type: u16::from_be_bytes([data[8], data[9]]),
            header_type: u16::from_be_bytes([data[10], data[11]]),
            metadata_offset: u32::from_be_bytes([data[12], data[13], data[14], data[15]]),
            header_len: u64::from_be_bytes([
                data[16], data[17], data[18], data[19],
                data[20], data[21], data[22], data[23],
            ]),
            data_len: u64::from_be_bytes([
                data[24], data[25], data[26], data[27],
                data[28], data[29], data[30], data[31],
            ]),
        };

        info!(
            "SELF header: version=0x{:x}, key_type=0x{:x}, metadata_offset=0x{:x}",
            header.version, header.key_type, header.metadata_offset
        );

        Ok(header)
    }

    /// Parse application info
    pub fn parse_app_info(data: &[u8], offset: usize) -> Result<AppInfo, LoaderError> {
        if data.len() < offset + 40 {
            return Err(LoaderError::InvalidSelf("Invalid app info offset".to_string()));
        }

        let info = AppInfo {
            auth_id: u64::from_be_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]),
            vendor_id: u32::from_be_bytes([
                data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11],
            ]),
            self_type: u32::from_be_bytes([
                data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15],
            ]),
            version: u64::from_be_bytes([
                data[offset + 16], data[offset + 17], data[offset + 18], data[offset + 19],
                data[offset + 20], data[offset + 21], data[offset + 22], data[offset + 23],
            ]),
            padding: u64::from_be_bytes([
                data[offset + 24], data[offset + 25], data[offset + 26], data[offset + 27],
                data[offset + 28], data[offset + 29], data[offset + 30], data[offset + 31],
            ]),
        };

        debug!(
            "App info: auth_id=0x{:x}, type=0x{:x}",
            info.auth_id, info.self_type
        );

        Ok(info)
    }

    /// Decrypt SELF file and extract ELF
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, LoaderError> {
        info!("Starting SELF decryption");

        let header = Self::parse_header(data)?;

        // For now, attempt to extract without full decryption
        // In a real implementation, this would:
        // 1. Parse metadata
        // 2. Decrypt metadata using MetaLV2 keys
        // 3. Extract and decrypt each section
        // 4. Reconstruct ELF file

        // Check if we can extract the embedded ELF header
        let elf_offset = header.header_len as usize;
        
        if data.len() < elf_offset + 4 {
            return Err(LoaderError::InvalidSelf("Invalid ELF offset".to_string()));
        }

        // Check for ELF magic at expected offset
        if data[elf_offset..elf_offset + 4] == [0x7F, b'E', b'L', b'F'] {
            info!("Found unencrypted ELF data");
            // Extract the ELF portion
            return Ok(data[elf_offset..].to_vec());
        }

        // Attempt basic decryption
        warn!("Encrypted SELF detected, attempting decryption");
        
        // Map SELF key_type to our KeyType enum
        // Key types in SELF header:
        // 0x01 = retail/production
        // 0x02 = debug  
        // 0x1c (28) = retail game NPDRM
        // 0x1d (29) = retail game disc
        // 0x1e (30) = retail game HDD
        // 0x02-0x10 = various system modules
        let key_type_enum = match header.key_type {
            0x0001 => KeyType::Retail,
            0x0002 => KeyType::Debug,
            0x0003 => KeyType::Lv0,
            0x0004 => KeyType::Lv1,
            0x0005 => KeyType::Lv2,
            0x0006 => KeyType::App,
            0x0007 => KeyType::IsoSpu,
            0x0008 => KeyType::Lv2,
            0x000C => KeyType::Vsh,
            0x001C => KeyType::Npd,      // NPDRM game
            0x001D => KeyType::Retail,   // Disc game
            0x001E => KeyType::Retail,   // HDD game
            _ => {
                return Err(LoaderError::DecryptionFailed(
                    format!("Unsupported key type: 0x{:02x}. This game requires decryption keys.\n\
                             To play encrypted games, you need to:\n\
                             1. Place PS3 firmware (PS3UPDAT.PUP) in the 'firmware/' folder\n\
                             2. Or provide a keys.txt file with the required keys\n\
                             3. Or use a decrypted EBOOT.ELF file", header.key_type)
                ))
            }
        };
        
        self.decrypt_with_key(data, &header, key_type_enum)
    }

    /// Decrypt SELF with specific key type
    fn decrypt_with_key(
        &self,
        data: &[u8],
        header: &SelfHeader,
        key_type: KeyType,
    ) -> Result<Vec<u8>, LoaderError> {
        let metadata_offset = header.metadata_offset as usize;
        
        if data.len() < metadata_offset + 16 {
            return Err(LoaderError::InvalidSelf("Invalid metadata offset".to_string()));
        }

        // Extract metadata info
        let mut key_pad = [0u8; 16];
        let mut iv_pad = [0u8; 16];
        key_pad.copy_from_slice(&data[metadata_offset..metadata_offset + 16]);
        iv_pad.copy_from_slice(&data[metadata_offset + 16..metadata_offset + 32]);

        debug!("Metadata key_pad: {:02x?}", &key_pad[..8]);
        debug!("Metadata iv_pad: {:02x?}", &iv_pad[..8]);

        // Get decryption key
        let key = self.crypto.get_key(key_type)
            .ok_or_else(|| LoaderError::DecryptionFailed("Key not available".to_string()))?;

        // Decrypt metadata
        let metadata_size = (header.header_len as usize) - metadata_offset;
        let encrypted_metadata = &data[metadata_offset..metadata_offset + metadata_size];
        
        let _decrypted_metadata = self.crypto.decrypt_aes(encrypted_metadata, key, &iv_pad)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Metadata decryption failed: {}", e)))?;

        // Parse decrypted metadata and extract sections
        // This is simplified - real implementation would parse all section headers
        
        // For now, return a placeholder error since full decryption requires proper keys
        Err(LoaderError::DecryptionFailed(
            "Full SELF decryption requires valid encryption keys".to_string()
        ))
    }

    /// Decrypt metadata section (MetaLV2)
    pub fn decrypt_metadata_lv2(
        &self,
        encrypted_data: &[u8],
        key: &[u8],
        iv: &[u8],
    ) -> Result<Vec<u8>, LoaderError> {
        debug!("Decrypting MetaLV2 metadata");
        
        self.crypto.decrypt_aes(encrypted_data, key, iv)
            .map_err(|e| LoaderError::DecryptionFailed(format!("MetaLV2 decryption failed: {}", e)))
    }
}

impl Default for SelfLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_magic() {
        assert_eq!(SELF_MAGIC, [0x53, 0x43, 0x45, 0x00]);
    }

    #[test]
    fn test_is_self() {
        let self_data = [0x53, 0x43, 0x45, 0x00, 0x00, 0x00];
        assert!(SelfLoader::is_self(&self_data));

        let elf_data = [0x7F, b'E', b'L', b'F', 0x00, 0x00];
        assert!(!SelfLoader::is_self(&elf_data));
    }
}
