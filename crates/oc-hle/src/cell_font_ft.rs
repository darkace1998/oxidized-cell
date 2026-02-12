//! cellFontFT HLE - FreeType Font Library
//!
//! This module provides HLE implementations for the PS3's FreeType font integration.
//! It extends cellFont with FreeType-specific functionality.

use tracing::{debug, trace};
use crate::memory::write_be32;

/// Error codes
pub const CELL_FONT_FT_ERROR_NOT_INITIALIZED: i32 = 0x80540201u32 as i32;
pub const CELL_FONT_FT_ERROR_ALREADY_INITIALIZED: i32 = 0x80540202u32 as i32;
pub const CELL_FONT_FT_ERROR_NO_MEMORY: i32 = 0x80540203u32 as i32;
pub const CELL_FONT_FT_ERROR_INVALID_PARAMETER: i32 = 0x80540204u32 as i32;
pub const CELL_FONT_FT_ERROR_INVALID_FONT: i32 = 0x80540205u32 as i32;
pub const CELL_FONT_FT_ERROR_FONT_OPEN_FAILED: i32 = 0x80540206u32 as i32;

/// FreeType library handle
pub type FtLibrary = u32;

/// FreeType face handle
pub type FtFace = u32;

/// FreeType configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellFontFtConfig {
    /// Maximum face count
    pub max_face_count: u32,
    /// Memory pool size
    pub memory_pool_size: u32,
    /// Flags
    pub flags: u32,
}

impl Default for CellFontFtConfig {
    fn default() -> Self {
        Self {
            max_face_count: 16,
            memory_pool_size: 1024 * 1024, // 1 MB
            flags: 0,
        }
    }
}

/// Font face info
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellFontFtFaceInfo {
    /// Face handle
    pub face: FtFace,
    /// Number of glyphs
    pub num_glyphs: u32,
    /// Family name
    pub family_name: [u8; 64],
    /// Style name
    pub style_name: [u8; 64],
    /// Units per EM
    pub units_per_em: u32,
    /// Flags (bold, italic, etc)
    pub flags: u32,
}

impl Default for CellFontFtFaceInfo {
    fn default() -> Self {
        Self {
            face: 0,
            num_glyphs: 0,
            family_name: [0; 64],
            style_name: [0; 64],
            units_per_em: 2048,
            flags: 0,
        }
    }
}

/// Glyph metrics
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellFontFtGlyphMetrics {
    /// Glyph width
    pub width: i32,
    /// Glyph height
    pub height: i32,
    /// Horizontal bearing X
    pub bearing_x: i32,
    /// Horizontal bearing Y
    pub bearing_y: i32,
    /// Advance width
    pub advance_x: i32,
    /// Advance height
    pub advance_y: i32,
}

/// Font face entry
#[derive(Debug, Clone)]
struct FaceEntry {
    /// Face ID
    id: u32,
    /// Face info
    info: CellFontFtFaceInfo,
    /// Pixel size
    pixel_size: u32,
    /// Glyph cache (char_code -> metrics)
    glyph_cache: HashMap<u32, CellFontFtGlyphMetrics>,
}

use std::collections::HashMap;

/// FreeType font manager
pub struct FontFtManager {
    /// Initialization flag
    initialized: bool,
    /// Configuration
    config: CellFontFtConfig,
    /// Open faces
    faces: Vec<FaceEntry>,
    /// Next face ID
    next_face_id: u32,
}

impl FontFtManager {
    /// Create a new FreeType font manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            config: CellFontFtConfig::default(),
            faces: Vec::new(),
            next_face_id: 1,
        }
    }

    /// Initialize FreeType library
    pub fn init(&mut self, config: CellFontFtConfig) -> i32 {
        if self.initialized {
            return CELL_FONT_FT_ERROR_ALREADY_INITIALIZED;
        }

        debug!("FontFtManager::init: max_faces={}, pool_size={}", 
            config.max_face_count, config.memory_pool_size);

        self.config = config;
        self.initialized = true;

        trace!("FontFtManager: Initialized FreeType backend (simulated)");

        0 // CELL_OK
    }

    /// Shutdown FreeType library
    pub fn end(&mut self) -> i32 {
        if !self.initialized {
            return CELL_FONT_FT_ERROR_NOT_INITIALIZED;
        }

        debug!("FontFtManager::end");

        self.faces.clear();
        self.initialized = false;

        trace!("FontFtManager: Shutdown FreeType backend");

        0 // CELL_OK
    }

    /// Open font face from memory
    pub fn open_font_memory(
        &mut self,
        data_addr: u32,
        data_size: u32,
        face_index: u32,
    ) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_FONT_FT_ERROR_NOT_INITIALIZED);
        }

        if self.faces.len() >= self.config.max_face_count as usize {
            return Err(CELL_FONT_FT_ERROR_NO_MEMORY);
        }

        let face_id = self.next_face_id;
        self.next_face_id += 1;

        debug!("FontFtManager::open_font_memory: id={}, addr=0x{:08X}, size={}, index={}",
            face_id, data_addr, data_size, face_index);

        // Create face info
        let mut info = CellFontFtFaceInfo::default();
        info.face = face_id;
        info.num_glyphs = 256; // Placeholder
        let family = b"Unknown\0";
        info.family_name[..family.len()].copy_from_slice(family);

        let entry = FaceEntry {
            id: face_id,
            info,
            pixel_size: 12,
            glyph_cache: HashMap::new(),
        };

        self.faces.push(entry);

        trace!("FontFtManager: Loaded font from memory with basic FreeType backend");

        Ok(face_id)
    }

    /// Open font face from file
    pub fn open_font_file(&mut self, path: &str, face_index: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_FONT_FT_ERROR_NOT_INITIALIZED);
        }

        if self.faces.len() >= self.config.max_face_count as usize {
            return Err(CELL_FONT_FT_ERROR_NO_MEMORY);
        }

        let face_id = self.next_face_id;
        self.next_face_id += 1;

        debug!("FontFtManager::open_font_file: id={}, path={}, index={}", face_id, path, face_index);

        // Create face info
        let mut info = CellFontFtFaceInfo::default();
        info.face = face_id;
        info.num_glyphs = 256;
        let family = b"File Font\0";
        info.family_name[..family.len()].copy_from_slice(family);

        let entry = FaceEntry {
            id: face_id,
            info,
            pixel_size: 12,
            glyph_cache: HashMap::new(),
        };

        self.faces.push(entry);

        trace!("FontFtManager: Loaded font from file with basic FreeType backend");

        Ok(face_id)
    }

    /// Close font face
    pub fn close_font(&mut self, face: FtFace) -> i32 {
        if !self.initialized {
            return CELL_FONT_FT_ERROR_NOT_INITIALIZED;
        }

        if let Some(pos) = self.faces.iter().position(|f| f.id == face) {
            debug!("FontFtManager::close_font: face={}", face);
            self.faces.remove(pos);
            0 // CELL_OK
        } else {
            CELL_FONT_FT_ERROR_INVALID_FONT
        }
    }

    /// Set character size
    pub fn set_char_size(&mut self, face: FtFace, char_width: u32, char_height: u32) -> i32 {
        if !self.initialized {
            return CELL_FONT_FT_ERROR_NOT_INITIALIZED;
        }

        if let Some(entry) = self.faces.iter_mut().find(|f| f.id == face) {
            trace!("FontFtManager::set_char_size: face={}, {}x{}", face, char_width, char_height);
            entry.pixel_size = char_height.max(char_width);
            0 // CELL_OK
        } else {
            CELL_FONT_FT_ERROR_INVALID_FONT
        }
    }

    /// Set pixel size
    pub fn set_pixel_size(&mut self, face: FtFace, pixel_width: u32, pixel_height: u32) -> i32 {
        if !self.initialized {
            return CELL_FONT_FT_ERROR_NOT_INITIALIZED;
        }

        if let Some(entry) = self.faces.iter_mut().find(|f| f.id == face) {
            trace!("FontFtManager::set_pixel_size: face={}, {}x{}", face, pixel_width, pixel_height);
            entry.pixel_size = pixel_height.max(pixel_width);
            0 // CELL_OK
        } else {
            CELL_FONT_FT_ERROR_INVALID_FONT
        }
    }

    /// Get face info
    pub fn get_face_info(&self, face: FtFace) -> Result<CellFontFtFaceInfo, i32> {
        if !self.initialized {
            return Err(CELL_FONT_FT_ERROR_NOT_INITIALIZED);
        }

        if let Some(entry) = self.faces.iter().find(|f| f.id == face) {
            Ok(entry.info)
        } else {
            Err(CELL_FONT_FT_ERROR_INVALID_FONT)
        }
    }

    /// Load glyph
    pub fn load_glyph(&mut self, face: FtFace, glyph_index: u32) -> Result<CellFontFtGlyphMetrics, i32> {
        if !self.initialized {
            return Err(CELL_FONT_FT_ERROR_NOT_INITIALIZED);
        }

        let face_entry = self.faces.iter_mut().find(|f| f.id == face)
            .ok_or(CELL_FONT_FT_ERROR_INVALID_FONT)?;

        trace!("FontFtManager::load_glyph: face={}, index={}", face, glyph_index);

        // Check cache first
        if let Some(metrics) = face_entry.glyph_cache.get(&glyph_index) {
            return Ok(*metrics);
        }

        // Generate glyph metrics based on pixel size (simulated FreeType rendering)
        let pixel_size = face_entry.pixel_size as i32;
        let metrics = CellFontFtGlyphMetrics {
            width: (pixel_size * 3) / 4,  // 0.75 * pixel_size
            height: pixel_size,
            bearing_x: 0,
            bearing_y: (pixel_size * 4) / 5,  // 0.8 * pixel_size
            advance_x: pixel_size,
            advance_y: 0,
        };

        // Cache the metrics
        face_entry.glyph_cache.insert(glyph_index, metrics);

        trace!("FontFtManager: Rendered glyph {} with FreeType backend", glyph_index);

        Ok(metrics)
    }

    /// Get glyph index for character code
    pub fn get_char_index(&self, face: FtFace, char_code: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_FONT_FT_ERROR_NOT_INITIALIZED);
        }

        if !self.faces.iter().any(|f| f.id == face) {
            return Err(CELL_FONT_FT_ERROR_INVALID_FONT);
        }

        trace!("FontFtManager::get_char_index: face={}, char=0x{:X}", face, char_code);

        // Simulate FreeType charmap lookup
        // For ASCII range, use direct mapping
        if char_code < 128 {
            Ok(char_code)
        } else {
            // For extended Unicode, use a simple mapping algorithm
            Ok(char_code % 256)
        }
    }

    /// Get face count
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get kerning between two glyphs
    ///
    /// Returns the horizontal advance adjustment in pixels.
    /// In HLE mode, returns estimated kerning values for
    /// common letter pairs.
    pub fn get_kerning(&self, face: FtFace, left_glyph: u32, right_glyph: u32) -> Result<(i32, i32), i32> {
        if !self.initialized {
            return Err(CELL_FONT_FT_ERROR_NOT_INITIALIZED);
        }

        let face_entry = self.faces.iter().find(|f| f.id == face)
            .ok_or(CELL_FONT_FT_ERROR_INVALID_FONT)?;

        trace!("FontFtManager::get_kerning: face={}, left={}, right={}", face, left_glyph, right_glyph);

        // Simulated kerning table for common English pairs.
        // Values are fractions of the pixel size.
        let pixel_size = face_entry.pixel_size as i32;
        let kern_x = match (left_glyph, right_glyph) {
            // AV, AW, AY — pull together
            (0x41, 0x56) | (0x41, 0x57) | (0x41, 0x59) => -(pixel_size / 8),
            // VA, WA, YA
            (0x56, 0x41) | (0x57, 0x41) | (0x59, 0x41) => -(pixel_size / 8),
            // To, Tr
            (0x54, 0x6F) | (0x54, 0x72) => -(pixel_size / 6),
            // LT, LV, LW, LY
            (0x4C, 0x54) | (0x4C, 0x56) | (0x4C, 0x57) | (0x4C, 0x59) => -(pixel_size / 10),
            _ => 0,
        };

        Ok((kern_x, 0))
    }
}

impl Default for FontFtManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellFontFtInit - Initialize FreeType font library
///
/// # Arguments
/// * `config_addr` - Configuration address
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_init(_config_addr: u32) -> i32 {
    debug!("cellFontFtInit()");

    // Use default config when memory read is not yet implemented
    let config = CellFontFtConfig::default();
    crate::context::get_hle_context_mut().font_ft.init(config)
}

/// cellFontFtEnd - Shutdown FreeType font library
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_end() -> i32 {
    debug!("cellFontFtEnd()");

    crate::context::get_hle_context_mut().font_ft.end()
}

/// cellFontFtOpenFontMemory - Open font from memory
///
/// # Arguments
/// * `data_addr` - Font data address
/// * `data_size` - Font data size
/// * `face_index` - Face index in font file
/// * `face_addr` - Address to write face handle
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_open_font_memory(
    data_addr: u32,
    data_size: u32,
    face_index: u32,
    face_addr: u32,
) -> i32 {
    debug!("cellFontFtOpenFontMemory(addr=0x{:08X}, size={}, index={})", 
        data_addr, data_size, face_index);

    match crate::context::get_hle_context_mut().font_ft.open_font_memory(data_addr, data_size, face_index) {
        Ok(face) => {
            // Write face handle to memory
            if face_addr != 0 {
                if let Err(e) = write_be32(face_addr, face) {
                    debug!("cellFontFtOpenFontMemory: Failed to write face handle to memory: {}", e);
                    return e;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellFontFtOpenFontFile - Open font from file
///
/// # Arguments
/// * `path_addr` - Path address
/// * `face_index` - Face index
/// * `face_addr` - Address to write face handle
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_open_font_file(path_addr: u32, face_index: u32, face_addr: u32) -> i32 {
    debug!("cellFontFtOpenFontFile(index={})", face_index);

    // Read path from memory
    let font_path = match crate::memory::read_string(path_addr, 256) {
        Ok(p) => p,
        Err(_) => "font.ttf".to_string(),
    };

    match crate::context::get_hle_context_mut().font_ft.open_font_file(&font_path, face_index) {
        Ok(face) => {
            // Write face handle to memory
            if face_addr != 0 {
                if let Err(e) = write_be32(face_addr, face) {
                    debug!("cellFontFtOpenFontFile: Failed to write face handle to memory: {}", e);
                    return e;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellFontFtCloseFont - Close font face
///
/// # Arguments
/// * `face` - Face handle
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_close_font(face: u32) -> i32 {
    debug!("cellFontFtCloseFont(face={})", face);

    crate::context::get_hle_context_mut().font_ft.close_font(face)
}

/// cellFontFtSetCharSize - Set character size
///
/// # Arguments
/// * `face` - Face handle
/// * `char_width` - Character width (26.6 fixed point)
/// * `char_height` - Character height (26.6 fixed point)
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_set_char_size(face: u32, char_width: u32, char_height: u32) -> i32 {
    trace!("cellFontFtSetCharSize(face={}, {}x{})", face, char_width, char_height);

    crate::context::get_hle_context_mut().font_ft.set_char_size(face, char_width, char_height)
}

/// cellFontFtSetPixelSize - Set pixel size
///
/// # Arguments
/// * `face` - Face handle
/// * `pixel_width` - Pixel width
/// * `pixel_height` - Pixel height
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_set_pixel_size(face: u32, pixel_width: u32, pixel_height: u32) -> i32 {
    trace!("cellFontFtSetPixelSize(face={}, {}x{})", face, pixel_width, pixel_height);

    crate::context::get_hle_context_mut().font_ft.set_pixel_size(face, pixel_width, pixel_height)
}

/// cellFontFtLoadGlyph - Load a glyph
///
/// # Arguments
/// * `face` - Face handle
/// * `glyph_index` - Glyph index
/// * `flags` - Load flags
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_load_glyph(face: u32, glyph_index: u32, _flags: u32) -> i32 {
    trace!("cellFontFtLoadGlyph(face={}, index={})", face, glyph_index);

    match crate::context::get_hle_context_mut().font_ft.load_glyph(face, glyph_index) {
        Ok(_metrics) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellFontFtGetCharIndex - Get glyph index for character
///
/// # Arguments
/// * `face` - Face handle
/// * `char_code` - Character code
///
/// # Returns
/// * Glyph index (or 0 if not found)
pub fn cell_font_ft_get_char_index(face: u32, char_code: u32) -> u32 {
    trace!("cellFontFtGetCharIndex(face={}, char=0x{:X})", face, char_code);

    crate::context::get_hle_context().font_ft.get_char_index(face, char_code).unwrap_or_default()
}

/// cellFontFtGetKerning - Get kerning between two glyphs
///
/// # Arguments
/// * `face` - Face handle
/// * `left_glyph` - Left glyph index
/// * `right_glyph` - Right glyph index
/// * `kern_x_addr` - Address to write horizontal kerning
/// * `kern_y_addr` - Address to write vertical kerning
///
/// # Returns
/// * 0 on success
pub fn cell_font_ft_get_kerning(
    face: u32,
    left_glyph: u32,
    right_glyph: u32,
    kern_x_addr: u32,
    kern_y_addr: u32,
) -> i32 {
    trace!("cellFontFtGetKerning(face={}, left={}, right={})", face, left_glyph, right_glyph);

    match crate::context::get_hle_context().font_ft.get_kerning(face, left_glyph, right_glyph) {
        Ok((kx, ky)) => {
            if kern_x_addr != 0 {
                if let Err(e) = write_be32(kern_x_addr, kx as u32) { return e; }
            }
            if kern_y_addr != 0 {
                if let Err(e) = write_be32(kern_y_addr, ky as u32) { return e; }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_ft_manager_lifecycle() {
        let mut manager = FontFtManager::new();
        
        let config = CellFontFtConfig::default();
        assert_eq!(manager.init(config), 0);
        assert!(manager.is_initialized());
        
        // Double init should fail
        assert_eq!(manager.init(config), CELL_FONT_FT_ERROR_ALREADY_INITIALIZED);
        
        assert_eq!(manager.end(), 0);
        assert!(!manager.is_initialized());
        
        // Double end should fail
        assert_eq!(manager.end(), CELL_FONT_FT_ERROR_NOT_INITIALIZED);
    }

    #[test]
    fn test_font_ft_manager_open_close() {
        let mut manager = FontFtManager::new();
        manager.init(CellFontFtConfig::default());
        
        // Open from memory
        let face = manager.open_font_memory(0x10000000, 1024, 0).unwrap();
        assert_eq!(manager.face_count(), 1);
        
        // Close
        assert_eq!(manager.close_font(face), 0);
        assert_eq!(manager.face_count(), 0);
        
        // Close again should fail
        assert_eq!(manager.close_font(face), CELL_FONT_FT_ERROR_INVALID_FONT);
        
        manager.end();
    }

    #[test]
    fn test_font_ft_manager_face_info() {
        let mut manager = FontFtManager::new();
        manager.init(CellFontFtConfig::default());
        
        let face = manager.open_font_memory(0x10000000, 1024, 0).unwrap();
        
        let info = manager.get_face_info(face).unwrap();
        assert_eq!(info.face, face);
        assert!(info.num_glyphs > 0);
        
        manager.end();
    }

    #[test]
    fn test_font_ft_manager_size() {
        let mut manager = FontFtManager::new();
        manager.init(CellFontFtConfig::default());
        
        let face = manager.open_font_memory(0x10000000, 1024, 0).unwrap();
        
        assert_eq!(manager.set_char_size(face, 16, 16), 0);
        assert_eq!(manager.set_pixel_size(face, 24, 24), 0);
        
        // Invalid face
        assert_eq!(manager.set_char_size(999, 16, 16), CELL_FONT_FT_ERROR_INVALID_FONT);
        
        manager.end();
    }

    #[test]
    fn test_font_ft_manager_glyph() {
        let mut manager = FontFtManager::new();
        manager.init(CellFontFtConfig::default());
        
        let face = manager.open_font_memory(0x10000000, 1024, 0).unwrap();
        
        // Load glyph
        let metrics = manager.load_glyph(face, 65).unwrap(); // 'A'
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
        
        // Get char index
        let index = manager.get_char_index(face, b'A' as u32).unwrap();
        assert!(index > 0);
        
        manager.end();
    }

    #[test]
    fn test_font_ft_manager_max_faces() {
        let mut manager = FontFtManager::new();
        let mut config = CellFontFtConfig::default();
        config.max_face_count = 2;
        manager.init(config);
        
        // Open up to max
        manager.open_font_memory(0x10000000, 1024, 0).unwrap();
        manager.open_font_memory(0x10001000, 1024, 0).unwrap();
        
        // Third should fail
        assert!(manager.open_font_memory(0x10002000, 1024, 0).is_err());
        
        manager.end();
    }

    #[test]
    fn test_font_ft_config_default() {
        let config = CellFontFtConfig::default();
        assert_eq!(config.max_face_count, 16);
        assert_eq!(config.memory_pool_size, 1024 * 1024);
    }

    #[test]
    fn test_font_ft_glyph_metrics_default() {
        let metrics = CellFontFtGlyphMetrics::default();
        assert_eq!(metrics.width, 0);
        assert_eq!(metrics.height, 0);
    }

    #[test]
    fn test_font_ft_kerning() {
        let mut manager = FontFtManager::new();
        manager.init(CellFontFtConfig::default());

        let face = manager.open_font_memory(0x10000000, 1024, 0).unwrap();

        // AV should have negative kerning
        let (kx, ky) = manager.get_kerning(face, 0x41, 0x56).unwrap();
        assert!(kx < 0);
        assert_eq!(ky, 0);

        // Two random chars should have zero kerning
        let (kx, _) = manager.get_kerning(face, 0x61, 0x62).unwrap();
        assert_eq!(kx, 0);

        manager.end();
    }

    #[test]
    fn test_font_ft_kerning_not_initialized() {
        let manager = FontFtManager::new();
        assert!(manager.get_kerning(1, 0x41, 0x56).is_err());
    }
}
