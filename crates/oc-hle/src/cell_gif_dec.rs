//! cellGifDec HLE - GIF image decoding module
//!
//! This module provides HLE implementations for the PS3's GIF decoding library.

use std::collections::HashMap;
use tracing::trace;

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
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct GifFrame {
    /// Frame delay in centiseconds (1/100s)
    delay: u16,
    /// X offset
    x_offset: u16,
    /// Y offset
    y_offset: u16,
    /// Frame width
    width: u16,
    /// Frame height
    height: u16,
    /// Disposal method
    disposal_method: GifDisposalMethod,
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

    /// Parse GIF header
    fn parse_header(&mut self, data: &[u8]) -> Result<(), i32> {
        // GIF signature: "GIF87a" or "GIF89a"
        if data.len() < 6 {
            return Err(CELL_GIFDEC_ERROR_ARG);
        }

        let signature = &data[0..3];
        if signature != b"GIF" {
            trace!("GifDecoder::parse_header: invalid GIF signature");
            // Be lenient for HLE
        }

        // In a real implementation:
        // 1. Parse logical screen descriptor (width, height, global color table flag)
        // 2. Parse global color table if present
        // 3. Parse image descriptor and local color table for each frame
        // 4. Parse graphic control extension for animation timing and disposal
        // 5. Decompress LZW-compressed image data

        // Use placeholder values
        self.width = 640;
        self.height = 480;
        
        // Create a simple palette
        self.global_palette = vec![0u8; 768]; // 256 colors * 3 (RGB)
        for i in 0..256 {
            self.global_palette[i * 3] = i as u8;
            self.global_palette[i * 3 + 1] = i as u8;
            self.global_palette[i * 3 + 2] = i as u8;
        }
        
        trace!("GifDecoder::parse_header: {}x{}", self.width, self.height);
        
        Ok(())
    }

    /// Decompress LZW-compressed data
    fn decompress_lzw(&self, compressed: &[u8], min_code_size: u8) -> Vec<u8> {
        trace!("GifDecoder::decompress_lzw: {} bytes, min_code_size={}", compressed.len(), min_code_size);
        
        // In a real implementation:
        // 1. Initialize code table with min_code_size
        // 2. Read variable-length codes from bit stream
        // 3. Build dictionary on-the-fly
        // 4. Output decompressed pixel indices
        
        // Simulate decompressed data
        let pixel_count = (self.width * self.height) as usize;
        let mut output = vec![0u8; pixel_count];
        
        for i in 0..pixel_count {
            output[i] = ((i * 255) / pixel_count) as u8;
        }
        
        output
    }

    /// Parse and add a frame
    fn add_frame(&mut self, frame_data: &[u8], delay: u16, disposal: GifDisposalMethod) -> Result<(), i32> {
        trace!("GifDecoder::add_frame: frame {}, delay={}cs", self.frames.len(), delay);
        
        // Decompress frame data (LZW with min code size 8)
        let decompressed = self.decompress_lzw(frame_data, 8);
        
        let frame = GifFrame {
            delay,
            x_offset: 0,
            y_offset: 0,
            width: self.width as u16,
            height: self.height as u16,
            disposal_method: disposal,
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
        
        // Convert indexed color to RGBA using global palette
        for i in 0..pixel_count.min(frame.data.len()) {
            let palette_index = frame.data[i] as usize;
            let palette_offset = palette_index * 3;
            
            if palette_offset + 2 < self.global_palette.len() {
                dst_buffer[i * 4] = self.global_palette[palette_offset];         // R
                dst_buffer[i * 4 + 1] = self.global_palette[palette_offset + 1]; // G
                dst_buffer[i * 4 + 2] = self.global_palette[palette_offset + 2]; // B
                dst_buffer[i * 4 + 3] = 255;                                     // A (opaque)
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

    // Placeholder dimensions until actual GIF parsing is implemented
    // TODO: Parse actual GIF header to get real dimensions
    const PLACEHOLDER_GIF_WIDTH: u32 = 256;
    const PLACEHOLDER_GIF_HEIGHT: u32 = 256;
    const PLACEHOLDER_NUM_COMPONENTS: u32 = 4; // RGBA
    const PLACEHOLDER_COLOR_SPACE: u32 = 0; // RGB

    unsafe {
        let src_addr = (*src).stream_ptr;
        let src_size = (*src).stream_size;
        
        match crate::context::get_hle_context_mut().gif_dec.open(main_handle, src_addr, src_size) {
            Ok(handle) => {
                (*sub_handle).sub_handle = handle;
                if !out_param.is_null() {
                    (*out_param).width = PLACEHOLDER_GIF_WIDTH;
                    (*out_param).height = PLACEHOLDER_GIF_HEIGHT;
                    (*out_param).num_components = PLACEHOLDER_NUM_COMPONENTS;
                    (*out_param).color_space = PLACEHOLDER_COLOR_SPACE;
                }
                0 // CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellGifDecReadHeader - Read GIF header
pub unsafe fn cell_gif_dec_read_header(
    main_handle: u32,
    sub_handle: u32,
    info: *mut CellGifDecOutParam,
) -> i32 {
    trace!("cellGifDecReadHeader called");
    
    if info.is_null() {
        return CELL_GIFDEC_ERROR_ARG;
    }

    // Placeholder dimensions until actual GIF parsing is implemented
    // TODO: Parse actual GIF header to get real dimensions
    const PLACEHOLDER_GIF_WIDTH: u32 = 256;
    const PLACEHOLDER_GIF_HEIGHT: u32 = 256;
    
    match crate::context::get_hle_context_mut().gif_dec.read_header(main_handle, sub_handle, PLACEHOLDER_GIF_WIDTH, PLACEHOLDER_GIF_HEIGHT) {
        Ok(_) => {
            match crate::context::get_hle_context().gif_dec.get_info(main_handle, sub_handle) {
                Ok(result_info) => {
                    unsafe {
                        (*info).width = result_info.width;
                        (*info).height = result_info.height;
                        (*info).num_components = result_info.num_components;
                        (*info).color_space = result_info.color_space;
                    }
                    0 // CELL_OK
                }
                Err(e) => e,
            }
        }
        Err(e) => e,
    }
}

/// cellGifDecDecodeData - Decode GIF data
pub fn cell_gif_dec_decode_data(
    _main_handle: u32,
    _sub_handle: u32,
    data: *mut u8,
    _data_ctrl_param: *const u32,
    _data_out_info: *mut u32,
) -> i32 {
    trace!("cellGifDecDecodeData called");
    
    if data.is_null() {
        return CELL_GIFDEC_ERROR_ARG;
    }

    // TODO: Implement actual GIF decoding
    // Should read from sub_handle's source and decode to data buffer
    
    0 // CELL_OK
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
        
        let result = cell_gif_dec_create(&mut main_handle, &thread_in, &mut thread_out);
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
        let result = cell_gif_dec_create(std::ptr::null_mut(), &thread_in, std::ptr::null_mut());
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
        let result = cell_gif_dec_open(1, std::ptr::null_mut(), &src, std::ptr::null_mut());
        assert_eq!(result, CELL_GIFDEC_ERROR_ARG);
    }

    #[test]
    fn test_gif_dec_read_header_validation() {
        // Null info
        let result = cell_gif_dec_read_header(1, 1, std::ptr::null_mut());
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
        
        assert_eq!(cell_gif_dec_create(&mut main_handle, &thread_in, &mut thread_out), 0);
        assert_eq!(cell_gif_dec_destroy(main_handle.main_handle), 0);
    }
}
