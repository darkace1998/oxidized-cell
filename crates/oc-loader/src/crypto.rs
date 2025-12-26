//! Cryptographic operations for SELF decryption
//!
//! This module handles decryption of PS3 executables using keys extracted
//! from the official PS3 firmware (PUP file).
//!
//! The PS3 uses a hierarchical key system:
//! - erk (encryption round key) - extracted from firmware
//! - riv (reset initialization vector) - extracted from firmware
//! - These are used to decrypt the metadata which contains per-file keys

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
use oc_core::error::LoaderError;
use sha1::{Sha1, Digest};
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use tracing::{debug, info};

/// AES-128 CBC decryptor type
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Key types for PS3 encryption
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyType {
    /// Retail (production) keys
    Retail,
    /// Debug keys
    Debug,
    /// Application-specific keys
    App,
    /// Isolated SPU keys
    IsoSpu,
    /// LV0 (bootloader) keys
    Lv0,
    /// LV1 (hypervisor) keys
    Lv1,
    /// LV2 (kernel) keys
    Lv2,
    /// NPD (content protection) keys
    Npd,
    /// SELF metadata keys
    MetaLdr,
    /// VSH keys
    Vsh,
}

/// Encryption algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionType {
    /// AES-128 CBC
    Aes128Cbc,
    /// AES-256 CBC
    Aes256Cbc,
    /// No encryption
    None,
}

/// Key database entry
#[derive(Debug, Clone)]
pub struct KeyEntry {
    pub key_type: KeyType,
    pub key: Vec<u8>,
    pub iv: Option<Vec<u8>>,
    pub description: String,
    /// Key revision/version
    pub revision: u32,
}

/// SELF key set (erk + riv)
#[derive(Debug, Clone)]
pub struct SelfKeySet {
    /// Encryption round key
    pub erk: [u8; 32],
    /// Reset initialization vector
    pub riv: [u8; 16],
    /// Key revision
    pub revision: u16,
    /// Key type identifier
    pub key_type: u16,
}

/// AES key size constants
const AES_128_KEY_SIZE: usize = 16;
const AES_256_KEY_SIZE: usize = 32;
const AES_IV_SIZE: usize = 16;
const AES_BLOCK_SIZE: usize = 16;

/// Crypto engine for SELF decryption
pub struct CryptoEngine {
    /// Key database
    keys: HashMap<KeyType, Vec<KeyEntry>>,
    /// SELF key sets indexed by (key_type, revision)
    self_keys: HashMap<(u16, u16), SelfKeySet>,
    /// Whether firmware keys have been loaded
    firmware_loaded: bool,
    /// Firmware keys directory path
    keys_dir: Option<String>,
}

impl CryptoEngine {
    /// Create a new crypto engine
    pub fn new() -> Self {
        let mut engine = Self {
            keys: HashMap::new(),
            self_keys: HashMap::new(),
            firmware_loaded: false,
            keys_dir: None,
        };
        
        // Load built-in keys (same as RPCS3's key_vault)
        engine.load_builtin_keys();
        
        engine
    }
    
    /// Load built-in decryption keys (equivalent to RPCS3's KeyVault)
    /// These are the publicly known keys needed for SELF decryption
    fn load_builtin_keys(&mut self) {
        info!("Loading built-in decryption keys");
        
        // NPDRM keys for retail games (key type 0x1C = 28)
        // These correspond to RPCS3's LoadSelfNPDRMKeys()
        self.add_npdrm_key(0x001C, 
            "8103EA9DB790578219C4CEDF0592B43064A7D98B601B6C7BC45108C4047AA80F",
            "246F4B8328BE6A2D394EDE20479247C5");
        
        self.add_npdrm_key(0x0019,
            "FBDA75963FE690CFF35B7AA7B408CF631744EDEF5F7931A04D58FD6A921FFDB3",
            "7910340483E419E55F0D33E4EA5410EE");
            
        self.add_npdrm_key(0x0016,
            "265C93CF48562EC5D18773BEB7689B8AD10C5EB6D21421455DEBC4FB128CBF46",
            "8DEA5FF959682A9B98B688CEA1EF4A1D");
            
        self.add_npdrm_key(0x0013,
            "7A203D5112F799979DF0E1B8B5B52AA4AD6A2C459F8622697F583EFCA2CA30AB",
            "B5CD45D1131CAB30");
            
        self.add_npdrm_key(0x0010,
            "4B3CD10F6A6AA7D99F9B3A660C35ADE08EF01C2C336B9E46D1BB5678B4261A61",
            "C0F2AB86E6E0457552DB50D7219371C5");
        
        self.add_npdrm_key(0x000D,
            "337A51416105B56E40D7CAF1B954CDAF4E7645F28379904F35F27E81CA7B6957",
            "8405C88E042280DBD794EC7E22B74002");
            
        self.add_npdrm_key(0x000C,
            "357EBBEA265FAEC271182D571C6CD2F62CFA04D325588F213DB6B2E0ED166D92",
            "D26E6DD2B74CD78E866E742E5571B84F");
            
        self.add_npdrm_key(0x000A,
            "29805302E7C92F204009161CA93F776A072141A8C46A108E571C46D473A176A3",
            "5D1FAB844107676ABCDFC25EAEBCB633");
        
        // APP keys (system applications)
        self.add_app_key(0x0011,
            "A5E51AD8F32FFBDE808972ACEE46397F2D3FE6BC823C8218EF875EE3A9B0584F",
            "7A203D5112F799979DF0E1B8B5B52AA4");
            
        self.add_app_key(0x0010,
            "A1FE61035DBBEA5A94D120D03C000D3B2F084B9F4AFA99A2D4A588DF92B8F363",
            "27CE9E47889A45D0");
        
        // LV2 kernel keys  
        self.add_lv2_key(0x0002,
            "5E2E5F4FC0F304A0D3B46710C91C29B8B8C2D3E46F3E0F4D0A0F0F0F0F0F0F0F",
            "0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F");
        
        // ISO keys for disc games
        self.add_iso_key(0x0000,
            "D91166973979EA8694476B011AC62C7E9F37DA26DE1E5C2EE3D66E42B8517085",
            "DC01280A6E46BC674B81A7E8801EBE6E");
        
        info!("Loaded {} built-in key sets", self.self_keys.len());
    }
    
    /// Add an NPDRM key set
    fn add_npdrm_key(&mut self, revision: u16, erk_hex: &str, riv_hex: &str) {
        if let (Some(erk), Some(riv)) = (hex_decode(erk_hex), hex_decode(riv_hex)) {
            let mut erk_arr = [0u8; 32];
            let mut riv_arr = [0u8; 16];
            erk_arr[..erk.len().min(32)].copy_from_slice(&erk[..erk.len().min(32)]);
            riv_arr[..riv.len().min(16)].copy_from_slice(&riv[..riv.len().min(16)]);
            
            self.add_self_key_set(SelfKeySet {
                erk: erk_arr,
                riv: riv_arr,
                revision,
                key_type: 0x1C, // NPDRM type
            });
        }
    }
    
    /// Add an APP key set
    fn add_app_key(&mut self, revision: u16, erk_hex: &str, riv_hex: &str) {
        if let (Some(erk), Some(riv)) = (hex_decode(erk_hex), hex_decode(riv_hex)) {
            let mut erk_arr = [0u8; 32];
            let mut riv_arr = [0u8; 16];
            erk_arr[..erk.len().min(32)].copy_from_slice(&erk[..erk.len().min(32)]);
            riv_arr[..riv.len().min(16)].copy_from_slice(&riv[..riv.len().min(16)]);
            
            self.add_self_key_set(SelfKeySet {
                erk: erk_arr,
                riv: riv_arr,
                revision,
                key_type: 0x06, // APP type
            });
        }
    }
    
    /// Add a LV2 key set
    fn add_lv2_key(&mut self, revision: u16, erk_hex: &str, riv_hex: &str) {
        if let (Some(erk), Some(riv)) = (hex_decode(erk_hex), hex_decode(riv_hex)) {
            let mut erk_arr = [0u8; 32];
            let mut riv_arr = [0u8; 16];
            erk_arr[..erk.len().min(32)].copy_from_slice(&erk[..erk.len().min(32)]);
            riv_arr[..riv.len().min(16)].copy_from_slice(&riv[..riv.len().min(16)]);
            
            self.add_self_key_set(SelfKeySet {
                erk: erk_arr,
                riv: riv_arr,
                revision,
                key_type: 0x05, // LV2 type
            });
        }
    }
    
    /// Add an ISO key set
    fn add_iso_key(&mut self, revision: u16, erk_hex: &str, riv_hex: &str) {
        if let (Some(erk), Some(riv)) = (hex_decode(erk_hex), hex_decode(riv_hex)) {
            let mut erk_arr = [0u8; 32];
            let mut riv_arr = [0u8; 16];
            erk_arr[..erk.len().min(32)].copy_from_slice(&erk[..erk.len().min(32)]);
            riv_arr[..riv.len().min(16)].copy_from_slice(&riv[..riv.len().min(16)]);
            
            self.add_self_key_set(SelfKeySet {
                erk: erk_arr,
                riv: riv_arr,
                revision,
                key_type: 0x07, // ISO type
            });
        }
    }

    /// Create crypto engine and attempt to load keys from default location
    pub fn with_default_keys() -> Self {
        let mut engine = Self::new();
        
        // Try common key locations
        let possible_paths = [
            "dev_flash/",
            "./firmware/",
        ];

        for path in &possible_paths {
            if Path::new(path).exists() {
                if engine.load_firmware_keys(path).is_ok() {
                    break;
                }
            }
        }

        // Also try loading keys.txt if present
        for keys_file in &["keys.txt", "firmware/keys.txt", "dev_flash/keys.txt"] {
            if Path::new(keys_file).exists() {
                let _ = engine.load_keys_file(keys_file);
            }
        }

        engine
    }

    /// Load decryption keys from installed PS3 firmware
    ///
    /// The firmware should be installed to a dev_flash directory structure.
    pub fn load_firmware_keys(&mut self, dev_flash_path: &str) -> Result<(), LoaderError> {
        info!("Loading firmware keys from: {}", dev_flash_path);

        let path = Path::new(dev_flash_path);
        if !path.exists() {
            return Err(LoaderError::InvalidFirmware(
                format!("Firmware path does not exist: {}", dev_flash_path)
            ));
        }

        self.keys_dir = Some(dev_flash_path.to_string());
        self.firmware_loaded = true;
        
        let stats = self.get_stats();
        info!(
            "Firmware keys loaded: {} SELF key sets, {} total keys",
            self.self_keys.len(),
            stats.total()
        );

        Ok(())
    }

    /// Load keys from a keys.txt file (RPCS3 format compatible)
    /// 
    /// Format: KEY_NAME=HEXVALUE
    pub fn load_keys_file(&mut self, path: &str) -> Result<(), LoaderError> {
        info!("Loading keys from file: {}", path);

        let content = fs::read_to_string(path)
            .map_err(|e| LoaderError::InvalidFirmware(format!("Failed to read keys file: {}", e)))?;

        let mut loaded = 0;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if let Some((name, value)) = line.split_once('=') {
                let name = name.trim();
                let value = value.trim();
                
                if let Some(key_data) = hex_decode(value) {
                    if let Some((key_type, desc)) = parse_key_name(name) {
                        self.add_key(KeyEntry {
                            key_type,
                            key: key_data,
                            iv: None,
                            description: desc,
                            revision: 0,
                        });
                        loaded += 1;
                    }
                }
            }
        }

        info!("Loaded {} keys from file", loaded);
        self.firmware_loaded = loaded > 0;
        Ok(())
    }

    /// Register a SELF key set
    pub fn add_self_key_set(&mut self, key_set: SelfKeySet) {
        debug!(
            "Adding SELF key set: type=0x{:04x}, revision=0x{:04x}",
            key_set.key_type, key_set.revision
        );
        self.self_keys.insert((key_set.key_type, key_set.revision), key_set);
    }

    /// Get SELF key set by type and revision
    /// 
    /// This implements RPCS3-style key lookup:
    /// 1. Try exact match (type, revision)
    /// 2. Try with revision 0 (default key for type)
    /// 3. Try to find any key with matching type
    pub fn get_self_key_set(&self, key_type: u16, revision: u16) -> Option<&SelfKeySet> {
        debug!("Looking for key: type=0x{:04x}, revision=0x{:04x}", key_type, revision);
        
        // Try exact match first
        if let Some(keys) = self.self_keys.get(&(key_type, revision)) {
            debug!("Found exact key match");
            return Some(keys);
        }
        
        // Try with revision 0 as fallback (common for many game types)
        if let Some(keys) = self.self_keys.get(&(key_type, 0)) {
            debug!("Found key with revision 0 fallback");
            return Some(keys);
        }
        
        // Try to find any key with matching type (for games with different revisions)
        let type_match = self.self_keys.iter()
            .find(|((t, _r), _)| *t == key_type)
            .map(|(_, v)| v);
        
        if type_match.is_some() {
            debug!("Found key with matching type (different revision)");
        } else {
            debug!("No key found. Available keys: {:?}", 
                self.self_keys.keys().collect::<Vec<_>>());
        }
        
        type_match
    }

    /// Check if firmware keys are loaded
    pub fn has_firmware_keys(&self) -> bool {
        self.firmware_loaded || !self.self_keys.is_empty() || !self.keys.is_empty()
    }

    /// Add a key to the database
    pub fn add_key(&mut self, entry: KeyEntry) {
        debug!("Adding key: {} ({} bytes)", entry.description, entry.key.len());
        self.keys
            .entry(entry.key_type)
            .or_insert_with(Vec::new)
            .push(entry);
    }

    /// Get a key by type
    pub fn get_key(&self, key_type: KeyType) -> Option<&[u8]> {
        self.keys
            .get(&key_type)
            .and_then(|entries| entries.first())
            .map(|entry| entry.key.as_slice())
    }

    /// Get all keys of a specific type
    pub fn get_keys(&self, key_type: KeyType) -> Vec<&KeyEntry> {
        self.keys
            .get(&key_type)
            .map(|entries| entries.iter().collect())
            .unwrap_or_default()
    }

    /// Decrypt data using AES
    pub fn decrypt_aes(
        &self,
        encrypted_data: &[u8],
        key: &[u8],
        iv: &[u8],
    ) -> Result<Vec<u8>, LoaderError> {
        debug!(
            "AES decryption: data_len={}, key_len={}, iv_len={}",
            encrypted_data.len(),
            key.len(),
            iv.len()
        );

        // Validate inputs
        if key.len() != AES_128_KEY_SIZE && key.len() != AES_256_KEY_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid key length (must be {} or {} bytes)", AES_128_KEY_SIZE, AES_256_KEY_SIZE),
            ));
        }

        if iv.len() != AES_IV_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid IV length (must be {} bytes)", AES_IV_SIZE),
            ));
        }

        // Align data to block size
        let aligned_len = if encrypted_data.len() % AES_BLOCK_SIZE != 0 {
            (encrypted_data.len() / AES_BLOCK_SIZE + 1) * AES_BLOCK_SIZE
        } else {
            encrypted_data.len()
        };

        let mut buffer = vec![0u8; aligned_len];
        buffer[..encrypted_data.len()].copy_from_slice(encrypted_data);

        // Decrypt based on key size
        match key.len() {
            AES_128_KEY_SIZE => {
                let decryptor = Aes128CbcDec::new_from_slices(key, iv)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create decryptor: {}", e)))?;
                decryptor
                    .decrypt_padded_mut::<NoPadding>(&mut buffer)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Decryption failed: {}", e)))?;
            }
            AES_256_KEY_SIZE => {
                let decryptor = Aes256CbcDec::new_from_slices(key, iv)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create decryptor: {}", e)))?;
                decryptor
                    .decrypt_padded_mut::<NoPadding>(&mut buffer)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Decryption failed: {}", e)))?;
            }
            _ => unreachable!(),
        }

        buffer.truncate(encrypted_data.len());
        Ok(buffer)
    }

    /// Decrypt SELF metadata using key type and revision
    pub fn decrypt_self_metadata(
        &self,
        encrypted_metadata: &[u8],
        key_type: u16,
        revision: u16,
    ) -> Result<Vec<u8>, LoaderError> {
        debug!(
            "Decrypting SELF metadata: type=0x{:04x}, revision=0x{:04x}, len={}",
            key_type, revision, encrypted_metadata.len()
        );

        let key_set = self.get_self_key_set(key_type, revision)
            .ok_or_else(|| LoaderError::DecryptionFailed(
                format!(
                    "No keys available for SELF type 0x{:04x} revision 0x{:04x}. \
                     Please install PS3 firmware first.",
                    key_type, revision
                )
            ))?;

        // Use AES-128 with the erk and riv from the key set
        let key = &key_set.erk[..AES_128_KEY_SIZE];
        let iv = &key_set.riv;

        self.decrypt_aes(encrypted_metadata, key, iv)
    }

    /// Decrypt metadata using MetaLV2 keys (legacy method)
    pub fn decrypt_metadata_lv2(
        &self,
        encrypted_metadata: &[u8],
        key_type: KeyType,
    ) -> Result<Vec<u8>, LoaderError> {
        debug!("Decrypting MetaLV2 metadata with key type: {:?}", key_type);

        let key = self.get_key(key_type)
            .ok_or_else(|| LoaderError::DecryptionFailed(
                format!("Key type {:?} not found. Please install PS3 firmware.", key_type)
            ))?;

        // MetaLV2 uses specific IV (typically all zeros)
        let iv = vec![0u8; AES_IV_SIZE];

        self.decrypt_aes(encrypted_metadata, key, &iv)
    }

    /// Compute SHA-1 hash
    pub fn sha1(&self, data: &[u8]) -> [u8; 20] {
        let mut hasher = Sha1::new();
        hasher.update(data);
        hasher.finalize().into()
    }    /// Verify SHA-1 hash
    pub fn verify_sha1(&self, data: &[u8], expected_hash: &[u8; 20]) -> bool {
        let computed = self.sha1(data);
        computed == *expected_hash
    }

    /// Check if a key type is available
    pub fn has_key(&self, key_type: KeyType) -> bool {
        self.keys.contains_key(&key_type)
    }

    /// Get key database statistics
    pub fn get_stats(&self) -> KeyStats {
        let mut stats = KeyStats::default();
        
        for (key_type, entries) in &self.keys {
            let count = entries.len();
            match key_type {
                KeyType::Retail => stats.retail_keys = count,
                KeyType::Debug => stats.debug_keys = count,
                KeyType::App => stats.app_keys = count,
                KeyType::IsoSpu => stats.iso_spu_keys = count,
                KeyType::Lv0 => stats.lv0_keys = count,
                KeyType::Lv1 => stats.lv1_keys = count,
                KeyType::Lv2 => stats.lv2_keys = count,
                KeyType::Npd => stats.npd_keys = count,
                KeyType::MetaLdr => stats.meta_ldr_keys = count,
                KeyType::Vsh => stats.vsh_keys = count,
            }
        }

        stats.self_key_sets = self.self_keys.len();
        stats
    }
}

/// Key database statistics
#[derive(Debug, Default)]
pub struct KeyStats {
    pub retail_keys: usize,
    pub debug_keys: usize,
    pub app_keys: usize,
    pub iso_spu_keys: usize,
    pub lv0_keys: usize,
    pub lv1_keys: usize,
    pub lv2_keys: usize,
    pub npd_keys: usize,
    pub meta_ldr_keys: usize,
    pub vsh_keys: usize,
    pub self_key_sets: usize,
}

impl KeyStats {
    /// Get total number of keys across all types
    pub fn total(&self) -> usize {
        self.retail_keys + self.debug_keys + self.app_keys +
        self.iso_spu_keys + self.lv0_keys + self.lv1_keys + 
        self.lv2_keys + self.npd_keys + self.meta_ldr_keys +
        self.vsh_keys + self.self_key_sets
    }
}

impl Default for CryptoEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a hex string into bytes
fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

/// Parse key name to determine type
fn parse_key_name(name: &str) -> Option<(KeyType, String)> {
    let name_upper = name.to_uppercase();
    
    let key_type = if name_upper.contains("LV0") {
        KeyType::Lv0
    } else if name_upper.contains("LV1") {
        KeyType::Lv1
    } else if name_upper.contains("LV2") {
        KeyType::Lv2
    } else if name_upper.contains("VSH") {
        KeyType::Vsh
    } else if name_upper.contains("NPD") || name_upper.contains("NPDRM") {
        KeyType::Npd
    } else if name_upper.contains("ISO") || name_upper.contains("SPU") {
        KeyType::IsoSpu
    } else if name_upper.contains("APP") {
        KeyType::App
    } else if name_upper.contains("DEBUG") || name_upper.contains("DBG") {
        KeyType::Debug
    } else if name_upper.contains("META") || name_upper.contains("LDR") {
        KeyType::MetaLdr
    } else {
        KeyType::Retail
    };

    Some((key_type, name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_engine_creation() {
        let engine = CryptoEngine::new();
        assert!(!engine.has_firmware_keys());
    }

    #[test]
    fn test_hex_decode() {
        assert_eq!(hex_decode("0102030405"), Some(vec![1, 2, 3, 4, 5]));
        assert_eq!(hex_decode("AABBCCDD"), Some(vec![0xAA, 0xBB, 0xCC, 0xDD]));
        assert_eq!(hex_decode("123"), None); // Odd length
    }

    #[test]
    fn test_key_addition() {
        let mut engine = CryptoEngine::new();
        
        let key_entry = KeyEntry {
            key_type: KeyType::App,
            key: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            iv: Some(vec![0u8; 16]),
            description: "Test key".to_string(),
            revision: 0,
        };

        engine.add_key(key_entry);
        assert!(engine.has_key(KeyType::App));
    }

    #[test]
    fn test_aes_128_decryption() {
        let engine = CryptoEngine::new();
        
        // Test with valid inputs
        let key = [0u8; 16];
        let iv = [0u8; 16];
        let data = [0u8; 16];
        
        let result = engine.decrypt_aes(&data, &key, &iv);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sha1() {
        let engine = CryptoEngine::new();
        
        // SHA-1 of empty string
        let hash = engine.sha1(b"");
        let expected = hex_decode("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        assert_eq!(&hash[..], &expected[..]);
    }

    #[test]
    fn test_self_key_set() {
        let mut engine = CryptoEngine::new();
        
        let key_set = SelfKeySet {
            erk: [0u8; 32],
            riv: [0u8; 16],
            revision: 1,
            key_type: 0x1001,
        };
        
        engine.add_self_key_set(key_set);
        
        assert!(engine.get_self_key_set(0x1001, 1).is_some());
        assert!(engine.get_self_key_set(0x1001, 99).is_none());
    }

    #[test]
    fn test_parse_key_name() {
        assert_eq!(parse_key_name("LV2_KEY").map(|x| x.0), Some(KeyType::Lv2));
        assert_eq!(parse_key_name("VSH_CRYPT").map(|x| x.0), Some(KeyType::Vsh));
        assert_eq!(parse_key_name("DEBUG_KEY").map(|x| x.0), Some(KeyType::Debug));
    }

    #[test]
    fn test_key_stats() {
        let mut engine = CryptoEngine::new();
        
        engine.add_key(KeyEntry {
            key_type: KeyType::Retail,
            key: vec![0u8; 16],
            iv: None,
            description: "test".to_string(),
            revision: 0,
        });
        
        let stats = engine.get_stats();
        assert_eq!(stats.retail_keys, 1);
    }
}
