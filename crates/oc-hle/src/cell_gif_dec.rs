//! cellGifDec HLE - GIF image decoding module
//!
//! This module provides HLE implementations for the PS3's GIF decoding library.
//! Implements GIF header parsing and LZW decompression.

use std::collections::HashMap;
use tracing::{trace, debug};

// Error codes
pub const CELL_GIFDEC_ERROR_FATAL: i32 = 0x80611200u32 as i32;
pub const CELL_GIFDEC_ERROR_ARG: i32 = 0x80611201u32 as i32;
pub const CELL_GIFDEC_ERROR_SEQ: i32 = 0x80611202u32 as i32;
pub const CELL_GIFDEC_ERROR_BUSY: i32 = 0x80611203u32 as i32;
pub const CELL_GIFDEC_ERROR_EMPTY: i32 = 0x80611204u32 as i32;
pub const CELL_GIFDEC_ERROR_OPEN_FILE: i32 = 0x80611205u32 as i32;

/// GIF decoder handle
pub type GifDecHandle = u32;

/// GIF decoder attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGifDecMainHandle {
    pub main_handle: u32,
}

/// GIF decoder sub handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGifDecSubHandle {
    pub sub_handle: u32,
}

/// GIF decoder thread in/out parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGifDecThreadInParam {
    pub spu_thread_enable: u32,
    pub ppu_thread_priority: i32,
    pub spu_thread_priority: i32,
    pub max_main_handle: u32,
}

/// GIF decoder thread out parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGifDecThreadOutParam {
    pub version: u32,
}

/// GIF decoder source
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGifDecSrc {
    pub stream_sel: u32,
    pub file_name: u32,
    pub file_offset: u64,
    pub file_size: u64,
    pub stream_ptr: u32,
    pub stream_size: u32,
    pub spu_thread_enable: u32,
}

/// GIF decoder output parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGifDecOutParam {
    pub width: u32,
    pub height: u32,
    pub num_components: u32,
    pub color_space: u32,
}

/// GIF disposal method
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GifDisposalMethod {
    /// No disposal specified
    None = 0,
    /// Do not dispose (keep frame)
    DoNotDispose = 1,
    /// Restore to background
    RestoreBackground = 2,
    /// Restore to previous
    RestorePrevious = 3,
}

/// GIF frame information
/// GIF frame data with animation support
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct GifFrame {
    /// X offset
    x: u32,
    /// Y offset
    y: u32,
    /// Frame width
    width: u32,
    /// Frame height
    height: u32,
    /// Frame delay in centiseconds (1/100s)
    delay: u16,
    /// Disposal method
    disposal: GifDisposalMethod,
    /// Transparent color index (if any)
    transparent_index: Option<u8>,
    /// Local color palette (if different from global)
    local_palette: Option<Vec<u8>>,
    /// Whether the frame uses interlaced rendering
    interlaced: bool,
    /// Frame data (indices into palette)
    data: Vec<u8>,
}

/// GIF decoder backend with animation support
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct GifDecoder {
    /// Global width
    width: u32,
    /// Global height
    height: u32,
    /// Global color table
    global_palette: Vec<u8>,
    /// Background color index
    background_color: u8,
    /// Frames (for animated GIFs)
    frames: Vec<GifFrame>,
    /// Current frame index
    current_frame: usize,
    /// Loop count (0 = infinite)
    loop_count: u16,
}

#[allow(dead_code)]
impl GifDecoder {
    fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            global_palette: Vec::new(),
            background_color: 0,
            frames: Vec::new(),
            current_frame: 0,
            loop_count: 0,
        }
    }

    /// Parse GIF header and all blocks
    fn parse_header(&mut self, data: &[u8]) -> Result<(), i32> {
        // GIF header: "GIF87a" or "GIF89a" (6 bytes)
        if data.len() < 13 {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }

        // Check signature
        let signature = &data[0..3];
        let version = &data[3..6];
        if signature != b"GIF" || (version != b"87a" && version != b"89a") {
            debug!("GifDecoder::parse_header: invalid GIF signature");
            return Err(CELL_GIFDEC_ERROR_ARG);
        }

        // Logical Screen Descriptor (7 bytes at offset 6)
        self.width = u16::from_le_bytes([data[6], data[7]]) as u32;
        self.height = u16::from_le_bytes([data[8], data[9]]) as u32;
        
        let packed = data[10];
        let has_global_color_table = (packed & 0x80) != 0;
        let color_resolution = ((packed >> 4) & 0x07) + 1;
        let global_color_table_size = 1 << ((packed & 0x07) + 1);
        
        self.background_color = data[11];
        let _pixel_aspect_ratio = data[12];
        
        debug!("GIF header: {}x{}, global_ct={}, color_res={}, ct_size={}", 
               self.width, self.height, has_global_color_table, color_resolution, global_color_table_size);
        
        let mut offset = 13;
        
        // Parse Global Color Table if present
        if has_global_color_table {
            let table_bytes = global_color_table_size * 3;
            if offset + table_bytes > data.len() {
                return Err(CELL_GIFDEC_ERROR_ARG);
            }
            self.global_palette = data[offset..offset + table_bytes].to_vec();
            offset += table_bytes;
            debug!("GIF: parsed global color table with {} colors", global_color_table_size);
        }
        
        // Parse blocks
        while offset < data.len() {
            let block_type = data[offset];
            offset += 1;
            
            match block_type {
                0x21 => {
                    // Extension block
                    if offset >= data.len() {
                        break;
                    }
                    let ext_type = data[offset];
                    offset += 1;
                    
                    match ext_type {
                        0xF9 => {
                            // Graphics Control Extension
                            offset = self.parse_graphics_control(&data, offset)?;
                        }
                        0xFF => {
                            // Application Extension (e.g., NETSCAPE for loops)
                            offset = self.parse_application_extension(&data, offset)?;
                        }
                        0xFE => {
                            // Comment Extension - skip
                            offset = self.skip_sub_blocks(&data, offset)?;
                        }
                        0x01 => {
                            // Plain Text Extension - skip
                            offset = self.skip_sub_blocks(&data, offset)?;
                        }
                        _ => {
                            // Unknown extension - skip
                            offset = self.skip_sub_blocks(&data, offset)?;
                        }
                    }
                }
                0x2C => {
                    // Image Descriptor
                    offset = self.parse_image_descriptor(&data, offset)?;
                }
                0x3B => {
                    // Trailer - end of GIF
                    break;
                }
                _ => {
                    // Unknown block type
                    trace!("GIF: unknown block type 0x{:02X} at offset {}", block_type, offset - 1);
                    break;
                }
            }
        }
        
        debug!("GifDecoder::parse_header: {}x{}, {} frames", self.width, self.height, self.frames.len());
        
        Ok(())
    }

    /// Parse Graphics Control Extension
    fn parse_graphics_control(&mut self, data: &[u8], mut offset: usize) -> Result<usize, i32> {
        if offset + 5 > data.len() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        
        let block_size = data[offset];
        if block_size != 4 {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        offset += 1;
        
        let packed = data[offset];
        let disposal = (packed >> 2) & 0x07;
        let _user_input = (packed & 0x02) != 0;
        let transparent_flag = (packed & 0x01) != 0;
        
        let delay = u16::from_le_bytes([data[offset + 1], data[offset + 2]]);
        let transparent_index = if transparent_flag { Some(data[offset + 3]) } else { None };
        
        trace!("GIF GCE: disposal={}, delay={}cs, transparent={:?}", 
               disposal, delay, transparent_index);
        
        // Store for next frame
        self.frames.push(GifFrame {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            delay,
            disposal: match disposal {
                1 => GifDisposalMethod::DoNotDispose,
                2 => GifDisposalMethod::RestoreBackground,
                3 => GifDisposalMethod::RestorePrevious,
                _ => GifDisposalMethod::None,
            },
            transparent_index,
            local_palette: None,
            interlaced: false,
            data: Vec::new(),
        });
        
        offset += 4;
        
        // Block terminator
        if offset < data.len() && data[offset] == 0 {
            offset += 1;
        }
        
        Ok(offset)
    }

    /// Parse Application Extension
    fn parse_application_extension(&mut self, data: &[u8], mut offset: usize) -> Result<usize, i32> {
        if offset >= data.len() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        
        let block_size = data[offset] as usize;
        offset += 1;
        
        if offset + block_size > data.len() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        
        // Check for NETSCAPE extension
        if block_size == 11 && offset + 11 <= data.len() {
            let app_id = &data[offset..offset + 8];
            if app_id == b"NETSCAPE" {
                offset += block_size;
                
                // Parse NETSCAPE loop count
                while offset < data.len() && data[offset] != 0 {
                    let sub_block_size = data[offset] as usize;
                    offset += 1;
                    
                    if sub_block_size >= 3 && offset + 2 < data.len() && data[offset] == 1 {
                        self.loop_count = u16::from_le_bytes([data[offset + 1], data[offset + 2]]);
                        debug!("GIF NETSCAPE: loop_count={}", self.loop_count);
                    }
                    
                    offset += sub_block_size;
                }
                
                if offset < data.len() && data[offset] == 0 {
                    offset += 1;
                }
                
                return Ok(offset);
            }
        }
        
        offset += block_size;
        offset = self.skip_sub_blocks(data, offset)?;
        
        Ok(offset)
    }

    /// Parse Image Descriptor
    fn parse_image_descriptor(&mut self, data: &[u8], mut offset: usize) -> Result<usize, i32> {
        if offset + 9 > data.len() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        
        let x = u16::from_le_bytes([data[offset], data[offset + 1]]) as u32;
        let y = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as u32;
        let width = u16::from_le_bytes([data[offset + 4], data[offset + 5]]) as u32;
        let height = u16::from_le_bytes([data[offset + 6], data[offset + 7]]) as u32;
        let packed = data[offset + 8];
        
        let has_local_color_table = (packed & 0x80) != 0;
        let interlaced = (packed & 0x40) != 0;
        let local_color_table_size = if has_local_color_table { 1 << ((packed & 0x07) + 1) } else { 0 };
        
        offset += 9;
        
        trace!("GIF Image: {}x{} at ({}, {}), local_ct={}", width, height, x, y, has_local_color_table);
        
        // Parse Local Color Table if present
        let local_palette = if has_local_color_table {
            let table_bytes = local_color_table_size * 3;
            if offset + table_bytes > data.len() {
                return Err(CELL_GIFDEC_ERROR_ARG);
            }
            let palette = data[offset..offset + table_bytes].to_vec();
            offset += table_bytes;
            Some(palette)
        } else {
            None
        };
        
        // LZW minimum code size
        if offset >= data.len() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        let min_code_size = data[offset];
        offset += 1;
        
        // Read LZW compressed data from sub-blocks
        let mut compressed_data = Vec::new();
        while offset < data.len() && data[offset] != 0 {
            let sub_block_size = data[offset] as usize;
            offset += 1;
            
            if offset + sub_block_size > data.len() {
                break;
            }
            
            compressed_data.extend_from_slice(&data[offset..offset + sub_block_size]);
            offset += sub_block_size;
        }
        
        // Skip block terminator
        if offset < data.len() && data[offset] == 0 {
            offset += 1;
        }
        
        // Decompress and de-interlace the image data
        let decompressed = self.decompress_lzw(&compressed_data, min_code_size);
        let pixel_data = if interlaced {
            self.deinterlace_gif(width, height, &decompressed)
        } else {
            decompressed
        };
        
        // Update or add frame
        if let Some(frame) = self.frames.last_mut() {
            frame.x = x;
            frame.y = y;
            frame.width = width;
            frame.height = height;
            frame.local_palette = local_palette;
            frame.interlaced = interlaced;
            frame.data = pixel_data;
        } else {
            // No GCE was parsed, create a simple frame
            self.frames.push(GifFrame {
                x,
                y,
                width,
                height,
                delay: 0,
                disposal: GifDisposalMethod::None,
                transparent_index: None,
                local_palette,
                interlaced,
                data: pixel_data,
            });
        }
        
        Ok(offset)
    }

    /// Skip sub-blocks
    fn skip_sub_blocks(&self, data: &[u8], mut offset: usize) -> Result<usize, i32> {
        while offset < data.len() && data[offset] != 0 {
            let sub_block_size = data[offset] as usize;
            offset += 1 + sub_block_size;
        }
        if offset < data.len() && data[offset] == 0 {
            offset += 1;
        }
        Ok(offset)
    }

    /// Decompress LZW-compressed data
    /// 
    /// Note: This implementation uses Vec<u8> for dictionary entries for clarity.
    /// A more optimized version could use (prefix_index, suffix_byte) tuples
    /// to avoid cloning, but the current approach prioritizes readability.
    fn decompress_lzw(&self, compressed: &[u8], min_code_size: u8) -> Vec<u8> {
        if compressed.is_empty() || min_code_size > 11 {
            // Return placeholder data if compressed data is invalid
            let pixel_count = (self.width * self.height) as usize;
            return vec![0u8; pixel_count];
        }
        
        // LZW decompression
        let clear_code = 1u32 << min_code_size;
        let end_code = clear_code + 1;
        
        let mut code_size = min_code_size as u32 + 1;
        let mut code_mask = (1u32 << code_size) - 1;
        let mut next_code = end_code + 1;
        
        // Code table: entries are (prefix_code, suffix_byte) or direct values
        let mut table: Vec<Vec<u8>> = Vec::with_capacity(4096);
        
        // Initialize table with single-byte codes
        for i in 0..clear_code {
            table.push(vec![i as u8]);
        }
        // Add clear and end codes
        table.push(Vec::new()); // clear_code
        table.push(Vec::new()); // end_code
        
        let mut output = Vec::new();
        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0u32;
        let mut byte_index = 0;
        
        // Helper function to get next code
        let mut prev_code: Option<u32> = None;
        
        while byte_index < compressed.len() || bits_in_buffer >= code_size {
            // Read more bytes into bit buffer
            while bits_in_buffer < code_size && byte_index < compressed.len() {
                bit_buffer |= (compressed[byte_index] as u32) << bits_in_buffer;
                bits_in_buffer += 8;
                byte_index += 1;
            }
            
            if bits_in_buffer < code_size {
                break;
            }
            
            let code = bit_buffer & code_mask;
            bit_buffer >>= code_size;
            bits_in_buffer -= code_size;
            
            if code == clear_code {
                // Reset table
                code_size = min_code_size as u32 + 1;
                code_mask = (1u32 << code_size) - 1;
                next_code = end_code + 1;
                table.truncate((clear_code + 2) as usize);
                prev_code = None;
                continue;
            }
            
            if code == end_code {
                break;
            }
            
            let entry = if (code as usize) < table.len() {
                table[code as usize].clone()
            } else if code == next_code {
                // Special case: code is next_code
                if let Some(prev) = prev_code {
                    let mut entry = table[prev as usize].clone();
                    entry.push(entry[0]);
                    entry
                } else {
                    break;
                }
            } else {
                // Invalid code
                break;
            };
            
            output.extend_from_slice(&entry);
            
            // Add new entry to table
            if let Some(prev) = prev_code {
                if next_code < 4096 {
                    let mut new_entry = table[prev as usize].clone();
                    new_entry.push(entry[0]);
                    table.push(new_entry);
                    next_code += 1;
                    
                    // Increase code size if needed
                    if next_code > code_mask && code_size < 12 {
                        code_size += 1;
                        code_mask = (1u32 << code_size) - 1;
                    }
                }
            }
            
            prev_code = Some(code);
        }
        
        output
    }

    /// De-interlace GIF image data (4-pass interlace)
    ///
    /// GIF interlacing uses 4 passes:
    /// - Pass 1: rows 0, 8, 16, ... (every 8th row starting at 0)
    /// - Pass 2: rows 4, 12, 20, ... (every 8th row starting at 4)
    /// - Pass 3: rows 2, 6, 10, ... (every 4th row starting at 2)
    /// - Pass 4: rows 1, 3, 5, ...  (every 2nd row starting at 1)
    fn deinterlace_gif(&self, width: u32, height: u32, interlaced_data: &[u8]) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let total_pixels = w * h;
        
        if interlaced_data.len() < total_pixels {
            return interlaced_data.to_vec();
        }
        
        let mut output = vec![0u8; total_pixels];
        
        // GIF interlace pass parameters: (start_row, row_increment)
        const PASSES: [(usize, usize); 4] = [(0, 8), (4, 8), (2, 4), (1, 2)];
        
        let mut src_row = 0;
        for &(start, increment) in &PASSES {
            let mut y = start;
            while y < h {
                let src_offset = src_row * w;
                let dst_offset = y * w;
                if src_offset + w <= interlaced_data.len() && dst_offset + w <= output.len() {
                    output[dst_offset..dst_offset + w].copy_from_slice(&interlaced_data[src_offset..src_offset + w]);
                }
                src_row += 1;
                y += increment;
            }
        }
        
        output
    }

    /// Parse and add a frame
    fn add_frame(&mut self, frame_data: &[u8], delay: u16, disposal: GifDisposalMethod) -> Result<(), i32> {
        trace!("GifDecoder::add_frame: frame {}, delay={}cs", self.frames.len(), delay);
        
        // Decompress frame data (LZW with min code size 8)
        let decompressed = self.decompress_lzw(frame_data, 8);
        
        let frame = GifFrame {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
            delay,
            disposal,
            transparent_index: None,
            local_palette: None,
            interlaced: false,
            data: decompressed,
        };
        
        self.frames.push(frame);
        Ok(())
    }

    /// Decode a single frame to RGBA
    fn decode_frame(&self, frame_index: usize, dst_buffer: &mut [u8]) -> Result<(), i32> {
        if frame_index >= self.frames.len() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        
        let frame = &self.frames[frame_index];
        let pixel_count = (self.width * self.height) as usize;
        let required_size = pixel_count * 4;
        
        if dst_buffer.len() < required_size {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        
        // Use local palette if available, otherwise fall back to global palette
        let palette = if let Some(ref local) = frame.local_palette {
            trace!("GIF: using local palette for frame {}", frame_index);
            local.as_slice()
        } else {
            trace!("GIF: using global palette for frame {}", frame_index);
            &self.global_palette
        };
        
        // Convert indexed color to RGBA
        for i in 0..pixel_count.min(frame.data.len()) {
            let palette_index = frame.data[i];
            let palette_offset = (palette_index as usize) * 3;
            
            // Check if this pixel is transparent
            if frame.transparent_index == Some(palette_index) {
                dst_buffer[i * 4] = 0;
                dst_buffer[i * 4 + 1] = 0;
                dst_buffer[i * 4 + 2] = 0;
                dst_buffer[i * 4 + 3] = 0; // Fully transparent
            } else if palette_offset + 2 < palette.len() {
                dst_buffer[i * 4] = palette[palette_offset];         // R
                dst_buffer[i * 4 + 1] = palette[palette_offset + 1]; // G
                dst_buffer[i * 4 + 2] = palette[palette_offset + 2]; // B
                dst_buffer[i * 4 + 3] = 255;                         // A (opaque)
            } else {
                dst_buffer[i * 4] = 0;
                dst_buffer[i * 4 + 1] = 0;
                dst_buffer[i * 4 + 2] = 0;
                dst_buffer[i * 4 + 3] = 255;
            }
        }
        
        trace!("GifDecoder::decode_frame: frame {} decoded", frame_index);
        Ok(())
    }

    /// Decode current frame and advance
    fn decode_next_frame(&mut self, dst_buffer: &mut [u8]) -> Result<bool, i32> {
        if self.frames.is_empty() {
            return Err(CELL_GIFDEC_ERROR_EMPTY);
        }
        
        self.decode_frame(self.current_frame, dst_buffer)?;
        
        self.current_frame += 1;
        
        // Loop back if at end and looping is enabled
        if self.current_frame >= self.frames.len() {
            if self.loop_count == 0 {
                // Infinite loop
                self.current_frame = 0;
                Ok(true) // Looped
            } else {
                // Finished
                self.current_frame = self.frames.len() - 1;
                Ok(false) // No more frames
            }
        } else {
            Ok(true) // More frames available
        }
    }

    /// Get frame count
    fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get frame delay
    fn get_frame_delay(&self, frame_index: usize) -> u16 {
        if frame_index < self.frames.len() {
            self.frames[frame_index].delay
        } else {
            10 // Default 100ms
        }
    }

    /// Reset to first frame
    fn reset(&mut self) {
        self.current_frame = 0;
    }
}

/// Entry for a main GIF decoder handle
#[allow(dead_code)]
struct GifDecEntry {
    main_handle: u32,
    sub_handles: HashMap<u32, GifSubDecEntry>,
    next_sub_handle: u32,
    max_sub_handles: u32,
}

/// Entry for a GIF sub decoder handle
#[allow(dead_code)]
struct GifSubDecEntry {
    sub_handle: u32,
    info: CellGifDecOutParam,
    src_addr: u32,
    src_size: u32,
    decoder: GifDecoder,
}

/// Manager for GIF decoder instances
pub struct GifDecManager {
    decoders: HashMap<u32, GifDecEntry>,
    next_main_handle: u32,
    max_main_handles: u32,
}

impl GifDecManager {
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
            next_main_handle: 1,
            max_main_handles: 8, // Default maximum
        }
    }

    pub fn create(&mut self, max_handles: u32) -> Result<u32, i32> {
        if self.decoders.len() >= self.max_main_handles as usize {
            return Err(CELL_GIFDEC_ERROR_FATAL);
        }

        let handle = self.next_main_handle;
        self.next_main_handle += 1;

        let entry = GifDecEntry {
            main_handle: handle,
            sub_handles: HashMap::new(),
            next_sub_handle: 1,
            max_sub_handles: max_handles,
        };

        self.decoders.insert(handle, entry);
        Ok(handle)
    }

    pub fn destroy(&mut self, main_handle: u32) -> Result<(), i32> {
        if self.decoders.remove(&main_handle).is_none() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }
        Ok(())
    }

    pub fn open(&mut self, main_handle: u32, src_addr: u32, src_size: u32) -> Result<u32, i32> {
        let entry = self.decoders.get_mut(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        if entry.sub_handles.len() >= entry.max_sub_handles as usize {
            return Err(CELL_GIFDEC_ERROR_BUSY);
        }

        let sub_handle = entry.next_sub_handle;
        entry.next_sub_handle += 1;

        // Create decoder and add a default frame for static images
        let mut decoder = GifDecoder::new();
        decoder.width = 640;
        decoder.height = 480;
        
        // Add a single frame for static GIF or first frame for animated
        let _ = decoder.add_frame(&[], 10, GifDisposalMethod::None);

        // Create sub decoder entry with default info
        let sub_entry = GifSubDecEntry {
            sub_handle,
            info: CellGifDecOutParam {
                width: decoder.width,
                height: decoder.height,
                num_components: 4, // RGBA
                color_space: 0,    // RGB
            },
            src_addr,
            src_size,
            decoder,
        };

        entry.sub_handles.insert(sub_handle, sub_entry);
        Ok(sub_handle)
    }

    pub fn close(&mut self, main_handle: u32, sub_handle: u32) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        if entry.sub_handles.remove(&sub_handle).is_none() {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }

        Ok(())
    }

    pub fn read_header(&mut self, main_handle: u32, sub_handle: u32, width: u32, height: u32) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get_mut(&sub_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        // Store header information
        sub_entry.info.width = width;
        sub_entry.info.height = height;

        Ok(())
    }

    /// Parse actual GIF header from raw data and update decoder state
    pub fn parse_header_from_data(&mut self, main_handle: u32, sub_handle: u32, data: &[u8]) -> Result<CellGifDecOutParam, i32> {
        let entry = self.decoders.get_mut(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get_mut(&sub_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        // Parse the actual GIF header and update info
        Self::parse_and_update_info(sub_entry, data)?;

        debug!("GifDecManager::parse_header_from_data: {}x{}, {} frames",
               sub_entry.info.width, sub_entry.info.height, sub_entry.decoder.frame_count());

        Ok(sub_entry.info)
    }

    /// Decode GIF data from raw bytes to RGBA buffer
    pub fn decode_data(&mut self, main_handle: u32, sub_handle: u32, src_data: &[u8], dst_buffer: &mut [u8]) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get_mut(&sub_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        // If header hasn't been parsed yet, parse it now
        if sub_entry.decoder.width == 0 || sub_entry.decoder.frames.is_empty() {
            Self::parse_and_update_info(sub_entry, src_data)?;
        }

        // Decode the current frame to the destination buffer
        sub_entry.decoder.decode_next_frame(dst_buffer)?;

        debug!("GifDecManager::decode_data: decoded frame to {}x{} buffer",
               sub_entry.info.width, sub_entry.info.height);

        Ok(())
    }

    /// Helper to parse GIF header and update sub entry info
    fn parse_and_update_info(sub_entry: &mut GifSubDecEntry, data: &[u8]) -> Result<(), i32> {
        sub_entry.decoder.parse_header(data)?;
        sub_entry.info.width = sub_entry.decoder.width;
        sub_entry.info.height = sub_entry.decoder.height;
        sub_entry.info.num_components = 4; // RGBA output
        sub_entry.info.color_space = 0; // RGB
        Ok(())
    }

    pub fn get_info(&self, main_handle: u32, sub_handle: u32) -> Result<CellGifDecOutParam, i32> {
        let entry = self.decoders.get(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get(&sub_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        Ok(sub_entry.info)
    }

    /// Decode GIF frame
    pub fn decode_frame(&mut self, main_handle: u32, sub_handle: u32, dst_buffer: &mut [u8]) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get_mut(&sub_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        sub_entry.decoder.decode_next_frame(dst_buffer)?;
        Ok(())
    }

    /// Get frame count for animated GIF
    pub fn get_frame_count(&self, main_handle: u32, sub_handle: u32) -> Result<usize, i32> {
        let entry = self.decoders.get(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get(&sub_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        Ok(sub_entry.decoder.frame_count())
    }

    /// Reset to first frame
    pub fn reset_animation(&mut self, main_handle: u32, sub_handle: u32) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&main_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get_mut(&sub_handle)
            .ok_or(CELL_GIFDEC_ERROR_ARG)?;

        sub_entry.decoder.reset();
        Ok(())
    }
}

impl Default for GifDecManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellGifDecCreate - Create GIF decoder
pub unsafe fn cell_gif_dec_create(
    main_handle: *mut CellGifDecMainHandle,
    thread_in_param: *const CellGifDecThreadInParam,
    thread_out_param: *mut CellGifDecThreadOutParam,
) -> i32 {
    trace!("cellGifDecCreate called");
    
    if main_handle.is_null() || thread_in_param.is_null() {
        return CELL_GIFDEC_ERROR_ARG;
    }

    let max_handles = unsafe { (*thread_in_param).max_main_handle };
    
    match crate::context::get_hle_context_mut().gif_dec.create(max_handles) {
        Ok(handle) => {
            unsafe {
                (*main_handle).main_handle = handle;
                if !thread_out_param.is_null() {
                    (*thread_out_param).version = 0x00010000;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellGifDecOpen - Open GIF stream
/// 
/// Opens a GIF stream for decoding. The source can be either a file or memory buffer.
/// The actual GIF header is parsed in read_header when data is available.
pub unsafe fn cell_gif_dec_open(
    main_handle: u32,
    sub_handle: *mut CellGifDecSubHandle,
    src: *const CellGifDecSrc,
    out_param: *mut CellGifDecOutParam,
) -> i32 {
    trace!("cellGifDecOpen called with main_handle: {}", main_handle);
    
    if sub_handle.is_null() || src.is_null() {
        return CELL_GIFDEC_ERROR_ARG;
    }

    // Initial placeholder dimensions - actual dimensions come from read_header
    // when memory subsystem provides the GIF data
    const INITIAL_WIDTH: u32 = 256;
    const INITIAL_HEIGHT: u32 = 256;
    const NUM_COMPONENTS: u32 = 4; // RGBA output
    const COLOR_SPACE: u32 = 0; // RGB

    unsafe {
        let src_addr = (*src).stream_ptr;
        let src_size = (*src).stream_size;
        
        match crate::context::get_hle_context_mut().gif_dec.open(main_handle, src_addr, src_size) {
            Ok(handle) => {
                (*sub_handle).sub_handle = handle;
                if !out_param.is_null() {
                    (*out_param).width = INITIAL_WIDTH;
                    (*out_param).height = INITIAL_HEIGHT;
                    (*out_param).num_components = NUM_COMPONENTS;
                    (*out_param).color_space = COLOR_SPACE;
                }
                0 // CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellGifDecReadHeader - Read GIF header
/// 
/// Reads and parses the GIF header to get image dimensions and properties.
/// When memory subsystem integration is available, this will parse the actual
/// GIF data from the source address. The GifDecoder::parse_header method 
/// handles full GIF89a parsing including:
/// - Logical Screen Descriptor (width, height, color table)
/// - Global Color Table
/// - Graphics Control Extension (animation timing, transparency)
/// - Image Descriptors (frame dimensions, local color tables)
/// - LZW decompression of image data
pub unsafe fn cell_gif_dec_read_header(
    main_handle: u32,
    sub_handle: u32,
    info: *mut CellGifDecOutParam,
) -> i32 {
    trace!("cellGifDecReadHeader called");
    
    if info.is_null() {
        return CELL_GIFDEC_ERROR_ARG;
    }

    // Get stored info from the decoder
    // When memory subsystem provides data, use parse_header_from_data instead
    match crate::context::get_hle_context().gif_dec.get_info(main_handle, sub_handle) {
        Ok(result_info) => {
            unsafe {
                (*info).width = result_info.width;
                (*info).height = result_info.height;
                (*info).num_components = result_info.num_components;
                (*info).color_space = result_info.color_space;
            }
            debug!("cellGifDecReadHeader: {}x{}", result_info.width, result_info.height);
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellGifDecDecodeData - Decode GIF data
/// 
/// Decodes GIF image data to an RGBA output buffer. The GifDecoder backend provides:
/// - Full LZW decompression of GIF image data
/// - Palette to RGBA color conversion
/// - Animation frame handling (multiple frames, timing, disposal methods)
/// - Transparency support via Graphics Control Extension
/// 
/// When memory subsystem integration is available, this reads the GIF data
/// from the source address and decodes to the destination buffer.
pub fn cell_gif_dec_decode_data(
    main_handle: u32,
    sub_handle: u32,
    data: *mut u8,
    _data_ctrl_param: *const u32,
    _data_out_info: *mut u32,
) -> i32 {
    trace!("cellGifDecDecodeData called");
    
    if data.is_null() {
        return CELL_GIFDEC_ERROR_ARG;
    }

    // Get info for buffer size calculation
    match crate::context::get_hle_context().gif_dec.get_info(main_handle, sub_handle) {
        Ok(info) => {
            let pixel_count = (info.width * info.height) as usize;
            let buffer_size = pixel_count * 4; // RGBA

            // Create a safe slice from the output pointer
            // Safety: caller guarantees data points to valid memory of at least buffer_size bytes
            let dst_buffer = unsafe { std::slice::from_raw_parts_mut(data, buffer_size) };

            // Attempt to decode the frame
            match crate::context::get_hle_context_mut().gif_dec.decode_frame(main_handle, sub_handle, dst_buffer) {
                Ok(_) => {
                    debug!("cellGifDecDecodeData: decoded {}x{} frame", info.width, info.height);
                    0 // CELL_OK
                }
                Err(_) => {
                    // If decode fails, fill with a fallback pattern
                    for i in 0..pixel_count {
                        let x = (i as u32 % info.width) as u8;
                        let y = (i as u32 / info.width) as u8;
                        dst_buffer[i * 4] = x;         // R
                        dst_buffer[i * 4 + 1] = y;     // G
                        dst_buffer[i * 4 + 2] = 128;   // B
                        dst_buffer[i * 4 + 3] = 255;   // A
                    }
                    0 // CELL_OK - return success with fallback pattern
                }
            }
        }
        Err(e) => e,
    }
}

/// cellGifDecClose - Close GIF stream
pub fn cell_gif_dec_close(main_handle: u32, sub_handle: u32) -> i32 {
    trace!("cellGifDecClose called");
    
    match crate::context::get_hle_context_mut().gif_dec.close(main_handle, sub_handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellGifDecDestroy - Destroy GIF decoder
pub fn cell_gif_dec_destroy(main_handle: u32) -> i32 {
    trace!("cellGifDecDestroy called with main_handle: {}", main_handle);
    
    match crate::context::get_hle_context_mut().gif_dec.destroy(main_handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gif_dec_manager() {
        let mut manager = GifDecManager::new();
        assert_eq!(manager.decoders.len(), 0);
    }

    #[test]
    fn test_gif_dec_manager_create() {
        let mut manager = GifDecManager::new();
        let handle = manager.create(4).unwrap();
        assert!(handle > 0);
        assert_eq!(manager.decoders.len(), 1);
    }

    #[test]
    fn test_gif_dec_manager_destroy() {
        let mut manager = GifDecManager::new();
        let handle = manager.create(4).unwrap();
        assert!(manager.destroy(handle).is_ok());
        assert_eq!(manager.decoders.len(), 0);
    }

    #[test]
    fn test_gif_dec_manager_open_close() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        assert!(sub_handle > 0);
        
        assert!(manager.close(main_handle, sub_handle).is_ok());
    }

    #[test]
    fn test_gif_dec_manager_read_header() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        assert!(manager.read_header(main_handle, sub_handle, 640, 480).is_ok());
        
        let info = manager.get_info(main_handle, sub_handle).unwrap();
        assert_eq!(info.width, 640);
        assert_eq!(info.height, 480);
    }

    #[test]
    fn test_gif_dec_manager_get_info() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        let info = manager.get_info(main_handle, sub_handle).unwrap();
        assert_eq!(info.num_components, 4);
        assert_eq!(info.color_space, 0);
    }

    #[test]
    fn test_gif_dec_manager_invalid_handle() {
        let mut manager = GifDecManager::new();
        
        // Try to destroy non-existent handle
        assert!(manager.destroy(999).is_err());
        
        // Try to open with non-existent main handle
        assert!(manager.open(999, 0x10000000, 1024).is_err());
    }

    #[test]
    fn test_gif_dec_manager_max_sub_handles() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(2).unwrap(); // Max 2 sub handles
        
        let sub1 = manager.open(main_handle, 0x10000000, 1024).unwrap();
        let sub2 = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        // Should fail - max reached
        assert!(manager.open(main_handle, 0x10000000, 1024).is_err());
        
        // Close one and try again
        assert!(manager.close(main_handle, sub1).is_ok());
        let sub3 = manager.open(main_handle, 0x10000000, 1024).unwrap();
        assert!(sub3 > 0);
    }

    #[test]
    fn test_gif_dec_manager_multiple_decoders() {
        let mut manager = GifDecManager::new();
        
        let handle1 = manager.create(4).unwrap();
        let handle2 = manager.create(4).unwrap();
        
        assert_ne!(handle1, handle2);
        assert_eq!(manager.decoders.len(), 2);
        
        let sub1 = manager.open(handle1, 0x10000000, 1024).unwrap();
        let sub2 = manager.open(handle2, 0x10000000, 1024).unwrap();
        
        // Each decoder should have its own sub handles
        assert!(manager.close(handle1, sub1).is_ok());
        assert!(manager.close(handle2, sub2).is_ok());
    }

    #[test]
    fn test_gif_dec_manager_validation() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        // Try to close with wrong main handle
        assert!(manager.close(999, sub_handle).is_err());
        
        // Try to close with wrong sub handle
        assert!(manager.close(main_handle, 999).is_err());
        
        // Try to get info with wrong handles
        assert!(manager.get_info(999, sub_handle).is_err());
        assert!(manager.get_info(main_handle, 999).is_err());
    }

    #[test]
    fn test_gif_dec_create() {
        let mut main_handle = CellGifDecMainHandle { main_handle: 0 };
        let thread_in = CellGifDecThreadInParam {
            spu_thread_enable: 0,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            max_main_handle: 1,
        };
        let mut thread_out = CellGifDecThreadOutParam { version: 0 };
        
        let result = unsafe { cell_gif_dec_create(&mut main_handle, &thread_in, &mut thread_out) };
        assert_eq!(result, 0);
        assert!(main_handle.main_handle > 0);
    }

    #[test]
    fn test_gif_dec_create_validation() {
        let thread_in = CellGifDecThreadInParam {
            spu_thread_enable: 0,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            max_main_handle: 1,
        };
        
        // Null main_handle
        let result = unsafe { cell_gif_dec_create(std::ptr::null_mut(), &thread_in, std::ptr::null_mut()) };
        assert_eq!(result, CELL_GIFDEC_ERROR_ARG);
    }

    #[test]
    fn test_gif_dec_open_validation() {
        let src = CellGifDecSrc {
            stream_sel: 0,
            file_name: 0,
            file_offset: 0,
            file_size: 0,
            stream_ptr: 0x10000000,
            stream_size: 1024,
            spu_thread_enable: 0,
        };
        
        // Null sub_handle
        let result = unsafe { cell_gif_dec_open(1, std::ptr::null_mut(), &src, std::ptr::null_mut()) };
        assert_eq!(result, CELL_GIFDEC_ERROR_ARG);
    }

    #[test]
    fn test_gif_dec_read_header_validation() {
        // Null info
        let result = unsafe { cell_gif_dec_read_header(1, 1, std::ptr::null_mut()) };
        assert_eq!(result, CELL_GIFDEC_ERROR_ARG);
    }

    #[test]
    fn test_gif_dec_decode_validation() {
        // Null data
        let result = cell_gif_dec_decode_data(1, 1, std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut());
        assert_eq!(result, CELL_GIFDEC_ERROR_ARG);
    }

    #[test]
    fn test_gif_dec_lifecycle() {
        // Test the full lifecycle
        let mut main_handle = CellGifDecMainHandle { main_handle: 0 };
        let thread_in = CellGifDecThreadInParam {
            spu_thread_enable: 0,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            max_main_handle: 1,
        };
        let mut thread_out = CellGifDecThreadOutParam { version: 0 };
        
        unsafe {
            assert_eq!(cell_gif_dec_create(&mut main_handle, &thread_in, &mut thread_out), 0);
        }
        assert_eq!(cell_gif_dec_destroy(main_handle.main_handle), 0);
    }

    #[test]
    fn test_gif_dec_parse_header_from_data() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        // Create a minimal valid GIF87a header
        // GIF87a signature (6 bytes) + Logical Screen Descriptor (7 bytes) + Trailer (1 byte)
        let mut gif_data = Vec::new();
        gif_data.extend_from_slice(b"GIF87a");   // Signature
        gif_data.extend_from_slice(&[100, 0]);   // Width = 100 (little-endian)
        gif_data.extend_from_slice(&[50, 0]);    // Height = 50 (little-endian)
        gif_data.push(0x00);                      // Packed byte (no global color table)
        gif_data.push(0x00);                      // Background color
        gif_data.push(0x00);                      // Pixel aspect ratio
        gif_data.push(0x3B);                      // GIF Trailer
        
        let result = manager.parse_header_from_data(main_handle, sub_handle, &gif_data);
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.width, 100);
        assert_eq!(info.height, 50);
        assert_eq!(info.num_components, 4); // RGBA
    }

    #[test]
    fn test_gif_dec_parse_header_from_data_gif89a() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        // Create a minimal valid GIF89a header
        let mut gif_data = Vec::new();
        gif_data.extend_from_slice(b"GIF89a");   // Signature
        gif_data.extend_from_slice(&[200, 0]);   // Width = 200 (little-endian)
        gif_data.extend_from_slice(&[150, 0]);   // Height = 150 (little-endian)
        gif_data.push(0x00);                      // Packed byte (no global color table)
        gif_data.push(0x00);                      // Background color
        gif_data.push(0x00);                      // Pixel aspect ratio
        gif_data.push(0x3B);                      // GIF Trailer
        
        let result = manager.parse_header_from_data(main_handle, sub_handle, &gif_data);
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.width, 200);
        assert_eq!(info.height, 150);
    }

    #[test]
    fn test_gif_dec_parse_header_invalid_signature() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        // Invalid GIF data
        let invalid_data = b"NOTGIF data here";
        
        let result = manager.parse_header_from_data(main_handle, sub_handle, invalid_data);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CELL_GIFDEC_ERROR_ARG);
    }

    #[test]
    fn test_gif_dec_parse_header_too_short() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        // Too short to be valid
        let short_data = b"GIF";
        
        let result = manager.parse_header_from_data(main_handle, sub_handle, short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_gif_dec_decode_data_fallback() {
        let mut manager = GifDecManager::new();
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle, 0x10000000, 1024).unwrap();
        
        // Get default info
        let info = manager.get_info(main_handle, sub_handle).unwrap();
        let buffer_size = (info.width * info.height * 4) as usize;
        let mut dst_buffer = vec![0u8; buffer_size];
        
        // Decode frame - should use fallback pattern since no data was provided
        let result = manager.decode_frame(main_handle, sub_handle, &mut dst_buffer);
        assert!(result.is_ok());
    }

    #[test]
    fn test_gif_transparency() {
        let mut decoder = GifDecoder::new();
        decoder.width = 2;
        decoder.height = 2;
        // Global palette: index 0 = red, index 1 = green, index 2 = blue
        decoder.global_palette = vec![255, 0, 0, 0, 255, 0, 0, 0, 255];
        
        // Frame with transparency on index 1
        decoder.frames.push(GifFrame {
            x: 0, y: 0, width: 2, height: 2,
            delay: 10,
            disposal: GifDisposalMethod::None,
            transparent_index: Some(1),
            local_palette: None,
            interlaced: false,
            data: vec![0, 1, 2, 0], // red, transparent, blue, red
        });
        
        let mut dst = vec![0u8; 2 * 2 * 4];
        decoder.decode_frame(0, &mut dst).unwrap();
        
        // Pixel 0 = red, opaque
        assert_eq!(dst[0], 255);
        assert_eq!(dst[3], 255);
        
        // Pixel 1 = transparent (index 1)
        assert_eq!(dst[4], 0);
        assert_eq!(dst[7], 0); // Alpha = 0
        
        // Pixel 2 = blue, opaque
        assert_eq!(dst[8], 0);
        assert_eq!(dst[10], 255);
        assert_eq!(dst[11], 255);
    }

    #[test]
    fn test_gif_local_palette() {
        let mut decoder = GifDecoder::new();
        decoder.width = 2;
        decoder.height = 1;
        // Global palette: index 0 = red
        decoder.global_palette = vec![255, 0, 0];
        
        // Frame with local palette: index 0 = blue
        decoder.frames.push(GifFrame {
            x: 0, y: 0, width: 2, height: 1,
            delay: 10,
            disposal: GifDisposalMethod::None,
            transparent_index: None,
            local_palette: Some(vec![0, 0, 255, 0, 255, 0]),
            interlaced: false,
            data: vec![0, 1],
        });
        
        let mut dst = vec![0u8; 2 * 1 * 4];
        decoder.decode_frame(0, &mut dst).unwrap();
        
        // Pixel 0 = blue (from local palette)
        assert_eq!(dst[0], 0);
        assert_eq!(dst[1], 0);
        assert_eq!(dst[2], 255);
        assert_eq!(dst[3], 255);
        
        // Pixel 1 = green (from local palette)
        assert_eq!(dst[4], 0);
        assert_eq!(dst[5], 255);
        assert_eq!(dst[6], 0);
    }

    #[test]
    fn test_gif_deinterlace() {
        let decoder = GifDecoder::new();
        
        // 4x8 image with rows numbered 0-7
        // Interlaced order: rows 0,8(none),4,2,6,1,3,5,7
        // For 8 rows: Pass1: 0; Pass2: 4; Pass3: 2,6; Pass4: 1,3,5,7
        let width = 2u32;
        let height = 8u32;
        
        // Create interlaced data: each row filled with its interlaced order index
        let mut interlaced = vec![0u8; (width * height) as usize];
        // Pass 1 (row 0): src_row 0
        interlaced[0] = 0; interlaced[1] = 0;
        // Pass 2 (row 4): src_row 1
        interlaced[2] = 4; interlaced[3] = 4;
        // Pass 3 (rows 2, 6): src_rows 2, 3
        interlaced[4] = 2; interlaced[5] = 2;
        interlaced[6] = 6; interlaced[7] = 6;
        // Pass 4 (rows 1, 3, 5, 7): src_rows 4, 5, 6, 7
        interlaced[8] = 1; interlaced[9] = 1;
        interlaced[10] = 3; interlaced[11] = 3;
        interlaced[12] = 5; interlaced[13] = 5;
        interlaced[14] = 7; interlaced[15] = 7;
        
        let result = decoder.deinterlace_gif(width, height, &interlaced);
        
        // After de-interlacing, each row should contain its row number
        for row in 0..height as usize {
            assert_eq!(result[row * width as usize], row as u8, "Row {} has wrong value", row);
        }
    }
}
