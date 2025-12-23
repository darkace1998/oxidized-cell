//! RSX texture handling

use bitflags::bitflags;

bitflags! {
    /// Texture format flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextureFormat: u8 {
        const ARGB8 = 0x85;
        const DXT1 = 0x86;
        const DXT3 = 0x87;
        const DXT5 = 0x88;
        const A8R8G8B8 = 0x8A;
        const R5G6B5 = 0x8B;
    }
}

bitflags! {
    /// Texture filter modes
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextureFilter: u8 {
        const NEAREST = 1;
        const LINEAR = 2;
    }
}

bitflags! {
    /// Texture wrap modes
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextureWrap: u8 {
        const REPEAT = 1;
        const MIRRORED_REPEAT = 2;
        const CLAMP_TO_EDGE = 3;
        const CLAMP_TO_BORDER = 4;
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
            min_filter: TextureFilter::LINEAR,
            mag_filter: TextureFilter::LINEAR,
            wrap_s: TextureWrap::REPEAT,
            wrap_t: TextureWrap::REPEAT,
            wrap_r: TextureWrap::REPEAT,
            is_cubemap: false,
        }
    }

    /// Get size in bytes for this texture
    pub fn byte_size(&self) -> u32 {
        let bytes_per_pixel = match self.format {
            0x85 | 0x8A => 4, // ARGB8, A8R8G8B8
            0x8B => 2,         // R5G6B5
            0x86 => 1,         // DXT1 (0.5 bytes per pixel in 4x4 blocks = 8 bytes per block)
            0x87 | 0x88 => 1,  // DXT3/DXT5 (1 byte per pixel in 4x4 blocks = 16 bytes per block)
            _ => 4,
        };

        let mut size = 0u32;
        let mut w = self.width as u32;
        let mut h = self.height as u32;

        for _ in 0..self.mipmap_levels {
            if self.format == 0x86 || self.format == 0x87 || self.format == 0x88 {
                // Block-compressed formats: size in 4x4 blocks
                let blocks_w = w.div_ceil(4);
                let blocks_h = h.div_ceil(4);
                let block_size = if self.format == 0x86 { 8 } else { 16 };
                size += blocks_w * blocks_h * block_size;
            } else {
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
}
