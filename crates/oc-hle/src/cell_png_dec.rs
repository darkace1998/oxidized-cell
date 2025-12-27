//! cellPngDec HLE - PNG Image Decoder
//!
//! This module provides HLE implementations for the PS3's PNG decoding library.

use std::collections::HashMap;
use tracing::trace;

/// PNG decoder handle
pub type PngDecHandle = u32;

/// PNG decoder main handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecMainHandle {
    pub main_handle: u32,
}

/// PNG decoder sub handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecSubHandle {
    pub sub_handle: u32,
}

/// PNG decoder thread parameters (input)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecThreadInParam {
    pub spu_thread_enable: u32,
    pub ppu_thread_priority: i32,
    pub spu_thread_priority: i32,
    pub max_main_handle: u32,
}

/// PNG decoder thread parameters (output)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecThreadOutParam {
    pub version: u32,
}

/// PNG decoder source
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecSrc {
    pub stream_sel: u32,
    pub file_name: u32,
    pub file_offset: u64,
    pub file_size: u64,
    pub stream_ptr: u32,
    pub stream_size: u32,
    pub spu_thread_enable: u32,
}

/// PNG decoder open info
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecOpnInfo {
    pub init_space_allocated: u32,
}

/// PNG decoder information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecInfo {
    pub image_width: u32,
    pub image_height: u32,
    pub num_components: u32,
    pub color_space: u32,
    pub bit_depth: u32,
    pub interlace_method: u32,
    pub chunk_information: u32,
}

/// PNG decoder in/out parameters
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecInParam {
    pub command_ptr: u32,
    pub down_scale: u32,
    pub color_space: u32,
    pub pack_flag: u32,
    pub alpha_select: u32,
}

/// PNG decoder output parameters
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPngDecOutParam {
    pub output_width: u32,
    pub output_height: u32,
    pub output_components: u32,
    pub output_bit_depth: u32,
    pub output_mode: u32,
    pub output_color_space: u32,
    pub use_memory_space: u32,
}

/// Error codes
pub const CELL_PNGDEC_ERROR_FATAL: i32 = -1;
pub const CELL_PNGDEC_ERROR_ARG: i32 = -2;
pub const CELL_PNGDEC_ERROR_SEQ: i32 = -3;
pub const CELL_PNGDEC_ERROR_BUSY: i32 = -4;
pub const CELL_PNGDEC_ERROR_EMPTY: i32 = -5;

/// PNG color type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngColorType {
    /// Grayscale
    Grayscale = 0,
    /// RGB
    Rgb = 2,
    /// Palette/Indexed
    Palette = 3,
    /// Grayscale + Alpha
    GrayscaleAlpha = 4,
    /// RGBA
    Rgba = 6,
}

/// PNG decoder backend
#[derive(Debug, Clone)]
struct PngDecoder {
    /// Image width
    width: u32,
    /// Image height
    height: u32,
    /// Bit depth (1, 2, 4, 8, 16)
    bit_depth: u8,
    /// Color type
    color_type: PngColorType,
    /// Interlace method (0=none, 1=Adam7)
    interlace: u8,
}

#[allow(dead_code)]
impl PngDecoder {
    /// Create a new PNG decoder
    fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            bit_depth: 8,
            color_type: PngColorType::Rgba,
            interlace: 0,
        }
    }

    /// Parse PNG header (simplified implementation)
    fn parse_header(&mut self, data: &[u8]) -> Result<(), i32> {
        // PNG signature: 137 80 78 71 13 10 26 10
        const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
        
        if data.len() < 8 {
            return Err(CELL_PNGDEC_ERROR_ARG);
        }
        
        if data[0..8] != PNG_SIGNATURE {
            trace!("PngDecoder::parse_header: invalid PNG signature");
            // For HLE, we'll be lenient and use placeholder values
        }
        
        // In a real implementation, we would:
        // 1. Find IHDR chunk
        // 2. Parse width (4 bytes)
        // 3. Parse height (4 bytes)
        // 4. Parse bit depth (1 byte)
        // 5. Parse color type (1 byte)
        // 6. Parse compression method (1 byte)
        // 7. Parse filter method (1 byte)
        // 8. Parse interlace method (1 byte)
        
        // Use placeholder values
        self.width = 1920;
        self.height = 1080;
        self.bit_depth = 8;
        self.color_type = PngColorType::Rgba;
        self.interlace = 0;
        
        trace!("PngDecoder::parse_header: {}x{}, bit_depth={}, color_type={:?}", 
               self.width, self.height, self.bit_depth, self.color_type);
        
        Ok(())
    }

    /// Decode PNG data to RGBA format
    fn decode(&self, _src_data: &[u8], dst_buffer: &mut [u8], out_width: u32, out_height: u32) -> Result<(), i32> {
        trace!("PngDecoder::decode: {}x{} -> {}x{}", self.width, self.height, out_width, out_height);
        
        let pixel_count = (out_width * out_height) as usize;
        let required_size = pixel_count * 4;
        
        if dst_buffer.len() < required_size {
            return Err(CELL_PNGDEC_ERROR_ARG);
        }
        
        // In a real implementation, we would:
        // 1. Decompress IDAT chunks using zlib/inflate
        // 2. Unfilter each scanline (PNG filter types 0-4)
        // 3. Deinterlace if interlace method is Adam7
        // 4. Convert to requested output format based on color type
        
        // For HLE, simulate decoding by filling with test pattern
        for i in 0..pixel_count {
            let x = (i as u32 % out_width) as u8;
            let y = (i as u32 / out_width) as u8;
            
            dst_buffer[i * 4] = x.wrapping_mul(255) / 255;     // R
            dst_buffer[i * 4 + 1] = y.wrapping_mul(255) / 255; // G
            dst_buffer[i * 4 + 2] = 128;                       // B
            dst_buffer[i * 4 + 3] = 255;                       // A
        }
        
        Ok(())
    }

    /// Convert from indexed/palette color to RGBA
    fn convert_palette_to_rgba(&self, palette: &[u8], indices: &[u8], output: &mut [u8]) -> Result<(), i32> {
        trace!("PngDecoder::convert_palette_to_rgba: {} indices", indices.len());
        
        if palette.len() < 3 {
            return Err(CELL_PNGDEC_ERROR_ARG);
        }
        
        for (i, &index) in indices.iter().enumerate() {
            let palette_idx = (index as usize) * 3;
            if palette_idx + 2 < palette.len() && i * 4 + 3 < output.len() {
                output[i * 4] = palette[palette_idx];         // R
                output[i * 4 + 1] = palette[palette_idx + 1]; // G
                output[i * 4 + 2] = palette[palette_idx + 2]; // B
                output[i * 4 + 3] = 255;                      // A
            }
        }
        
        Ok(())
    }

    /// Convert grayscale to RGBA
    fn convert_grayscale_to_rgba(&self, grayscale: &[u8], output: &mut [u8]) -> Result<(), i32> {
        trace!("PngDecoder::convert_grayscale_to_rgba: {} pixels", grayscale.len());
        
        for (i, &gray) in grayscale.iter().enumerate() {
            if i * 4 + 3 < output.len() {
                output[i * 4] = gray;     // R
                output[i * 4 + 1] = gray; // G
                output[i * 4 + 2] = gray; // B
                output[i * 4 + 3] = 255;  // A
            }
        }
        
        Ok(())
    }

    /// Convert grayscale+alpha to RGBA
    fn convert_grayscale_alpha_to_rgba(&self, ga_data: &[u8], output: &mut [u8]) -> Result<(), i32> {
        trace!("PngDecoder::convert_grayscale_alpha_to_rgba: {} bytes", ga_data.len());
        
        for i in 0..(ga_data.len() / 2) {
            if i * 4 + 3 < output.len() && i * 2 + 1 < ga_data.len() {
                let gray = ga_data[i * 2];
                let alpha = ga_data[i * 2 + 1];
                
                output[i * 4] = gray;     // R
                output[i * 4 + 1] = gray; // G
                output[i * 4 + 2] = gray; // B
                output[i * 4 + 3] = alpha; // A
            }
        }
        
        Ok(())
    }

    /// Get number of components for color type
    fn get_components(&self) -> u32 {
        match self.color_type {
            PngColorType::Grayscale => 1,
            PngColorType::Rgb => 3,
            PngColorType::Palette => 1, // Indices
            PngColorType::GrayscaleAlpha => 2,
            PngColorType::Rgba => 4,
        }
    }
}

/// PNG decoder entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PngDecEntry {
    /// Main handle ID
    main_handle: u32,
    /// Sub handles
    sub_handles: HashMap<u32, PngSubDecEntry>,
    /// Next sub handle ID
    next_sub_id: u32,
    /// Max sub handles
    max_sub_handles: u32,
}

/// PNG sub decoder entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PngSubDecEntry {
    /// Sub handle ID
    sub_handle: u32,
    /// Image info
    info: Option<CellPngDecInfo>,
    /// Decode parameters
    in_param: Option<CellPngDecInParam>,
    /// Output parameters
    out_param: Option<CellPngDecOutParam>,
    /// Decoder backend
    decoder: PngDecoder,
}

/// PNG decoder manager
pub struct PngDecManager {
    /// Main decoders
    decoders: HashMap<u32, PngDecEntry>,
    /// Next main handle ID
    next_main_id: u32,
}

impl PngDecManager {
    /// Create a new PNG decoder manager
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
            next_main_id: 1,
        }
    }

    /// Create a PNG decoder instance
    pub fn create(&mut self, max_main_handle: u32) -> Result<u32, i32> {
        if max_main_handle == 0 {
            return Err(CELL_PNGDEC_ERROR_ARG);
        }

        let main_id = self.next_main_id;
        self.next_main_id += 1;

        let entry = PngDecEntry {
            main_handle: main_id,
            sub_handles: HashMap::new(),
            next_sub_id: 1,
            max_sub_handles: max_main_handle,
        };

        self.decoders.insert(main_id, entry);

        trace!("PngDecManager::create: main_id={}", main_id);

        Ok(main_id)
    }

    /// Destroy PNG decoder instance
    pub fn destroy(&mut self, main_handle: u32) -> i32 {
        if let Some(_entry) = self.decoders.remove(&main_handle) {
            trace!("PngDecManager::destroy: main_id={}", main_handle);
            0 // CELL_OK
        } else {
            CELL_PNGDEC_ERROR_ARG
        }
    }

    /// Open PNG for decoding
    pub fn open(&mut self, main_handle: u32) -> Result<u32, i32> {
        if let Some(entry) = self.decoders.get_mut(&main_handle) {
            if entry.sub_handles.len() >= entry.max_sub_handles as usize {
                return Err(CELL_PNGDEC_ERROR_BUSY);
            }

            let sub_id = entry.next_sub_id;
            entry.next_sub_id += 1;

            let sub_entry = PngSubDecEntry {
                sub_handle: sub_id,
                info: None,
                in_param: None,
                out_param: None,
                decoder: PngDecoder::new(),
            };

            entry.sub_handles.insert(sub_id, sub_entry);

            trace!("PngDecManager::open: main_id={}, sub_id={}", main_handle, sub_id);

            Ok(sub_id)
        } else {
            Err(CELL_PNGDEC_ERROR_ARG)
        }
    }

    /// Close PNG decoder
    pub fn close(&mut self, main_handle: u32, sub_handle: u32) -> i32 {
        if let Some(entry) = self.decoders.get_mut(&main_handle) {
            if entry.sub_handles.remove(&sub_handle).is_some() {
                trace!("PngDecManager::close: main_id={}, sub_id={}", main_handle, sub_handle);
                0 // CELL_OK
            } else {
                CELL_PNGDEC_ERROR_ARG
            }
        } else {
            CELL_PNGDEC_ERROR_ARG
        }
    }

    /// Set PNG info
    pub fn set_info(&mut self, main_handle: u32, sub_handle: u32, info: CellPngDecInfo) -> i32 {
        if let Some(entry) = self.decoders.get_mut(&main_handle) {
            if let Some(sub_entry) = entry.sub_handles.get_mut(&sub_handle) {
                sub_entry.info = Some(info);
                trace!("PngDecManager::set_info: main_id={}, sub_id={}, width={}, height={}", 
                    main_handle, sub_handle, info.image_width, info.image_height);
                0 // CELL_OK
            } else {
                CELL_PNGDEC_ERROR_ARG
            }
        } else {
            CELL_PNGDEC_ERROR_ARG
        }
    }

    /// Set decoding parameters
    pub fn set_parameter(&mut self, main_handle: u32, sub_handle: u32, 
                         in_param: CellPngDecInParam, out_param: CellPngDecOutParam) -> i32 {
        if let Some(entry) = self.decoders.get_mut(&main_handle) {
            if let Some(sub_entry) = entry.sub_handles.get_mut(&sub_handle) {
                sub_entry.in_param = Some(in_param);
                sub_entry.out_param = Some(out_param);
                trace!("PngDecManager::set_parameter: main_id={}, sub_id={}", 
                    main_handle, sub_handle);
                0 // CELL_OK
            } else {
                CELL_PNGDEC_ERROR_ARG
            }
        } else {
            CELL_PNGDEC_ERROR_ARG
        }
    }

    /// Check if handle is valid
    pub fn is_valid(&self, main_handle: u32) -> bool {
        self.decoders.contains_key(&main_handle)
    }

    /// Check if sub handle is valid
    pub fn is_sub_valid(&self, main_handle: u32, sub_handle: u32) -> bool {
        if let Some(entry) = self.decoders.get(&main_handle) {
            entry.sub_handles.contains_key(&sub_handle)
        } else {
            false
        }
    }

    /// Get decoder count
    pub fn decoder_count(&self) -> usize {
        self.decoders.len()
    }

    /// Get sub decoder count
    pub fn sub_decoder_count(&self, main_handle: u32) -> usize {
        if let Some(entry) = self.decoders.get(&main_handle) {
            entry.sub_handles.len()
        } else {
            0
        }
    }

    /// Decode PNG data
    pub fn decode_data(&mut self, main_handle: u32, sub_handle: u32, src_data: &[u8], dst_buffer: &mut [u8]) -> Result<(), i32> {
        if let Some(entry) = self.decoders.get_mut(&main_handle) {
            if let Some(sub_entry) = entry.sub_handles.get_mut(&sub_handle) {
                // Parse header if not already done
                if sub_entry.info.is_none() {
                    sub_entry.decoder.parse_header(src_data)?;
                    
                    // Update info
                    let info = CellPngDecInfo {
                        image_width: sub_entry.decoder.width,
                        image_height: sub_entry.decoder.height,
                        num_components: sub_entry.decoder.get_components(),
                        color_space: sub_entry.decoder.color_type as u32,
                        bit_depth: sub_entry.decoder.bit_depth as u32,
                        interlace_method: sub_entry.decoder.interlace as u32,
                        chunk_information: 0,
                    };
                    sub_entry.info = Some(info);
                }
                
                // Decode to output buffer
                let out_width = sub_entry.info.as_ref().map(|i| i.image_width).unwrap_or(1920);
                let out_height = sub_entry.info.as_ref().map(|i| i.image_height).unwrap_or(1080);
                
                sub_entry.decoder.decode(src_data, dst_buffer, out_width, out_height)?;
                
                trace!("PngDecManager::decode_data: main_id={}, sub_id={}", main_handle, sub_handle);
                Ok(())
            } else {
                Err(CELL_PNGDEC_ERROR_ARG)
            }
        } else {
            Err(CELL_PNGDEC_ERROR_ARG)
        }
    }
}

impl Default for PngDecManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellPngDecCreate - Create PNG decoder
///
/// # Arguments
/// * `mainHandle` - Main handle address
/// * `threadInParam` - Thread input parameters
/// * `threadOutParam` - Thread output parameters
///
/// # Returns
/// * 0 on success
pub fn cell_png_dec_create(
    main_handle_addr: u32,
    thread_in_param_addr: u32,
    _thread_out_param_addr: u32,
) -> i32 {
    trace!("cellPngDecCreate(main_handle_addr={:#x}, thread_in_param_addr={:#x})",
        main_handle_addr, thread_in_param_addr);

    // Validate parameters
    if main_handle_addr == 0 || thread_in_param_addr == 0 {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Create PNG decoder instance through global manager
    // Note: actual memory write requires memory subsystem integration
    match crate::context::get_hle_context_mut().png_dec.create(4) { // Default max handles
        Ok(_handle_id) => {
            // Handle ID should be written to main_handle_addr in actual implementation
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellPngDecDestroy - Destroy PNG decoder
///
/// # Arguments
/// * `mainHandle` - Main handle
///
/// # Returns
/// * 0 on success
pub fn cell_png_dec_destroy(main_handle: u32) -> i32 {
    trace!("cellPngDecDestroy(main_handle={})", main_handle);

    // Validate parameters
    if main_handle == 0 {
        return CELL_PNGDEC_ERROR_ARG;
    }

    crate::context::get_hle_context_mut().png_dec.destroy(main_handle)
}

/// cellPngDecOpen - Open PNG for decoding
///
/// # Arguments
/// * `mainHandle` - Main handle
/// * `subHandle` - Sub handle address
/// * `src` - Source information
/// * `openInfo` - Open information
///
/// # Returns
/// * 0 on success
pub fn cell_png_dec_open(
    main_handle: u32,
    sub_handle_addr: u32,
    src_addr: u32,
    _open_info_addr: u32,
) -> i32 {
    trace!("cellPngDecOpen(main_handle={}, sub_handle_addr={:#x}, src_addr={:#x})",
        main_handle, sub_handle_addr, src_addr);

    // Validate parameters
    if main_handle == 0 || sub_handle_addr == 0 || src_addr == 0 {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Open through global manager
    // Note: actual memory write requires memory subsystem integration
    match crate::context::get_hle_context_mut().png_dec.open(main_handle) {
        Ok(_sub_handle) => {
            // Sub handle should be written to sub_handle_addr in actual implementation
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellPngDecClose - Close PNG decoder
///
/// # Arguments
/// * `mainHandle` - Main handle
/// * `subHandle` - Sub handle
///
/// # Returns
/// * 0 on success
pub fn cell_png_dec_close(main_handle: u32, sub_handle: u32) -> i32 {
    trace!("cellPngDecClose(main_handle={}, sub_handle={})", main_handle, sub_handle);

    // Validate parameters
    if main_handle == 0 || sub_handle == 0 {
        return CELL_PNGDEC_ERROR_ARG;
    }

    crate::context::get_hle_context_mut().png_dec.close(main_handle, sub_handle)
}

/// cellPngDecReadHeader - Read PNG header
///
/// # Arguments
/// * `mainHandle` - Main handle
/// * `subHandle` - Sub handle
/// * `info` - Info structure address
///
/// # Returns
/// * 0 on success
pub fn cell_png_dec_read_header(
    main_handle: u32,
    sub_handle: u32,
    info_addr: u32,
) -> i32 {
    trace!("cellPngDecReadHeader(main_handle={}, sub_handle={}, info_addr={:#x})",
        main_handle, sub_handle, info_addr);

    // Validate parameters
    if main_handle == 0 || sub_handle == 0 || info_addr == 0 {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Validate sub handle exists through global manager
    if !crate::context::get_hle_context().png_dec.is_sub_valid(main_handle, sub_handle) {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Set placeholder info through global manager
    // Note: actual PNG header parsing requires file/memory access
    let info = CellPngDecInfo {
        image_width: 1920,
        image_height: 1080,
        num_components: 4,
        color_space: 0,
        bit_depth: 8,
        interlace_method: 0,
        chunk_information: 0,
    };
    crate::context::get_hle_context_mut().png_dec.set_info(main_handle, sub_handle, info);

    0 // CELL_OK
}

/// cellPngDecSetParameter - Set decoding parameters
///
/// # Arguments
/// * `mainHandle` - Main handle
/// * `subHandle` - Sub handle
/// * `inParam` - Input parameters
/// * `outParam` - Output parameters
///
/// # Returns
/// * 0 on success
pub fn cell_png_dec_set_parameter(
    main_handle: u32,
    sub_handle: u32,
    in_param_addr: u32,
    _out_param_addr: u32,
) -> i32 {
    trace!("cellPngDecSetParameter(main_handle={}, sub_handle={}, in_param_addr={:#x})",
        main_handle, sub_handle, in_param_addr);

    // Validate parameters
    if main_handle == 0 || sub_handle == 0 || in_param_addr == 0 {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Validate sub handle exists through global manager
    if !crate::context::get_hle_context().png_dec.is_sub_valid(main_handle, sub_handle) {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Set parameters through global manager
    // Note: actual parameter reading requires memory access
    let in_param = CellPngDecInParam {
        command_ptr: 0,
        down_scale: 1,
        color_space: 0,
        pack_flag: 0,
        alpha_select: 0,
    };
    let out_param = CellPngDecOutParam {
        output_width: 1920,
        output_height: 1080,
        output_components: 4,
        output_bit_depth: 8,
        output_mode: 0,
        output_color_space: 0,
        use_memory_space: 0,
    };
    crate::context::get_hle_context_mut().png_dec.set_parameter(main_handle, sub_handle, in_param, out_param);

    0 // CELL_OK
}

/// cellPngDecDecodeData - Decode PNG data
///
/// # Arguments
/// * `mainHandle` - Main handle
/// * `subHandle` - Sub handle
/// * `data` - Output data buffer address
/// * `dataOutInfo` - Output info address
///
/// # Returns
/// * 0 on success
pub fn cell_png_dec_decode_data(
    main_handle: u32,
    sub_handle: u32,
    data_addr: u32,
    _data_out_info_addr: u32,
) -> i32 {
    trace!("cellPngDecDecodeData(main_handle={}, sub_handle={}, data_addr={:#x})",
        main_handle, sub_handle, data_addr);

    // Validate parameters
    if main_handle == 0 || sub_handle == 0 || data_addr == 0 {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Validate sub handle exists through global manager
    if !crate::context::get_hle_context().png_dec.is_sub_valid(main_handle, sub_handle) {
        return CELL_PNGDEC_ERROR_ARG;
    }

    // Actual PNG decoding requires memory and decoder backend integration
    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_dec_manager() {
        let mut manager = PngDecManager::new();
        
        // Create decoder
        let main_handle = manager.create(4);
        assert!(main_handle.is_ok());
        let main_handle = main_handle.unwrap();
        
        assert!(manager.is_valid(main_handle));
        assert_eq!(manager.decoder_count(), 1);
        
        // Destroy decoder
        assert_eq!(manager.destroy(main_handle), 0);
        assert!(!manager.is_valid(main_handle));
        assert_eq!(manager.decoder_count(), 0);
    }

    #[test]
    fn test_png_dec_manager_open_close() {
        let mut manager = PngDecManager::new();
        
        let main_handle = manager.create(4).unwrap();
        
        // Open sub decoder
        let sub_handle = manager.open(main_handle);
        assert!(sub_handle.is_ok());
        let sub_handle = sub_handle.unwrap();
        
        assert!(manager.is_sub_valid(main_handle, sub_handle));
        assert_eq!(manager.sub_decoder_count(main_handle), 1);
        
        // Close sub decoder
        assert_eq!(manager.close(main_handle, sub_handle), 0);
        assert!(!manager.is_sub_valid(main_handle, sub_handle));
        assert_eq!(manager.sub_decoder_count(main_handle), 0);
        
        manager.destroy(main_handle);
    }

    #[test]
    fn test_png_dec_manager_multiple_sub() {
        let mut manager = PngDecManager::new();
        
        let main_handle = manager.create(4).unwrap();
        
        // Open multiple sub decoders
        let sub1 = manager.open(main_handle).unwrap();
        let sub2 = manager.open(main_handle).unwrap();
        let sub3 = manager.open(main_handle).unwrap();
        
        assert_eq!(manager.sub_decoder_count(main_handle), 3);
        assert_ne!(sub1, sub2);
        assert_ne!(sub2, sub3);
        
        // Close all
        manager.close(main_handle, sub1);
        manager.close(main_handle, sub2);
        manager.close(main_handle, sub3);
        
        assert_eq!(manager.sub_decoder_count(main_handle), 0);
        
        manager.destroy(main_handle);
    }

    #[test]
    fn test_png_dec_manager_max_sub_handles() {
        let mut manager = PngDecManager::new();
        
        let main_handle = manager.create(2).unwrap();
        
        // Open up to max
        let sub1 = manager.open(main_handle).unwrap();
        let sub2 = manager.open(main_handle).unwrap();
        
        // Try to open beyond max
        assert!(manager.open(main_handle).is_err());
        
        manager.close(main_handle, sub1);
        manager.close(main_handle, sub2);
        manager.destroy(main_handle);
    }

    #[test]
    fn test_png_dec_manager_set_info() {
        let mut manager = PngDecManager::new();
        
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle).unwrap();
        
        let info = CellPngDecInfo {
            image_width: 1920,
            image_height: 1080,
            num_components: 4,
            color_space: 0,
            bit_depth: 8,
            interlace_method: 0,
            chunk_information: 0,
        };
        
        assert_eq!(manager.set_info(main_handle, sub_handle, info), 0);
        
        manager.close(main_handle, sub_handle);
        manager.destroy(main_handle);
    }

    #[test]
    fn test_png_dec_manager_set_parameter() {
        let mut manager = PngDecManager::new();
        
        let main_handle = manager.create(4).unwrap();
        let sub_handle = manager.open(main_handle).unwrap();
        
        let in_param = CellPngDecInParam {
            command_ptr: 0,
            down_scale: 1,
            color_space: 0,
            pack_flag: 0,
            alpha_select: 0,
        };
        
        let out_param = CellPngDecOutParam {
            output_width: 1920,
            output_height: 1080,
            output_components: 4,
            output_bit_depth: 8,
            output_mode: 0,
            output_color_space: 0,
            use_memory_space: 0,
        };
        
        assert_eq!(manager.set_parameter(main_handle, sub_handle, in_param, out_param), 0);
        
        manager.close(main_handle, sub_handle);
        manager.destroy(main_handle);
    }

    #[test]
    fn test_png_dec_manager_validation() {
        let mut manager = PngDecManager::new();
        
        // Invalid max handles
        assert!(manager.create(0).is_err());
        
        // Invalid main handle
        assert!(manager.destroy(9999) != 0);
        assert!(manager.open(9999).is_err());
        
        let main_handle = manager.create(4).unwrap();
        
        // Invalid sub handle
        assert!(manager.close(main_handle, 9999) != 0);
        
        manager.destroy(main_handle);
    }

    #[test]
    fn test_png_dec_create() {
        let result = cell_png_dec_create(0x10000000, 0x10001000, 0x10002000);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_png_dec_create_validation() {
        // Null main handle
        assert!(cell_png_dec_create(0, 0x10001000, 0x10002000) != 0);
        
        // Null thread in param
        assert!(cell_png_dec_create(0x10000000, 0, 0x10002000) != 0);
    }

    #[test]
    fn test_png_dec_destroy_validation() {
        // Invalid handle
        assert!(cell_png_dec_destroy(0) != 0);
    }

    #[test]
    fn test_png_dec_open_validation() {
        // Invalid main handle
        assert!(cell_png_dec_open(0, 0x10000000, 0x10001000, 0x10002000) != 0);
        
        // Null sub handle addr
        assert!(cell_png_dec_open(1, 0, 0x10001000, 0x10002000) != 0);
        
        // Null src addr
        assert!(cell_png_dec_open(1, 0x10000000, 0, 0x10002000) != 0);
    }

    #[test]
    fn test_png_dec_close_validation() {
        // Invalid handles
        assert!(cell_png_dec_close(0, 1) != 0);
        assert!(cell_png_dec_close(1, 0) != 0);
    }

    #[test]
    fn test_png_dec_read_header_validation() {
        // Invalid handles
        assert!(cell_png_dec_read_header(0, 1, 0x10000000) != 0);
        assert!(cell_png_dec_read_header(1, 0, 0x10000000) != 0);
        
        // Null info addr
        assert!(cell_png_dec_read_header(1, 1, 0) != 0);
    }

    #[test]
    fn test_png_dec_set_parameter_validation() {
        // Invalid handles
        assert!(cell_png_dec_set_parameter(0, 1, 0x10000000, 0x10001000) != 0);
        assert!(cell_png_dec_set_parameter(1, 0, 0x10000000, 0x10001000) != 0);
        
        // Null in param
        assert!(cell_png_dec_set_parameter(1, 1, 0, 0x10001000) != 0);
    }

    #[test]
    fn test_png_dec_decode_data_validation() {
        // Invalid handles
        assert!(cell_png_dec_decode_data(0, 1, 0x10000000, 0x10001000) != 0);
        assert!(cell_png_dec_decode_data(1, 0, 0x10000000, 0x10001000) != 0);
        
        // Null data addr
        assert!(cell_png_dec_decode_data(1, 1, 0, 0x10001000) != 0);
    }

    #[test]
    fn test_png_dec_error_codes() {
        assert_eq!(CELL_PNGDEC_ERROR_FATAL, -1);
        assert_eq!(CELL_PNGDEC_ERROR_ARG, -2);
        assert_eq!(CELL_PNGDEC_ERROR_SEQ, -3);
        assert_eq!(CELL_PNGDEC_ERROR_BUSY, -4);
        assert_eq!(CELL_PNGDEC_ERROR_EMPTY, -5);
    }

    #[test]
    fn test_png_dec_structures() {
        let info = CellPngDecInfo {
            image_width: 1920,
            image_height: 1080,
            num_components: 3,
            color_space: 0,
            bit_depth: 8,
            interlace_method: 0,
            chunk_information: 0,
        };
        assert_eq!(info.image_width, 1920);
        assert_eq!(info.image_height, 1080);
    }
}
