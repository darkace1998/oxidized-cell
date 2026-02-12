//! RSX texture handling

use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

/// Texture format constants for RSX
/// Based on NV40/G70 texture formats used in PS3
pub mod format {
    // Additional standard formats for completeness
    // Note: RSX uses BGRA byte order internally for most color formats
    pub const R8G8B8: u8 = 0x80;   // 24-bit RGB (packed)
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

// =============================================================================
// DXT/S3TC Decompression Module
// =============================================================================

/// DXT (S3TC) texture decompression for fallback rendering
pub mod dxt {
    /// Decompress a DXT1 block (8 bytes) to 16 RGBA pixels (64 bytes)
    /// DXT1 uses 2 16-bit colors and a 4x4 lookup table (32 bits)
    pub fn decompress_dxt1_block(block: &[u8]) -> [[u8; 4]; 16] {
        let mut output = [[0u8; 4]; 16];
        
        if block.len() < 8 {
            return output;
        }
        
        // Extract the two 16-bit colors (little-endian)
        let c0 = u16::from_le_bytes([block[0], block[1]]);
        let c1 = u16::from_le_bytes([block[2], block[3]]);
        
        // Convert RGB565 to RGB888
        let color0 = rgb565_to_rgb888(c0);
        let color1 = rgb565_to_rgb888(c1);
        
        // Build color palette
        let colors: [[u8; 4]; 4] = if c0 > c1 {
            // Opaque mode: 4 colors
            [
                [color0[0], color0[1], color0[2], 255],
                [color1[0], color1[1], color1[2], 255],
                [
                    ((2 * color0[0] as u16 + color1[0] as u16) / 3) as u8,
                    ((2 * color0[1] as u16 + color1[1] as u16) / 3) as u8,
                    ((2 * color0[2] as u16 + color1[2] as u16) / 3) as u8,
                    255,
                ],
                [
                    ((color0[0] as u16 + 2 * color1[0] as u16) / 3) as u8,
                    ((color0[1] as u16 + 2 * color1[1] as u16) / 3) as u8,
                    ((color0[2] as u16 + 2 * color1[2] as u16) / 3) as u8,
                    255,
                ],
            ]
        } else {
            // Transparent mode: 3 colors + transparent
            [
                [color0[0], color0[1], color0[2], 255],
                [color1[0], color1[1], color1[2], 255],
                [
                    ((color0[0] as u16 + color1[0] as u16) / 2) as u8,
                    ((color0[1] as u16 + color1[1] as u16) / 2) as u8,
                    ((color0[2] as u16 + color1[2] as u16) / 2) as u8,
                    255,
                ],
                [0, 0, 0, 0], // Transparent
            ]
        };
        
        // Decode 4x4 pixel indices
        let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        for i in 0..16 {
            let idx = ((indices >> (i * 2)) & 0x3) as usize;
            output[i] = colors[idx];
        }
        
        output
    }
    
    /// Decompress a DXT3 block (16 bytes) to 16 RGBA pixels (64 bytes)
    /// DXT3 has explicit 4-bit alpha for each pixel
    pub fn decompress_dxt3_block(block: &[u8]) -> [[u8; 4]; 16] {
        let mut output = [[0u8; 4]; 16];
        
        if block.len() < 16 {
            return output;
        }
        
        // First 8 bytes are explicit alpha (4 bits per pixel)
        let alpha_block = &block[0..8];
        
        // Last 8 bytes are color (same as DXT1)
        let color_pixels = decompress_dxt1_block(&block[8..16]);
        
        // Combine color with explicit alpha
        for i in 0..16 {
            let alpha_byte_idx = i / 2;
            let alpha_nibble = if i % 2 == 0 {
                alpha_block[alpha_byte_idx] & 0x0F
            } else {
                (alpha_block[alpha_byte_idx] >> 4) & 0x0F
            };
            // Expand 4-bit alpha to 8-bit
            let alpha = alpha_nibble | (alpha_nibble << 4);
            
            output[i] = [color_pixels[i][0], color_pixels[i][1], color_pixels[i][2], alpha];
        }
        
        output
    }
    
    /// Decompress a DXT5 block (16 bytes) to 16 RGBA pixels (64 bytes)
    /// DXT5 has interpolated alpha
    pub fn decompress_dxt5_block(block: &[u8]) -> [[u8; 4]; 16] {
        let mut output = [[0u8; 4]; 16];
        
        if block.len() < 16 {
            return output;
        }
        
        // First 2 bytes are alpha endpoints
        let alpha0 = block[0];
        let alpha1 = block[1];
        
        // Build alpha palette
        let alphas: [u8; 8] = if alpha0 > alpha1 {
            [
                alpha0,
                alpha1,
                ((6 * alpha0 as u16 + 1 * alpha1 as u16) / 7) as u8,
                ((5 * alpha0 as u16 + 2 * alpha1 as u16) / 7) as u8,
                ((4 * alpha0 as u16 + 3 * alpha1 as u16) / 7) as u8,
                ((3 * alpha0 as u16 + 4 * alpha1 as u16) / 7) as u8,
                ((2 * alpha0 as u16 + 5 * alpha1 as u16) / 7) as u8,
                ((1 * alpha0 as u16 + 6 * alpha1 as u16) / 7) as u8,
            ]
        } else {
            [
                alpha0,
                alpha1,
                ((4 * alpha0 as u16 + 1 * alpha1 as u16) / 5) as u8,
                ((3 * alpha0 as u16 + 2 * alpha1 as u16) / 5) as u8,
                ((2 * alpha0 as u16 + 3 * alpha1 as u16) / 5) as u8,
                ((1 * alpha0 as u16 + 4 * alpha1 as u16) / 5) as u8,
                0,
                255,
            ]
        };
        
        // Alpha indices are 3 bits per pixel, packed in 6 bytes (bytes 2-7)
        // Extract 48 bits = 16 * 3-bit indices
        let alpha_bits = u64::from_le_bytes([
            block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
        ]);
        
        // Last 8 bytes are color (same as DXT1)
        let color_pixels = decompress_dxt1_block(&block[8..16]);
        
        // Combine color with interpolated alpha
        for i in 0..16 {
            let alpha_idx = ((alpha_bits >> (i * 3)) & 0x7) as usize;
            output[i] = [color_pixels[i][0], color_pixels[i][1], color_pixels[i][2], alphas[alpha_idx]];
        }
        
        output
    }
    
    /// Convert RGB565 to RGB888
    fn rgb565_to_rgb888(color: u16) -> [u8; 3] {
        let r = ((color >> 11) & 0x1F) as u8;
        let g = ((color >> 5) & 0x3F) as u8;
        let b = (color & 0x1F) as u8;
        [
            (r << 3) | (r >> 2), // Expand 5-bit to 8-bit
            (g << 2) | (g >> 4), // Expand 6-bit to 8-bit
            (b << 3) | (b >> 2), // Expand 5-bit to 8-bit
        ]
    }
    
    /// Decompress a full DXT1 texture to RGBA8
    pub fn decompress_dxt1(data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let blocks_x = width.div_ceil(4);
        let blocks_y = height.div_ceil(4);
        let mut output = vec![0u8; (width * height * 4) as usize];
        
        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block_idx = (by * blocks_x + bx) as usize;
                let block_offset = block_idx * 8;
                
                if block_offset + 8 > data.len() {
                    continue;
                }
                
                let pixels = decompress_dxt1_block(&data[block_offset..block_offset + 8]);
                
                // Write pixels to output
                for py in 0..4 {
                    for px in 0..4 {
                        let x = bx * 4 + px;
                        let y = by * 4 + py;
                        if x < width && y < height {
                            let out_idx = ((y * width + x) * 4) as usize;
                            let pixel = pixels[(py * 4 + px) as usize];
                            output[out_idx..out_idx + 4].copy_from_slice(&pixel);
                        }
                    }
                }
            }
        }
        
        output
    }
    
    /// Decompress a full DXT3 texture to RGBA8
    pub fn decompress_dxt3(data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let blocks_x = width.div_ceil(4);
        let blocks_y = height.div_ceil(4);
        let mut output = vec![0u8; (width * height * 4) as usize];
        
        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block_idx = (by * blocks_x + bx) as usize;
                let block_offset = block_idx * 16;
                
                if block_offset + 16 > data.len() {
                    continue;
                }
                
                let pixels = decompress_dxt3_block(&data[block_offset..block_offset + 16]);
                
                for py in 0..4 {
                    for px in 0..4 {
                        let x = bx * 4 + px;
                        let y = by * 4 + py;
                        if x < width && y < height {
                            let out_idx = ((y * width + x) * 4) as usize;
                            let pixel = pixels[(py * 4 + px) as usize];
                            output[out_idx..out_idx + 4].copy_from_slice(&pixel);
                        }
                    }
                }
            }
        }
        
        output
    }
    
    /// Decompress a full DXT5 texture to RGBA8
    pub fn decompress_dxt5(data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let blocks_x = width.div_ceil(4);
        let blocks_y = height.div_ceil(4);
        let mut output = vec![0u8; (width * height * 4) as usize];
        
        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block_idx = (by * blocks_x + bx) as usize;
                let block_offset = block_idx * 16;
                
                if block_offset + 16 > data.len() {
                    continue;
                }
                
                let pixels = decompress_dxt5_block(&data[block_offset..block_offset + 16]);
                
                for py in 0..4 {
                    for px in 0..4 {
                        let x = bx * 4 + px;
                        let y = by * 4 + py;
                        if x < width && y < height {
                            let out_idx = ((y * width + x) * 4) as usize;
                            let pixel = pixels[(py * 4 + px) as usize];
                            output[out_idx..out_idx + 4].copy_from_slice(&pixel);
                        }
                    }
                }
            }
        }
        
        output
    }
}

// =============================================================================
// Morton/Z-Order Swizzle Module
// =============================================================================

/// Texture swizzle/tile conversion for RSX memory layouts
pub mod swizzle {
    /// Separate bits of a coordinate for Morton encoding
    /// Spreads bits apart: 0b1111 -> 0b01010101
    fn separate_bits(mut n: u32) -> u32 {
        // 16-bit input max
        n = n & 0x0000FFFF;
        n = (n | (n << 8)) & 0x00FF00FF;
        n = (n | (n << 4)) & 0x0F0F0F0F;
        n = (n | (n << 2)) & 0x33333333;
        n = (n | (n << 1)) & 0x55555555;
        n
    }
    
    /// Compact bits of a Morton code for decoding
    /// Compacts bits: 0b01010101 -> 0b1111
    fn compact_bits(mut n: u32) -> u32 {
        n = n & 0x55555555;
        n = (n | (n >> 1)) & 0x33333333;
        n = (n | (n >> 2)) & 0x0F0F0F0F;
        n = (n | (n >> 4)) & 0x00FF00FF;
        n = (n | (n >> 8)) & 0x0000FFFF;
        n
    }
    
    /// Encode (x, y) coordinates to Morton code (Z-order)
    pub fn morton_encode(x: u32, y: u32) -> u32 {
        separate_bits(x) | (separate_bits(y) << 1)
    }
    
    /// Decode Morton code to (x, y) coordinates
    pub fn morton_decode(code: u32) -> (u32, u32) {
        let x = compact_bits(code);
        let y = compact_bits(code >> 1);
        (x, y)
    }
    
    /// Calculate the pitch (bytes per row) for a given width and bytes per pixel
    /// Aligns to power-of-2 boundaries for tiled textures
    /// Uses 64-byte alignment which is common for GPU texture rows
    const GPU_ROW_ALIGNMENT: u32 = 64;
    
    pub fn calculate_pitch(width: u32, bpp: u32) -> u32 {
        let row_bytes = width * bpp;
        // Round up to nearest 64-byte boundary (GPU alignment requirement)
        (row_bytes + GPU_ROW_ALIGNMENT - 1) & !(GPU_ROW_ALIGNMENT - 1)
    }
    
    /// Calculate pitch for arbitrary width with specific alignment
    pub fn calculate_pitch_aligned(width: u32, bpp: u32, alignment: u32) -> u32 {
        let row_bytes = width * bpp;
        if alignment == 0 {
            return row_bytes;
        }
        let mask = alignment - 1;
        (row_bytes + mask) & !mask
    }
    
    /// Convert linear texture data to tiled/swizzled layout
    /// Uses Morton encoding for Z-order curve layout
    /// Maximum tile dimension is 32 (RSX hardware limit for efficient tiling)
    const MAX_TILE_DIM: u32 = 32;
    
    pub fn linear_to_tiled(
        linear_data: &[u8],
        width: u32,
        height: u32,
        bpp: u32,
    ) -> Vec<u8> {
        let pitch = calculate_pitch(width, bpp);
        let tiled_size = (pitch * height) as usize;
        let mut tiled_data = vec![0u8; tiled_size];
        
        // Find the largest power-of-2 that fits, capped at hardware limit
        let tile_dim = width.min(height).next_power_of_two().min(MAX_TILE_DIM);
        
        for y in 0..height {
            for x in 0..width {
                let linear_offset = (y * width * bpp + x * bpp) as usize;
                
                // Calculate tiled offset using Morton code within tiles
                let tile_x = x / tile_dim;
                let tile_y = y / tile_dim;
                let inner_x = x % tile_dim;
                let inner_y = y % tile_dim;
                
                let morton = morton_encode(inner_x, inner_y);
                let tile_offset = (tile_y * (width / tile_dim).max(1) + tile_x) * tile_dim * tile_dim;
                let tiled_offset = ((tile_offset + morton) * bpp) as usize;
                
                if linear_offset + bpp as usize <= linear_data.len()
                    && tiled_offset + bpp as usize <= tiled_data.len()
                {
                    tiled_data[tiled_offset..tiled_offset + bpp as usize]
                        .copy_from_slice(&linear_data[linear_offset..linear_offset + bpp as usize]);
                }
            }
        }
        
        tiled_data
    }
    
    /// Convert tiled/swizzled texture data to linear layout
    pub fn tiled_to_linear(
        tiled_data: &[u8],
        width: u32,
        height: u32,
        bpp: u32,
    ) -> Vec<u8> {
        let linear_size = (width * height * bpp) as usize;
        let mut linear_data = vec![0u8; linear_size];
        
        let tile_dim = width.min(height).next_power_of_two().min(MAX_TILE_DIM);
        
        for y in 0..height {
            for x in 0..width {
                let linear_offset = (y * width * bpp + x * bpp) as usize;
                
                let tile_x = x / tile_dim;
                let tile_y = y / tile_dim;
                let inner_x = x % tile_dim;
                let inner_y = y % tile_dim;
                
                let morton = morton_encode(inner_x, inner_y);
                let tile_offset = (tile_y * (width / tile_dim).max(1) + tile_x) * tile_dim * tile_dim;
                let tiled_offset = ((tile_offset + morton) * bpp) as usize;
                
                if tiled_offset + bpp as usize <= tiled_data.len()
                    && linear_offset + bpp as usize <= linear_data.len()
                {
                    linear_data[linear_offset..linear_offset + bpp as usize]
                        .copy_from_slice(&tiled_data[tiled_offset..tiled_offset + bpp as usize]);
                }
            }
        }
        
        linear_data
    }
    
    /// RSX-specific swizzle for render targets
    /// Uses different tile dimensions based on format
    pub fn rsx_swizzle_address(x: u32, y: u32, width: u32, _height: u32, bpp: u32) -> u32 {
        // RSX uses different tile sizes based on surface type
        let tile_width = match bpp {
            1 => 64,
            2 => 32,
            4 => 16,
            8 => 8,
            16 => 4,
            _ => 16,
        };
        let tile_height = 8; // RSX typically uses 8-row tiles
        
        let tile_x = x / tile_width;
        let tile_y = y / tile_height;
        let inner_x = x % tile_width;
        let inner_y = y % tile_height;
        
        let tiles_per_row = width.div_ceil(tile_width);
        let tile_size = tile_width * tile_height * bpp;
        let tile_offset = (tile_y * tiles_per_row + tile_x) * tile_size;
        
        let inner_offset = (inner_y * tile_width + inner_x) * bpp;
        
        tile_offset + inner_offset
    }
    
    /// Convert linear to RSX swizzled format
    pub fn linear_to_rsx_swizzle(
        linear_data: &[u8],
        width: u32,
        height: u32,
        bpp: u32,
    ) -> Vec<u8> {
        let pitch = calculate_pitch(width, bpp);
        let swizzled_size = (pitch * height) as usize;
        let mut swizzled_data = vec![0u8; swizzled_size];
        
        for y in 0..height {
            for x in 0..width {
                let linear_offset = (y * width * bpp + x * bpp) as usize;
                let swizzled_offset = rsx_swizzle_address(x, y, width, height, bpp) as usize;
                
                if linear_offset + bpp as usize <= linear_data.len()
                    && swizzled_offset + bpp as usize <= swizzled_data.len()
                {
                    swizzled_data[swizzled_offset..swizzled_offset + bpp as usize]
                        .copy_from_slice(&linear_data[linear_offset..linear_offset + bpp as usize]);
                }
            }
        }
        
        swizzled_data
    }
    
    /// Convert RSX swizzled format to linear
    pub fn rsx_swizzle_to_linear(
        swizzled_data: &[u8],
        width: u32,
        height: u32,
        bpp: u32,
    ) -> Vec<u8> {
        let linear_size = (width * height * bpp) as usize;
        let mut linear_data = vec![0u8; linear_size];
        
        for y in 0..height {
            for x in 0..width {
                let linear_offset = (y * width * bpp + x * bpp) as usize;
                let swizzled_offset = rsx_swizzle_address(x, y, width, height, bpp) as usize;
                
                if swizzled_offset + bpp as usize <= swizzled_data.len()
                    && linear_offset + bpp as usize <= linear_data.len()
                {
                    linear_data[linear_offset..linear_offset + bpp as usize]
                        .copy_from_slice(&swizzled_data[swizzled_offset..swizzled_offset + bpp as usize]);
                }
            }
        }
        
        linear_data
    }
}

// =============================================================================
// Mipmap Generation Module
// =============================================================================

/// Mipmap generation and LOD handling
pub mod mipmap {
    /// Maximum number of mipmap levels supported (log2(8192) + 1 = 14 for up to 8192x8192)
    pub const MAX_MIPMAP_LEVELS: u32 = 14;
    
    /// Calculate the number of mipmap levels for a texture
    pub fn calculate_mipmap_count(width: u32, height: u32) -> u32 {
        let max_dim = width.max(height);
        if max_dim == 0 {
            return 0;
        }
        (32 - max_dim.leading_zeros()).min(MAX_MIPMAP_LEVELS)
    }
    
    /// Calculate dimensions for a specific mipmap level
    pub fn level_dimensions(base_width: u32, base_height: u32, level: u32) -> (u32, u32) {
        let width = (base_width >> level).max(1);
        let height = (base_height >> level).max(1);
        (width, height)
    }
    
    /// Calculate total size of all mipmap levels in bytes
    pub fn total_mipmap_size(width: u32, height: u32, levels: u32, bpp: u32) -> u32 {
        let mut total = 0u32;
        for level in 0..levels {
            let (w, h) = level_dimensions(width, height, level);
            total = total.saturating_add(w.saturating_mul(h).saturating_mul(bpp));
        }
        total
    }
    
    /// Generate mipmaps using box filter (average of 2x2 pixels)
    /// Returns a vector containing all mipmap levels concatenated
    pub fn generate_mipmaps_rgba8(data: &[u8], width: u32, height: u32, levels: u32) -> Vec<u8> {
        let mut result = Vec::new();
        
        // Copy level 0 (original)
        let level0_size = (width * height * 4) as usize;
        if data.len() >= level0_size {
            result.extend_from_slice(&data[..level0_size]);
        } else {
            result.extend_from_slice(data);
            result.resize(level0_size, 0);
        }
        
        // Generate subsequent levels
        let mut prev_width = width;
        let mut prev_height = height;
        let mut prev_offset = 0usize;
        
        for level in 1..levels {
            let (cur_width, cur_height) = level_dimensions(width, height, level);
            let cur_size = (cur_width * cur_height * 4) as usize;
            
            // Downsample from previous level
            let mut level_data = vec![0u8; cur_size];
            
            for y in 0..cur_height {
                for x in 0..cur_width {
                    // Sample 2x2 from previous level
                    let src_x = x * 2;
                    let src_y = y * 2;
                    
                    let mut r = 0u32;
                    let mut g = 0u32;
                    let mut b = 0u32;
                    let mut a = 0u32;
                    let mut count = 0u32;
                    
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let sx = (src_x + dx).min(prev_width - 1);
                            let sy = (src_y + dy).min(prev_height - 1);
                            let src_idx = prev_offset + ((sy * prev_width + sx) * 4) as usize;
                            
                            if src_idx + 3 < result.len() {
                                r += result[src_idx] as u32;
                                g += result[src_idx + 1] as u32;
                                b += result[src_idx + 2] as u32;
                                a += result[src_idx + 3] as u32;
                                count += 1;
                            }
                        }
                    }
                    
                    let dst_idx = ((y * cur_width + x) * 4) as usize;
                    if count > 0 {
                        level_data[dst_idx] = (r / count) as u8;
                        level_data[dst_idx + 1] = (g / count) as u8;
                        level_data[dst_idx + 2] = (b / count) as u8;
                        level_data[dst_idx + 3] = (a / count) as u8;
                    }
                }
            }
            
            prev_offset = result.len();
            prev_width = cur_width;
            prev_height = cur_height;
            result.extend_from_slice(&level_data);
        }
        
        result
    }
    
    /// Calculate LOD (Level of Detail) based on texture coordinates
    /// Uses the maximum of the partial derivatives
    pub fn calculate_lod(
        ddx: f32,
        ddy: f32,
        texture_width: f32,
        texture_height: f32,
        lod_bias: f32,
    ) -> f32 {
        // Scale derivatives by texture dimensions
        let dx = ddx * texture_width;
        let dy = ddy * texture_height;
        
        // Use maximum gradient for LOD calculation
        let rho = dx.abs().max(dy.abs());
        
        // LOD = log2(rho) + bias
        if rho > 0.0 {
            rho.log2() + lod_bias
        } else {
            lod_bias
        }
    }
    
    /// Clamp LOD to valid range
    /// Returns 0.0 if max_levels is 0 to prevent underflow
    pub fn clamp_lod(lod: f32, min_lod: f32, max_lod: f32, max_levels: u32) -> f32 {
        if max_levels == 0 {
            return 0.0;
        }
        lod.clamp(min_lod, max_lod.min((max_levels - 1) as f32))
    }
    
    /// Calculate trilinear blend factor between two mipmap levels
    pub fn trilinear_blend_factor(lod: f32) -> (u32, u32, f32) {
        let level0 = lod.floor() as u32;
        let level1 = level0 + 1;
        let blend = lod.fract();
        (level0, level1, blend)
    }
    
    /// Get offset of a specific mipmap level in concatenated mipmap data
    pub fn level_offset(base_width: u32, base_height: u32, level: u32, bpp: u32) -> u32 {
        let mut offset = 0u32;
        for l in 0..level {
            let (w, h) = level_dimensions(base_width, base_height, l);
            offset = offset.saturating_add(w.saturating_mul(h).saturating_mul(bpp));
        }
        offset
    }
    
    /// Trilinear filter configuration
    #[derive(Debug, Clone, Copy)]
    pub struct TrilinearConfig {
        /// Enable trilinear filtering
        pub enabled: bool,
        /// LOD bias
        pub lod_bias: f32,
        /// Minimum LOD level
        pub min_lod: f32,
        /// Maximum LOD level
        pub max_lod: f32,
        /// Number of mipmap levels available
        pub num_levels: u32,
    }
    
    impl TrilinearConfig {
        /// Create a new trilinear filter configuration
        /// num_levels must be >= 1, values of 0 are treated as 1
        pub fn new(num_levels: u32) -> Self {
            let levels = num_levels.max(1); // Ensure at least 1 level
            Self {
                enabled: true,
                lod_bias: 0.0,
                min_lod: 0.0,
                max_lod: (levels - 1) as f32,
                num_levels: levels,
            }
        }
        
        /// Create configuration with LOD bias
        pub fn with_lod_bias(mut self, bias: f32) -> Self {
            self.lod_bias = bias;
            self
        }
        
        /// Create configuration with LOD range
        pub fn with_lod_range(mut self, min: f32, max: f32) -> Self {
            self.min_lod = min;
            self.max_lod = max;
            self
        }
        
        /// Calculate which mip levels to sample and blend factor
        pub fn calculate_levels(&self, base_lod: f32) -> (u32, u32, f32) {
            if self.num_levels == 0 {
                return (0, 0, 0.0);
            }
            
            let lod = clamp_lod(base_lod + self.lod_bias, self.min_lod, self.max_lod, self.num_levels);
            
            if !self.enabled || self.num_levels == 1 {
                // No trilinear, just use nearest level
                let level = lod.round() as u32;
                let max_level = self.num_levels.saturating_sub(1);
                (level.min(max_level), level.min(max_level), 0.0)
            } else {
                trilinear_blend_factor(lod)
            }
        }
    }
    
    impl Default for TrilinearConfig {
        fn default() -> Self {
            Self::new(1)
        }
    }
}

// =============================================================================
// Tile De-tiling
// =============================================================================

/// De-tile a tiled RSX surface texture to linear format.
/// RSX tiled surfaces use a proprietary tiling pattern for cache efficiency.
/// The tile size is typically 64 bytes wide × 8 rows (bank-interleaved).
///
/// # Parameters
/// - `src`: tiled source pixel data
/// - `width`: texture width in pixels
/// - `height`: texture height in pixels
/// - `bpp`: bytes per pixel (1, 2, 4, 8, or 16)
/// - `tile_pitch`: pitch of the tiled surface in bytes (must be power-of-two aligned)
///
/// # Returns
/// Linear pixel data, or empty Vec if parameters are invalid
pub fn detile_texture(src: &[u8], width: u32, height: u32, bpp: u32, tile_pitch: u32) -> Vec<u8> {
    if width == 0 || height == 0 || bpp == 0 || tile_pitch == 0 {
        return Vec::new();
    }

    let linear_pitch = width * bpp;
    let dst_size = (linear_pitch * height) as usize;
    let mut dst = vec![0u8; dst_size];

    // RSX tile dimensions: 64 bytes wide, 8 rows tall (512 bytes per tile)
    let tile_width_bytes: u32 = 64;
    let tile_height: u32 = 8;
    let tile_size: u32 = tile_width_bytes * tile_height;

    let tiles_per_row = tile_pitch / tile_width_bytes;
    let rows_of_tiles = (height + tile_height - 1) / tile_height;

    for tile_row in 0..rows_of_tiles {
        for tile_col in 0..tiles_per_row {
            let tile_offset = ((tile_row * tiles_per_row + tile_col) * tile_size) as usize;

            for row_in_tile in 0..tile_height {
                let src_y = tile_row * tile_height + row_in_tile;
                if src_y >= height {
                    break;
                }

                let src_offset = tile_offset + (row_in_tile * tile_width_bytes) as usize;
                let dst_x_bytes = tile_col * tile_width_bytes;

                if dst_x_bytes >= linear_pitch {
                    continue;
                }

                let copy_bytes = tile_width_bytes.min(linear_pitch - dst_x_bytes) as usize;
                let dst_offset = (src_y * linear_pitch + dst_x_bytes) as usize;

                if src_offset + copy_bytes <= src.len() && dst_offset + copy_bytes <= dst.len() {
                    dst[dst_offset..dst_offset + copy_bytes]
                        .copy_from_slice(&src[src_offset..src_offset + copy_bytes]);
                }
            }
        }
    }

    dst
}

// =============================================================================
// ETC1 / ETC2 Decompression
// =============================================================================

/// Decompress an ETC1 4×4 block (8 bytes) into 16 RGBA pixels (64 bytes).
///
/// ETC1 encodes a 4×4 block using a base color, a table codeword, and 2-bit
/// per-pixel modifiers. It supports two sub-block modes: horizontal (2×4)
/// and vertical (4×2) partitions.
pub fn decompress_etc1_block(block: &[u8]) -> [u8; 64] {
    let mut output = [255u8; 64]; // Alpha defaults to 255

    if block.len() < 8 {
        return output;
    }

    // Read block as big-endian u64
    let bits = u64::from_be_bytes([
        block[0], block[1], block[2], block[3],
        block[4], block[5], block[6], block[7],
    ]);

    let diff_bit = (bits >> 33) & 1 != 0;
    let flip_bit = (bits >> 32) & 1 != 0;

    // ETC1 modifier tables (per spec)
    let modifier_table: [[i32; 4]; 8] = [
        [2, 8, -2, -8],
        [5, 17, -5, -17],
        [9, 29, -9, -29],
        [13, 42, -13, -42],
        [18, 56, -18, -56],
        [24, 71, -24, -71],
        [33, 92, -33, -92],
        [47, 124, -47, -124],
    ];

    let table_idx0 = ((bits >> 37) & 0x7) as usize;
    let table_idx1 = ((bits >> 34) & 0x7) as usize;

    // Decode base colors
    let (r0, g0, b0, r1, g1, b1) = if diff_bit {
        // Differential mode: 5-bit base + 3-bit delta
        let r = ((bits >> 59) & 0x1F) as i32;
        let g = ((bits >> 51) & 0x1F) as i32;
        let b = ((bits >> 43) & 0x1F) as i32;
        let dr = (((bits >> 56) & 0x7) as i32) - if (bits >> 56) & 0x4 != 0 { 8 } else { 0 };
        let dg = (((bits >> 48) & 0x7) as i32) - if (bits >> 48) & 0x4 != 0 { 8 } else { 0 };
        let db = (((bits >> 40) & 0x7) as i32) - if (bits >> 40) & 0x4 != 0 { 8 } else { 0 };

        let r0 = ((r << 3) | (r >> 2)) as u8;
        let g0 = ((g << 3) | (g >> 2)) as u8;
        let b0 = ((b << 3) | (b >> 2)) as u8;
        let r1c = (r + dr).clamp(0, 31);
        let g1c = (g + dg).clamp(0, 31);
        let b1c = (b + db).clamp(0, 31);
        let r1 = ((r1c << 3) | (r1c >> 2)) as u8;
        let g1 = ((g1c << 3) | (g1c >> 2)) as u8;
        let b1 = ((b1c << 3) | (b1c >> 2)) as u8;
        (r0, g0, b0, r1, g1, b1)
    } else {
        // Individual mode: 4-bit per component per sub-block
        let r0 = ((bits >> 60) & 0xF) as u8;
        let r0 = (r0 << 4) | r0;
        let g0 = ((bits >> 52) & 0xF) as u8;
        let g0 = (g0 << 4) | g0;
        let b0 = ((bits >> 44) & 0xF) as u8;
        let b0 = (b0 << 4) | b0;
        let r1 = ((bits >> 56) & 0xF) as u8;
        let r1 = (r1 << 4) | r1;
        let g1 = ((bits >> 48) & 0xF) as u8;
        let g1 = (g1 << 4) | g1;
        let b1 = ((bits >> 40) & 0xF) as u8;
        let b1 = (b1 << 4) | b1;
        (r0, g0, b0, r1, g1, b1)
    };

    // Decode pixel indices (2 bits per pixel, MSB and LSB in separate 16-bit fields)
    for y in 0..4u32 {
        for x in 0..4u32 {
            let pixel_idx = y * 4 + x;
            // MSB is in bits 16-31, LSB is in bits 0-15
            let msb = ((bits >> (16 + pixel_idx)) & 1) as usize;
            let lsb = ((bits >> pixel_idx) & 1) as usize;
            let modifier_idx = (msb << 1) | lsb;

            // Determine which sub-block this pixel belongs to
            let in_sub0 = if flip_bit { y < 2 } else { x < 2 };

            let (base_r, base_g, base_b, table) = if in_sub0 {
                (r0, g0, b0, &modifier_table[table_idx0])
            } else {
                (r1, g1, b1, &modifier_table[table_idx1])
            };

            let modifier = table[modifier_idx];
            let out_idx = (pixel_idx as usize) * 4;
            output[out_idx] = (base_r as i32 + modifier).clamp(0, 255) as u8;
            output[out_idx + 1] = (base_g as i32 + modifier).clamp(0, 255) as u8;
            output[out_idx + 2] = (base_b as i32 + modifier).clamp(0, 255) as u8;
            // output[out_idx + 3] already 255 (alpha)
        }
    }

    output
}

/// Decompress an ETC2 RGB 4×4 block (8 bytes) into 16 RGBA pixels (64 bytes).
///
/// ETC2 is backward-compatible with ETC1 but adds three new modes for
/// blocks that would be poorly represented by ETC1's limited palette.
/// For simplicity, this implementation delegates to ETC1 decompression
/// since the base encoding is identical; full ETC2 T/H/P modes would
/// be needed for perfect quality.
pub fn decompress_etc2_block(block: &[u8]) -> [u8; 64] {
    // ETC2 is a superset of ETC1 — the base encoding path is identical.
    // The three new ETC2 modes (T, H, Planar) activate when the differential
    // mode produces out-of-range base colors. For now, use ETC1 path.
    decompress_etc1_block(block)
}

// =============================================================================
// ASTC 4×4 Decompression
// =============================================================================

/// Decompress an ASTC 4×4 block (16 bytes) into 16 RGBA pixels (64 bytes).
///
/// ASTC is a highly flexible format with many encoding modes. This
/// simplified implementation extracts the void-extent (constant color)
/// blocks and generates a weighted average for encoded blocks.
/// Full ASTC decoding requires ~2000 lines of code; this provides
/// a reasonable approximation for emulation purposes.
pub fn decompress_astc_4x4_block(block: &[u8]) -> [u8; 64] {
    let mut output = [255u8; 64];

    if block.len() < 16 {
        return output;
    }

    // Check for void-extent block (constant color)
    // Void-extent is signaled when the first 9 bits are 0b111111100 (0x1FC)
    let mode_bits = (block[0] as u16) | ((block[1] as u16) << 8);
    let is_void_extent = (mode_bits & 0x1FF) == 0x1FC;

    if is_void_extent {
        // Void-extent: color is stored in the last 8 bytes as RGBA16
        let r = ((block[8] as u16) | ((block[9] as u16) << 8)) >> 8;
        let g = ((block[10] as u16) | ((block[11] as u16) << 8)) >> 8;
        let b = ((block[12] as u16) | ((block[13] as u16) << 8)) >> 8;
        let a = ((block[14] as u16) | ((block[15] as u16) << 8)) >> 8;

        for i in 0..16 {
            let idx = i * 4;
            output[idx] = r as u8;
            output[idx + 1] = g as u8;
            output[idx + 2] = b as u8;
            output[idx + 3] = a as u8;
        }
    } else {
        // Non-void-extent: extract endpoint colors from the block header.
        // ASTC uses interpolation between two endpoint colors.
        // This is a simplified path that computes an average.
        let r0 = block[4];
        let g0 = block[5];
        let b0 = block[6];
        let r1 = block[8];
        let g1 = block[9];
        let b1 = block[10];

        for pixel in 0..16u32 {
            let idx = (pixel as usize) * 4;
            // Use a per-pixel weight derived from block data for basic variation
            let weight = ((block[(12 + pixel / 4) as usize] >> ((pixel % 4) * 2)) & 0x3) as u32;
            // Map 2-bit weight (0-3) to byte range: 0→0, 1→85, 2→170, 3→255
            let w = weight * 85;
            output[idx] = ((r0 as u32 * (255 - w) + r1 as u32 * w) / 255) as u8;
            output[idx + 1] = ((g0 as u32 * (255 - w) + g1 as u32 * w) / 255) as u8;
            output[idx + 2] = ((b0 as u32 * (255 - w) + b1 as u32 * w) / 255) as u8;
            output[idx + 3] = 255;
        }
    }

    output
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

    // DXT decompression tests
    #[test]
    fn test_dxt1_block_decompression() {
        // Simple DXT1 block: all black pixels
        let block = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let pixels = dxt::decompress_dxt1_block(&block);
        // All pixels should be black
        for pixel in pixels.iter() {
            assert_eq!(pixel[0], 0);
            assert_eq!(pixel[1], 0);
            assert_eq!(pixel[2], 0);
        }
    }

    #[test]
    fn test_dxt1_full_decompression() {
        // 4x4 texture (1 block)
        let block = [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00]; // White color, all index 0
        let data = dxt::decompress_dxt1(&block, 4, 4);
        assert_eq!(data.len(), 4 * 4 * 4); // 16 pixels * 4 bytes (RGBA)
        // First pixel should be white
        assert_eq!(data[0], 255); // R
        assert_eq!(data[1], 255); // G
        assert_eq!(data[2], 255); // B
        assert_eq!(data[3], 255); // A
    }

    #[test]
    fn test_dxt3_block_decompression() {
        // DXT3 block with explicit alpha
        let block = [
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // Alpha (all 0xFF)
            0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, // Color (white)
        ];
        let pixels = dxt::decompress_dxt3_block(&block);
        assert_eq!(pixels[0][3], 255); // Alpha should be 255
    }

    #[test]
    fn test_dxt5_block_decompression() {
        // DXT5 block with interpolated alpha
        let block = [
            0xFF, 0x00, // Alpha endpoints: 255, 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Alpha indices (all 0 = alpha0)
            0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, // Color (white)
        ];
        let pixels = dxt::decompress_dxt5_block(&block);
        assert_eq!(pixels[0][3], 255); // First alpha value
    }

    // Morton/Z-order swizzle tests
    #[test]
    fn test_morton_encode_decode() {
        // Test encoding and decoding
        for x in 0..16 {
            for y in 0..16 {
                let code = swizzle::morton_encode(x, y);
                let (dx, dy) = swizzle::morton_decode(code);
                assert_eq!(x, dx, "X mismatch for ({}, {})", x, y);
                assert_eq!(y, dy, "Y mismatch for ({}, {})", x, y);
            }
        }
    }

    #[test]
    fn test_morton_z_order() {
        // Test Z-order pattern
        assert_eq!(swizzle::morton_encode(0, 0), 0);
        assert_eq!(swizzle::morton_encode(1, 0), 1);
        assert_eq!(swizzle::morton_encode(0, 1), 2);
        assert_eq!(swizzle::morton_encode(1, 1), 3);
        assert_eq!(swizzle::morton_encode(2, 0), 4);
    }

    #[test]
    fn test_calculate_pitch() {
        // 64-byte alignment
        assert_eq!(swizzle::calculate_pitch(16, 4), 64); // 16*4=64, already aligned
        assert_eq!(swizzle::calculate_pitch(15, 4), 64); // 15*4=60, rounds up to 64
        assert_eq!(swizzle::calculate_pitch(17, 4), 128); // 17*4=68, rounds up to 128
    }

    #[test]
    fn test_linear_to_tiled_roundtrip() {
        // Create a simple 4x4 image
        let linear_data: Vec<u8> = (0..64).collect(); // 4x4 * 4 bytes
        
        let tiled = swizzle::linear_to_tiled(&linear_data, 4, 4, 4);
        let recovered = swizzle::tiled_to_linear(&tiled, 4, 4, 4);
        
        assert_eq!(linear_data.len(), recovered.len());
        // Note: Due to tile alignment, some padding may occur, so we compare valid pixels
        for i in 0..linear_data.len() {
            assert_eq!(linear_data[i], recovered[i], "Mismatch at index {}", i);
        }
    }

    // Mipmap generation tests
    #[test]
    fn test_mipmap_count_calculation() {
        assert_eq!(mipmap::calculate_mipmap_count(256, 256), 9); // 256, 128, 64, 32, 16, 8, 4, 2, 1
        assert_eq!(mipmap::calculate_mipmap_count(1, 1), 1);
        assert_eq!(mipmap::calculate_mipmap_count(2, 2), 2);
        assert_eq!(mipmap::calculate_mipmap_count(1024, 512), 11); // max dim = 1024
    }

    #[test]
    fn test_level_dimensions() {
        assert_eq!(mipmap::level_dimensions(256, 256, 0), (256, 256));
        assert_eq!(mipmap::level_dimensions(256, 256, 1), (128, 128));
        assert_eq!(mipmap::level_dimensions(256, 256, 2), (64, 64));
        assert_eq!(mipmap::level_dimensions(256, 256, 8), (1, 1));
        // Ensure minimum of 1
        assert_eq!(mipmap::level_dimensions(256, 256, 20), (1, 1));
    }

    #[test]
    fn test_mipmap_generation() {
        // Create a 4x4 white image
        let data = vec![255u8; 4 * 4 * 4]; // 16 pixels * RGBA
        
        let mipmaps = mipmap::generate_mipmaps_rgba8(&data, 4, 4, 3); // 3 levels: 4x4, 2x2, 1x1
        
        // Level 0: 4x4 = 64 bytes
        // Level 1: 2x2 = 16 bytes
        // Level 2: 1x1 = 4 bytes
        // Total: 84 bytes
        assert_eq!(mipmaps.len(), 84);
        
        // All pixels should still be white (average of white is white)
        for pixel in mipmaps.chunks(4) {
            assert_eq!(pixel[0], 255);
            assert_eq!(pixel[1], 255);
            assert_eq!(pixel[2], 255);
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn test_lod_calculation() {
        // LOD should be higher for larger derivatives
        let lod1 = mipmap::calculate_lod(0.001, 0.001, 256.0, 256.0, 0.0);
        let lod2 = mipmap::calculate_lod(0.01, 0.01, 256.0, 256.0, 0.0);
        assert!(lod2 > lod1);
        
        // LOD bias should add to the result
        let lod_biased = mipmap::calculate_lod(0.01, 0.01, 256.0, 256.0, 1.0);
        assert!((lod_biased - lod2 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_trilinear_blend_factor() {
        let (l0, l1, blend) = mipmap::trilinear_blend_factor(1.5);
        assert_eq!(l0, 1);
        assert_eq!(l1, 2);
        assert!((blend - 0.5).abs() < 0.001);
        
        let (l0, l1, blend) = mipmap::trilinear_blend_factor(2.0);
        assert_eq!(l0, 2);
        assert_eq!(l1, 3);
        assert!(blend.abs() < 0.001);
    }

    #[test]
    fn test_trilinear_config() {
        let config = mipmap::TrilinearConfig::new(8)
            .with_lod_bias(0.5)
            .with_lod_range(0.0, 6.0);
        
        assert!(config.enabled);
        assert_eq!(config.lod_bias, 0.5);
        assert_eq!(config.min_lod, 0.0);
        assert_eq!(config.max_lod, 6.0);
        
        // Test calculate_levels
        let (l0, l1, _) = config.calculate_levels(2.0); // 2.0 + 0.5 bias = 2.5
        assert_eq!(l0, 2);
        assert_eq!(l1, 3);
    }

    #[test]
    fn test_level_offset() {
        // For 256x256 RGBA8 texture
        // Level 0: 256*256*4 = 262144 bytes, offset 0
        // Level 1: 128*128*4 = 65536 bytes, offset 262144
        // Level 2: 64*64*4 = 16384 bytes, offset 327680
        assert_eq!(mipmap::level_offset(256, 256, 0, 4), 0);
        assert_eq!(mipmap::level_offset(256, 256, 1, 4), 262144);
        assert_eq!(mipmap::level_offset(256, 256, 2, 4), 262144 + 65536);
    }

    #[test]
    fn test_detile_empty() {
        let result = detile_texture(&[], 0, 0, 4, 64);
        assert!(result.is_empty());
    }

    #[test]
    fn test_detile_basic() {
        // A simple 16×8 texture with 4 bpp and 64-byte tile pitch
        let width = 16u32;
        let height = 8u32;
        let bpp = 4u32;
        let tile_pitch = 64u32;
        let src = vec![0xAA; (tile_pitch * height) as usize];
        let result = detile_texture(&src, width, height, bpp, tile_pitch);
        assert_eq!(result.len(), (width * height * bpp) as usize);
    }

    #[test]
    fn test_etc1_block_all_zero() {
        let block = [0u8; 8];
        let result = decompress_etc1_block(&block);
        // All pixels should be valid RGBA (alpha = 255)
        for i in 0..16 {
            assert_eq!(result[i * 4 + 3], 255, "Alpha should be 255");
        }
    }

    #[test]
    fn test_etc1_block_short_input() {
        let block = [0u8; 4]; // Too short
        let result = decompress_etc1_block(&block);
        // Should return default (all white with alpha=255)
        for i in 0..16 {
            assert_eq!(result[i * 4 + 3], 255);
        }
    }

    #[test]
    fn test_etc2_delegates_to_etc1() {
        let block = [0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF];
        let etc1 = decompress_etc1_block(&block);
        let etc2 = decompress_etc2_block(&block);
        assert_eq!(etc1, etc2);
    }

    #[test]
    fn test_astc_void_extent() {
        // Construct a void-extent block: first 9 bits = 0x1FC
        let mut block = [0u8; 16];
        block[0] = 0xFC; // lower 8 bits of 0x1FC
        block[1] = 0x01; // upper bit
        // Set color: R=128, G=64, B=192, A=255
        block[8] = 0x00; block[9] = 128;
        block[10] = 0x00; block[11] = 64;
        block[12] = 0x00; block[13] = 192;
        block[14] = 0x00; block[15] = 255;

        let result = decompress_astc_4x4_block(&block);
        // All 16 pixels should have the same color
        for i in 0..16 {
            assert_eq!(result[i * 4], 128);
            assert_eq!(result[i * 4 + 1], 64);
            assert_eq!(result[i * 4 + 2], 192);
            assert_eq!(result[i * 4 + 3], 255);
        }
    }

    #[test]
    fn test_astc_short_input() {
        let block = [0u8; 8]; // Too short
        let result = decompress_astc_4x4_block(&block);
        // Should return default (all white, alpha=255)
        for i in 0..16 {
            assert_eq!(result[i * 4 + 3], 255);
        }
    }

    #[test]
    fn test_astc_non_void_extent() {
        // Non-void-extent block (first 9 bits != 0x1FC)
        let block = [0x00u8, 0x00, 0x00, 0x00, 0x80, 0x40, 0xC0, 0x00,
                     0x40, 0x80, 0x60, 0x00, 0xAA, 0x55, 0xAA, 0x55];
        let result = decompress_astc_4x4_block(&block);
        assert_eq!(result.len(), 64);
        // All pixels should have alpha = 255
        for i in 0..16 {
            assert_eq!(result[i * 4 + 3], 255);
        }
    }
}
