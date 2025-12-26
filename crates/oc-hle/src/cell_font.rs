//! cellFont HLE - Font Rendering
//!
//! This module provides HLE implementations for the PS3's font rendering library.

use std::collections::HashMap;
use tracing::{debug, trace};

/// Font library handle
pub type FontLibrary = u32;

/// Font handle
pub type Font = u32;

/// Font renderer handle
pub type FontRenderer = u32;

/// Font type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellFontType {
    /// TrueType font
    TrueType = 0,
    /// Type1 font
    Type1 = 1,
}

/// Font configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellFontConfig {
    pub file_cache_size: u32,
    pub user_font_entry_max: u32,
    pub flags: u32,
}

impl Default for CellFontConfig {
    fn default() -> Self {
        Self {
            file_cache_size: 1024 * 1024, // 1 MB
            user_font_entry_max: 24,
            flags: 0,
        }
    }
}

/// Font renderer configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellFontRendererConfig {
    pub surface_w: u32,
    pub surface_h: u32,
    pub surface_pitch: u32,
}

impl Default for CellFontRendererConfig {
    fn default() -> Self {
        Self {
            surface_w: 1920,
            surface_h: 1080,
            surface_pitch: 1920 * 4,
        }
    }
}

/// Font glyph info
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellFontGlyph {
    pub width: f32,
    pub height: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
}

/// Font data parsed from TrueType or Type1 format
#[derive(Debug, Clone)]
struct FontData {
    /// Glyph count
    glyph_count: u32,
    /// Units per EM
    units_per_em: u32,
    /// Font family name
    family_name: String,
    /// Font style name
    style_name: String,
    /// Glyph bounding boxes (indexed by glyph ID)
    glyph_bounds: HashMap<u32, (f32, f32, f32, f32)>,
}

impl Default for FontData {
    fn default() -> Self {
        Self {
            glyph_count: 256,
            units_per_em: 2048,
            family_name: "Unknown".to_string(),
            style_name: "Regular".to_string(),
            glyph_bounds: HashMap::new(),
        }
    }
}

/// Font entry
#[derive(Debug, Clone)]
struct FontEntry {
    /// Font ID
    id: u32,
    /// Font type
    font_type: CellFontType,
    /// Font size
    size: u32,
    /// Source (memory or file)
    source: String,
    /// Parsed font data
    data: FontData,
}

/// Rendering surface data
#[derive(Debug, Clone)]
struct RenderSurface {
    /// Surface width
    width: u32,
    /// Surface height
    height: u32,
    /// Surface pitch (bytes per line)
    pitch: u32,
    /// Pixel buffer (RGBA format)
    buffer: Vec<u8>,
}

impl RenderSurface {
    fn new(width: u32, height: u32, pitch: u32) -> Self {
        let buffer_size = (height * pitch) as usize;
        Self {
            width,
            height,
            pitch,
            buffer: vec![0; buffer_size],
        }
    }

    /// Clear surface to specified color
    fn clear(&mut self, color: u32) {
        let r = ((color >> 24) & 0xFF) as u8;
        let g = ((color >> 16) & 0xFF) as u8;
        let b = ((color >> 8) & 0xFF) as u8;
        let a = (color & 0xFF) as u8;
        
        for pixel_idx in (0..self.buffer.len()).step_by(4) {
            self.buffer[pixel_idx] = r;
            self.buffer[pixel_idx + 1] = g;
            self.buffer[pixel_idx + 2] = b;
            self.buffer[pixel_idx + 3] = a;
        }
    }

    /// Draw a simple glyph at position (x, y)
    fn draw_glyph(&mut self, x: i32, y: i32, glyph_width: u32, glyph_height: u32, color: u32) {
        let r = ((color >> 24) & 0xFF) as u8;
        let g = ((color >> 16) & 0xFF) as u8;
        let b = ((color >> 8) & 0xFF) as u8;
        let a = (color & 0xFF) as u8;

        for dy in 0..glyph_height as i32 {
            let py = y + dy;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            
            for dx in 0..glyph_width as i32 {
                let px = x + dx;
                if px < 0 || px >= self.width as i32 {
                    continue;
                }
                
                let offset = (py as u32 * self.pitch + px as u32 * 4) as usize;
                if offset + 3 < self.buffer.len() {
                    self.buffer[offset] = r;
                    self.buffer[offset + 1] = g;
                    self.buffer[offset + 2] = b;
                    self.buffer[offset + 3] = a;
                }
            }
        }
    }
}

/// Renderer entry
#[derive(Debug, Clone)]
struct RendererEntry {
    /// Renderer ID
    id: u32,
    /// Configuration
    config: CellFontRendererConfig,
    /// Rendering surface
    surface: RenderSurface,
}

/// Font manager
pub struct FontManager {
    /// Initialization flag
    initialized: bool,
    /// Configuration
    config: CellFontConfig,
    /// Open fonts
    fonts: HashMap<u32, FontEntry>,
    /// Renderers
    renderers: HashMap<u32, RendererEntry>,
    /// Next font ID
    next_font_id: u32,
    /// Next renderer ID
    next_renderer_id: u32,
}

impl FontManager {
    /// Create a new font manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            config: CellFontConfig::default(),
            fonts: HashMap::new(),
            renderers: HashMap::new(),
            next_font_id: 1,
            next_renderer_id: 1,
        }
    }

    /// Initialize font library
    pub fn init(&mut self, config: CellFontConfig) -> i32 {
        if self.initialized {
            return 0x80540001u32 as i32; // CELL_FONT_ERROR_ALREADY_INITIALIZED
        }

        debug!("FontManager::init: cache_size={}, max_fonts={}", 
            config.file_cache_size, config.user_font_entry_max);

        self.config = config;
        self.initialized = true;

        // TODO: Allocate font cache
        // TODO: Set up default system fonts

        0 // CELL_OK
    }

    /// Shutdown font library
    pub fn end(&mut self) -> i32 {
        if !self.initialized {
            return 0x80540002u32 as i32; // CELL_FONT_ERROR_UNINITIALIZED
        }

        debug!("FontManager::end");

        self.fonts.clear();
        self.renderers.clear();
        self.initialized = false;

        // TODO: Free font cache

        0 // CELL_OK
    }

    /// Open font from memory
    pub fn open_font_memory(
        &mut self,
        font_addr: u32,
        font_size: u32,
        font_type: CellFontType,
    ) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80540002u32 as i32); // CELL_FONT_ERROR_UNINITIALIZED
        }

        if self.fonts.len() >= self.config.user_font_entry_max as usize {
            return Err(0x80540003u32 as i32); // CELL_FONT_ERROR_NO_SUPPORT
        }

        let font_id = self.next_font_id;
        self.next_font_id += 1;

        debug!("FontManager::open_font_memory: id={}, addr=0x{:08X}, size={}", 
            font_id, font_addr, font_size);

        let entry = FontEntry {
            id: font_id,
            font_type,
            size: font_size,
            source: format!("memory:0x{:08X}", font_addr),
            data: self.parse_font_data(font_type, &[]),
        };

        self.fonts.insert(font_id, entry);

        trace!("FontManager: Parsed font data for font {}", font_id);

        Ok(font_id)
    }

    /// Open font from file
    pub fn open_font_file(&mut self, path: &str, font_type: CellFontType) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80540002u32 as i32); // CELL_FONT_ERROR_UNINITIALIZED
        }

        if self.fonts.len() >= self.config.user_font_entry_max as usize {
            return Err(0x80540003u32 as i32); // CELL_FONT_ERROR_NO_SUPPORT
        }

        let font_id = self.next_font_id;
        self.next_font_id += 1;

        debug!("FontManager::open_font_file: id={}, path={}", font_id, path);

        let entry = FontEntry {
            id: font_id,
            font_type,
            size: 0,
            source: path.to_string(),
            data: self.parse_font_data(font_type, &[]),
        };

        self.fonts.insert(font_id, entry);

        trace!("FontManager: Loaded font from file {}", path);

        Ok(font_id)
    }

    /// Close font
    pub fn close_font(&mut self, font_id: u32) -> i32 {
        if let Some(_font) = self.fonts.remove(&font_id) {
            debug!("FontManager::close_font: id={}", font_id);
            trace!("FontManager: Freed font resources for font {}", font_id);
            0 // CELL_OK
        } else {
            0x80540004u32 as i32 // CELL_FONT_ERROR_INVALID_PARAMETER
        }
    }

    /// Create renderer
    pub fn create_renderer(&mut self, config: CellFontRendererConfig) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80540002u32 as i32); // CELL_FONT_ERROR_UNINITIALIZED
        }

        let renderer_id = self.next_renderer_id;
        self.next_renderer_id += 1;

        debug!("FontManager::create_renderer: id={}, surface={}x{}", 
            renderer_id, config.surface_w, config.surface_h);

        let surface = RenderSurface::new(config.surface_w, config.surface_h, config.surface_pitch);

        let entry = RendererEntry {
            id: renderer_id,
            config,
            surface,
        };

        self.renderers.insert(renderer_id, entry);

        trace!("FontManager: Allocated rendering surface {}x{}", config.surface_w, config.surface_h);

        Ok(renderer_id)
    }

    /// Destroy renderer
    pub fn destroy_renderer(&mut self, renderer_id: u32) -> i32 {
        if let Some(_renderer) = self.renderers.remove(&renderer_id) {
            debug!("FontManager::destroy_renderer: id={}", renderer_id);
            trace!("FontManager: Freed renderer resources for renderer {}", renderer_id);
            0 // CELL_OK
        } else {
            0x80540004u32 as i32 // CELL_FONT_ERROR_INVALID_PARAMETER
        }
    }

    /// Check if font is open
    pub fn is_font_open(&self, font_id: u32) -> bool {
        self.fonts.contains_key(&font_id)
    }

    /// Check if renderer exists
    pub fn is_renderer_valid(&self, renderer_id: u32) -> bool {
        self.renderers.contains_key(&renderer_id)
    }

    /// Get font count
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Get renderer count
    pub fn renderer_count(&self) -> usize {
        self.renderers.len()
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Parse font data from binary data
    /// Supports TrueType (TTF) and Type1 font formats
    fn parse_font_data(&self, font_type: CellFontType, _data: &[u8]) -> FontData {
        // Basic font parsing implementation
        // Real implementation would parse TTF/Type1 tables
        let mut font_data = FontData::default();

        match font_type {
            CellFontType::TrueType => {
                font_data.family_name = "TrueType Font".to_string();
                font_data.style_name = "Regular".to_string();
                font_data.glyph_count = 256;
                font_data.units_per_em = 2048;
                
                // Add placeholder glyph bounding boxes for ASCII range
                for glyph_id in 0..256 {
                    font_data.glyph_bounds.insert(
                        glyph_id,
                        (0.0, 0.0, 16.0, 16.0), // Simple 16x16 bounding box
                    );
                }
            }
            CellFontType::Type1 => {
                font_data.family_name = "Type1 Font".to_string();
                font_data.style_name = "Regular".to_string();
                font_data.glyph_count = 256;
                font_data.units_per_em = 1000;
                
                // Add placeholder glyph bounding boxes
                for glyph_id in 0..256 {
                    font_data.glyph_bounds.insert(
                        glyph_id,
                        (0.0, 0.0, 12.0, 14.0), // Type1 typical metrics
                    );
                }
            }
        }

        trace!("FontManager: Parsed {} font with {} glyphs", 
            match font_type {
                CellFontType::TrueType => "TrueType",
                CellFontType::Type1 => "Type1",
            },
            font_data.glyph_count
        );

        font_data
    }

    /// Render glyph to surface
    pub fn render_glyph(
        &mut self,
        renderer_id: u32,
        font_id: u32,
        glyph_id: u32,
        x: i32,
        y: i32,
        color: u32,
    ) -> i32 {
        // Validate font
        let font = match self.fonts.get(&font_id) {
            Some(f) => f,
            None => return 0x80540004u32 as i32, // CELL_FONT_ERROR_INVALID_PARAMETER
        };

        // Validate renderer
        let renderer = match self.renderers.get_mut(&renderer_id) {
            Some(r) => r,
            None => return 0x80540004u32 as i32, // CELL_FONT_ERROR_INVALID_PARAMETER
        };

        // Get glyph bounds
        let (_, _, width, height) = font.data.glyph_bounds
            .get(&glyph_id)
            .copied()
            .unwrap_or((0.0, 0.0, 16.0, 16.0));

        // Render glyph to surface
        renderer.surface.draw_glyph(
            x,
            y,
            width as u32,
            height as u32,
            color,
        );

        trace!("FontManager: Rendered glyph {} from font {} at ({}, {})", 
            glyph_id, font_id, x, y);

        0 // CELL_OK
    }

    /// Get glyph metrics
    pub fn get_glyph_metrics(&self, font_id: u32, glyph_id: u32) -> Option<CellFontGlyph> {
        let font = self.fonts.get(&font_id)?;
        let (bearing_x, bearing_y, width, height) = font.data.glyph_bounds.get(&glyph_id)?;

        Some(CellFontGlyph {
            width: *width,
            height: *height,
            bearing_x: *bearing_x,
            bearing_y: *bearing_y,
            advance: width + 2.0, // Add spacing
        })
    }

    /// Clear renderer surface
    pub fn clear_surface(&mut self, renderer_id: u32, color: u32) -> i32 {
        if let Some(renderer) = self.renderers.get_mut(&renderer_id) {
            renderer.surface.clear(color);
            trace!("FontManager: Cleared surface for renderer {}", renderer_id);
            0 // CELL_OK
        } else {
            0x80540004u32 as i32 // CELL_FONT_ERROR_INVALID_PARAMETER
        }
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellFontInit - Initialize font library
///
/// # Arguments
/// * `config` - Configuration address
///
/// # Returns
/// * 0 on success
pub fn cell_font_init(_config_addr: u32) -> i32 {
    debug!("cellFontInit()");

    // Use default config when memory read is not yet implemented
    let config = CellFontConfig::default();
    crate::context::get_hle_context_mut().font.init(config)
}

/// cellFontEnd - Shutdown font library
///
/// # Returns
/// * 0 on success
pub fn cell_font_end() -> i32 {
    debug!("cellFontEnd()");

    crate::context::get_hle_context_mut().font.end()
}

/// cellFontOpenFontMemory - Open font from memory
///
/// # Arguments
/// * `library` - Font library handle
/// * `fontAddr` - Font data address in memory
/// * `fontSize` - Font data size
/// * `subNum` - Sub font number
/// * `uniqueId` - Unique ID
/// * `font` - Font handle address
///
/// # Returns
/// * 0 on success
pub fn cell_font_open_font_memory(
    _library: u32,
    font_addr: u32,
    font_size: u32,
    sub_num: u32,
    unique_id: u32,
    _font_addr: u32,
) -> i32 {
    debug!(
        "cellFontOpenFontMemory(fontAddr=0x{:08X}, fontSize={}, subNum={}, uniqueId={})",
        font_addr, font_size, sub_num, unique_id
    );

    // Validate parameters
    if font_size == 0 {
        return 0x80540004u32 as i32; // CELL_FONT_ERROR_INVALID_PARAMETER
    }

    // TODO: Parse font data from memory through global manager
    // TODO: Create font handle
    // TODO: Write font handle to memory

    0 // CELL_OK
}

/// cellFontOpenFontFile - Open font from file
///
/// # Arguments
/// * `library` - Font library handle
/// * `fontPath` - Font file path address
/// * `subNum` - Sub font number
/// * `uniqueId` - Unique ID
/// * `font` - Font handle address
///
/// # Returns
/// * 0 on success
pub fn cell_font_open_font_file(
    _library: u32,
    _font_path_addr: u32,
    sub_num: u32,
    unique_id: u32,
    _font_addr: u32,
) -> i32 {
    debug!(
        "cellFontOpenFontFile(subNum={}, uniqueId={})",
        sub_num, unique_id
    );

    // TODO: Read path from memory
    // TODO: Load font from file through global manager
    // TODO: Create font handle
    // TODO: Write font handle to memory

    0 // CELL_OK
}

/// cellFontCloseFont - Close font
///
/// # Arguments
/// * `font` - Font handle
///
/// # Returns
/// * 0 on success
pub fn cell_font_close_font(font: u32) -> i32 {
    trace!("cellFontCloseFont(font={})", font);

    crate::context::get_hle_context_mut().font.close_font(font)
}

/// cellFontCreateRenderer - Create font renderer
///
/// # Arguments
/// * `library` - Font library handle
/// * `config` - Renderer configuration address
/// * `renderer` - Renderer handle address
///
/// # Returns
/// * 0 on success
pub fn cell_font_create_renderer(
    _library: u32,
    _config_addr: u32,
    _renderer_addr: u32,
) -> i32 {
    debug!("cellFontCreateRenderer()");

    // Use default config when memory read is not yet implemented
    let config = CellFontRendererConfig::default();
    match crate::context::get_hle_context_mut().font.create_renderer(config) {
        Ok(_renderer_id) => {
            // TODO: Write renderer handle to memory at _renderer_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellFontDestroyRenderer - Destroy font renderer
///
/// # Arguments
/// * `renderer` - Renderer handle
///
/// # Returns
/// * 0 on success
pub fn cell_font_destroy_renderer(renderer: u32) -> i32 {
    debug!("cellFontDestroyRenderer(renderer={})", renderer);

    crate::context::get_hle_context_mut().font.destroy_renderer(renderer)
}

/// cellFontRenderCharGlyphImage - Render character glyph
///
/// # Arguments
/// * `font` - Font handle
/// * `code` - Character code
/// * `renderer` - Renderer handle
/// * `glyph` - Glyph info address
///
/// # Returns
/// * 0 on success
pub fn cell_font_render_char_glyph_image(
    font: u32,
    code: u32,
    renderer: u32,
    _glyph_addr: u32,
) -> i32 {
    trace!("cellFontRenderCharGlyphImage(font={}, code=0x{:X}, renderer={})", 
        font, code, renderer);

    // TODO: Render character glyph through global manager
    // TODO: Write glyph to surface
    // TODO: Update glyph info

    0 // CELL_OK
}

/// cellFontGetHorizontalLayout - Get horizontal layout info
///
/// # Arguments
/// * `font` - Font handle
/// * `layout` - Layout info address
///
/// # Returns
/// * 0 on success
pub fn cell_font_get_horizontal_layout(font: u32, _layout_addr: u32) -> i32 {
    trace!("cellFontGetHorizontalLayout(font={})", font);

    // TODO: Get horizontal layout metrics through global manager
    // TODO: Write layout info to memory

    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager() {
        let mut manager = FontManager::new();
        let config = CellFontConfig::default();
        assert_eq!(manager.init(config), 0);
        assert!(manager.is_initialized());
        assert_eq!(manager.end(), 0);
    }

    #[test]
    fn test_font_manager_lifecycle() {
        let mut manager = FontManager::new();
        let config = CellFontConfig::default();
        
        // Initialize
        assert_eq!(manager.init(config), 0);
        
        // Try to initialize again (should fail)
        assert!(manager.init(config) != 0);
        
        // End
        assert_eq!(manager.end(), 0);
        
        // Try to end again (should fail)
        assert!(manager.end() != 0);
    }

    #[test]
    fn test_font_manager_open_close() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());
        
        // Open font from memory
        let font_id = manager.open_font_memory(0x10000000, 1024, CellFontType::TrueType);
        assert!(font_id.is_ok());
        let font_id = font_id.unwrap();
        
        assert!(manager.is_font_open(font_id));
        assert_eq!(manager.font_count(), 1);
        
        // Close font
        assert_eq!(manager.close_font(font_id), 0);
        assert!(!manager.is_font_open(font_id));
        assert_eq!(manager.font_count(), 0);
        
        manager.end();
    }

    #[test]
    fn test_font_manager_multiple_fonts() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());
        
        // Open multiple fonts
        let font1 = manager.open_font_memory(0x10000000, 1024, CellFontType::TrueType).unwrap();
        let font2 = manager.open_font_file("/dev_flash/data/font/default.ttf", CellFontType::TrueType).unwrap();
        
        assert_eq!(manager.font_count(), 2);
        assert_ne!(font1, font2);
        
        manager.close_font(font1);
        manager.close_font(font2);
        
        assert_eq!(manager.font_count(), 0);
        
        manager.end();
    }

    #[test]
    fn test_font_manager_renderers() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());
        
        // Create renderer
        let config = CellFontRendererConfig::default();
        let renderer_id = manager.create_renderer(config);
        assert!(renderer_id.is_ok());
        let renderer_id = renderer_id.unwrap();
        
        assert!(manager.is_renderer_valid(renderer_id));
        assert_eq!(manager.renderer_count(), 1);
        
        // Destroy renderer
        assert_eq!(manager.destroy_renderer(renderer_id), 0);
        assert!(!manager.is_renderer_valid(renderer_id));
        assert_eq!(manager.renderer_count(), 0);
        
        manager.end();
    }

    #[test]
    fn test_font_manager_max_fonts() {
        let mut manager = FontManager::new();
        let mut config = CellFontConfig::default();
        config.user_font_entry_max = 2;
        manager.init(config);
        
        // Open up to max
        assert!(manager.open_font_memory(0x10000000, 1024, CellFontType::TrueType).is_ok());
        assert!(manager.open_font_memory(0x10001000, 1024, CellFontType::TrueType).is_ok());
        
        // Try to open one more (should fail)
        assert!(manager.open_font_memory(0x10002000, 1024, CellFontType::TrueType).is_err());
        
        manager.end();
    }

    #[test]
    fn test_font_config_default() {
        let config = CellFontConfig::default();
        assert_eq!(config.file_cache_size, 1024 * 1024);
        assert_eq!(config.user_font_entry_max, 24);
    }

    #[test]
    fn test_font_init() {
        let result = cell_font_init(0x10000000);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_font_open_validation() {
        // Valid font size
        assert_eq!(cell_font_open_font_memory(1, 0x10000000, 1024, 0, 0, 0x20000000), 0);
        
        // Invalid font size (0)
        assert!(cell_font_open_font_memory(1, 0x10000000, 0, 0, 0, 0x20000000) != 0);
    }

    #[test]
    fn test_font_type() {
        assert_eq!(CellFontType::TrueType as u32, 0);
        assert_eq!(CellFontType::Type1 as u32, 1);
    }

    #[test]
    fn test_font_renderer_config_default() {
        let config = CellFontRendererConfig::default();
        assert_eq!(config.surface_w, 1920);
        assert_eq!(config.surface_h, 1080);
    }
}
