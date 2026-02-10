//! Shader cache with disk persistence
//!
//! Caches compiled SPIR-V shaders to disk for faster loading on subsequent runs.
//! Features:
//! - Hash-based shader lookup
//! - Disk persistence with header validation
//! - Driver version tracking for cache invalidation
//! - Hit/miss statistics

use super::types::*;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Shader cache magic number
const CACHE_MAGIC: u32 = 0x5348_4452; // "SHDR"
/// Cache format version - increment when format changes
const CACHE_VERSION: u32 = 2;

/// Driver version for cache invalidation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverVersion {
    /// Major version number
    pub major: u32,
    /// Minor version number
    pub minor: u32,
    /// Patch version number
    pub patch: u32,
    /// Driver-specific build identifier
    pub build_id: u32,
}

impl DriverVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            build_id: 0,
        }
    }

    pub fn with_build_id(mut self, build_id: u32) -> Self {
        self.build_id = build_id;
        self
    }

    /// Encode version to a single u64 for storage
    pub fn encode(&self) -> u64 {
        ((self.major as u64) << 48)
            | ((self.minor as u64) << 32)
            | ((self.patch as u64) << 16)
            | (self.build_id as u64)
    }

    /// Decode version from encoded u64
    pub fn decode(encoded: u64) -> Self {
        Self {
            major: ((encoded >> 48) & 0xFFFF) as u32,
            minor: ((encoded >> 32) & 0xFFFF) as u32,
            patch: ((encoded >> 16) & 0xFFFF) as u32,
            build_id: (encoded & 0xFFFF) as u32,
        }
    }
}

impl Default for DriverVersion {
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

/// Persistent shader cache
pub struct ShaderCache {
    cache_dir: PathBuf,
    vertex_cache: HashMap<u64, SpirVModule>,
    fragment_cache: HashMap<u64, SpirVModule>,
    /// Current driver version - used to validate cache entries
    driver_version: DriverVersion,
    /// Cache statistics
    stats: CacheStats,
}

impl ShaderCache {
    /// Create a new shader cache with the given directory
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        
        // Create cache directory if needed
        if !cache_dir.exists() {
            let _ = fs::create_dir_all(&cache_dir);
        }

        Self {
            cache_dir,
            vertex_cache: HashMap::new(),
            fragment_cache: HashMap::new(),
            driver_version: DriverVersion::default(),
            stats: CacheStats::default(),
        }
    }

    /// Create a new shader cache with a specific driver version
    pub fn with_driver_version<P: AsRef<Path>>(cache_dir: P, version: DriverVersion) -> Self {
        let mut cache = Self::new(cache_dir);
        cache.driver_version = version;
        cache
    }

    /// Set the driver version and invalidate incompatible cache entries
    pub fn set_driver_version(&mut self, version: DriverVersion) {
        if self.driver_version != version {
            // Clear memory caches when driver version changes
            self.vertex_cache.clear();
            self.fragment_cache.clear();
            self.driver_version = version;
            self.stats.invalidations += 1;
        }
    }

    /// Get the current driver version
    pub fn driver_version(&self) -> &DriverVersion {
        &self.driver_version
    }

    /// Get cached vertex shader
    pub fn get_vertex(&mut self, hash: u64) -> Option<SpirVModule> {
        // Check memory cache first
        if let Some(module) = self.vertex_cache.get(&hash) {
            self.stats.memory_hits += 1;
            return Some(module.clone());
        }

        // Try loading from disk
        let path = self.cache_dir.join(format!("vp_{:016x}.spv", hash));
        if let Ok((module, cached_version)) = self.load_from_disk_with_version(&path, ShaderStage::VERTEX) {
            // Check driver version compatibility
            if cached_version == self.driver_version.encode() {
                self.vertex_cache.insert(hash, module.clone());
                self.stats.disk_hits += 1;
                return Some(module);
            } else {
                // Driver version mismatch - invalidate entry
                let _ = fs::remove_file(&path);
                self.stats.invalidations += 1;
            }
        }

        self.stats.misses += 1;
        None
    }

    /// Get cached fragment shader
    pub fn get_fragment(&mut self, hash: u64) -> Option<SpirVModule> {
        // Check memory cache first
        if let Some(module) = self.fragment_cache.get(&hash) {
            self.stats.memory_hits += 1;
            return Some(module.clone());
        }

        // Try loading from disk
        let path = self.cache_dir.join(format!("fp_{:016x}.spv", hash));
        if let Ok((module, cached_version)) = self.load_from_disk_with_version(&path, ShaderStage::FRAGMENT) {
            // Check driver version compatibility
            if cached_version == self.driver_version.encode() {
                self.fragment_cache.insert(hash, module.clone());
                self.stats.disk_hits += 1;
                return Some(module);
            } else {
                // Driver version mismatch - invalidate entry
                let _ = fs::remove_file(&path);
                self.stats.invalidations += 1;
            }
        }

        self.stats.misses += 1;
        None
    }

    /// Store vertex shader in cache
    pub fn store_vertex(&mut self, hash: u64, module: &SpirVModule) {
        self.vertex_cache.insert(hash, module.clone());
        self.stats.stores += 1;
        
        let path = self.cache_dir.join(format!("vp_{:016x}.spv", hash));
        let _ = self.save_to_disk_with_version(&path, module);
    }

    /// Store fragment shader in cache
    pub fn store_fragment(&mut self, hash: u64, module: &SpirVModule) {
        self.fragment_cache.insert(hash, module.clone());
        self.stats.stores += 1;
        
        let path = self.cache_dir.join(format!("fp_{:016x}.spv", hash));
        let _ = self.save_to_disk_with_version(&path, module);
    }

    /// Load shader from disk with version checking
    fn load_from_disk_with_version(&self, path: &Path, stage: ShaderStage) -> Result<(SpirVModule, u64), String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| e.to_string())?;

        // Verify header (magic + version + driver_version + size = 20 bytes)
        if data.len() < 20 {
            return Err("Cache file too small".to_string());
        }

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let driver_version_lo = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let driver_version_hi = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let size = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

        if magic != CACHE_MAGIC {
            return Err("Invalid cache magic".to_string());
        }
        if version != CACHE_VERSION {
            return Err("Cache version mismatch".to_string());
        }

        let driver_version_encoded = ((driver_version_hi as u64) << 32) | (driver_version_lo as u64);

        let bytecode_bytes = &data[20..];
        if bytecode_bytes.len() != size as usize {
            return Err("Cache size mismatch".to_string());
        }

        // Convert bytes to u32s
        let bytecode: Vec<u32> = bytecode_bytes
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        Ok((SpirVModule { bytecode, stage }, driver_version_encoded))
    }

    /// Save shader to disk with driver version
    fn save_to_disk_with_version(&self, path: &Path, module: &SpirVModule) -> Result<(), String> {
        let mut file = File::create(path).map_err(|e| e.to_string())?;

        // Write header
        file.write_all(&CACHE_MAGIC.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&CACHE_VERSION.to_le_bytes()).map_err(|e| e.to_string())?;
        
        // Write driver version (as u64 split into two u32s)
        let driver_encoded = self.driver_version.encode();
        file.write_all(&(driver_encoded as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&((driver_encoded >> 32) as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        
        let size = (module.bytecode.len() * 4) as u32;
        file.write_all(&size.to_le_bytes()).map_err(|e| e.to_string())?;

        // Write bytecode
        for word in &module.bytecode {
            file.write_all(&word.to_le_bytes()).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Clear all caches (memory and disk)
    pub fn clear(&mut self) {
        self.vertex_cache.clear();
        self.fragment_cache.clear();
        self.stats = CacheStats::default();

        // Remove disk cache files
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "spv").unwrap_or(false) {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            memory_hits: self.stats.memory_hits,
            disk_hits: self.stats.disk_hits,
            misses: self.stats.misses,
            stores: self.stats.stores,
            invalidations: self.stats.invalidations,
            vertex_count: self.vertex_cache.len(),
            fragment_count: self.fragment_cache.len(),
        }
    }

    /// Reset cache statistics
    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }

    /// Invalidate all cached shaders for a specific hash pattern
    pub fn invalidate_by_prefix(&mut self, prefix: &str) {
        // Clear memory caches
        self.vertex_cache.clear();
        self.fragment_cache.clear();

        // Remove disk cache files matching prefix
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with(prefix) && path.extension().map(|e| e == "spv").unwrap_or(false) {
                        let _ = fs::remove_file(path);
                        self.stats.invalidations += 1;
                    }
                }
            }
        }
    }

    /// Get hit rate as a percentage (0-100)
    pub fn hit_rate(&self) -> f64 {
        let total = self.stats.memory_hits + self.stats.disk_hits + self.stats.misses;
        if total == 0 {
            0.0
        } else {
            ((self.stats.memory_hits + self.stats.disk_hits) as f64 / total as f64) * 100.0
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of shaders found in memory cache
    pub memory_hits: u64,
    /// Number of shaders loaded from disk cache
    pub disk_hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of shaders stored
    pub stores: u64,
    /// Number of cache entries invalidated
    pub invalidations: u64,
    /// Number of vertex shaders in memory
    pub vertex_count: usize,
    /// Number of fragment shaders in memory
    pub fragment_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_cache_roundtrip() {
        let temp_dir = env::temp_dir().join("oc_shader_cache_test");
        let _ = fs::remove_dir_all(&temp_dir);
        
        let mut cache = ShaderCache::new(&temp_dir);
        
        let module = SpirVModule {
            bytecode: vec![0x07230203, 0x00010000, 0x00080001, 10, 0],
            stage: ShaderStage::VERTEX,
        };

        cache.store_vertex(0x1234, &module);
        
        // Clear memory cache to force disk load
        cache.vertex_cache.clear();
        
        let loaded = cache.get_vertex(0x1234).expect("Should load from disk");
        assert_eq!(loaded.bytecode, module.bytecode);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_driver_version_encoding() {
        let version = DriverVersion::new(1, 2, 3).with_build_id(4);
        let encoded = version.encode();
        let decoded = DriverVersion::decode(encoded);
        
        assert_eq!(decoded.major, 1);
        assert_eq!(decoded.minor, 2);
        assert_eq!(decoded.patch, 3);
        assert_eq!(decoded.build_id, 4);
    }

    #[test]
    fn test_cache_invalidation_on_driver_change() {
        let temp_dir = env::temp_dir().join("oc_shader_cache_test_driver");
        let _ = fs::remove_dir_all(&temp_dir);
        
        // Create cache with driver v1.0.0
        let mut cache = ShaderCache::with_driver_version(&temp_dir, DriverVersion::new(1, 0, 0));
        
        let module = SpirVModule {
            bytecode: vec![0x07230203, 0x00010000, 0x00080001, 10, 0],
            stage: ShaderStage::VERTEX,
        };

        cache.store_vertex(0x5678, &module);
        cache.vertex_cache.clear();
        
        // Should load successfully with same driver version
        assert!(cache.get_vertex(0x5678).is_some());
        
        // Change driver version
        cache.set_driver_version(DriverVersion::new(2, 0, 0));
        cache.vertex_cache.clear();
        
        // Should fail to load due to version mismatch
        assert!(cache.get_vertex(0x5678).is_none());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cache_stats() {
        let temp_dir = env::temp_dir().join("oc_shader_cache_test_stats");
        let _ = fs::remove_dir_all(&temp_dir);
        
        let mut cache = ShaderCache::new(&temp_dir);
        
        let module = SpirVModule {
            bytecode: vec![0x07230203, 0x00010000],
            stage: ShaderStage::FRAGMENT,
        };

        // Store should increment stores count
        cache.store_fragment(0xABCD, &module);
        assert_eq!(cache.stats().stores, 1);
        
        // Memory hit
        let _ = cache.get_fragment(0xABCD);
        assert_eq!(cache.stats().memory_hits, 1);
        
        // Cache miss
        let _ = cache.get_fragment(0xDEAD);
        assert_eq!(cache.stats().misses, 1);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
