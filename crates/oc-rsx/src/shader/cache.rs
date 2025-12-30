//! Shader cache with disk persistence
//!
//! Caches compiled SPIR-V shaders to disk for faster loading on subsequent runs.

use super::types::*;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Shader cache magic number
const CACHE_MAGIC: u32 = 0x5348_4452; // "SHDR"
const CACHE_VERSION: u32 = 1;

/// Persistent shader cache
pub struct ShaderCache {
    cache_dir: PathBuf,
    vertex_cache: HashMap<u64, SpirVModule>,
    fragment_cache: HashMap<u64, SpirVModule>,
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
        }
    }

    /// Get cached vertex shader
    pub fn get_vertex(&mut self, hash: u64) -> Option<SpirVModule> {
        // Check memory cache first
        if let Some(module) = self.vertex_cache.get(&hash) {
            return Some(module.clone());
        }

        // Try loading from disk
        let path = self.cache_dir.join(format!("vp_{:016x}.spv", hash));
        if let Ok(module) = Self::load_from_disk(&path, ShaderStage::VERTEX) {
            self.vertex_cache.insert(hash, module.clone());
            return Some(module);
        }

        None
    }

    /// Get cached fragment shader
    pub fn get_fragment(&mut self, hash: u64) -> Option<SpirVModule> {
        // Check memory cache first
        if let Some(module) = self.fragment_cache.get(&hash) {
            return Some(module.clone());
        }

        // Try loading from disk
        let path = self.cache_dir.join(format!("fp_{:016x}.spv", hash));
        if let Ok(module) = Self::load_from_disk(&path, ShaderStage::FRAGMENT) {
            self.fragment_cache.insert(hash, module.clone());
            return Some(module);
        }

        None
    }

    /// Store vertex shader in cache
    pub fn store_vertex(&mut self, hash: u64, module: &SpirVModule) {
        self.vertex_cache.insert(hash, module.clone());
        
        let path = self.cache_dir.join(format!("vp_{:016x}.spv", hash));
        let _ = Self::save_to_disk(&path, module);
    }

    /// Store fragment shader in cache
    pub fn store_fragment(&mut self, hash: u64, module: &SpirVModule) {
        self.fragment_cache.insert(hash, module.clone());
        
        let path = self.cache_dir.join(format!("fp_{:016x}.spv", hash));
        let _ = Self::save_to_disk(&path, module);
    }

    /// Load shader from disk
    fn load_from_disk(path: &Path, stage: ShaderStage) -> Result<SpirVModule, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| e.to_string())?;

        // Verify header
        if data.len() < 12 {
            return Err("Cache file too small".to_string());
        }

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        if magic != CACHE_MAGIC {
            return Err("Invalid cache magic".to_string());
        }
        if version != CACHE_VERSION {
            return Err("Cache version mismatch".to_string());
        }

        let bytecode_bytes = &data[12..];
        if bytecode_bytes.len() != size as usize {
            return Err("Cache size mismatch".to_string());
        }

        // Convert bytes to u32s
        let bytecode: Vec<u32> = bytecode_bytes
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        Ok(SpirVModule { bytecode, stage })
    }

    /// Save shader to disk
    fn save_to_disk(path: &Path, module: &SpirVModule) -> Result<(), String> {
        let mut file = File::create(path).map_err(|e| e.to_string())?;

        // Write header
        file.write_all(&CACHE_MAGIC.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&CACHE_VERSION.to_le_bytes()).map_err(|e| e.to_string())?;
        
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
            vertex_count: self.vertex_cache.len(),
            fragment_count: self.fragment_cache.len(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub vertex_count: usize,
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
}
