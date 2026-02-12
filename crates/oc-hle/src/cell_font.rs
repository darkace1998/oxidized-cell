//! cellFont HLE - Font Rendering
//!
//! This module provides HLE implementations for the PS3's font rendering library.

use std::collections::HashMap;
use tracing::{debug, trace};
use crate::memory::{write_be32, read_bytes};

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
    /// Unicode codepoint → glyph index mapping (cmap)
    cmap: HashMap<u32, u32>,
    /// Kerning pairs: (left_glyph, right_glyph) → x_advance adjustment
    kerning: HashMap<(u32, u32), i16>,
}

impl Default for FontData {
    fn default() -> Self {
        Self {
            glyph_count: 256,
            units_per_em: 2048,
            family_name: "Unknown".to_string(),
            style_name: "Regular".to_string(),
            glyph_bounds: HashMap::new(),
            cmap: HashMap::new(),
            kerning: HashMap::new(),
        }
    }
}

/// Font entry
#[allow(dead_code)]
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
#[allow(dead_code)]
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

        // Allocate font cache (simulated via Rust HashMap)
        // Set up default system fonts (font entries ready to be loaded)
        trace!("FontManager: Allocated font cache of {} bytes", config.file_cache_size);
        trace!("FontManager: Set up default system fonts capacity of {}", config.user_font_entry_max);

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

        // Free font cache (automatic in Rust)
        trace!("FontManager: Freed font cache");

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

        // Parse font data from memory
        let font_data_bytes = match read_bytes(font_addr, font_size.min(1024)) {
            Ok(data) => data,
            Err(_) => Vec::new(), // Fallback to empty data if memory read fails
        };

        let entry = FontEntry {
            id: font_id,
            font_type,
            size: font_size,
            source: format!("memory:0x{:08X}", font_addr),
            data: self.parse_font_data(font_type, &font_data_bytes),
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
    /// Supports TrueType (TTF) and Type1 font formats.
    /// Parses the offset table, then walks the table directory to find
    /// 'head', 'name', 'maxp', 'cmap', and 'kern' tables.
    fn parse_font_data(&self, font_type: CellFontType, data: &[u8]) -> FontData {
        let mut font_data = FontData::default();

        match font_type {
            CellFontType::TrueType => {
                if data.len() >= 12 {
                    // --- Offset Table ---
                    let sfnt_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
                    trace!("FontManager: TrueType sfnt=0x{:08X}, tables={}", sfnt_version, num_tables);

                    // --- Walk Table Directory (starts at offset 12) ---
                    let mut head_offset: Option<usize> = None;
                    let mut head_length: usize = 0;
                    let mut maxp_offset: Option<usize> = None;
                    let mut name_offset: Option<usize> = None;
                    let mut name_length: usize = 0;
                    let mut cmap_offset: Option<usize> = None;
                    let mut cmap_length: usize = 0;
                    let mut kern_offset: Option<usize> = None;
                    let mut kern_length: usize = 0;

                    for i in 0..num_tables {
                        let entry_start = 12 + i * 16;
                        if entry_start + 16 > data.len() { break; }
                        let tag = &data[entry_start..entry_start + 4];
                        let offset = u32::from_be_bytes([
                            data[entry_start + 8], data[entry_start + 9],
                            data[entry_start + 10], data[entry_start + 11],
                        ]) as usize;
                        let length = u32::from_be_bytes([
                            data[entry_start + 12], data[entry_start + 13],
                            data[entry_start + 14], data[entry_start + 15],
                        ]) as usize;

                        match tag {
                            b"head" => { head_offset = Some(offset); head_length = length; }
                            b"maxp" => { maxp_offset = Some(offset); }
                            b"name" => { name_offset = Some(offset); name_length = length; }
                            b"cmap" => { cmap_offset = Some(offset); cmap_length = length; }
                            b"kern" => { kern_offset = Some(offset); kern_length = length; }
                            _ => {}
                        }
                    }

                    // --- 'head' table: unitsPerEm (at offset 18 inside table) ---
                    if let Some(off) = head_offset {
                        if off + head_length.min(54) <= data.len() && head_length >= 54 {
                            font_data.units_per_em =
                                u16::from_be_bytes([data[off + 18], data[off + 19]]) as u32;
                            trace!("FontManager: head.unitsPerEm = {}", font_data.units_per_em);
                        }
                    }

                    // --- 'maxp' table: numGlyphs (at offset 4 inside table) ---
                    if let Some(off) = maxp_offset {
                        if off + 6 <= data.len() {
                            font_data.glyph_count =
                                u16::from_be_bytes([data[off + 4], data[off + 5]]) as u32;
                            trace!("FontManager: maxp.numGlyphs = {}", font_data.glyph_count);
                        }
                    }

                    // --- 'name' table: family + style names ---
                    if let Some(off) = name_offset {
                        Self::parse_name_table(data, off, name_length, &mut font_data);
                    }

                    // --- 'cmap' table: Unicode → glyph mapping ---
                    if let Some(off) = cmap_offset {
                        Self::parse_cmap_table(data, off, cmap_length, &mut font_data);
                    }

                    // --- 'kern' table: kerning pairs ---
                    if let Some(off) = kern_offset {
                        Self::parse_kern_table(data, off, kern_length, &mut font_data);
                    }
                } else {
                    font_data.units_per_em = 2048;
                    font_data.glyph_count = 256;
                }

                // Fill family/style defaults if name table was empty
                if font_data.family_name == "Unknown" {
                    font_data.family_name = "TrueType Font".to_string();
                }
                if font_data.style_name == "Regular" || font_data.style_name.is_empty() {
                    font_data.style_name = "Regular".to_string();
                }

                // Build default cmap (identity for ASCII) when no cmap table was found
                if font_data.cmap.is_empty() {
                    for cp in 0x20u32..0x7F {
                        font_data.cmap.insert(cp, cp);
                    }
                }

                // Populate placeholder glyph bounds for all known glyphs
                for glyph_id in 0..font_data.glyph_count.min(65536) {
                    font_data.glyph_bounds.entry(glyph_id).or_insert(
                        (0.0, 0.0, 16.0, 16.0),
                    );
                }
            }
            CellFontType::Type1 => {
                if data.len() >= 2 && data[0] == 0x80 && data[1] == 0x01 {
                    trace!("FontManager: Type1 PFB format detected");
                }

                font_data.family_name = "Type1 Font".to_string();
                font_data.style_name = "Regular".to_string();
                font_data.glyph_count = 256;
                font_data.units_per_em = 1000;

                for glyph_id in 0..256 {
                    font_data.glyph_bounds.insert(glyph_id, (0.0, 0.0, 12.0, 14.0));
                }

                // Identity cmap for ASCII
                for cp in 0x20u32..0x7F {
                    font_data.cmap.insert(cp, cp);
                }
            }
        }

        trace!("FontManager: Parsed {} font with {} glyphs, {} cmap entries, {} kern pairs",
            match font_type { CellFontType::TrueType => "TrueType", CellFontType::Type1 => "Type1" },
            font_data.glyph_count,
            font_data.cmap.len(),
            font_data.kerning.len(),
        );

        font_data
    }

    /// Parse the TrueType 'name' table for family and style names
    fn parse_name_table(data: &[u8], off: usize, length: usize, font_data: &mut FontData) {
        let end = (off + length).min(data.len());
        if off + 6 > end { return; }

        let count = u16::from_be_bytes([data[off + 2], data[off + 3]]) as usize;
        let string_offset = u16::from_be_bytes([data[off + 4], data[off + 5]]) as usize;
        let storage_base = off + string_offset;

        for i in 0..count {
            let rec = off + 6 + i * 12;
            if rec + 12 > end { break; }

            let _platform = u16::from_be_bytes([data[rec], data[rec + 1]]);
            let _encoding = u16::from_be_bytes([data[rec + 2], data[rec + 3]]);
            let _language = u16::from_be_bytes([data[rec + 4], data[rec + 5]]);
            let name_id = u16::from_be_bytes([data[rec + 6], data[rec + 7]]);
            let str_len = u16::from_be_bytes([data[rec + 8], data[rec + 9]]) as usize;
            let str_off = u16::from_be_bytes([data[rec + 10], data[rec + 11]]) as usize;

            let abs_off = storage_base + str_off;
            if abs_off + str_len > data.len() { continue; }
            let raw = &data[abs_off..abs_off + str_len];

            // Try UTF-16BE first (platform 3), fall back to ASCII
            let text = if _platform == 3 && raw.len() >= 2 {
                let chars: Vec<u16> = raw.chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&chars)
            } else {
                String::from_utf8_lossy(raw).to_string()
            };

            if text.is_empty() { continue; }

            match name_id {
                1 => {
                    font_data.family_name = text;
                    trace!("FontManager: name.family = {}", font_data.family_name);
                }
                2 => {
                    font_data.style_name = text;
                    trace!("FontManager: name.style = {}", font_data.style_name);
                }
                _ => {}
            }
        }
    }

    /// Parse the TrueType 'cmap' table to build Unicode→glyph mapping
    fn parse_cmap_table(data: &[u8], off: usize, length: usize, font_data: &mut FontData) {
        let end = (off + length).min(data.len());
        if off + 4 > end { return; }

        let num_sub = u16::from_be_bytes([data[off + 2], data[off + 3]]) as usize;

        // Walk sub-tables, prefer platform 3 (Windows) encoding 1 (Unicode BMP)
        let mut best_subtable_off: Option<usize> = None;
        for i in 0..num_sub {
            let rec = off + 4 + i * 8;
            if rec + 8 > end { break; }
            let platform = u16::from_be_bytes([data[rec], data[rec + 1]]);
            let encoding = u16::from_be_bytes([data[rec + 2], data[rec + 3]]);
            let sub_off = u32::from_be_bytes([data[rec + 4], data[rec + 5], data[rec + 6], data[rec + 7]]) as usize;
            if (platform == 3 && encoding == 1) || (platform == 0) {
                best_subtable_off = Some(off + sub_off);
                if platform == 3 { break; } // prefer Windows table
            }
        }

        if let Some(sub) = best_subtable_off {
            if sub + 2 > data.len() { return; }
            let format = u16::from_be_bytes([data[sub], data[sub + 1]]);

            match format {
                4 => Self::parse_cmap_format4(data, sub, font_data),
                0 => Self::parse_cmap_format0(data, sub, font_data),
                _ => {
                    trace!("FontManager: unsupported cmap format {}, falling back to identity", format);
                }
            }
        }
    }

    /// Parse cmap format 0 (byte encoding)
    fn parse_cmap_format0(data: &[u8], off: usize, font_data: &mut FontData) {
        if off + 6 + 256 > data.len() { return; }
        for cp in 0u32..256 {
            let glyph_id = data[off + 6 + cp as usize] as u32;
            if glyph_id != 0 {
                font_data.cmap.insert(cp, glyph_id);
            }
        }
    }

    /// Parse cmap format 4 (segment mapping to delta values)
    fn parse_cmap_format4(data: &[u8], off: usize, font_data: &mut FontData) {
        if off + 14 > data.len() { return; }
        let seg_count = u16::from_be_bytes([data[off + 6], data[off + 7]]) as usize / 2;
        if seg_count == 0 { return; }

        let end_codes = off + 14;
        let start_codes = end_codes + seg_count * 2 + 2; // +2 for reservedPad
        let deltas = start_codes + seg_count * 2;
        let offsets = deltas + seg_count * 2;

        if offsets + seg_count * 2 > data.len() { return; }

        for seg in 0..seg_count {
            let ec = u16::from_be_bytes([data[end_codes + seg * 2], data[end_codes + seg * 2 + 1]]) as u32;
            let sc = u16::from_be_bytes([data[start_codes + seg * 2], data[start_codes + seg * 2 + 1]]) as u32;
            let delta = i16::from_be_bytes([data[deltas + seg * 2], data[deltas + seg * 2 + 1]]) as i32;
            let range_off = u16::from_be_bytes([data[offsets + seg * 2], data[offsets + seg * 2 + 1]]) as usize;

            if sc == 0xFFFF { break; }

            for cp in sc..=ec {
                let glyph_id = if range_off == 0 {
                    ((cp as i32 + delta) & 0xFFFF) as u32
                } else {
                    let idx = offsets + seg * 2 + range_off + (cp - sc) as usize * 2;
                    if idx + 2 <= data.len() {
                        let gid = u16::from_be_bytes([data[idx], data[idx + 1]]) as u32;
                        if gid != 0 { ((gid as i32 + delta) & 0xFFFF) as u32 } else { 0 }
                    } else {
                        0
                    }
                };
                if glyph_id != 0 {
                    font_data.cmap.insert(cp, glyph_id);
                }
            }
        }
    }

    /// Parse the TrueType 'kern' table (format 0) for kerning pairs
    fn parse_kern_table(data: &[u8], off: usize, length: usize, font_data: &mut FontData) {
        let end = (off + length).min(data.len());
        if off + 4 > end { return; }

        let _version = u16::from_be_bytes([data[off], data[off + 1]]);
        let num_subtables = u16::from_be_bytes([data[off + 2], data[off + 3]]) as usize;
        let mut pos = off + 4;

        for _ in 0..num_subtables {
            if pos + 6 > end { break; }
            let _sub_version = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let sub_length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            let coverage = u16::from_be_bytes([data[pos + 4], data[pos + 5]]);
            let format = (coverage >> 8) & 0xFF;

            if format == 0 && pos + 8 <= end {
                // Kern format 0
                let n_pairs = u16::from_be_bytes([data[pos + 6], data[pos + 7]]) as usize;
                let pairs_start = pos + 14; // skip header
                for p in 0..n_pairs {
                    let pair_off = pairs_start + p * 6;
                    if pair_off + 6 > end { break; }
                    let left = u16::from_be_bytes([data[pair_off], data[pair_off + 1]]) as u32;
                    let right = u16::from_be_bytes([data[pair_off + 2], data[pair_off + 3]]) as u32;
                    let value = i16::from_be_bytes([data[pair_off + 4], data[pair_off + 5]]);
                    font_data.kerning.insert((left, right), value);
                }
                trace!("FontManager: kern format 0 loaded {} pairs", n_pairs);
            }

            pos += sub_length.max(6);
        }
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

    // ========================================================================
    // Unicode Codepoint → Glyph Mapping
    // ========================================================================

    /// Map a Unicode codepoint to a glyph index for the given font
    pub fn get_glyph_index(&self, font_id: u32, codepoint: u32) -> Option<u32> {
        let font = self.fonts.get(&font_id)?;
        font.data.cmap.get(&codepoint).copied()
    }

    /// Get the number of cmap entries for a font
    pub fn get_cmap_entry_count(&self, font_id: u32) -> usize {
        self.fonts.get(&font_id)
            .map(|f| f.data.cmap.len())
            .unwrap_or(0)
    }

    // ========================================================================
    // Kerning
    // ========================================================================

    /// Get the kerning value between two glyphs
    ///
    /// Returns the horizontal advance adjustment in font units.
    pub fn get_kerning(&self, font_id: u32, left_glyph: u32, right_glyph: u32) -> Option<i16> {
        let font = self.fonts.get(&font_id)?;
        font.data.kerning.get(&(left_glyph, right_glyph)).copied()
    }

    /// Get the number of kerning pairs for a font
    pub fn get_kerning_pair_count(&self, font_id: u32) -> usize {
        self.fonts.get(&font_id)
            .map(|f| f.data.kerning.len())
            .unwrap_or(0)
    }

    // ========================================================================
    // System Font Loading
    // ========================================================================

    /// Known PS3 system font paths under /dev_flash/data/font/
    pub const SYSTEM_FONT_PATHS: &'static [&'static str] = &[
        "/dev_flash/data/font/SCE-PS3-RD-R-LATIN.TTF",
        "/dev_flash/data/font/SCE-PS3-RD-B-LATIN.TTF",
        "/dev_flash/data/font/SCE-PS3-RD-L-LATIN.TTF",
        "/dev_flash/data/font/SCE-PS3-NR-R-JPN.TTF",
        "/dev_flash/data/font/SCE-PS3-NR-B-JPN.TTF",
        "/dev_flash/data/font/SCE-PS3-NR-L-JPN.TTF",
        "/dev_flash/data/font/SCE-PS3-YG-R-KOR.TTF",
        "/dev_flash/data/font/SCE-PS3-DH-R-CGB.TTF",
        "/dev_flash/data/font/SCE-PS3-CP-R-KANA.TTF",
    ];

    /// Open a system font by its well-known path
    ///
    /// System fonts reside under `/dev_flash/data/font/`.  In HLE mode
    /// we create a font entry with placeholder data so callers can
    /// query metrics and render glyphs using the default metrics.
    pub fn open_system_font(&mut self, path: &str) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80540002u32 as i32); // CELL_FONT_ERROR_UNINITIALIZED
        }

        if self.fonts.len() >= self.config.user_font_entry_max as usize {
            return Err(0x80540003u32 as i32); // CELL_FONT_ERROR_NO_SUPPORT
        }

        debug!("FontManager::open_system_font: {}", path);

        // Derive a meaningful family name from the path
        let family = path.rsplit('/').next().unwrap_or("SystemFont")
            .trim_end_matches(".TTF")
            .trim_end_matches(".ttf");

        let font_id = self.next_font_id;
        self.next_font_id += 1;

        let mut font_data = FontData::default();
        font_data.family_name = family.to_string();
        font_data.glyph_count = 65535; // system fonts support large Unicode ranges
        font_data.units_per_em = 2048;

        // Populate identity cmap for BMP
        for cp in 0x20u32..0x7F {
            font_data.cmap.insert(cp, cp);
        }
        // CJK Unified Ideographs (representative range)
        for cp in 0x4E00u32..0x4E80 {
            font_data.cmap.insert(cp, cp);
        }
        // Katakana
        for cp in 0x30A0u32..0x3100 {
            font_data.cmap.insert(cp, cp);
        }
        // Hangul Syllables (representative range)
        for cp in 0xAC00u32..0xAC80 {
            font_data.cmap.insert(cp, cp);
        }

        // Placeholder glyph bounds
        for (_, &gid) in font_data.cmap.iter() {
            font_data.glyph_bounds.entry(gid).or_insert((0.0, 0.0, 16.0, 16.0));
        }

        let entry = FontEntry {
            id: font_id,
            font_type: CellFontType::TrueType,
            size: 0,
            source: path.to_string(),
            data: font_data,
        };

        self.fonts.insert(font_id, entry);

        Ok(font_id)
    }

    /// Get font family name
    pub fn get_font_family(&self, font_id: u32) -> Option<&str> {
        self.fonts.get(&font_id).map(|f| f.data.family_name.as_str())
    }

    /// Get font style name
    pub fn get_font_style(&self, font_id: u32) -> Option<&str> {
        self.fonts.get(&font_id).map(|f| f.data.style_name.as_str())
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
    font_handle_addr: u32,
) -> i32 {
    debug!(
        "cellFontOpenFontMemory(fontAddr=0x{:08X}, fontSize={}, subNum={}, uniqueId={})",
        font_addr, font_size, sub_num, unique_id
    );

    // Validate parameters
    if font_size == 0 {
        return 0x80540004u32 as i32; // CELL_FONT_ERROR_INVALID_PARAMETER
    }

    // Parse font data from memory through global manager
    match crate::context::get_hle_context_mut().font.open_font_memory(
        font_addr,
        font_size,
        crate::cell_font::CellFontType::TrueType,
    ) {
        Ok(font_id) => {
            // Write font handle to memory
            if font_handle_addr != 0 {
                if let Err(e) = write_be32(font_handle_addr, font_id) {
                    debug!("cellFontOpenFontMemory: Failed to write font handle to memory: {}", e);
                    return e;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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
    font_path_addr: u32,
    sub_num: u32,
    unique_id: u32,
    font_handle_addr: u32,
) -> i32 {
    debug!(
        "cellFontOpenFontFile(subNum={}, uniqueId={})",
        sub_num, unique_id
    );

    // Read path from memory
    let font_path = match crate::memory::read_string(font_path_addr, 256) {
        Ok(p) => p,
        Err(_) => "/dev_flash/data/font/default.ttf".to_string(),
    };

    // Load font from file through global manager
    match crate::context::get_hle_context_mut().font.open_font_file(
        &font_path,
        crate::cell_font::CellFontType::TrueType,
    ) {
        Ok(font_id) => {
            // Write font handle to memory
            if font_handle_addr != 0 {
                if let Err(e) = write_be32(font_handle_addr, font_id) {
                    debug!("cellFontOpenFontFile: Failed to write font handle to memory: {}", e);
                    return e;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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
    renderer_addr: u32,
) -> i32 {
    debug!("cellFontCreateRenderer()");

    // Use default config when memory read is not yet implemented
    let config = CellFontRendererConfig::default();
    match crate::context::get_hle_context_mut().font.create_renderer(config) {
        Ok(renderer_id) => {
            // Write renderer handle to memory
            if renderer_addr != 0 {
                if let Err(e) = write_be32(renderer_addr, renderer_id) {
                    debug!("cellFontCreateRenderer: Failed to write renderer handle to memory: {}", e);
                    return e;
                }
            }
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
    glyph_addr: u32,
) -> i32 {
    trace!("cellFontRenderCharGlyphImage(font={}, code=0x{:X}, renderer={})", 
        font, code, renderer);

    // Render character glyph through global manager
    let result = crate::context::get_hle_context_mut().font.render_glyph(
        renderer,
        font,
        code,
        0, // x position
        0, // y position
        0xFFFFFFFF, // white color
    );

    if result != 0 {
        return result;
    }

    // Write glyph info to memory
    if glyph_addr != 0 {
        if let Some(glyph) = crate::context::get_hle_context().font.get_glyph_metrics(font, code) {
            // CellFontGlyph struct: width, height, bearing_x, bearing_y, advance (5 floats = 20 bytes)
            if let Err(e) = write_be32(glyph_addr, glyph.width as i32 as u32) { return e; }
            if let Err(e) = write_be32(glyph_addr + 4, glyph.height as i32 as u32) { return e; }
            if let Err(e) = write_be32(glyph_addr + 8, glyph.bearing_x as i32 as u32) { return e; }
            if let Err(e) = write_be32(glyph_addr + 12, glyph.bearing_y as i32 as u32) { return e; }
            if let Err(e) = write_be32(glyph_addr + 16, glyph.advance as i32 as u32) { return e; }
        }
    }

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
pub fn cell_font_get_horizontal_layout(font: u32, layout_addr: u32) -> i32 {
    trace!("cellFontGetHorizontalLayout(font={})", font);

    // Get horizontal layout metrics through global manager
    if layout_addr == 0 {
        return 0x80540004u32 as i32; // CELL_FONT_ERROR_INVALID_PARAMETER
    }

    // Check if font exists
    if !crate::context::get_hle_context().font.is_font_open(font) {
        return 0x80540004u32 as i32; // CELL_FONT_ERROR_INVALID_PARAMETER
    }

    // Write layout info to memory
    // CellFontHorizontalLayout struct:
    // - baselineY: f32 (offset 0)
    // - lineHeight: f32 (offset 4)
    // - effectHeight: f32 (offset 8)
    
    // Use simulated metrics
    let baseline_y: f32 = 12.0;
    let line_height: f32 = 16.0;
    let effect_height: f32 = 0.0;
    
    if let Err(e) = write_be32(layout_addr, baseline_y.to_bits()) { return e; }
    if let Err(e) = write_be32(layout_addr + 4, line_height.to_bits()) { return e; }
    if let Err(e) = write_be32(layout_addr + 8, effect_height.to_bits()) { return e; }

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
        // Valid font size (use address 0 to skip memory write)
        assert_eq!(cell_font_open_font_memory(1, 0x10000000, 1024, 0, 0, 0), 0);
        
        // Invalid font size (0)
        assert!(cell_font_open_font_memory(1, 0x10000000, 0, 0, 0, 0) != 0);
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

    // ========================================================================
    // TrueType Parsing Tests
    // ========================================================================

    #[test]
    fn test_font_truetype_parsing_empty() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());

        // Opening with empty data should still succeed with default metrics
        let id = manager.open_font_memory(0x10000000, 0, CellFontType::TrueType).unwrap();
        assert!(manager.is_font_open(id));

        // Should have a default ASCII cmap
        assert!(manager.get_cmap_entry_count(id) > 0);

        manager.end();
    }

    // ========================================================================
    // Cmap / Unicode Tests
    // ========================================================================

    #[test]
    fn test_font_cmap_lookup() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());

        let id = manager.open_font_memory(0x10000000, 0, CellFontType::TrueType).unwrap();

        // ASCII 'A' (0x41) should be mapped
        let glyph = manager.get_glyph_index(id, 0x41);
        assert!(glyph.is_some());

        // Space (0x20) should be mapped
        assert!(manager.get_glyph_index(id, 0x20).is_some());

        // Very high codepoint unlikely to be mapped with default data
        assert!(manager.get_glyph_index(id, 0xFFFFFF).is_none());

        manager.end();
    }

    // ========================================================================
    // Kerning Tests
    // ========================================================================

    #[test]
    fn test_font_kerning_empty() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());

        let id = manager.open_font_memory(0x10000000, 0, CellFontType::TrueType).unwrap();

        // With no real kern table loaded, kerning should be None
        assert_eq!(manager.get_kerning_pair_count(id), 0);
        assert!(manager.get_kerning(id, 0x41, 0x56).is_none());

        manager.end();
    }

    // ========================================================================
    // System Font Tests
    // ========================================================================

    #[test]
    fn test_font_system_font_loading() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());

        let id = manager.open_system_font("/dev_flash/data/font/SCE-PS3-RD-R-LATIN.TTF").unwrap();
        assert!(manager.is_font_open(id));

        // Should have a family name derived from the path
        let family = manager.get_font_family(id).unwrap();
        assert!(family.contains("SCE-PS3-RD-R-LATIN"));

        // Should have cmap entries for ASCII and CJK ranges
        assert!(manager.get_cmap_entry_count(id) > 95); // at least ASCII printable

        manager.end();
    }

    #[test]
    fn test_font_system_font_paths() {
        assert!(!FontManager::SYSTEM_FONT_PATHS.is_empty());
        assert!(FontManager::SYSTEM_FONT_PATHS.iter().all(|p| p.starts_with("/dev_flash/")));
    }

    #[test]
    fn test_font_family_style() {
        let mut manager = FontManager::new();
        manager.init(CellFontConfig::default());

        let id = manager.open_font_memory(0x10000000, 0, CellFontType::TrueType).unwrap();
        assert!(manager.get_font_family(id).is_some());
        assert!(manager.get_font_style(id).is_some());

        manager.end();
    }
}
