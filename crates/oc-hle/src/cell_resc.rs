//! cellResc HLE - Resolution Scaler
//!
//! This module provides HLE implementations for the PS3's resolution scaling library.
//! It handles resolution conversion, aspect ratio handling, and upscaling/downscaling.

use tracing::{debug, trace};

/// Error codes
pub const CELL_RESC_ERROR_NOT_INITIALIZED: i32 = 0x80210301u32 as i32;
pub const CELL_RESC_ERROR_REINITIALIZED: i32 = 0x80210302u32 as i32;
pub const CELL_RESC_ERROR_BAD_ALIGNMENT: i32 = 0x80210303u32 as i32;
pub const CELL_RESC_ERROR_BAD_ARGUMENT: i32 = 0x80210304u32 as i32;
pub const CELL_RESC_ERROR_LESS_MEMORY: i32 = 0x80210305u32 as i32;
pub const CELL_RESC_ERROR_GCM_FLIP_QUE_FULL: i32 = 0x80210306u32 as i32;

/// Display mode flags
pub const CELL_RESC_720x480: u32 = 0x01;
pub const CELL_RESC_720x576: u32 = 0x02;
pub const CELL_RESC_1280x720: u32 = 0x04;
pub const CELL_RESC_1920x1080: u32 = 0x08;

/// Palette format
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellRescPalTemporalMode {
    /// No temporal filter
    None = 0,
    /// 50Hz temporal filter
    Filter50 = 1,
    /// 60Hz temporal filter
    Filter60 = 2,
}

impl Default for CellRescPalTemporalMode {
    fn default() -> Self {
        CellRescPalTemporalMode::None
    }
}

/// Buffer mode
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellRescBufferMode {
    /// Single buffer
    A1B1 = 0,
    /// Double buffer (alternate)
    A2B2 = 1,
}

impl Default for CellRescBufferMode {
    fn default() -> Self {
        CellRescBufferMode::A1B1
    }
}

/// Aspect ratio
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellRescRatioConvertMode {
    /// Letterbox (maintain aspect with black bars)
    Letterbox = 0,
    /// Full screen (stretch to fill)
    FullScreen = 1,
    /// Pan and scan (crop to fill)
    PanScan = 2,
}

impl Default for CellRescRatioConvertMode {
    fn default() -> Self {
        CellRescRatioConvertMode::Letterbox
    }
}

/// RESC initialization parameters
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellRescInitConfig {
    /// Size of structure
    pub size: u32,
    /// Resource policy
    pub resource_policy: u32,
    /// Display modes supported
    pub display_modes: u32,
    /// Interpolation mode
    pub interpolation_mode: u32,
    /// Interlace filter (0 = off, 1 = on)
    pub interlace_filter: u32,
}

impl Default for CellRescInitConfig {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            resource_policy: 0,
            display_modes: CELL_RESC_720x480 | CELL_RESC_720x576 | CELL_RESC_1280x720 | CELL_RESC_1920x1080,
            interpolation_mode: 0,
            interlace_filter: 0,
        }
    }
}

/// Source info
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellRescSrc {
    /// Color format
    pub format: u32,
    /// Pitch
    pub pitch: u32,
    /// Width
    pub width: u16,
    /// Height
    pub height: u16,
    /// Source offset
    pub offset: u32,
}

/// Destination info
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellRescDsts {
    /// Format
    pub format: u32,
    /// Pitch
    pub pitch: u32,
    /// Width (in bytes)
    pub width_byte: u32,
    /// Height
    pub height: u32,
}

/// RESC manager state
pub struct RescManager {
    /// Initialization flag
    initialized: bool,
    /// Configuration
    config: CellRescInitConfig,
    /// Source buffer info
    src: CellRescSrc,
    /// Destination info for different resolutions
    dsts: [CellRescDsts; 4],
    /// Current display mode
    display_mode: u32,
    /// Buffer mode
    buffer_mode: CellRescBufferMode,
    /// PAL temporal mode
    pal_temporal_mode: CellRescPalTemporalMode,
    /// Aspect ratio mode
    ratio_mode: CellRescRatioConvertMode,
    /// Flip handler set
    flip_handler_set: bool,
    /// RSX scaling enabled
    rsx_scaling_enabled: bool,
    /// Current scale factor X
    scale_x: f32,
    /// Current scale factor Y
    scale_y: f32,
    /// Bilinear filtering enabled
    bilinear_filter: bool,
    /// Flip count (for synchronization)
    flip_count: u64,
}

impl RescManager {
    /// Create a new RESC manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            config: CellRescInitConfig::default(),
            src: CellRescSrc::default(),
            dsts: [CellRescDsts::default(); 4],
            display_mode: CELL_RESC_1280x720,
            buffer_mode: CellRescBufferMode::default(),
            pal_temporal_mode: CellRescPalTemporalMode::default(),
            ratio_mode: CellRescRatioConvertMode::default(),
            flip_handler_set: false,
            rsx_scaling_enabled: false,
            scale_x: 1.0,
            scale_y: 1.0,
            bilinear_filter: true,
            flip_count: 0,
        }
    }

    /// Initialize RESC
    pub fn init(&mut self, config: CellRescInitConfig) -> i32 {
        if self.initialized {
            return CELL_RESC_ERROR_REINITIALIZED;
        }

        debug!("RescManager::init: display_modes=0x{:X}", config.display_modes);

        self.config = config;
        self.initialized = true;
        
        // Initialize RSX scaling connection
        self.rsx_scaling_enabled = true;
        // Enable bilinear filter by default (mode 0) or based on config
        // interpolation_mode: 0 = bilinear, 1 = 4-tap, etc.
        self.bilinear_filter = true; // Default to enabled
        
        debug!("RescManager RSX scaling initialized: bilinear={}", self.bilinear_filter);

        0 // CELL_OK
    }

    /// Exit/cleanup RESC
    pub fn exit(&mut self) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        debug!("RescManager::exit");

        self.initialized = false;
        self.flip_handler_set = false;

        0 // CELL_OK
    }

    /// Set source buffer info
    pub fn set_src(&mut self, src: CellRescSrc) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        trace!("RescManager::set_src: {}x{}, pitch={}", src.width, src.height, src.pitch);

        self.src = src;

        0 // CELL_OK
    }

    /// Set destination info for a display mode
    pub fn set_dst(&mut self, mode_index: u32, dst: CellRescDsts) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        if mode_index >= 4 {
            return CELL_RESC_ERROR_BAD_ARGUMENT;
        }

        trace!("RescManager::set_dst: mode={}, pitch={}", mode_index, dst.pitch);

        self.dsts[mode_index as usize] = dst;

        0 // CELL_OK
    }

    /// Set display mode
    pub fn set_display_mode(&mut self, mode: u32) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        // Validate mode is one of supported modes
        const VALID_MODES: [u32; 4] = [
            CELL_RESC_720x480,
            CELL_RESC_720x576,
            CELL_RESC_1280x720,
            CELL_RESC_1920x1080,
        ];
        
        if !VALID_MODES.contains(&mode) {
            return CELL_RESC_ERROR_BAD_ARGUMENT;
        }

        debug!("RescManager::set_display_mode: 0x{:X}", mode);

        self.display_mode = mode;

        0 // CELL_OK
    }

    /// Set buffer mode
    pub fn set_buffer_mode(&mut self, mode: CellRescBufferMode) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        trace!("RescManager::set_buffer_mode: {:?}", mode);

        self.buffer_mode = mode;

        0 // CELL_OK
    }

    /// Set PAL temporal mode
    pub fn set_pal_temporal_mode(&mut self, mode: CellRescPalTemporalMode) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        trace!("RescManager::set_pal_temporal_mode: {:?}", mode);

        self.pal_temporal_mode = mode;

        0 // CELL_OK
    }

    /// Set aspect ratio conversion mode
    pub fn set_ratio_convert_mode(&mut self, mode: CellRescRatioConvertMode) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        trace!("RescManager::set_ratio_convert_mode: {:?}", mode);

        self.ratio_mode = mode;

        0 // CELL_OK
    }

    /// Set flip handler
    pub fn set_flip_handler(&mut self, _handler: u32) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        trace!("RescManager::set_flip_handler");

        self.flip_handler_set = true;

        0 // CELL_OK
    }

    /// Set VSReporter (vertical sync reporter)
    pub fn set_vs_reporter(&mut self, _reporter: u32) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        trace!("RescManager::set_vs_reporter");

        0 // CELL_OK
    }

    /// Get number of display buffers
    pub fn get_num_display_buffers(&self) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_RESC_ERROR_NOT_INITIALIZED);
        }

        // Based on buffer mode
        match self.buffer_mode {
            CellRescBufferMode::A1B1 => Ok(1),
            CellRescBufferMode::A2B2 => Ok(2),
        }
    }

    /// Get display buffer size
    pub fn get_display_buffer_size(&self) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_RESC_ERROR_NOT_INITIALIZED);
        }

        // Calculate based on display mode
        let (width, height) = match self.display_mode {
            CELL_RESC_720x480 => (720, 480),
            CELL_RESC_720x576 => (720, 576),
            CELL_RESC_1280x720 => (1280, 720),
            CELL_RESC_1920x1080 => (1920, 1080),
            _ => (1280, 720),
        };

        // Assume 4 bytes per pixel (ARGB)
        Ok(width * height * 4)
    }

    /// Get last flip time
    pub fn get_last_flip_time(&self) -> Result<u64, i32> {
        if !self.initialized {
            return Err(CELL_RESC_ERROR_NOT_INITIALIZED);
        }

        // Return simulated time
        Ok(0)
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get current display mode
    pub fn get_display_mode(&self) -> u32 {
        self.display_mode
    }

    // ========================================================================
    // RSX Backend Integration for Scaling
    // ========================================================================

    /// Check if RSX scaling is enabled
    pub fn is_rsx_scaling_enabled(&self) -> bool {
        self.rsx_scaling_enabled
    }

    /// Get current scale factors
    pub fn get_scale_factors(&self) -> (f32, f32) {
        (self.scale_x, self.scale_y)
    }

    /// Calculate scale factors based on source and destination
    pub fn calculate_scale_factors(&mut self) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        // Get destination dimensions based on display mode
        let (dst_width, dst_height) = match self.display_mode {
            CELL_RESC_720x480 => (720u32, 480u32),
            CELL_RESC_720x576 => (720, 576),
            CELL_RESC_1280x720 => (1280, 720),
            CELL_RESC_1920x1080 => (1920, 1080),
            _ => (1280, 720),
        };

        let src_width = self.src.width as u32;
        let src_height = self.src.height as u32;

        if src_width > 0 && src_height > 0 {
            self.scale_x = dst_width as f32 / src_width as f32;
            self.scale_y = dst_height as f32 / src_height as f32;

            // Apply aspect ratio mode
            match self.ratio_mode {
                CellRescRatioConvertMode::Letterbox => {
                    // Maintain aspect ratio, use minimum scale
                    let min_scale = self.scale_x.min(self.scale_y);
                    self.scale_x = min_scale;
                    self.scale_y = min_scale;
                }
                CellRescRatioConvertMode::FullScreen => {
                    // Use independent scales (stretch)
                }
                CellRescRatioConvertMode::PanScan => {
                    // Maintain aspect ratio, use maximum scale (will crop)
                    let max_scale = self.scale_x.max(self.scale_y);
                    self.scale_x = max_scale;
                    self.scale_y = max_scale;
                }
            }

            debug!(
                "RescManager: calculated scale factors: x={:.3}, y={:.3}",
                self.scale_x, self.scale_y
            );
        }

        0 // CELL_OK
    }

    /// Perform scaling and flip operation (RSX integration point)
    pub fn convert_and_flip(&mut self, buffer_idx: u32) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        trace!("RescManager::convert_and_flip: buffer_idx={}", buffer_idx);

        // Calculate scale factors if not already done
        if self.scale_x == 1.0 && self.scale_y == 1.0 {
            self.calculate_scale_factors();
        }

        // In a real implementation, this would:
        // 1. Read source buffer from RSX memory
        // 2. Apply scaling using the calculated factors
        // 3. Apply bilinear filtering if enabled
        // 4. Write to destination buffer
        // 5. Queue flip command to RSX

        self.flip_count += 1;

        debug!(
            "RescManager: convert_and_flip completed, flip_count={}",
            self.flip_count
        );

        0 // CELL_OK
    }

    /// Get flip count for synchronization
    pub fn get_flip_count(&self) -> u64 {
        self.flip_count
    }

    /// Set bilinear filter mode
    pub fn set_bilinear_filter(&mut self, enable: bool) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        self.bilinear_filter = enable;
        trace!("RescManager::set_bilinear_filter: {}", enable);

        0 // CELL_OK
    }

    /// Check if bilinear filter is enabled
    pub fn is_bilinear_filter_enabled(&self) -> bool {
        self.bilinear_filter
    }
}

impl Default for RescManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellRescInit - Initialize RESC
///
/// # Arguments
/// * `config_addr` - Configuration address
///
/// # Returns
/// * 0 on success
pub fn cell_resc_init(_config_addr: u32) -> i32 {
    debug!("cellRescInit()");

    // Use default config when memory read is not yet implemented
    let config = CellRescInitConfig::default();
    crate::context::get_hle_context_mut().resc.init(config)
}

/// cellRescExit - Exit/cleanup RESC
///
/// # Returns
/// * 0 on success
pub fn cell_resc_exit() -> i32 {
    debug!("cellRescExit()");

    crate::context::get_hle_context_mut().resc.exit()
}

/// cellRescSetDisplayMode - Set display mode
///
/// # Arguments
/// * `display_mode` - Display mode
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_display_mode(display_mode: u32) -> i32 {
    debug!("cellRescSetDisplayMode(mode=0x{:X})", display_mode);

    crate::context::get_hle_context_mut().resc.set_display_mode(display_mode)
}

/// cellRescSetSrc - Set source buffer information
///
/// # Arguments
/// * `buffer_mode` - Buffer mode
/// * `src_addr` - Source info address
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_src(buffer_mode: u32, _src_addr: u32) -> i32 {
    debug!("cellRescSetSrc(buffer_mode={})", buffer_mode);

    // Use default source when memory read is not yet implemented
    let src = CellRescSrc {
        format: 0,
        pitch: 1920 * 4,
        width: 1920,
        height: 1080,
        offset: 0,
    };

    let mode = if buffer_mode == 1 {
        CellRescBufferMode::A2B2
    } else {
        CellRescBufferMode::A1B1
    };

    let mut ctx = crate::context::get_hle_context_mut();
    let result = ctx.resc.set_buffer_mode(mode);
    if result != 0 {
        return result;
    }
    ctx.resc.set_src(src)
}

/// cellRescSetDsts - Set destination information
///
/// # Arguments
/// * `buffer_mode` - Buffer mode
/// * `dsts_addr` - Destination info address
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_dsts(buffer_mode: u32, _dsts_addr: u32) -> i32 {
    debug!("cellRescSetDsts(buffer_mode={})", buffer_mode);

    // Use default destination when memory read is not yet implemented
    let dst = CellRescDsts {
        format: 0,
        pitch: 1920 * 4,
        width_byte: 1920 * 4,
        height: 1080,
    };

    crate::context::get_hle_context_mut().resc.set_dst(buffer_mode, dst)
}

/// cellRescSetPalTemporalMode - Set PAL temporal mode
///
/// # Arguments
/// * `mode` - PAL temporal mode
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_pal_temporal_mode(mode: u32) -> i32 {
    debug!("cellRescSetPalTemporalMode(mode={})", mode);

    let pal_mode = match mode {
        1 => CellRescPalTemporalMode::Filter50,
        2 => CellRescPalTemporalMode::Filter60,
        _ => CellRescPalTemporalMode::None,
    };

    crate::context::get_hle_context_mut().resc.set_pal_temporal_mode(pal_mode)
}

/// cellRescSetConvertAndFlip - Convert and flip buffer
///
/// # Arguments
/// * `idx` - Buffer index
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_convert_and_flip(idx: u32) -> i32 {
    trace!("cellRescSetConvertAndFlip(idx={})", idx);

    // Check if initialized through global manager
    if !crate::context::get_hle_context().resc.is_initialized() {
        return CELL_RESC_ERROR_NOT_INITIALIZED;
    }

    // TODO: Perform actual scaling and flip through RSX backend

    0 // CELL_OK
}

/// cellRescSetWaitFlip - Set wait for flip
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_wait_flip() -> i32 {
    trace!("cellRescSetWaitFlip()");

    // Check if initialized through global manager
    if !crate::context::get_hle_context().resc.is_initialized() {
        return CELL_RESC_ERROR_NOT_INITIALIZED;
    }

    // TODO: Wait for flip to complete

    0 // CELL_OK
}

/// cellRescGetNumDisplayBuffers - Get number of display buffers
///
/// # Arguments
/// * `num_addr` - Address to write number
///
/// # Returns
/// * 0 on success
pub fn cell_resc_get_num_display_buffers(_num_addr: u32) -> i32 {
    trace!("cellRescGetNumDisplayBuffers()");

    match crate::context::get_hle_context().resc.get_num_display_buffers() {
        Ok(_num) => {
            // TODO: Write num to memory at _num_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellRescGetDisplayBufferSize - Get display buffer size
///
/// # Arguments
/// * `size_addr` - Address to write size
///
/// # Returns
/// * 0 on success
pub fn cell_resc_get_display_buffer_size(_size_addr: u32) -> i32 {
    trace!("cellRescGetDisplayBufferSize()");

    match crate::context::get_hle_context().resc.get_display_buffer_size() {
        Ok(_size) => {
            // TODO: Write size to memory at _size_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellRescGetLastFlipTime - Get last flip time
///
/// # Arguments
/// * `time_addr` - Address to write time
///
/// # Returns
/// * 0 on success
pub fn cell_resc_get_last_flip_time(_time_addr: u32) -> i32 {
    trace!("cellRescGetLastFlipTime()");

    match crate::context::get_hle_context().resc.get_last_flip_time() {
        Ok(_time) => {
            // TODO: Write time to memory at _time_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellRescSetFlipHandler - Set flip handler callback
///
/// # Arguments
/// * `handler` - Handler callback address
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_flip_handler(handler: u32) -> i32 {
    debug!("cellRescSetFlipHandler(handler=0x{:08X})", handler);

    crate::context::get_hle_context_mut().resc.set_flip_handler(handler)
}

/// cellRescSetVsReporter - Set vertical sync reporter
///
/// # Arguments
/// * `reporter` - Reporter callback address
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_vs_reporter(reporter: u32) -> i32 {
    debug!("cellRescSetVsReporter(reporter=0x{:08X})", reporter);

    crate::context::get_hle_context_mut().resc.set_vs_reporter(reporter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resc_manager_lifecycle() {
        let mut manager = RescManager::new();
        
        // Initialize
        let config = CellRescInitConfig::default();
        assert_eq!(manager.init(config), 0);
        assert!(manager.is_initialized());
        
        // Double init should fail
        assert_eq!(manager.init(config), CELL_RESC_ERROR_REINITIALIZED);
        
        // Exit
        assert_eq!(manager.exit(), 0);
        assert!(!manager.is_initialized());
        
        // Exit again should fail
        assert_eq!(manager.exit(), CELL_RESC_ERROR_NOT_INITIALIZED);
    }

    #[test]
    fn test_resc_manager_display_mode() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Set valid modes
        assert_eq!(manager.set_display_mode(CELL_RESC_720x480), 0);
        assert_eq!(manager.get_display_mode(), CELL_RESC_720x480);
        
        assert_eq!(manager.set_display_mode(CELL_RESC_1920x1080), 0);
        assert_eq!(manager.get_display_mode(), CELL_RESC_1920x1080);
        
        // Invalid mode
        assert_eq!(manager.set_display_mode(0xFF), CELL_RESC_ERROR_BAD_ARGUMENT);
        
        manager.exit();
    }

    #[test]
    fn test_resc_manager_buffer_size() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Check buffer size for different modes
        manager.set_display_mode(CELL_RESC_720x480);
        assert_eq!(manager.get_display_buffer_size().unwrap(), 720 * 480 * 4);
        
        manager.set_display_mode(CELL_RESC_1920x1080);
        assert_eq!(manager.get_display_buffer_size().unwrap(), 1920 * 1080 * 4);
        
        manager.exit();
    }

    #[test]
    fn test_resc_manager_num_buffers() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Default is single buffer
        assert_eq!(manager.get_num_display_buffers().unwrap(), 1);
        
        // Set double buffer mode
        manager.set_buffer_mode(CellRescBufferMode::A2B2);
        assert_eq!(manager.get_num_display_buffers().unwrap(), 2);
        
        manager.exit();
    }

    #[test]
    fn test_resc_manager_not_initialized() {
        let manager = RescManager::new();
        
        // All operations should fail when not initialized
        assert_eq!(manager.get_num_display_buffers(), Err(CELL_RESC_ERROR_NOT_INITIALIZED));
        assert_eq!(manager.get_display_buffer_size(), Err(CELL_RESC_ERROR_NOT_INITIALIZED));
        assert_eq!(manager.get_last_flip_time(), Err(CELL_RESC_ERROR_NOT_INITIALIZED));
    }

    #[test]
    fn test_resc_config_default() {
        let config = CellRescInitConfig::default();
        assert!(config.display_modes & CELL_RESC_1920x1080 != 0);
        assert!(config.display_modes & CELL_RESC_1280x720 != 0);
    }

    #[test]
    fn test_resc_display_mode_flags() {
        assert_eq!(CELL_RESC_720x480, 0x01);
        assert_eq!(CELL_RESC_720x576, 0x02);
        assert_eq!(CELL_RESC_1280x720, 0x04);
        assert_eq!(CELL_RESC_1920x1080, 0x08);
    }

    #[test]
    fn test_resc_error_codes() {
        assert_ne!(CELL_RESC_ERROR_NOT_INITIALIZED, 0);
        assert_ne!(CELL_RESC_ERROR_REINITIALIZED, 0);
        assert_ne!(CELL_RESC_ERROR_BAD_ARGUMENT, 0);
    }

    // ========================================================================
    // RSX Scaling Integration Tests
    // ========================================================================

    #[test]
    fn test_resc_rsx_scaling_enabled() {
        let mut manager = RescManager::new();
        
        // Before init, scaling should be disabled
        assert!(!manager.is_rsx_scaling_enabled());
        
        // After init, scaling should be enabled
        manager.init(CellRescInitConfig::default());
        assert!(manager.is_rsx_scaling_enabled());
        
        manager.exit();
    }

    #[test]
    fn test_resc_scale_factors_default() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Default scale factors should be 1.0
        let (scale_x, scale_y) = manager.get_scale_factors();
        assert_eq!(scale_x, 1.0);
        assert_eq!(scale_y, 1.0);
        
        manager.exit();
    }

    #[test]
    fn test_resc_calculate_scale_factors() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Set source to 720p
        let src = CellRescSrc {
            format: 0,
            pitch: 1280 * 4,
            width: 1280,
            height: 720,
            offset: 0,
        };
        manager.set_src(src);
        
        // Set display mode to 1080p
        manager.set_display_mode(CELL_RESC_1920x1080);
        
        // Calculate scale factors
        assert_eq!(manager.calculate_scale_factors(), 0);
        
        let (scale_x, scale_y) = manager.get_scale_factors();
        // For letterbox mode (default), both should be same as minimum
        // 1920/1280 = 1.5, 1080/720 = 1.5, so both should be 1.5
        assert!((scale_x - 1.5).abs() < 0.01);
        assert!((scale_y - 1.5).abs() < 0.01);
        
        manager.exit();
    }

    #[test]
    fn test_resc_convert_and_flip() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Initial flip count should be 0
        assert_eq!(manager.get_flip_count(), 0);
        
        // Perform convert and flip
        assert_eq!(manager.convert_and_flip(0), 0);
        assert_eq!(manager.get_flip_count(), 1);
        
        assert_eq!(manager.convert_and_flip(1), 0);
        assert_eq!(manager.get_flip_count(), 2);
        
        manager.exit();
    }

    #[test]
    fn test_resc_bilinear_filter() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Default should be enabled
        assert!(manager.is_bilinear_filter_enabled());
        
        // Disable it
        assert_eq!(manager.set_bilinear_filter(false), 0);
        assert!(!manager.is_bilinear_filter_enabled());
        
        // Re-enable
        assert_eq!(manager.set_bilinear_filter(true), 0);
        assert!(manager.is_bilinear_filter_enabled());
        
        manager.exit();
    }

    #[test]
    fn test_resc_ratio_modes_scaling() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Set source to 4:3 (640x480)
        let src = CellRescSrc {
            format: 0,
            pitch: 640 * 4,
            width: 640,
            height: 480,
            offset: 0,
        };
        manager.set_src(src);
        
        // Set display mode to 16:9 (1280x720)
        manager.set_display_mode(CELL_RESC_1280x720);
        
        // Test letterbox mode
        manager.set_ratio_convert_mode(CellRescRatioConvertMode::Letterbox);
        manager.calculate_scale_factors();
        let (scale_x_lb, scale_y_lb) = manager.get_scale_factors();
        assert_eq!(scale_x_lb, scale_y_lb); // Should be equal for letterbox
        
        // Test fullscreen mode (stretch)
        manager.set_ratio_convert_mode(CellRescRatioConvertMode::FullScreen);
        manager.calculate_scale_factors();
        let (scale_x_fs, scale_y_fs) = manager.get_scale_factors();
        // 1280/640 = 2.0, 720/480 = 1.5
        assert!((scale_x_fs - 2.0).abs() < 0.01);
        assert!((scale_y_fs - 1.5).abs() < 0.01);
        
        manager.exit();
    }
}
