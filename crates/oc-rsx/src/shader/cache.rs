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

/// LRU shader cache with configurable maximum entry count.
/// When the cache exceeds `max_entries`, the least-recently-used entry is evicted.
pub struct LruShaderCache {
    /// Maximum number of entries before eviction
    max_entries: usize,
    /// Entries stored as (hash, bytecode, last_access_order)
    entries: Vec<(u64, Vec<u32>, u64)>,
    /// Monotonic access counter
    access_counter: u64,
    /// Cache statistics
    pub stats: LruCacheStats,
}

/// Statistics for the LRU shader cache.
#[derive(Debug, Default, Clone)]
pub struct LruCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub insertions: u64,
}

impl LruShaderCache {
    /// Create a new LRU cache with the given maximum entry count.
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            entries: Vec::new(),
            access_counter: 0,
            stats: LruCacheStats::default(),
        }
    }

    /// Look up a shader by hash. Returns bytecode if found and updates access order.
    pub fn get(&mut self, hash: u64) -> Option<&[u32]> {
        self.access_counter += 1;
        if let Some(entry) = self.entries.iter_mut().find(|(h, _, _)| *h == hash) {
            entry.2 = self.access_counter;
            self.stats.hits += 1;
            Some(&entry.1)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Insert a shader into the cache. Evicts the LRU entry if at capacity.
    pub fn insert(&mut self, hash: u64, bytecode: Vec<u32>) {
        // Check if already present
        if let Some(entry) = self.entries.iter_mut().find(|(h, _, _)| *h == hash) {
            self.access_counter += 1;
            entry.1 = bytecode;
            entry.2 = self.access_counter;
            return;
        }
        
        // Evict LRU if at capacity
        if self.entries.len() >= self.max_entries {
            // Find entry with smallest access_counter (LRU)
            if let Some(lru_idx) = self.entries.iter()
                .enumerate()
                .min_by_key(|(_, (_, _, access))| *access)
                .map(|(idx, _)| idx)
            {
                self.entries.swap_remove(lru_idx);
                self.stats.evictions += 1;
            }
        }
        
        self.access_counter += 1;
        self.entries.push((hash, bytecode, self.access_counter));
        self.stats.insertions += 1;
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_counter = 0;
    }

    /// Get the hit rate as a percentage (0.0 - 100.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.stats.hits + self.stats.misses;
        if total == 0 { 0.0 } else { (self.stats.hits as f64 / total as f64) * 100.0 }
    }
}

impl Default for LruShaderCache {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Combined pipeline state hash for linked vertex+fragment program caching.
/// Instead of caching VP and FP separately, this combines them with
/// relevant pipeline state into a single hash for the full pipeline.
pub struct PipelineStateHasher;

impl PipelineStateHasher {
    /// Compute a combined hash for a linked VP+FP pipeline.
    ///
    /// Combines:
    /// - Vertex program hash
    /// - Fragment program hash
    /// - Blend state (enable, src/dst factors, equation)
    /// - Depth state (enable, func, write)
    /// - Cull state (enable, mode)
    /// - MSAA sample count
    pub fn compute(
        vp_hash: u64,
        fp_hash: u64,
        blend_enable: bool,
        blend_src: u32,
        blend_dst: u32,
        blend_eq: u32,
        depth_enable: bool,
        depth_func: u32,
        depth_write: bool,
        cull_enable: bool,
        cull_mode: u32,
        sample_count: u8,
    ) -> u64 {
        // FNV-1a hash combining
        let mut hash: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x100000001b3;
        
        for byte in vp_hash.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(prime);
        }
        for byte in fp_hash.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(prime);
        }
        
        hash ^= blend_enable as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= blend_src as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= blend_dst as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= blend_eq as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= depth_enable as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= depth_func as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= depth_write as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= cull_enable as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= cull_mode as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= sample_count as u64;
        hash = hash.wrapping_mul(prime);
        
        hash
    }
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

#[cfg(test)]
mod lru_tests {
    use super::*;

    #[test]
    fn test_lru_cache_basic() {
        let mut cache = LruShaderCache::new(4);
        assert!(cache.is_empty());
        
        cache.insert(0x1234, vec![1, 2, 3]);
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
        
        let result = cache.get(0x1234);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &[1, 2, 3]);
        assert_eq!(cache.stats.hits, 1);
    }

    #[test]
    fn test_lru_cache_miss() {
        let mut cache = LruShaderCache::new(4);
        cache.insert(0x1234, vec![1, 2, 3]);
        
        assert!(cache.get(0x5678).is_none());
        assert_eq!(cache.stats.misses, 1);
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = LruShaderCache::new(3);
        
        cache.insert(0x1, vec![1]);
        cache.insert(0x2, vec![2]);
        cache.insert(0x3, vec![3]);
        assert_eq!(cache.len(), 3);
        
        // Access 0x1 to make it recently used
        cache.get(0x1);
        
        // Insert 0x4 — should evict 0x2 (LRU)
        cache.insert(0x4, vec![4]);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.stats.evictions, 1);
        
        // 0x2 should be evicted
        assert!(cache.get(0x2).is_none());
        // 0x1, 0x3, 0x4 should remain
        assert!(cache.get(0x1).is_some());
        assert!(cache.get(0x3).is_some());
        assert!(cache.get(0x4).is_some());
    }

    #[test]
    fn test_lru_update_existing() {
        let mut cache = LruShaderCache::new(4);
        cache.insert(0x1, vec![1, 2, 3]);
        cache.insert(0x1, vec![4, 5, 6]); // Update
        
        assert_eq!(cache.len(), 1); // Should not add duplicate
        assert_eq!(cache.get(0x1).unwrap(), &[4, 5, 6]);
    }

    #[test]
    fn test_lru_hit_rate() {
        let mut cache = LruShaderCache::new(10);
        cache.insert(0x1, vec![1]);
        
        cache.get(0x1); // Hit
        cache.get(0x1); // Hit
        cache.get(0x2); // Miss
        
        let rate = cache.hit_rate();
        // 2 hits, 1 miss = 66.67%
        assert!(rate > 66.0 && rate < 67.0);
    }

    #[test]
    fn test_lru_clear() {
        let mut cache = LruShaderCache::new(10);
        cache.insert(0x1, vec![1]);
        cache.insert(0x2, vec![2]);
        assert_eq!(cache.len(), 2);
        
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_pipeline_state_hasher() {
        let hash1 = PipelineStateHasher::compute(
            0x1234, 0x5678, true, 1, 2, 3, true, 4, true, false, 0, 4
        );
        let hash2 = PipelineStateHasher::compute(
            0x1234, 0x5678, true, 1, 2, 3, true, 4, true, false, 0, 4
        );
        assert_eq!(hash1, hash2); // Same inputs = same hash
        
        let hash3 = PipelineStateHasher::compute(
            0x1234, 0x5678, false, 1, 2, 3, true, 4, true, false, 0, 4
        );
        assert_ne!(hash1, hash3); // Different blend_enable = different hash
    }

    #[test]
    fn test_pipeline_state_hasher_all_different() {
        let hash1 = PipelineStateHasher::compute(0, 0, false, 0, 0, 0, false, 0, false, false, 0, 1);
        let hash2 = PipelineStateHasher::compute(1, 0, false, 0, 0, 0, false, 0, false, false, 0, 1);
        let hash3 = PipelineStateHasher::compute(0, 1, false, 0, 0, 0, false, 0, false, false, 0, 1);
        
        // All should be different
        assert_ne!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_ne!(hash2, hash3);
    }
}
