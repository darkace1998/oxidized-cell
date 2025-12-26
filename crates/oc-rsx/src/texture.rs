//! RSX texture handling

use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

/// Texture format constants for RSX
/// Based on NV40/G70 texture formats used in PS3
pub mod format {
    // Standard uncompressed formats
    pub const B8: u8 = 0x81;
    pub const A1R5G5B5: u8 = 0x82;
    pub const A4R4G4B4: u8 = 0x83;
    pub const R5G6B5: u8 = 0x84;
    pub const ARGB8: u8 = 0x85;
    pub const DXT1: u8 = 0x86;
    pub const DXT3: u8 = 0x87;
    pub const DXT5: u8 = 0x88;
    pub const A8R8G8B8: u8 = 0x8A;
    pub const XRGB8: u8 = 0x8B;
    
    // 32-bit depth/HDR formats
    pub const G8B8: u8 = 0x8C;
    pub const R6G5B5: u8 = 0x8D;
    pub const DEPTH24_D8: u8 = 0x8E;
    pub const DEPTH24_D8_FLOAT: u8 = 0x8F;
    pub const DEPTH16: u8 = 0x90;
    pub const DEPTH16_FLOAT: u8 = 0x91;
    pub const X16: u8 = 0x92;
    pub const Y16_X16: u8 = 0x93;
    pub const R5G5B5A1: u8 = 0x94;
    pub const HILO8: u8 = 0x95;
    pub const HILO_S8: u8 = 0x96;
    pub const W16_Z16_Y16_X16_FLOAT: u8 = 0x97;
    pub const W32_Z32_Y32_X32_FLOAT: u8 = 0x98;
    pub const X32_FLOAT: u8 = 0x99;
    pub const D1R5G5B5: u8 = 0x9A;
    pub const D8R8G8B8: u8 = 0x9B;
    pub const Y16_X16_FLOAT: u8 = 0x9C;
    
    // ETC/EAC compressed formats (common in mobile, some PS3 games use)
    pub const ETC1_RGB8: u8 = 0xA0;
    pub const ETC2_RGB8: u8 = 0xA1;
    pub const ETC2_RGB8A1: u8 = 0xA2;
    pub const ETC2_RGBA8: u8 = 0xA3;
    pub const EAC_R11: u8 = 0xA4;
    pub const EAC_RG11: u8 = 0xA5;
    pub const EAC_R11_SIGNED: u8 = 0xA6;
    pub const EAC_RG11_SIGNED: u8 = 0xA7;
    
    // ASTC compressed formats
    pub const ASTC_4X4: u8 = 0xB0;
    pub const ASTC_5X4: u8 = 0xB1;
    pub const ASTC_5X5: u8 = 0xB2;
    pub const ASTC_6X5: u8 = 0xB3;
    pub const ASTC_6X6: u8 = 0xB4;
    pub const ASTC_8X5: u8 = 0xB5;
    pub const ASTC_8X6: u8 = 0xB6;
    pub const ASTC_8X8: u8 = 0xB7;
    pub const ASTC_10X5: u8 = 0xB8;
    pub const ASTC_10X6: u8 = 0xB9;
    pub const ASTC_10X8: u8 = 0xBA;
    pub const ASTC_10X10: u8 = 0xBB;
    pub const ASTC_12X10: u8 = 0xBC;
    pub const ASTC_12X12: u8 = 0xBD;
    
    /// Get bytes per pixel for a format (returns 0 for compressed)
    pub fn bytes_per_pixel(format: u8) -> u32 {
        match format {
            B8 => 1,
            A1R5G5B5 | A4R4G4B4 | R5G6B5 | R5G5B5A1 | D1R5G5B5 | G8B8 | R6G5B5 | DEPTH16 | DEPTH16_FLOAT | X16 | HILO8 | HILO_S8 => 2,
            ARGB8 | A8R8G8B8 | XRGB8 | DEPTH24_D8 | DEPTH24_D8_FLOAT | D8R8G8B8 | X32_FLOAT | Y16_X16 | Y16_X16_FLOAT => 4,
            W16_Z16_Y16_X16_FLOAT => 8,
            W32_Z32_Y32_X32_FLOAT => 16,
            // Block compressed formats return 0 - use block_size() function instead
            DXT1 | ETC1_RGB8 | ETC2_RGB8 | ETC2_RGB8A1 | EAC_R11 | EAC_R11_SIGNED |
            DXT3 | DXT5 | ETC2_RGBA8 | EAC_RG11 | EAC_RG11_SIGNED |
            ASTC_4X4 | ASTC_5X4 | ASTC_5X5 | ASTC_6X5 | ASTC_6X6 | ASTC_8X5 | ASTC_8X6 | ASTC_8X8 |
            ASTC_10X5 | ASTC_10X6 | ASTC_10X8 | ASTC_10X10 | ASTC_12X10 | ASTC_12X12 => 0,
            _ => 4, // Default to 4 bytes
        }
    }
    
    /// Get block size for compressed formats (width, height, bytes)
    pub fn block_size(format: u8) -> (u32, u32, u32) {
        match format {
            DXT1 | ETC1_RGB8 | ETC2_RGB8 | ETC2_RGB8A1 | EAC_R11 | EAC_R11_SIGNED => (4, 4, 8),
            DXT3 | DXT5 | ETC2_RGBA8 | EAC_RG11 | EAC_RG11_SIGNED => (4, 4, 16),
            ASTC_4X4 => (4, 4, 16),
            ASTC_5X4 => (5, 4, 16),
            ASTC_5X5 => (5, 5, 16),
            ASTC_6X5 => (6, 5, 16),
            ASTC_6X6 => (6, 6, 16),
            ASTC_8X5 => (8, 5, 16),
            ASTC_8X6 => (8, 6, 16),
            ASTC_8X8 => (8, 8, 16),
            ASTC_10X5 => (10, 5, 16),
            ASTC_10X6 => (10, 6, 16),
            ASTC_10X8 => (10, 8, 16),
            ASTC_10X10 => (10, 10, 16),
            ASTC_12X10 => (12, 10, 16),
            ASTC_12X12 => (12, 12, 16),
            _ => (1, 1, 0), // Not block compressed
        }
    }
    
    /// Check if format is compressed
    pub fn is_compressed(format: u8) -> bool {
        matches!(format, 
            DXT1 | DXT3 | DXT5 |
            ETC1_RGB8 | ETC2_RGB8 | ETC2_RGB8A1 | ETC2_RGBA8 | 
            EAC_R11 | EAC_RG11 | EAC_R11_SIGNED | EAC_RG11_SIGNED |
            ASTC_4X4 | ASTC_5X4 | ASTC_5X5 | ASTC_6X5 | ASTC_6X6 | ASTC_8X5 | ASTC_8X6 | ASTC_8X8 |
            ASTC_10X5 | ASTC_10X6 | ASTC_10X8 | ASTC_10X10 | ASTC_12X10 | ASTC_12X12
        )
    }
    
    /// Check if format has alpha channel
    pub fn has_alpha(format: u8) -> bool {
        matches!(format,
            A1R5G5B5 | A4R4G4B4 | ARGB8 | A8R8G8B8 | R5G5B5A1 | DXT3 | DXT5 |
            ETC2_RGB8A1 | ETC2_RGBA8 | ASTC_4X4 | ASTC_5X4 | ASTC_5X5 | ASTC_6X5 | ASTC_6X6 |
            ASTC_8X5 | ASTC_8X6 | ASTC_8X8 | ASTC_10X5 | ASTC_10X6 | ASTC_10X8 | ASTC_10X10 | ASTC_12X10 | ASTC_12X12
        )
    }
    
    /// Check if format is a depth format
    pub fn is_depth(format: u8) -> bool {
        matches!(format, DEPTH24_D8 | DEPTH24_D8_FLOAT | DEPTH16 | DEPTH16_FLOAT)
    }
}

/// Texture filter modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilter {
    Nearest = 1,
    Linear = 2,
}

impl Default for TextureFilter {
    fn default() -> Self {
        Self::Linear
    }
}

/// Texture wrap modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureWrap {
    Repeat = 1,
    MirroredRepeat = 2,
    ClampToEdge = 3,
    ClampToBorder = 4,
}

impl Default for TextureWrap {
    fn default() -> Self {
        Self::Repeat
    }
}

/// Texture descriptor
#[derive(Debug, Clone)]
pub struct Texture {
    /// GPU memory offset
    pub offset: u32,
    /// Texture format
    pub format: u8,
    /// Texture width
    pub width: u16,
    /// Texture height
    pub height: u16,
    /// Texture depth (for 3D textures)
    pub depth: u16,
    /// Number of mipmap levels
    pub mipmap_levels: u8,
    /// Texture pitch (stride)
    pub pitch: u16,
    /// Minification filter
    pub min_filter: TextureFilter,
    /// Magnification filter
    pub mag_filter: TextureFilter,
    /// Wrap mode U
    pub wrap_s: TextureWrap,
    /// Wrap mode V
    pub wrap_t: TextureWrap,
    /// Wrap mode W
    pub wrap_r: TextureWrap,
    /// Whether texture is cubemap
    pub is_cubemap: bool,
    /// Anisotropic filtering level (1.0 = disabled, 2.0, 4.0, 8.0, 16.0)
    pub anisotropy: f32,
    /// LOD bias
    pub lod_bias: f32,
    /// Minimum LOD level
    pub min_lod: f32,
    /// Maximum LOD level
    pub max_lod: f32,
}

impl Texture {
    /// Create a new texture
    pub fn new() -> Self {
        Self {
            offset: 0,
            format: 0,
            width: 0,
            height: 0,
            depth: 1,
            mipmap_levels: 1,
            pitch: 0,
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Linear,
            wrap_s: TextureWrap::Repeat,
            wrap_t: TextureWrap::Repeat,
            wrap_r: TextureWrap::Repeat,
            is_cubemap: false,
            anisotropy: 1.0,
            lod_bias: 0.0,
            min_lod: -1000.0,
            max_lod: 1000.0,
        }
    }

    /// Get size in bytes for this texture
    pub fn byte_size(&self) -> u32 {
        let mut size = 0u32;
        let mut w = self.width as u32;
        let mut h = self.height as u32;

        for _ in 0..self.mipmap_levels {
            if format::is_compressed(self.format) {
                // Block-compressed formats: size in blocks
                let (block_w, block_h, block_bytes) = format::block_size(self.format);
                let blocks_w = w.div_ceil(block_w);
                let blocks_h = h.div_ceil(block_h);
                size += blocks_w * blocks_h * block_bytes;
            } else {
                let bytes_per_pixel = format::bytes_per_pixel(self.format);
                size += w * h * bytes_per_pixel;
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }

        if self.is_cubemap {
            size *= 6; // 6 faces for cubemap
        }

        size
    }
}

impl Default for Texture {
    fn default() -> Self {
        Self::new()
    }
}

/// Texture cache for storing uploaded texture data
pub struct TextureCache {
    /// Cached textures
    textures: Vec<CachedTexture>,
    /// Maximum cache size in bytes
    max_size: usize,
    /// Current cache size
    current_size: usize,
}

/// A cached texture entry
#[derive(Clone)]
struct CachedTexture {
    /// GPU memory offset
    offset: u32,
    /// Texture descriptor
    descriptor: Texture,
    /// Cached texture data
    data: Vec<u8>,
    /// Last access timestamp
    last_used: u64,
}

impl TextureCache {
    /// Create a new texture cache
    pub fn new(max_size: usize) -> Self {
        Self {
            textures: Vec::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Get cached texture
    pub fn get(&mut self, offset: u32, timestamp: u64) -> Option<(&Texture, &[u8])> {
        if let Some(cached) = self.textures.iter_mut().find(|t| t.offset == offset) {
            cached.last_used = timestamp;
            Some((&cached.descriptor, cached.data.as_slice()))
        } else {
            None
        }
    }

    /// Insert texture into cache
    pub fn insert(&mut self, offset: u32, descriptor: Texture, data: Vec<u8>, timestamp: u64) {
        // Remove existing entry if present
        if let Some(pos) = self.textures.iter().position(|t| t.offset == offset) {
            let old = self.textures.remove(pos);
            self.current_size -= old.data.len();
        }

        let data_len = data.len();
        self.textures.push(CachedTexture {
            offset,
            descriptor,
            data,
            last_used: timestamp,
        });
        self.current_size += data_len;

        // Evict least recently used entries if cache is full
        while self.current_size > self.max_size && !self.textures.is_empty() {
            if let Some(lru_pos) = self.find_lru() {
                let old = self.textures.remove(lru_pos);
                self.current_size -= old.data.len();
            }
        }
    }

    /// Find least recently used texture
    fn find_lru(&self) -> Option<usize> {
        self.textures
            .iter()
            .enumerate()
            .min_by_key(|(_, t)| t.last_used)
            .map(|(i, _)| i)
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.textures.clear();
        self.current_size = 0;
    }

    /// Invalidate entries at or after offset
    pub fn invalidate(&mut self, offset: u32) {
        self.textures.retain(|t| {
            if t.offset >= offset {
                self.current_size -= t.data.len();
                false
            } else {
                true
            }
        });
    }

    /// Get cache statistics
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.textures.len(), self.current_size, self.max_size)
    }
}

/// Texture sampler configuration for accurate sampling
#[derive(Debug, Clone)]
pub struct TextureSampler {
    /// Minification filter
    pub min_filter: TextureFilter,
    /// Magnification filter
    pub mag_filter: TextureFilter,
    /// Mipmap filter
    pub mipmap_filter: TextureFilter,
    /// Wrap mode U
    pub wrap_s: TextureWrap,
    /// Wrap mode V
    pub wrap_t: TextureWrap,
    /// Wrap mode W
    pub wrap_r: TextureWrap,
    /// Anisotropic filtering level (1.0-16.0)
    pub max_anisotropy: f32,
    /// LOD bias
    pub lod_bias: f32,
    /// Minimum LOD
    pub min_lod: f32,
    /// Maximum LOD
    pub max_lod: f32,
    /// Border color (RGBA)
    pub border_color: [f32; 4],
    /// Compare mode for depth textures
    pub compare_enable: bool,
    /// Compare function
    pub compare_func: u32,
}

impl TextureSampler {
    /// Create a new texture sampler with default settings
    pub fn new() -> Self {
        Self {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Linear,
            mipmap_filter: TextureFilter::Linear,
            wrap_s: TextureWrap::Repeat,
            wrap_t: TextureWrap::Repeat,
            wrap_r: TextureWrap::Repeat,
            max_anisotropy: 1.0,
            lod_bias: 0.0,
            min_lod: -1000.0,
            max_lod: 1000.0,
            border_color: [0.0, 0.0, 0.0, 0.0],
            compare_enable: false,
            compare_func: 0, // NEVER
        }
    }

    /// Create a sampler with anisotropic filtering
    pub fn with_anisotropy(mut self, level: f32) -> Self {
        self.max_anisotropy = level.clamp(1.0, 16.0);
        self
    }

    /// Create a sampler with LOD bias
    pub fn with_lod_bias(mut self, bias: f32) -> Self {
        self.lod_bias = bias;
        self
    }

    /// Create a sampler with LOD range
    pub fn with_lod_range(mut self, min: f32, max: f32) -> Self {
        self.min_lod = min;
        self.max_lod = max;
        self
    }

    /// Create a sampler for depth comparison
    pub fn with_compare(mut self, func: u32) -> Self {
        self.compare_enable = true;
        self.compare_func = func;
        self
    }

    /// Apply sampler configuration to a texture
    pub fn apply_to_texture(&self, texture: &mut Texture) {
        texture.min_filter = self.min_filter;
        texture.mag_filter = self.mag_filter;
        texture.wrap_s = self.wrap_s;
        texture.wrap_t = self.wrap_t;
        texture.wrap_r = self.wrap_r;
        texture.anisotropy = self.max_anisotropy;
        texture.lod_bias = self.lod_bias;
        texture.min_lod = self.min_lod;
        texture.max_lod = self.max_lod;
    }
}

impl Default for TextureSampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Asynchronous texture loading system
pub struct AsyncTextureLoader {
    /// Sender for texture load requests
    request_sender: Sender<TextureLoadRequest>,
    /// Receiver for loaded textures
    result_receiver: Receiver<TextureLoadResult>,
    /// Number of worker threads
    worker_count: usize,
}

/// Texture load request
#[derive(Clone)]
struct TextureLoadRequest {
    /// Texture ID
    id: u64,
    /// GPU memory offset
    offset: u32,
    /// Texture descriptor
    descriptor: Texture,
}

/// Texture load result
#[derive(Clone)]
struct TextureLoadResult {
    /// Texture ID
    id: u64,
    /// GPU memory offset
    offset: u32,
    /// Texture descriptor
    descriptor: Texture,
    /// Loaded texture data
    data: Vec<u8>,
    /// Whether loading succeeded
    success: bool,
}

impl AsyncTextureLoader {
    /// Create a new async texture loader
    pub fn new(worker_count: usize) -> Self {
        let (request_sender, request_receiver) = channel::<TextureLoadRequest>();
        let (result_sender, result_receiver) = channel::<TextureLoadResult>();

        // Spawn worker threads using shared Arc for receiver
        use std::sync::{Arc, Mutex};
        let shared_rx = Arc::new(Mutex::new(request_receiver));

        for _ in 0..worker_count {
            let rx = Arc::clone(&shared_rx);
            let tx = result_sender.clone();

            thread::spawn(move || {
                loop {
                    let request = {
                        let locked = rx.lock().unwrap();
                        locked.recv()
                    };

                    match request {
                        Ok(req) => {
                            // Simulate texture loading
                            let size = req.descriptor.byte_size() as usize;
                            let data = vec![0u8; size]; // In real implementation, would read from memory
                            
                            let result = TextureLoadResult {
                                id: req.id,
                                offset: req.offset,
                                descriptor: req.descriptor,
                                data,
                                success: true,
                            };

                            if tx.send(result).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        Self {
            request_sender,
            result_receiver,
            worker_count,
        }
    }

    /// Request to load a texture asynchronously
    pub fn load_async(&self, id: u64, offset: u32, descriptor: Texture) -> Result<(), String> {
        let request = TextureLoadRequest {
            id,
            offset,
            descriptor,
        };

        self.request_sender
            .send(request)
            .map_err(|e| format!("Failed to send load request: {}", e))
    }

    /// Check for completed texture loads
    pub fn poll_completed(&self) -> Vec<(u64, u32, Texture, Vec<u8>)> {
        let mut results = Vec::new();
        
        while let Ok(result) = self.result_receiver.try_recv() {
            if result.success {
                results.push((result.id, result.offset, result.descriptor, result.data));
            }
        }

        results
    }

    /// Get number of worker threads
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_byte_size() {
        let mut tex = Texture::new();
        tex.width = 256;
        tex.height = 256;
        tex.format = 0x8A; // A8R8G8B8
        tex.mipmap_levels = 1;
        assert_eq!(tex.byte_size(), 256 * 256 * 4);
    }

    #[test]
    fn test_texture_cache() {
        let mut cache = TextureCache::new(1000);

        let tex = Texture::new();
        let data = vec![1, 2, 3, 4];
        cache.insert(0x1000, tex, data.clone(), 1);

        let (_, cached_data) = cache.get(0x1000, 2).unwrap();
        assert_eq!(cached_data, data.as_slice());
    }

    #[test]
    fn test_texture_cache_eviction() {
        let mut cache = TextureCache::new(20);

        let tex1 = Texture::new();
        cache.insert(0x1000, tex1, vec![0; 15], 1);

        let tex2 = Texture::new();
        cache.insert(0x2000, tex2, vec![0; 15], 2);

        // Should evict first texture (LRU)
        assert!(cache.get(0x1000, 3).is_none());
        assert!(cache.get(0x2000, 3).is_some());
    }

    #[test]
    fn test_texture_cache_invalidate() {
        let mut cache = TextureCache::new(1000);

        cache.insert(0x1000, Texture::new(), vec![1, 2, 3], 1);
        cache.insert(0x2000, Texture::new(), vec![4, 5, 6], 2);

        cache.invalidate(0x1500);

        assert!(cache.get(0x1000, 3).is_some());
        assert!(cache.get(0x2000, 3).is_none());
    }

    #[test]
    fn test_texture_anisotropy() {
        let mut tex = Texture::new();
        tex.anisotropy = 16.0;
        assert_eq!(tex.anisotropy, 16.0);
    }

    #[test]
    fn test_texture_lod() {
        let mut tex = Texture::new();
        tex.lod_bias = 0.5;
        tex.min_lod = 0.0;
        tex.max_lod = 10.0;
        assert_eq!(tex.lod_bias, 0.5);
        assert_eq!(tex.min_lod, 0.0);
        assert_eq!(tex.max_lod, 10.0);
    }

    #[test]
    fn test_texture_format_bytes_per_pixel() {
        assert_eq!(format::bytes_per_pixel(format::ARGB8), 4);
        assert_eq!(format::bytes_per_pixel(format::R5G6B5), 2);
        assert_eq!(format::bytes_per_pixel(format::B8), 1);
        assert_eq!(format::bytes_per_pixel(format::W16_Z16_Y16_X16_FLOAT), 8);
        assert_eq!(format::bytes_per_pixel(format::W32_Z32_Y32_X32_FLOAT), 16);
    }

    #[test]
    fn test_texture_format_compressed() {
        assert!(format::is_compressed(format::DXT1));
        assert!(format::is_compressed(format::DXT3));
        assert!(format::is_compressed(format::DXT5));
        assert!(format::is_compressed(format::ETC1_RGB8));
        assert!(format::is_compressed(format::ETC2_RGB8));
        assert!(format::is_compressed(format::ASTC_4X4));
        assert!(!format::is_compressed(format::ARGB8));
        assert!(!format::is_compressed(format::R5G6B5));
    }

    #[test]
    fn test_texture_format_block_size() {
        assert_eq!(format::block_size(format::DXT1), (4, 4, 8));
        assert_eq!(format::block_size(format::DXT3), (4, 4, 16));
        assert_eq!(format::block_size(format::DXT5), (4, 4, 16));
        assert_eq!(format::block_size(format::ASTC_8X8), (8, 8, 16));
        assert_eq!(format::block_size(format::ARGB8), (1, 1, 0)); // Not block compressed
    }

    #[test]
    fn test_texture_format_has_alpha() {
        assert!(format::has_alpha(format::ARGB8));
        assert!(format::has_alpha(format::A8R8G8B8));
        assert!(format::has_alpha(format::DXT3));
        assert!(format::has_alpha(format::DXT5));
        assert!(!format::has_alpha(format::R5G6B5));
        assert!(!format::has_alpha(format::DXT1));
    }

    #[test]
    fn test_texture_format_is_depth() {
        assert!(format::is_depth(format::DEPTH24_D8));
        assert!(format::is_depth(format::DEPTH16));
        assert!(!format::is_depth(format::ARGB8));
    }

    #[test]
    fn test_texture_compressed_byte_size() {
        let mut tex = Texture::new();
        tex.width = 256;
        tex.height = 256;
        tex.format = format::DXT1;
        tex.mipmap_levels = 1;
        // DXT1: 4x4 blocks, 8 bytes per block
        // 256/4 = 64 blocks per dimension, 64*64 = 4096 blocks, 4096*8 = 32768 bytes
        assert_eq!(tex.byte_size(), 32768);
    }
}
