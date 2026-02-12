//! SELF file loader

use oc_core::error::LoaderError;
use crate::crypto::CryptoEngine;
use tracing::{debug, info, warn};
use flate2::read::ZlibDecoder;
use std::io::Read;

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

        // Log raw header bytes for debugging
        info!(
            "SELF header raw bytes: {:02x} {:02x} {:02x} {:02x}  {:02x} {:02x} {:02x} {:02x}  {:02x} {:02x} {:02x} {:02x}  {:02x} {:02x} {:02x} {:02x}",
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
            data[8], data[9], data[10], data[11],
            data[12], data[13], data[14], data[15]
        );

        info!(
            "SELF header: version=0x{:08x}, se_flags=0x{:04x}, se_type=0x{:04x}, metadata_offset=0x{:08x}",
            header.version, header.key_type, header.header_type, header.metadata_offset
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
        // Note: This is for unencrypted SELF files (debug builds, homebrew, etc.)
        // If data at this offset is NOT ELF magic, we need full decryption
        if data[elf_offset..elf_offset + 4] == [0x7F, b'E', b'L', b'F'] {
            info!("Found unencrypted ELF data at offset 0x{:x} (header_len=0x{:x})", 
                elf_offset, header.header_len);
            // Extract the ELF portion
            return Ok(data[elf_offset..].to_vec());
        }

        // Data is encrypted - need full decryption
        info!(
            "No ELF magic at offset 0x{:x}, data is encrypted. Bytes: {:02x?}",
            elf_offset,
            &data[elf_offset..elf_offset + 4]
        );
        warn!("Encrypted SELF detected, attempting full decryption...");
        
        // Parse the extended header to get app info offset
        // Extended header starts at offset 32 (after main SELF header)
        if data.len() < 32 + 80 {
            return Err(LoaderError::InvalidSelf("File too small for extended header".to_string()));
        }
        
        // Read ext_hdr (extended header) - 80 bytes starting at offset 32
        let ext_hdr_offset = 32usize;
        let app_info_offset = u64::from_be_bytes([
            data[ext_hdr_offset + 8], data[ext_hdr_offset + 9], 
            data[ext_hdr_offset + 10], data[ext_hdr_offset + 11],
            data[ext_hdr_offset + 12], data[ext_hdr_offset + 13], 
            data[ext_hdr_offset + 14], data[ext_hdr_offset + 15],
        ]) as usize;
        
        let elf_hdr_offset = u64::from_be_bytes([
            data[ext_hdr_offset + 16], data[ext_hdr_offset + 17], 
            data[ext_hdr_offset + 18], data[ext_hdr_offset + 19],
            data[ext_hdr_offset + 20], data[ext_hdr_offset + 21], 
            data[ext_hdr_offset + 22], data[ext_hdr_offset + 23],
        ]) as usize;
        
        info!(
            "Extended header: app_info_offset=0x{:x}, elf_hdr_offset=0x{:x}",
            app_info_offset, elf_hdr_offset
        );
        
        // Parse app info at the specified offset
        let app_info = Self::parse_app_info(data, app_info_offset)?;
        
        // The program_type from app_info determines which key set to use
        // RPCS3 approach: 
        //   - program_type (from AppInfo.self_type) selects key array (APP, NPDRM, LV2, etc.)
        //   - se_flags (from SCE header at offset 0x08, 2 bytes) is the revision
        //   - program_sceversion (from AppInfo.version) is only used for LV1/LV2/ISO keys
        //
        // For APP keys (type 4): only program_type and revision matter
        // For NPDRM keys (type 8): only program_type and revision matter
        let program_type = app_info.self_type;
        let se_flags = header.key_type; // This is se_flags from SCE header (revision)
        let program_sceversion = app_info.version;
        
        info!(
            "SELF info: program_type={}, se_flags=0x{:04x}, program_sceversion=0x{:016x}",
            program_type, se_flags, program_sceversion
        );
        
        self.decrypt_with_program_type(data, &header, program_type, se_flags, program_sceversion)
    }

    /// Decrypt SELF with specific program type (matching RPCS3's approach)
    fn decrypt_with_program_type(
        &self,
        data: &[u8],
        header: &SelfHeader,
        program_type: u32,
        se_flags: u16,
        _program_sceversion: u64,
    ) -> Result<Vec<u8>, LoaderError> {
        // Check if this is a DEBUG SELF (se_flags & 0x8000)
        // Debug SELFs have unencrypted metadata
        let is_debug = (se_flags & 0x8000) == 0x8000;
        
        if is_debug {
            info!("DEBUG SELF detected - metadata is not encrypted");
        }
        
        // Map program_type to internal key type (they're the same, 1:1 mapping)
        // RPCS3's KeyVault::FindSelfKey() uses program_type directly as the switch case
        // KEY_LV0 = 1, KEY_LV1 = 2, KEY_LV2 = 3, KEY_APP = 4, KEY_ISO = 5, 
        // KEY_LDR = 6, KEY_UNK7 = 7, KEY_NPDRM = 8
        let key_type: u16 = match program_type {
            1 => 1,  // KEY_LV0
            2 => 2,  // KEY_LV1
            3 => 3,  // KEY_LV2
            4 => 4,  // KEY_APP (retail games from disc)
            5 => 5,  // KEY_ISO (SPU isolated modules)
            6 => 6,  // KEY_LDR (secure loader)
            7 => 7,  // KEY_UNK7
            8 => 8,  // KEY_NPDRM (PSN downloaded content)
            _ => {
                return Err(LoaderError::DecryptionFailed(format!(
                    "Unknown program_type: {}. Expected 1-8.\n\
                     This may be an unsupported SELF format.",
                    program_type
                )));
            }
        };
        
        // For APP/NPDRM keys: revision = se_flags & 0x7FFF (remove debug flag)
        // For LV1/LV2/ISO: revision is used differently (version-based)
        // RPCS3's GetSelfAPPKey() and GetSelfNPDRMKey() only look at revision
        let revision = se_flags & 0x7FFF; // Remove debug flag (0x8000) for key lookup
        
        info!(
            "Key lookup: key_type={} (program_type={}), revision=0x{:04x}, is_debug={}",
            key_type, program_type, revision, is_debug
        );
        
        // Get the key set using key type and revision (matching RPCS3's KeyVault::FindSelfKey)
        let key_set = self.crypto.get_self_key_set(key_type, revision)
            .ok_or_else(|| {
                let available_keys = self.crypto.list_available_keys();
                let key_count = self.crypto.self_key_count();
                
                let key_type_name = match key_type {
                    1 => "LV0 (bootloader)",
                    2 => "LV1 (hypervisor)",
                    3 => "LV2 (kernel)",
                    4 => "APP (retail disc games)",
                    5 => "ISO (SPU modules)",
                    6 => "LDR (secure loader)",
                    7 => "UNK7",
                    8 => "NPDRM (PSN content)",
                    _ => "Unknown",
                };
                
                LoaderError::DecryptionFailed(format!(
                    "DECRYPTION KEY NOT FOUND\n\n\
                     Needed key: type={} ({}), revision=0x{:04x}\n\
                     Available: {} key sets loaded\n\n\
                     This game requires a specific decryption key that is not currently available.\n\n\
                     Key Information:\n\
                     - Type {} is for: {}\n\
                     - Revision 0x{:04x} corresponds to specific PS3 firmware version\n\n\
                     Built-in APP keys (for retail disc games):\n\
                     - Revisions 0x0000 to 0x001D (firmware 1.00 to 4.88)\n\n\
                     Troubleshooting:\n\
                     1. If revision > 0x001D: You need newer keys from PS3 firmware 4.90+\n\
                     2. If type is 8 (NPDRM): PSN games need .rap license files\n\
                     3. Check if this is a debug SELF (homebrew) - may need different keys\n\n\
                     Available keys:\n{}",
                    key_type, key_type_name, revision,
                    key_count,
                    key_type, key_type_name, revision,
                    available_keys
                ))
            })?;
                   
        info!("Found key set: type={}, rev=0x{:04x}", key_set.key_type, key_set.revision);
        
        let metadata_offset = header.metadata_offset as usize;
        
        if data.len() < metadata_offset + 64 {
            return Err(LoaderError::InvalidSelf("Invalid metadata offset".to_string()));
        }

        // Metadata info is 64 bytes: key[16] + key_pad[16] + iv[16] + iv_pad[16]
        // It's located at: sce_header(32) + se_meta offset
        let meta_info_offset = 32 + metadata_offset;
        
        if data.len() < meta_info_offset + 64 {
            return Err(LoaderError::InvalidSelf("File too small for metadata info".to_string()));
        }
        
        debug!("Reading metadata info at offset 0x{:x}", meta_info_offset);

        // Read metadata info
        let mut metadata_info = [0u8; 64];
        metadata_info.copy_from_slice(&data[meta_info_offset..meta_info_offset + 64]);
        
        let decrypted_meta_info = if is_debug {
            // Debug SELF - metadata is not encrypted
            metadata_info.to_vec()
        } else {
            // Use the key from the key set for AES-256 decryption
            let key = &key_set.erk;
            let iv = &key_set.riv;
            
            debug!("ERK (first 8 bytes): {:02x?}", &key[..8]);
            debug!("RIV (first 8 bytes): {:02x?}", &iv[..8]);
            
            // Decrypt metadata info with AES-256-CBC
            self.crypto.decrypt_aes(&metadata_info, key, iv)
                .map_err(|e| LoaderError::DecryptionFailed(format!("Metadata info decryption failed: {}", e)))?
        };
        
        // Check if decryption was successful by verifying padding bytes
        // key_pad[16] and iv_pad[16] should be zeros if decryption worked
        if decrypted_meta_info.len() < 64 {
            return Err(LoaderError::DecryptionFailed(
                "Decrypted metadata info too small".to_string()
            ));
        }
        
        let key_pad = &decrypted_meta_info[16..32];
        let iv_pad = &decrypted_meta_info[48..64];
        
        if key_pad[0] != 0 || iv_pad[0] != 0 {
            // Log what we got for debugging
            warn!("Metadata decryption verification FAILED!");
            warn!("  key_pad[0]=0x{:02x} (expected 0x00), iv_pad[0]=0x{:02x} (expected 0x00)", 
                key_pad[0], iv_pad[0]);
            warn!("  key_pad: {:02x?}", key_pad);
            warn!("  iv_pad: {:02x?}", iv_pad);
            warn!("  First 16 bytes (data_key would be): {:02x?}", &decrypted_meta_info[0..16]);
            
            return Err(LoaderError::DecryptionFailed(format!(
                "Metadata decryption verification failed (key_pad[0]=0x{:02x}, iv_pad[0]=0x{:02x}). \
                 This usually means wrong decryption key was used. The SELF may require a different key revision.",
                key_pad[0], iv_pad[0]
            )));
        }
        
        info!("Metadata info decrypted successfully!");
        
        // Extract the actual key and IV from decrypted metadata info
        // Structure: key[16] + key_pad[16] + iv[16] + iv_pad[16]
        let data_key: [u8; 16] = decrypted_meta_info[0..16].try_into().unwrap();
        let data_iv: [u8; 16] = decrypted_meta_info[32..48].try_into().unwrap();
        
        debug!("Data key: {:02x?}", data_key);
        debug!("Data IV: {:02x?}", data_iv);
        
        // Now decrypt the metadata headers using AES-CTR
        // Metadata headers are located after metadata info
        let meta_headers_offset = meta_info_offset + 64;
        let expected_offset = 32 + metadata_offset + 64;
        
        if (header.header_len as usize) <= expected_offset {
            return Err(LoaderError::InvalidSelf(format!(
                "Header length 0x{:x} is too small for expected offset 0x{:x}",
                header.header_len, expected_offset
            )));
        }
        
        let meta_headers_size = (header.header_len as usize) - expected_offset;
        
        debug!("Metadata headers: offset=0x{:x}, size=0x{:x} (header_len=0x{:x}, metadata_offset=0x{:x})", 
            meta_headers_offset, meta_headers_size, header.header_len, metadata_offset);
        
        if meta_headers_size == 0 || meta_headers_size > 1024 * 1024 {
            return Err(LoaderError::InvalidSelf(format!(
                "Invalid metadata headers size: 0x{:x}. Header layout may be different.",
                meta_headers_size
            )));
        }
        
        if data.len() < meta_headers_offset + meta_headers_size {
            return Err(LoaderError::InvalidSelf("File too small for metadata headers".to_string()));
        }
        
        let encrypted_headers = &data[meta_headers_offset..meta_headers_offset + meta_headers_size];
        
        // Decrypt metadata headers with AES-128-CTR
        let decrypted_headers = self.crypto.decrypt_aes_ctr(encrypted_headers, &data_key, &data_iv)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Metadata headers decryption failed: {}", e)))?;
        
        // Parse metadata header (first 32 bytes)
        if decrypted_headers.len() < 32 {
            return Err(LoaderError::InvalidSelf("Metadata headers too small".to_string()));
        }
        
        let meta_hdr = Self::parse_metadata_header(&decrypted_headers)?;
        info!("Metadata header: {} sections, {} keys", meta_hdr.section_count, meta_hdr.key_count);
        
        // Sanity check - if section_count is 0 or unreasonably high, metadata decryption likely failed
        if meta_hdr.section_count == 0 || meta_hdr.section_count > 100 {
            return Err(LoaderError::DecryptionFailed(format!(
                "Invalid metadata header: section_count={}, key_count={}. Metadata decryption may have failed.",
                meta_hdr.section_count, meta_hdr.key_count
            )));
        }
        
        // Parse metadata section headers
        let section_hdr_offset = 32; // After MetadataHeader
        let section_hdr_size = 48; // Each MetadataSectionHeader is 48 bytes
        
        let mut section_headers = Vec::new();
        for i in 0..meta_hdr.section_count as usize {
            let offset = section_hdr_offset + i * section_hdr_size;
            if decrypted_headers.len() < offset + section_hdr_size {
                return Err(LoaderError::InvalidSelf("Metadata section headers truncated".to_string()));
            }
            let section_hdr = Self::parse_section_header(&decrypted_headers[offset..])?;
            section_headers.push(section_hdr);
        }
        
        // Extract data keys from the decrypted metadata headers
        // Keys are stored after section headers as a flat array of 16-byte blocks
        // RPCS3: memcpy(data_keys.get(), metadata_headers.get() + sizeof(meta_hdr) + meta_hdr.section_count * sizeof(MetadataSectionHeader), data_keys_length);
        // Each section's key_idx and iv_idx point directly into this array
        let keys_offset = section_hdr_offset + (meta_hdr.section_count as usize * section_hdr_size);
        let data_keys_length = meta_hdr.key_count as usize * 16;
        
        // Store all keys as a flat array of 16-byte blocks
        let mut data_keys: Vec<[u8; 16]> = Vec::new();
        for i in 0..meta_hdr.key_count as usize {
            let key_offset = keys_offset + i * 16;
            if decrypted_headers.len() >= key_offset + 16 {
                let mut key = [0u8; 16];
                key.copy_from_slice(&decrypted_headers[key_offset..key_offset + 16]);
                data_keys.push(key);
            }
        }
        
        info!("Extracted {} data keys (total {} bytes) from offset 0x{:x}", data_keys.len(), data_keys_length, keys_offset);
        // Log first few keys to debug
        for (i, key) in data_keys.iter().enumerate().take(20) {
            info!("  data_keys[{}] = {:02x?}", i, key);
        }
        
        // Now we need to read the ELF header to understand the structure
        // Extended header starts at offset 32
        let ext_hdr_offset = 32usize;
        
        // Parse ext_hdr to get ELF header offset
        let elf_offset = u64::from_be_bytes([
            data[ext_hdr_offset + 16], data[ext_hdr_offset + 17],
            data[ext_hdr_offset + 18], data[ext_hdr_offset + 19],
            data[ext_hdr_offset + 20], data[ext_hdr_offset + 21],
            data[ext_hdr_offset + 22], data[ext_hdr_offset + 23],
        ]) as usize;
        
        let phdr_offset = u64::from_be_bytes([
            data[ext_hdr_offset + 24], data[ext_hdr_offset + 25],
            data[ext_hdr_offset + 26], data[ext_hdr_offset + 27],
            data[ext_hdr_offset + 28], data[ext_hdr_offset + 29],
            data[ext_hdr_offset + 30], data[ext_hdr_offset + 31],
        ]) as usize;
        
        debug!("ELF header offset: 0x{:x}, Program header offset: 0x{:x}", elf_offset, phdr_offset);
        
        // Read ELF header
        if data.len() < elf_offset + 64 {
            return Err(LoaderError::InvalidSelf("File too small for ELF header".to_string()));
        }
        
        // Check ELF magic
        if data[elf_offset..elf_offset + 4] != [0x7F, b'E', b'L', b'F'] {
            return Err(LoaderError::InvalidSelf("Invalid ELF magic in embedded ELF".to_string()));
        }
        
        let is_elf64 = data[elf_offset + 4] == 2;
        debug!("ELF format: {}", if is_elf64 { "64-bit" } else { "32-bit" });
        
        // Parse ELF header to get entry point and program header info
        let (e_entry, e_phoff, e_phentsize, e_phnum) = if is_elf64 {
            let entry = u64::from_be_bytes(data[elf_offset + 24..elf_offset + 32].try_into().unwrap());
            let phoff = u64::from_be_bytes(data[elf_offset + 32..elf_offset + 40].try_into().unwrap());
            let phentsize = u16::from_be_bytes(data[elf_offset + 54..elf_offset + 56].try_into().unwrap());
            let phnum = u16::from_be_bytes(data[elf_offset + 56..elf_offset + 58].try_into().unwrap());
            (entry, phoff, phentsize, phnum)
        } else {
            let entry = u32::from_be_bytes(data[elf_offset + 24..elf_offset + 28].try_into().unwrap()) as u64;
            let phoff = u32::from_be_bytes(data[elf_offset + 28..elf_offset + 32].try_into().unwrap()) as u64;
            let phentsize = u16::from_be_bytes(data[elf_offset + 42..elf_offset + 44].try_into().unwrap());
            let phnum = u16::from_be_bytes(data[elf_offset + 44..elf_offset + 46].try_into().unwrap());
            (entry, phoff, phentsize, phnum)
        };
        
        info!("ELF: entry=0x{:x}, phoff=0x{:x}, phentsize={}, phnum={}", e_entry, e_phoff, e_phentsize, e_phnum);
        
        // Read program headers from the SELF file
        if data.len() < phdr_offset + (e_phnum as usize * e_phentsize as usize) {
            return Err(LoaderError::InvalidSelf("File too small for program headers".to_string()));
        }
        
        // Parse program headers
        let mut program_headers = Vec::new();
        for i in 0..e_phnum as usize {
            let ph_off = phdr_offset + i * e_phentsize as usize;
            let (p_type, p_offset, p_vaddr, p_filesz, p_memsz) = if is_elf64 {
                let p_type = u32::from_be_bytes(data[ph_off..ph_off + 4].try_into().unwrap());
                let p_offset = u64::from_be_bytes(data[ph_off + 8..ph_off + 16].try_into().unwrap());
                let p_vaddr = u64::from_be_bytes(data[ph_off + 16..ph_off + 24].try_into().unwrap());
                let p_filesz = u64::from_be_bytes(data[ph_off + 32..ph_off + 40].try_into().unwrap());
                let p_memsz = u64::from_be_bytes(data[ph_off + 40..ph_off + 48].try_into().unwrap());
                (p_type, p_offset, p_vaddr, p_filesz, p_memsz)
            } else {
                let p_type = u32::from_be_bytes(data[ph_off..ph_off + 4].try_into().unwrap());
                let p_offset = u32::from_be_bytes(data[ph_off + 4..ph_off + 8].try_into().unwrap()) as u64;
                let p_vaddr = u32::from_be_bytes(data[ph_off + 8..ph_off + 12].try_into().unwrap()) as u64;
                let p_filesz = u32::from_be_bytes(data[ph_off + 16..ph_off + 20].try_into().unwrap()) as u64;
                let p_memsz = u32::from_be_bytes(data[ph_off + 20..ph_off + 24].try_into().unwrap()) as u64;
                (p_type, p_offset, p_vaddr, p_filesz, p_memsz)
            };
            program_headers.push((p_type, p_offset, p_vaddr, p_filesz, p_memsz));
        }
        
        // Build the output ELF file
        // Start with the ELF header (copy from SELF)
        let elf_header_size = if is_elf64 { 64 } else { 52 };
        let mut elf_data = data[elf_offset..elf_offset + elf_header_size].to_vec();
        
        // The e_phoff in the ELF header tells where program headers are located
        // We need to use this value, not recalculate it
        debug!("ELF e_phoff (from header): 0x{:x}", e_phoff);
        
        // Calculate total ELF size needed
        let mut max_offset: u64 = elf_header_size as u64;
        for (_, p_offset, _, p_filesz, _) in &program_headers {
            let end = p_offset + p_filesz;
            if end > max_offset {
                max_offset = end;
            }
        }
        
        // Resize to accommodate all data
        elf_data.resize(max_offset as usize, 0);
        
        // Copy program headers
        let ph_start = e_phoff as usize;
        let ph_total_size = e_phnum as usize * e_phentsize as usize;
        if elf_data.len() < ph_start + ph_total_size {
            elf_data.resize(ph_start + ph_total_size, 0);
        }
        elf_data[ph_start..ph_start + ph_total_size].copy_from_slice(
            &data[phdr_offset..phdr_offset + ph_total_size]
        );
        
        info!("Processing {} section headers for decryption", section_headers.len());
        
        // Decrypt and copy each section
        let mut sections_written = 0;
        for (i, section_hdr) in section_headers.iter().enumerate() {
            // Only process type 2 sections (actual segment data)
            // Type 1 = section hash (SHA-1), Type 2 = segment data, Type 3 = version info
            if section_hdr.section_type != 2 {
                info!("Section {} has type {} (not segment data, skipping)", i, section_hdr.section_type);
                continue;
            }
            
            if section_hdr.data_size == 0 {
                info!("Section {} has data_size=0, skipping", i);
                continue;
            }
            
            info!("Section {}: type={}, offset=0x{:x}, size=0x{:x}, encrypted={}, compressed={}, key_idx={}, iv_idx={}, program_idx={}",
                i, section_hdr.section_type, section_hdr.data_offset, section_hdr.data_size, 
                section_hdr.encrypted, section_hdr.compressed, 
                section_hdr.key_index, section_hdr.iv_index, section_hdr.section_index);
            
            let section_offset = section_hdr.data_offset as usize;
            let section_size = section_hdr.data_size as usize;
            
            if data.len() < section_offset + section_size {
                warn!("Section {} extends beyond file, skipping", i);
                continue;
            }
            
            // Get the matching program header to find the destination offset
            // This is needed for proper CTR decryption when sections share the same key/IV
            let section_idx = section_hdr.section_index as usize;
            let dest_p_offset = if section_idx < program_headers.len() {
                program_headers[section_idx].1 // p_offset
            } else {
                0
            };
            
            info!("Section {}: destination p_offset=0x{:x}", i, dest_p_offset);
            
            let encrypted_section = &data[section_offset..section_offset + section_size];
            
            // Log raw encrypted bytes at entry point offset if this is the entry segment
            if section_hdr.section_index == 1 {
                let entry_offset = 0x8a60usize; // We know this is the entry point offset within section 1
                if encrypted_section.len() > entry_offset + 16 {
                    info!("Section 1 RAW ENCRYPTED bytes at entry offset 0x8a60: {:02x?}", 
                        &encrypted_section[entry_offset..entry_offset+16]);
                }
            }
            
            // Check if section is encrypted
            let decrypted_section = if section_hdr.encrypted == 3 {
                // IMPORTANT: Check if this "encrypted" section already contains decrypted data.
                // This can happen with pre-decrypted ISOs where the SELF structure is preserved
                // but the data has already been decrypted. Applying decryption again would corrupt it.
                
                // Heuristic 1: If section 0 starts with ELF magic, it's already decrypted
                if i == 0 && encrypted_section.len() >= 4 && encrypted_section[0..4] == [0x7F, b'E', b'L', b'F'] {
                    info!("Section 0 already contains ELF magic - data is NOT encrypted, skipping decryption");
                    return self.extract_from_fake_encrypted_self(data, &header, &section_headers, &program_headers, e_entry);
                }
                
                // Get the key and IV for this section - direct indices into flat data_keys array
                // RPCS3: memcpy(data_key, data_keys.get() + meta_shdr[i].key_idx * 0x10, 0x10);
                // RPCS3: memcpy(data_iv, data_keys.get() + meta_shdr[i].iv_idx * 0x10, 0x10);
                let key_idx = section_hdr.key_index as usize;
                let iv_idx = section_hdr.iv_index as usize;
                
                if key_idx < data_keys.len() && iv_idx < data_keys.len() {
                    let key = &data_keys[key_idx];
                    let iv = &data_keys[iv_idx];
                    
                    info!("Section {} decryption: key_idx={}, iv_idx={}, key={:02x?}, iv={:02x?}", 
                        i, key_idx, iv_idx, key, iv);
                    
                    info!("Section {} encrypted size: {} bytes, offset in file: 0x{:x}, dest offset: 0x{:x}", 
                        i, encrypted_section.len(), section_offset, dest_p_offset);
                    
                    // Each section is decrypted independently with its own key/IV, starting from counter 0.
                    // RPCS3 does NOT adjust the counter based on destination offset - each section 
                    // has its own key/IV pair from data_keys and decryption always starts at counter 0.
                    let result = self.crypto.decrypt_aes_ctr(encrypted_section, key, iv)
                        .unwrap_or_else(|e| {
                            warn!("Section {} AES-CTR decryption failed: {:?}", i, e);
                            encrypted_section.to_vec()
                        });
                    info!("Section {} decrypted: first 16 bytes = {:02x?}", i, &result[0..16.min(result.len())]);
                    
                    // Check decrypted bytes at entry offset for Section 1
                    if section_hdr.section_index == 1 {
                        let entry_offset = 0x8a60usize;
                        if result.len() > entry_offset + 16 {
                            info!("Section 1 DECRYPTED bytes at entry offset 0x8a60: {:02x?}", 
                                &result[entry_offset..entry_offset+16]);
                        }
                    }
                    
                    result
                } else {
                    warn!("Section {} key_idx {} or iv_idx {} out of range (have {} keys)", 
                        i, key_idx, iv_idx, data_keys.len());
                    encrypted_section.to_vec()
                }
            } else {
                // Not encrypted, use as-is
                info!("Section {} not encrypted (encrypted field = {})", i, section_hdr.encrypted);
                encrypted_section.to_vec()
            };
            
            // Decompress if needed
            let final_section = if section_hdr.compressed == 2 {
                // zlib compressed
                let mut decoder = ZlibDecoder::new(&decrypted_section[..]);
                let mut decompressed = Vec::new();
                if decoder.read_to_end(&mut decompressed).is_ok() {
                    decompressed
                } else {
                    warn!("Failed to decompress section {}, using raw data", i);
                    decrypted_section
                }
            } else {
                decrypted_section
            };
            
            // Copy to the correct position in ELF
            // Find the matching program header using section_index (program_idx)
            let section_idx = section_hdr.section_index as usize;
            if section_idx >= program_headers.len() {
                // This shouldn't happen for type 2 sections, but log it
                warn!("Section {} (type 2) has program_idx {} but only {} program headers exist", 
                    i, section_idx, program_headers.len());
                continue;
            }
            
            let (p_type, p_offset, p_vaddr, p_filesz, _p_memsz) = program_headers[section_idx];
            
            // Check if entry point falls within this segment's virtual address range
            let entry_in_segment = e_entry >= p_vaddr && e_entry < (p_vaddr + p_filesz);
            
            info!("Section {} -> PHDR {}: p_type={}, p_vaddr=0x{:x}..0x{:x}, p_offset=0x{:x}, p_filesz=0x{:x} {}",
                i, section_idx, p_type, p_vaddr, p_vaddr + p_filesz, p_offset, p_filesz,
                if entry_in_segment { "<-- ENTRY POINT IS HERE" } else { "" });
            
            // If this is the segment containing the entry point, log what's at the entry point offset
            if entry_in_segment {
                let entry_offset_in_section = (e_entry - p_vaddr) as usize;
                if final_section.len() > entry_offset_in_section + 4 {
                    info!("Entry point within section {}: offset_in_section=0x{:x}, bytes={:02x?}",
                        i, entry_offset_in_section, &final_section[entry_offset_in_section..entry_offset_in_section+4]);
                } else {
                    warn!("Entry point offset 0x{:x} is beyond section size 0x{:x}!", entry_offset_in_section, final_section.len());
                }
            }
            
            let dest_offset = p_offset as usize;
            let section_data = &final_section[..];
                
                // Check if this section data starts with ELF magic
                // If so, it contains segment data that starts at file offset 0
                let has_elf_magic = section_data.len() >= 4 && 
                    section_data[0..4] == [0x7F, b'E', b'L', b'F'];
                
                if has_elf_magic && dest_offset == 0 {
                    // This section contains segment data that starts at file offset 0,
                    // which means it includes the ELF header and program headers area.
                    // 
                    // IMPORTANT: We overlay this onto elf_data rather than replacing it
                    // entirely. The previous approach (elf_data.clear() + extend_from_slice)
                    // would discard any buffer space needed for later sections that have
                    // higher destination offsets. By overlaying, subsequent sections can
                    // still write to their correct positions in the buffer.
                    debug!("Section {} contains ELF header at offset 0, overlaying onto buffer", i);
                    
                    // Ensure buffer is large enough for this section's data
                    if section_data.len() > elf_data.len() {
                        elf_data.resize(section_data.len(), 0);
                    }
                    // Overlay section data at offset 0 (existing data beyond this is preserved)
                    elf_data[0..section_data.len()].copy_from_slice(section_data);
                } else {
                    // Normal case - place at the specified offset
                    if elf_data.len() < dest_offset + section_data.len() {
                        elf_data.resize(dest_offset + section_data.len(), 0);
                    }
                    elf_data[dest_offset..dest_offset + section_data.len()].copy_from_slice(section_data);
                }
                
                debug!("Wrote section {} ({} bytes) to ELF offset 0x{:x}", i, section_data.len(), dest_offset);
                
                // Log what bytes are at the entry point if this section contains it
                let entry_in_segment = e_entry >= p_vaddr && e_entry < (p_vaddr + p_filesz);
                if entry_in_segment {
                    let entry_file_offset = dest_offset + (e_entry - p_vaddr) as usize;
                    if elf_data.len() >= entry_file_offset + 4 {
                        let entry_bytes = &elf_data[entry_file_offset..entry_file_offset + 4];
                        info!("Entry point 0x{:x} is at file offset 0x{:x}: {:02x?}", 
                            e_entry, entry_file_offset, entry_bytes);
                    }
                }
                
                sections_written += 1;
        }
        
        info!("Wrote {} sections to ELF", sections_written);
        
        // Validate the ELF data before returning
        if elf_data.len() < 64 {
            return Err(LoaderError::DecryptionFailed(format!(
                "Extracted ELF is too small ({} bytes). Section extraction may have failed.",
                elf_data.len()
            )));
        }
        
        // Verify ELF magic at start
        if elf_data[0..4] != [0x7F, b'E', b'L', b'F'] {
            return Err(LoaderError::DecryptionFailed(
                "Extracted data does not have valid ELF magic. Decryption may have failed.".to_string()
            ));
        }
        
        info!("ELF extracted successfully! Size: {} bytes", elf_data.len());
        Ok(elf_data)
    }
    
    /// Parse metadata header from decrypted data
    fn parse_metadata_header(data: &[u8]) -> Result<MetadataHeader, LoaderError> {
        if data.len() < 32 {
            return Err(LoaderError::InvalidSelf("Metadata header too small".to_string()));
        }
        
        Ok(MetadataHeader {
            signature_input_length: u64::from_be_bytes(data[0..8].try_into().unwrap()),
            unknown1: u32::from_be_bytes(data[8..12].try_into().unwrap()),
            section_count: u32::from_be_bytes(data[12..16].try_into().unwrap()),
            key_count: u32::from_be_bytes(data[16..20].try_into().unwrap()),
            optional_header_size: u32::from_be_bytes(data[20..24].try_into().unwrap()),
            unknown2: u64::from_be_bytes(data[24..32].try_into().unwrap()),
            unknown3: 0,
        })
    }
    
    /// Parse metadata section header from decrypted data
    fn parse_section_header(data: &[u8]) -> Result<MetadataSectionHeader, LoaderError> {
        if data.len() < 48 {
            return Err(LoaderError::InvalidSelf("Section header too small".to_string()));
        }
        
        Ok(MetadataSectionHeader {
            data_offset: u64::from_be_bytes(data[0..8].try_into().unwrap()),
            data_size: u64::from_be_bytes(data[8..16].try_into().unwrap()),
            section_type: u32::from_be_bytes(data[16..20].try_into().unwrap()),
            section_index: u32::from_be_bytes(data[20..24].try_into().unwrap()),
            hashed: u32::from_be_bytes(data[24..28].try_into().unwrap()),
            sha1_index: u32::from_be_bytes(data[28..32].try_into().unwrap()),
            encrypted: u32::from_be_bytes(data[32..36].try_into().unwrap()),
            key_index: u32::from_be_bytes(data[36..40].try_into().unwrap()),
            iv_index: u32::from_be_bytes(data[40..44].try_into().unwrap()),
            compressed: u32::from_be_bytes(data[44..48].try_into().unwrap()),
        })
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

    /// Extract ELF from a "fake encrypted" SELF - where the SELF structure exists
    /// but the section data is already decrypted. This happens with pre-decrypted ISOs.
    fn extract_from_fake_encrypted_self(
        &self,
        data: &[u8],
        _header: &SelfHeader,
        section_headers: &[MetadataSectionHeader],
        program_headers: &[(u32, u64, u64, u64, u64)],
        _e_entry: u64,
    ) -> Result<Vec<u8>, LoaderError> {
        info!("Extracting ELF from pre-decrypted SELF file (fake encrypted)");
        
        // Build the ELF by copying sections WITHOUT decryption
        let mut elf_data = Vec::new();
        let mut sections_written = 0;
        
        for (i, section_hdr) in section_headers.iter().enumerate() {
            // Only process type 2 (PHDR-backed sections)
            if section_hdr.section_type != 2 {
                continue;
            }
            
            let section_offset = section_hdr.data_offset as usize;
            let section_size = section_hdr.data_size as usize;
            
            if section_offset + section_size > data.len() {
                warn!("Section {} data range exceeds file size", i);
                continue;
            }
            
            let section_data = &data[section_offset..section_offset + section_size];
            
            // Decompress if needed
            let final_section = if section_hdr.compressed == 2 {
                let mut decoder = ZlibDecoder::new(&section_data[..]);
                let mut decompressed = Vec::new();
                if decoder.read_to_end(&mut decompressed).is_ok() {
                    decompressed
                } else {
                    section_data.to_vec()
                }
            } else {
                section_data.to_vec()
            };
            
            // Find destination offset from program header
            let section_idx = section_hdr.section_index as usize;
            if section_idx >= program_headers.len() {
                continue;
            }
            
            let (_p_type, p_offset, _p_vaddr, _p_filesz, _) = program_headers[section_idx];
            let dest_offset = p_offset as usize;
            
            info!("Section {} (pre-decrypted): {} bytes -> offset 0x{:x}", 
                i, final_section.len(), dest_offset);
            
            // Write to ELF buffer
            if final_section.len() >= 4 && final_section[0..4] == [0x7F, b'E', b'L', b'F'] && dest_offset == 0 {
                if final_section.len() > elf_data.len() {
                    elf_data.resize(final_section.len(), 0);
                }
                elf_data[0..final_section.len()].copy_from_slice(&final_section);
            } else {
                if elf_data.len() < dest_offset + final_section.len() {
                    elf_data.resize(dest_offset + final_section.len(), 0);
                }
                elf_data[dest_offset..dest_offset + final_section.len()].copy_from_slice(&final_section);
            }
            
            sections_written += 1;
        }
        
        info!("Extracted {} sections from pre-decrypted SELF", sections_written);
        
        // Verify we got valid ELF
        if elf_data.len() < 4 || elf_data[0..4] != [0x7F, b'E', b'L', b'F'] {
            return Err(LoaderError::DecryptionFailed(
                "Failed to extract valid ELF from pre-decrypted SELF".to_string()
            ));
        }
        
        Ok(elf_data)
    }
}

impl Default for SelfLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfLoader {
    /// Verify that the currently loaded decryption keys are valid.
    ///
    /// Runs [`CryptoEngine::verify_all_key_sets`] and logs a summary.
    /// Returns `Ok(valid_count)` if at least one key set validated,
    /// or `Err` if no key sets are loaded or none pass validation.
    pub fn verify_decryption_keys(&self) -> Result<usize, LoaderError> {
        let (valid, invalid, results) = self.crypto.verify_all_key_sets();

        info!(
            "Key verification: {} valid, {} invalid, {} total",
            valid, invalid, results.len()
        );

        for (kt, rev, ok) in &results {
            let status = if *ok { "OK" } else { "FAIL" };
            debug!("  key_type={}, revision=0x{:04x}: {}", kt, rev, status);
        }

        if results.is_empty() {
            return Err(LoaderError::DecryptionFailed(
                "No decryption key sets loaded".to_string(),
            ));
        }

        if valid == 0 {
            return Err(LoaderError::DecryptionFailed(format!(
                "All {} loaded key sets failed validation", invalid
            )));
        }

        Ok(valid)
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

    #[test]
    fn test_verify_decryption_keys_default() {
        // Default SelfLoader has built-in keys
        let loader = SelfLoader::new();
        let result = loader.verify_decryption_keys();
        // Built-in keys should validate successfully
        assert!(result.is_ok());
        let valid_count = result.unwrap();
        assert!(valid_count > 0);
    }
}
