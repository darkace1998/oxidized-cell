//! cellGcmSys HLE - Graphics Command Management System
//!
//! This module provides HLE implementations for the PS3's RSX graphics system.
//! It manages display buffers, graphics memory, and the command FIFO.

use tracing::{debug, trace};

/// Maximum number of display buffers
pub const CELL_GCM_MAX_DISPLAY_BUFFERS: usize = 8;

/// GCM configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGcmConfig {
    /// Local memory address
    pub local_addr: u32,
    /// Local memory size
    pub local_size: u32,
    /// I/O memory address (main memory mapped for RSX)
    pub io_addr: u32,
    /// I/O memory size
    pub io_size: u32,
    /// Memory frequency (MHz)
    pub mem_frequency: u32,
    /// Core frequency (MHz)
    pub core_frequency: u32,
}

impl Default for CellGcmConfig {
    fn default() -> Self {
        Self {
            local_addr: 0xC0000000,  // RSX local memory base
            local_size: 256 * 1024 * 1024,  // 256 MB
            io_addr: 0,
            io_size: 0,
            mem_frequency: 650,  // MHz
            core_frequency: 500,  // MHz
        }
    }
}

/// GCM display buffer
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellGcmDisplayBuffer {
    /// Buffer address offset
    pub offset: u32,
    /// Pitch (bytes per line)
    pub pitch: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// GCM flip mode
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellGcmFlipMode {
    /// VSYNC mode (wait for vertical sync)
    Vsync = 1,
    /// HSYNC mode (wait for horizontal sync)
    Hsync = 2,
}

impl Default for CellGcmFlipMode {
    fn default() -> Self {
        CellGcmFlipMode::Vsync
    }
}

/// GCM manager state
pub struct GcmManager {
    /// Initialization flag
    initialized: bool,
    /// Current configuration
    config: CellGcmConfig,
    /// Display buffers
    display_buffers: [CellGcmDisplayBuffer; CELL_GCM_MAX_DISPLAY_BUFFERS],
    /// Current flip mode
    flip_mode: CellGcmFlipMode,
    /// Current display buffer
    current_buffer: u32,
    /// Command buffer context address
    context_addr: u32,
    /// Command buffer size
    context_size: u32,
}

impl GcmManager {
    /// Create a new GCM manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            config: CellGcmConfig::default(),
            display_buffers: [CellGcmDisplayBuffer::default(); CELL_GCM_MAX_DISPLAY_BUFFERS],
            flip_mode: CellGcmFlipMode::default(),
            current_buffer: 0,
            context_addr: 0,
            context_size: 0,
        }
    }

    /// Initialize GCM system
    pub fn init(&mut self, context_addr: u32, context_size: u32) -> i32 {
        if self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        debug!(
            "GcmManager::init: context_addr=0x{:08X}, context_size=0x{:X}",
            context_addr, context_size
        );

        self.context_addr = context_addr;
        self.context_size = context_size;
        self.initialized = true;

        // TODO: Initialize RSX command buffer
        // TODO: Set up graphics memory allocation
        // TODO: Configure display settings

        0 // CELL_OK
    }

    /// Set flip mode
    pub fn set_flip_mode(&mut self, mode: CellGcmFlipMode) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        trace!("GcmManager::set_flip_mode: {:?}", mode);
        self.flip_mode = mode;

        // TODO: Configure flip mode in RSX

        0 // CELL_OK
    }

    /// Set flip (queue buffer swap)
    pub fn set_flip(&mut self, buffer_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }

        trace!("GcmManager::set_flip: buffer_id={}", buffer_id);
        self.current_buffer = buffer_id;

        // TODO: Queue flip command to RSX
        // TODO: Update current display buffer

        0 // CELL_OK
    }

    /// Set display buffer configuration
    pub fn set_display_buffer(
        &mut self,
        buffer_id: u32,
        offset: u32,
        pitch: u32,
        width: u32,
        height: u32,
    ) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }

        // Validate buffer parameters
        if width == 0 || height == 0 || pitch == 0 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }

        debug!(
            "GcmManager::set_display_buffer: id={}, offset=0x{:X}, pitch={}, {}x{}",
            buffer_id, offset, pitch, width, height
        );

        // Store buffer configuration
        self.display_buffers[buffer_id as usize] = CellGcmDisplayBuffer {
            offset,
            pitch,
            width,
            height,
        };

        // TODO: Configure display buffer in RSX

        0 // CELL_OK
    }

    /// Get current configuration
    pub fn get_configuration(&self) -> CellGcmConfig {
        self.config
    }

    /// Convert address to RSX offset
    pub fn address_to_offset(&self, address: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410001u32 as i32); // CELL_GCM_ERROR_FAILURE
        }

        // Check if address is in RSX local memory
        if address >= self.config.local_addr
            && address < (self.config.local_addr + self.config.local_size)
        {
            Ok(address - self.config.local_addr)
        }
        // Check if address is in I/O memory (main memory mapped for RSX)
        else if self.config.io_size > 0
            && address >= self.config.io_addr
            && address < (self.config.io_addr + self.config.io_size)
        {
            Ok(address - self.config.io_addr + self.config.local_size)
        } else {
            Err(0x80410002u32 as i32) // CELL_GCM_ERROR_INVALID_VALUE
        }
    }

    /// Get display buffer info
    pub fn get_display_buffer(&self, buffer_id: u32) -> Option<&CellGcmDisplayBuffer> {
        if buffer_id < CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
            Some(&self.display_buffers[buffer_id as usize])
        } else {
            None
        }
    }
}

impl Default for GcmManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellGcmInit - Initialize the graphics system
///
/// # Arguments
/// * `context_addr` - Address for command buffer context
/// * `context_size` - Size of command buffer
/// * `config` - Configuration structure
///
/// # Returns
/// * 0 on success
/// * Error code on failure
pub fn cell_gcm_init(context_addr: u32, context_size: u32, _config_addr: u32) -> i32 {
    debug!(
        "cellGcmInit(context_addr=0x{:08X}, context_size=0x{:X})",
        context_addr, context_size
    );

    // Validate parameters
    if context_size < 1024 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    crate::context::get_hle_context_mut().gcm.init(context_addr, context_size)
}

/// cellGcmSetFlipMode - Set display flip mode
///
/// # Arguments
/// * `mode` - Flip mode (VSYNC or HSYNC)
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_flip_mode(mode: u32) -> i32 {
    let flip_mode = if mode == 1 {
        CellGcmFlipMode::Vsync
    } else {
        CellGcmFlipMode::Hsync
    };
    trace!("cellGcmSetFlipMode(mode={:?})", flip_mode);

    crate::context::get_hle_context_mut().gcm.set_flip_mode(flip_mode)
}

/// cellGcmSetFlip - Flip display buffer
///
/// # Arguments
/// * `buffer_id` - Buffer ID to flip to
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_flip(buffer_id: u32) -> i32 {
    trace!("cellGcmSetFlip(buffer_id={})", buffer_id);

    if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    crate::context::get_hle_context_mut().gcm.set_flip(buffer_id)
}

/// cellGcmSetDisplayBuffer - Configure display buffer
///
/// # Arguments
/// * `buffer_id` - Buffer ID (0 or 1 for double buffering)
/// * `offset` - Memory offset of buffer
/// * `pitch` - Pitch (bytes per line)
/// * `width` - Width in pixels
/// * `height` - Height in pixels
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_display_buffer(
    buffer_id: u32,
    offset: u32,
    pitch: u32,
    width: u32,
    height: u32,
) -> i32 {
    debug!(
        "cellGcmSetDisplayBuffer(id={}, offset=0x{:X}, pitch={}, {}x{})",
        buffer_id, offset, pitch, width, height
    );

    // Validate parameters
    if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    if width == 0 || height == 0 || pitch == 0 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    crate::context::get_hle_context_mut().gcm.set_display_buffer(buffer_id, offset, pitch, width, height)
}

/// cellGcmGetConfiguration - Get current GCM configuration
///
/// # Arguments
/// * `config_addr` - Address to write configuration to
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_get_configuration(_config_addr: u32) -> i32 {
    trace!("cellGcmGetConfiguration()");

    let _config = crate::context::get_hle_context().gcm.get_configuration();
    // TODO: Write configuration to memory at _config_addr

    0 // CELL_OK
}

/// cellGcmAddressToOffset - Convert memory address to RSX offset
///
/// # Arguments
/// * `address` - Memory address
/// * `offset_addr` - Address to write offset to
///
/// # Returns
/// * 0 on success
/// * Error code if address is invalid
pub fn cell_gcm_address_to_offset(address: u32, _offset_addr: u32) -> i32 {
    trace!("cellGcmAddressToOffset(address=0x{:08X})", address);

    match crate::context::get_hle_context().gcm.address_to_offset(address) {
        Ok(_offset) => {
            // TODO: Write offset to memory at _offset_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellGcmGetTiledPitchSize - Calculate pitch size for tiled memory
///
/// # Arguments
/// * `pitch` - Pitch in pixels
///
/// # Returns
/// * Aligned pitch size
pub fn cell_gcm_get_tiled_pitch_size(pitch: u32) -> u32 {
    trace!("cellGcmGetTiledPitchSize(pitch={})", pitch);

    // Align to 64 bytes (minimum tile granularity)
    (pitch + 63) & !63
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcm_manager() {
        let mut manager = GcmManager::new();
        assert_eq!(manager.init(0x10000000, 1024 * 1024), 0);
        
        // Test display buffer configuration
        assert_eq!(manager.set_display_buffer(0, 0x1000, 1920 * 4, 1920, 1080), 0);
        
        // Test flip mode
        assert_eq!(manager.set_flip_mode(CellGcmFlipMode::Vsync), 0);
        
        // Test flip
        assert_eq!(manager.set_flip(0), 0);
        
        // Test configuration retrieval
        let config = manager.get_configuration();
        assert_eq!(config.local_addr, 0xC0000000);
    }

    #[test]
    fn test_gcm_manager_address_conversion() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Test valid local memory address
        let offset = manager.address_to_offset(0xC0001000);
        assert!(offset.is_ok());
        assert_eq!(offset.unwrap(), 0x1000);
        
        // Test invalid address
        let invalid = manager.address_to_offset(0x12345678);
        assert!(invalid.is_err());
    }

    #[test]
    fn test_gcm_manager_validation() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Test invalid buffer ID
        assert!(manager.set_display_buffer(99, 0, 1920, 1920, 1080) != 0);
        
        // Test invalid dimensions
        assert!(manager.set_display_buffer(0, 0, 0, 0, 0) != 0);
        
        // Test invalid flip buffer
        assert!(manager.set_flip(99) != 0);
    }

    #[test]
    fn test_gcm_init() {
        // Reset context first to ensure clean state
        crate::context::reset_hle_context();
        
        let result = cell_gcm_init(0x10000000, 1024 * 1024, 0);
        assert_eq!(result, 0);
        
        // Reset context to test invalid size
        crate::context::reset_hle_context();
        
        // Test invalid context size
        let result = cell_gcm_init(0x10000000, 512, 0);
        assert!(result != 0);
    }

    #[test]
    fn test_gcm_config_default() {
        let config = CellGcmConfig::default();
        assert_eq!(config.local_addr, 0xC0000000);
        assert_eq!(config.local_size, 256 * 1024 * 1024);
    }

    #[test]
    fn test_set_flip_mode() {
        // Reset context and initialize GCM
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        let result = cell_gcm_set_flip_mode(1);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_display_buffer_validation() {
        // Reset context and initialize GCM
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        // Valid call
        assert_eq!(cell_gcm_set_display_buffer(0, 0x1000, 1920 * 4, 1920, 1080), 0);
        
        // Invalid buffer ID
        assert!(cell_gcm_set_display_buffer(99, 0x1000, 1920 * 4, 1920, 1080) != 0);
        
        // Invalid dimensions
        assert!(cell_gcm_set_display_buffer(0, 0x1000, 0, 0, 0) != 0);
    }

    #[test]
    fn test_tiled_pitch_size() {
        assert_eq!(cell_gcm_get_tiled_pitch_size(100), 128);
        assert_eq!(cell_gcm_get_tiled_pitch_size(64), 64);
        assert_eq!(cell_gcm_get_tiled_pitch_size(65), 128);
    }
}
