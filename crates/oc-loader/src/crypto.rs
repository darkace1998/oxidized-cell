//! Cryptographic operations for SELF decryption
//!
//! Note: Actual decryption keys are not included for legal reasons.
//! This module provides the infrastructure for decryption when keys are available.

use oc_core::error::LoaderError;
use std::collections::HashMap;
use tracing::{debug, warn, info};
use aes::Aes128;
use cbc::{Decryptor, Encryptor};
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use sha1::{Sha1, Digest};
use serde::{Serialize, Deserialize};

/// Key types for PS3 encryption
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyType {
    /// Retail (production) keys
    Retail,
    /// Debug keys
    Debug,
    /// Application-specific keys
    App,
    /// Isolated SPU keys
    IsoSpu,
    /// LV1 (hypervisor) keys
    Lv1,
    /// LV2 (kernel) keys
    Lv2,
}

/// Encryption algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionType {
    /// AES-128 CBC
    Aes128Cbc,
    /// AES-256 CBC
    Aes256Cbc,
    /// No encryption
    None,
}

/// Key database entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEntry {
    pub key_type: KeyType,
    #[serde(with = "hex_serde")]
    pub key: Vec<u8>,
    #[serde(with = "optional_hex_serde")]
    pub iv: Option<Vec<u8>>,
    pub description: String,
}

// Helper modules for hex serialization
mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    
    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}

mod optional_hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    
    pub fn serialize<S>(bytes: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match bytes {
            Some(b) => serializer.serialize_some(&hex::encode(b)),
            None => serializer.serialize_none(),
        }
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        opt.map(|s| hex::decode(s).map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// AES key size constants
const AES_128_KEY_SIZE: usize = 16;
const AES_256_KEY_SIZE: usize = 32;
const AES_IV_SIZE: usize = 16;
const AES_BLOCK_SIZE: usize = 16;

/// Key file format for storing encryption keys
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyFileFormat {
    pub version: u32,
    pub keys: Vec<KeyEntry>,
}

/// Crypto engine for SELF decryption
pub struct CryptoEngine {
    keys: HashMap<KeyType, Vec<KeyEntry>>,
}

impl CryptoEngine {
    /// Create a new crypto engine
    pub fn new() -> Self {
        let mut engine = Self {
            keys: HashMap::new(),
        };

        // Initialize with placeholder keys
        // Real implementation would load actual keys from a secure key database
        engine.init_placeholder_keys();
        
        engine
    }

    /// Initialize placeholder keys for testing
    fn init_placeholder_keys(&mut self) {
        warn!("Using placeholder encryption keys - decryption will not work with real SELF files");

        // Add placeholder entries
        self.add_key(KeyEntry {
            key_type: KeyType::Debug,
            key: vec![0u8; AES_128_KEY_SIZE],
            iv: Some(vec![0u8; AES_IV_SIZE]),
            description: "Placeholder debug key".to_string(),
        });

        self.add_key(KeyEntry {
            key_type: KeyType::Retail,
            key: vec![0u8; AES_128_KEY_SIZE],
            iv: Some(vec![0u8; AES_IV_SIZE]),
            description: "Placeholder retail key".to_string(),
        });
    }

    /// Add a key to the database
    pub fn add_key(&mut self, entry: KeyEntry) {
        debug!("Adding key: {}", entry.description);
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

        if encrypted_data.len() % AES_BLOCK_SIZE != 0 {
            return Err(LoaderError::DecryptionFailed(
                "Encrypted data length must be multiple of 16".to_string(),
            ));
        }

        // Perform actual AES-128-CBC decryption
        if key.len() == AES_128_KEY_SIZE {
            type Aes128CbcDec = Decryptor<Aes128>;
            
            let cipher = Aes128CbcDec::new_from_slices(key, iv)
                .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create cipher: {:?}", e)))?;
            
            let mut buffer = encrypted_data.to_vec();
            let decrypted_data = cipher.decrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut buffer)
                .map_err(|e| LoaderError::DecryptionFailed(format!("Decryption failed: {:?}", e)))?;
            
            Ok(decrypted_data.to_vec())
        } else {
            // AES-256 would require Aes256 type
            return Err(LoaderError::DecryptionFailed(
                "AES-256 not yet implemented".to_string(),
            ));
        }
    }

    /// Encrypt data using AES
    pub fn encrypt_aes(
        &self,
        plaintext: &[u8],
        key: &[u8],
        iv: &[u8],
    ) -> Result<Vec<u8>, LoaderError> {
        debug!(
            "AES encryption: data_len={}, key_len={}, iv_len={}",
            plaintext.len(),
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

        // Perform actual AES-128-CBC encryption
        if key.len() == AES_128_KEY_SIZE {
            type Aes128CbcEnc = Encryptor<Aes128>;
            
            let cipher = Aes128CbcEnc::new_from_slices(key, iv)
                .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create cipher: {:?}", e)))?;
            
            let encrypted_data = cipher.encrypt_padded_vec_mut::<cbc::cipher::block_padding::Pkcs7>(plaintext);
            
            Ok(encrypted_data)
        } else {
            // AES-256 would require Aes256 type
            return Err(LoaderError::DecryptionFailed(
                "AES-256 not yet implemented".to_string(),
            ));
        }
    }

    /// Decrypt metadata using MetaLV2 keys
    pub fn decrypt_metadata_lv2(
        &self,
        encrypted_metadata: &[u8],
        key_type: KeyType,
    ) -> Result<Vec<u8>, LoaderError> {
        debug!("Decrypting MetaLV2 metadata with key type: {:?}", key_type);

        let key = self.get_key(key_type)
            .ok_or_else(|| LoaderError::DecryptionFailed("Key not found".to_string()))?;

        // MetaLV2 uses specific IV (typically all zeros)
        let iv = vec![0u8; AES_IV_SIZE];

        self.decrypt_aes(encrypted_metadata, key, &iv)
    }

    /// Verify SHA-1 hash
    pub fn verify_sha1(&self, data: &[u8], expected_hash: &[u8; 20]) -> bool {
        debug!("SHA-1 verification: data_len={}", data.len());
        
        let mut hasher = Sha1::new();
        hasher.update(data);
        let computed_hash = hasher.finalize();
        
        let result = computed_hash.as_slice() == expected_hash;
        
        if result {
            debug!("SHA-1 verification passed");
        } else {
            debug!("SHA-1 verification failed - hash mismatch");
        }
        
        result
    }
    
    /// Compute SHA-1 hash
    pub fn compute_sha1(&self, data: &[u8]) -> [u8; 20] {
        let mut hasher = Sha1::new();
        hasher.update(data);
        let hash = hasher.finalize();
        
        let mut result = [0u8; 20];
        result.copy_from_slice(&hash);
        result
    }

    /// Load keys from a file
    pub fn load_keys_from_file(&mut self, path: &str) -> Result<(), LoaderError> {
        use std::fs;
        use std::path::Path;
        
        info!("Loading encryption keys from: {}", path);
        
        let path_obj = Path::new(path);
        if !path_obj.exists() {
            return Err(LoaderError::DecryptionFailed(
                format!("Key file not found: {}", path)
            ));
        }
        
        let content = fs::read_to_string(path)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to read key file: {}", e)))?;
        
        // Try to parse as JSON
        let key_data: KeyFileFormat = serde_json::from_str(&content)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to parse key file: {}", e)))?;
        
        // Add all keys from the file
        for key_entry in key_data.keys {
            info!("Loaded key: {} ({:?})", key_entry.description, key_entry.key_type);
            self.add_key(key_entry);
        }
        
        info!("Successfully loaded {} key(s)", key_data.keys.len());
        Ok(())
    }
    
    /// Save keys to a file (for key management)
    pub fn save_keys_to_file(&self, path: &str) -> Result<(), LoaderError> {
        use std::fs;
        use std::path::Path;
        
        info!("Saving encryption keys to: {}", path);
        
        // Collect all keys
        let mut all_keys = Vec::new();
        for entries in self.keys.values() {
            all_keys.extend(entries.iter().cloned());
        }
        
        let key_data = KeyFileFormat {
            version: 1,
            keys: all_keys,
        };
        
        let content = serde_json::to_string_pretty(&key_data)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to serialize keys: {}", e)))?;
        
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create directory: {}", e)))?;
        }
        
        fs::write(path, content)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to write key file: {}", e)))?;
        
        info!("Successfully saved {} key(s)", key_data.keys.len());
        Ok(())
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
                KeyType::Lv1 => stats.lv1_keys = count,
                KeyType::Lv2 => stats.lv2_keys = count,
            }
        }

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
    pub lv1_keys: usize,
    pub lv2_keys: usize,
}

impl KeyStats {
    /// Get total number of keys across all types
    pub fn total(&self) -> usize {
        self.retail_keys + self.debug_keys + self.app_keys +
        self.iso_spu_keys + self.lv1_keys + self.lv2_keys
    }
}

impl Default for CryptoEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_engine_creation() {
        let engine = CryptoEngine::new();
        assert!(engine.has_key(KeyType::Debug));
        assert!(engine.has_key(KeyType::Retail));
    }

    #[test]
    fn test_key_addition() {
        let mut engine = CryptoEngine::new();
        
        let key_entry = KeyEntry {
            key_type: KeyType::App,
            key: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            iv: Some(vec![0u8; 16]),
            description: "Test key".to_string(),
        };

        engine.add_key(key_entry);
        assert!(engine.has_key(KeyType::App));
    }

    #[test]
    fn test_key_retrieval() {
        let engine = CryptoEngine::new();
        let key = engine.get_key(KeyType::Debug);
        assert!(key.is_some());
        assert_eq!(key.unwrap().len(), 16);
    }

    #[test]
    fn test_aes_validation() {
        let engine = CryptoEngine::new();
        
        // Test with invalid key length
        let result = engine.decrypt_aes(&[0u8; 16], &[0u8; 8], &[0u8; 16]);
        assert!(result.is_err());

        // Test with invalid IV length
        let result = engine.decrypt_aes(&[0u8; 16], &[0u8; 16], &[0u8; 8]);
        assert!(result.is_err());

        // Test with non-block-aligned data
        let result = engine.decrypt_aes(&[0u8; 15], &[0u8; 16], &[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_stats() {
        let engine = CryptoEngine::new();
        let stats = engine.get_stats();
        
        assert_eq!(stats.debug_keys, 1);
        assert_eq!(stats.retail_keys, 1);
    }

    #[test]
    fn test_key_types() {
        assert_ne!(KeyType::Retail, KeyType::Debug);
        assert_ne!(KeyType::App, KeyType::Lv1);
    }
}
