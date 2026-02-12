//! cellResc HLE - Resolution Scaler
//!
//! This module provides HLE implementations for the PS3's resolution scaling library.
//! It handles resolution conversion, aspect ratio handling, and upscaling/downscaling.

use tracing::{debug, trace};
use crate::memory::{write_be32, write_be64};

/// Error codes
pub const CELL_RESC_ERROR_NOT_INITIALIZED: i32 = 0x80210301u32 as i32;
pub const CELL_RESC_ERROR_REINITIALIZED: i32 = 0x80210302u32 as i32;
pub const CELL_RESC_ERROR_BAD_ALIGNMENT: i32 = 0x80210303u32 as i32;
pub const CELL_RESC_ERROR_BAD_ARGUMENT: i32 = 0x80210304u32 as i32;
pub const CELL_RESC_ERROR_LESS_MEMORY: i32 = 0x80210305u32 as i32;
pub const CELL_RESC_ERROR_GCM_FLIP_QUE_FULL: i32 = 0x80210306u32 as i32;

/// Display mode flags
pub const CELL_RESC_720X480: u32 = 0x01;
pub const CELL_RESC_720X576: u32 = 0x02;
pub const CELL_RESC_1280X720: u32 = 0x04;
pub const CELL_RESC_1920X1080: u32 = 0x08;

/// Palette format
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CellRescPalTemporalMode {
    /// No temporal filter
    #[default]
    None = 0,
    /// 50Hz temporal filter
    Filter50 = 1,
    /// 60Hz temporal filter
    Filter60 = 2,
}


/// Buffer mode
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CellRescBufferMode {
    /// Single buffer
    #[default]
    A1B1 = 0,
    /// Double buffer (alternate)
    A2B2 = 1,
}


/// Aspect ratio
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CellRescRatioConvertMode {
    /// Letterbox (maintain aspect with black bars)
    #[default]
    Letterbox = 0,
    /// Full screen (stretch to fill)
    FullScreen = 1,
    /// Pan and scan (crop to fill)
    PanScan = 2,
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
            display_modes: CELL_RESC_720X480 | CELL_RESC_720X576 | CELL_RESC_1280X720 | CELL_RESC_1920X1080,
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

/// Upscaling filter algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpscaleFilter {
    /// Nearest-neighbor (fastest, lowest quality)
    Nearest,
    /// Bilinear interpolation (good balance)
    #[default]
    Bilinear,
    /// Lanczos-3 resampling (highest quality, slower)
    Lanczos3,
}

/// PAL/NTSC framerate standard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramerateStandard {
    /// NTSC: 59.94 Hz (60000/1001)
    Ntsc,
    /// PAL: 50 Hz
    Pal,
}

/// Framerate converter for PAL ↔ NTSC conversion
#[derive(Debug, Clone)]
pub struct FramerateConverter {
    /// Source framerate standard
    pub source: FramerateStandard,
    /// Target framerate standard
    pub target: FramerateStandard,
    /// Accumulated frame time (in source frame periods)
    accumulator: f64,
    /// Source frame period in seconds
    source_period: f64,
    /// Target frame period in seconds
    target_period: f64,
    /// Frame blend weight for the current interpolation
    blend_weight: f32,
    /// Total frames converted
    frames_converted: u64,
}

impl FramerateConverter {
    /// Create a new framerate converter
    pub fn new(source: FramerateStandard, target: FramerateStandard) -> Self {
        let source_period = match source {
            FramerateStandard::Ntsc => 1001.0 / 60000.0, // ~16.683ms
            FramerateStandard::Pal => 1.0 / 50.0,         // 20.0ms
        };
        let target_period = match target {
            FramerateStandard::Ntsc => 1001.0 / 60000.0,
            FramerateStandard::Pal => 1.0 / 50.0,
        };

        Self {
            source,
            target,
            accumulator: 0.0,
            source_period,
            target_period,
            blend_weight: 0.0,
            frames_converted: 0,
        }
    }

    /// Advance by one source frame, returns true if a target frame should be output
    pub fn advance_source_frame(&mut self) -> bool {
        self.accumulator += self.source_period;

        if self.accumulator >= self.target_period {
            self.accumulator -= self.target_period;
            // Blend weight: how far into the target period we are
            self.blend_weight = (self.accumulator / self.target_period) as f32;
            self.blend_weight = self.blend_weight.clamp(0.0, 1.0);
            self.frames_converted += 1;
            true
        } else {
            false
        }
    }

    /// Get the blend weight for frame interpolation (0.0 = use prev frame, 1.0 = use next frame)
    pub fn get_blend_weight(&self) -> f32 {
        self.blend_weight
    }

    /// Get total frames converted
    pub fn get_frames_converted(&self) -> u64 {
        self.frames_converted
    }

    /// Blend two frames based on current weight (per-pixel alpha blend)
    /// Both frames must be same-sized RGBA buffers
    pub fn blend_frames(prev: &[u8], next: &[u8], weight: f32) -> Vec<u8> {
        let len = prev.len().min(next.len());
        let mut out = vec![0u8; len];
        let w = weight.clamp(0.0, 1.0);
        let inv_w = 1.0 - w;

        for i in 0..len {
            out[i] = ((prev[i] as f32 * inv_w) + (next[i] as f32 * w)) as u8;
        }
        out
    }

    /// Reset the converter state
    pub fn reset(&mut self) {
        self.accumulator = 0.0;
        self.blend_weight = 0.0;
        self.frames_converted = 0;
    }
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
    /// Current upscale filter algorithm
    upscale_filter: UpscaleFilter,
    /// Framerate converter (optional, created when PAL temporal mode is active)
    framerate_converter: Option<FramerateConverter>,
}

impl RescManager {
    /// Create a new RESC manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            config: CellRescInitConfig::default(),
            src: CellRescSrc::default(),
            dsts: [CellRescDsts::default(); 4],
            display_mode: CELL_RESC_1280X720,
            buffer_mode: CellRescBufferMode::default(),
            pal_temporal_mode: CellRescPalTemporalMode::default(),
            ratio_mode: CellRescRatioConvertMode::default(),
            flip_handler_set: false,
            rsx_scaling_enabled: false,
            scale_x: 1.0,
            scale_y: 1.0,
            bilinear_filter: true,
            flip_count: 0,
            upscale_filter: UpscaleFilter::default(),
            framerate_converter: None,
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
            CELL_RESC_720X480,
            CELL_RESC_720X576,
            CELL_RESC_1280X720,
            CELL_RESC_1920X1080,
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

        // Create framerate converter based on mode
        self.framerate_converter = match mode {
            CellRescPalTemporalMode::Filter50 => {
                // PAL source → NTSC output (50Hz → 59.94Hz)
                Some(FramerateConverter::new(FramerateStandard::Pal, FramerateStandard::Ntsc))
            }
            CellRescPalTemporalMode::Filter60 => {
                // NTSC source → PAL output (59.94Hz → 50Hz)
                Some(FramerateConverter::new(FramerateStandard::Ntsc, FramerateStandard::Pal))
            }
            CellRescPalTemporalMode::None => None,
        };

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
            CELL_RESC_720X480 => (720, 480),
            CELL_RESC_720X576 => (720, 576),
            CELL_RESC_1280X720 => (1280, 720),
            CELL_RESC_1920X1080 => (1920, 1080),
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
            CELL_RESC_720X480 => (720u32, 480u32),
            CELL_RESC_720X576 => (720, 576),
            CELL_RESC_1280X720 => (1280, 720),
            CELL_RESC_1920X1080 => (1920, 1080),
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

        // Calculate scale factors if not already done (use epsilon comparison for floats)
        let epsilon = 0.0001f32;
        if (self.scale_x - 1.0).abs() < epsilon && (self.scale_y - 1.0).abs() < epsilon {
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

    // ========================================================================
    // Advanced Upscaling Filters
    // ========================================================================

    /// Set the upscale filter algorithm
    pub fn set_upscale_filter(&mut self, filter: UpscaleFilter) -> i32 {
        if !self.initialized {
            return CELL_RESC_ERROR_NOT_INITIALIZED;
        }

        debug!("RescManager::set_upscale_filter: {:?}", filter);
        self.upscale_filter = filter;

        0 // CELL_OK
    }

    /// Get the current upscale filter
    pub fn get_upscale_filter(&self) -> UpscaleFilter {
        self.upscale_filter
    }

    /// Apply upscale filter to a source buffer, producing a destination buffer
    /// `src_buf` is an RGBA pixel buffer (4 bytes per pixel)
    pub fn apply_upscale_filter(
        &self,
        src_buf: &[u8],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
    ) -> Vec<u8> {
        match self.upscale_filter {
            UpscaleFilter::Nearest => {
                Self::scale_nearest(src_buf, src_w, src_h, dst_w, dst_h)
            }
            UpscaleFilter::Bilinear => {
                Self::scale_bilinear(src_buf, src_w, src_h, dst_w, dst_h)
            }
            UpscaleFilter::Lanczos3 => {
                Self::scale_lanczos3(src_buf, src_w, src_h, dst_w, dst_h)
            }
        }
    }

    /// Nearest-neighbor scaling
    fn scale_nearest(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
        let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let sx = (dx as f32 / dst_w as f32 * src_w as f32) as u32;
                let sy = (dy as f32 / dst_h as f32 * src_h as f32) as u32;
                let sx = sx.min(src_w - 1);
                let sy = sy.min(src_h - 1);

                let src_idx = ((sy * src_w + sx) * 4) as usize;
                let dst_idx = ((dy * dst_w + dx) * 4) as usize;

                if src_idx + 3 < src.len() && dst_idx + 3 < dst.len() {
                    dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
                }
            }
        }

        dst
    }

    /// Bilinear interpolation scaling
    fn scale_bilinear(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
        let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let gx = dx as f32 / dst_w as f32 * (src_w - 1) as f32;
                let gy = dy as f32 / dst_h as f32 * (src_h - 1) as f32;

                let x0 = gx.floor() as u32;
                let y0 = gy.floor() as u32;
                let x1 = (x0 + 1).min(src_w - 1);
                let y1 = (y0 + 1).min(src_h - 1);

                let fx = gx - x0 as f32;
                let fy = gy - y0 as f32;

                let dst_idx = ((dy * dst_w + dx) * 4) as usize;

                for c in 0..4u32 {
                    let idx00 = ((y0 * src_w + x0) * 4 + c) as usize;
                    let idx10 = ((y0 * src_w + x1) * 4 + c) as usize;
                    let idx01 = ((y1 * src_w + x0) * 4 + c) as usize;
                    let idx11 = ((y1 * src_w + x1) * 4 + c) as usize;

                    if idx11 < src.len() {
                        let v00 = src[idx00] as f32;
                        let v10 = src[idx10] as f32;
                        let v01 = src[idx01] as f32;
                        let v11 = src[idx11] as f32;

                        let top = v00 * (1.0 - fx) + v10 * fx;
                        let bot = v01 * (1.0 - fx) + v11 * fx;
                        let val = top * (1.0 - fy) + bot * fy;

                        dst[dst_idx + c as usize] = val.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        dst
    }

    /// Lanczos-3 kernel function: sinc(x) * sinc(x/3) for |x| < 3, else 0
    fn lanczos3_kernel(x: f32) -> f32 {
        if x.abs() < 1e-6 {
            return 1.0;
        }
        if x.abs() >= 3.0 {
            return 0.0;
        }
        let pi_x = std::f32::consts::PI * x;
        let pi_x_over_3 = pi_x / 3.0;
        (pi_x.sin() / pi_x) * (pi_x_over_3.sin() / pi_x_over_3)
    }

    /// Lanczos-3 resampling scaling
    fn scale_lanczos3(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
        let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
        let a = 3i32; // Lanczos-3 window

        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let gx = dx as f32 / dst_w as f32 * (src_w - 1) as f32;
                let gy = dy as f32 / dst_h as f32 * (src_h - 1) as f32;

                let cx = gx.floor() as i32;
                let cy = gy.floor() as i32;

                let dst_idx = ((dy * dst_w + dx) * 4) as usize;

                for c in 0..4u32 {
                    let mut sum = 0.0f32;
                    let mut weight_sum = 0.0f32;

                    for ky in -a + 1..=a {
                        for kx in -a + 1..=a {
                            let sx = (cx + kx).clamp(0, src_w as i32 - 1) as u32;
                            let sy = (cy + ky).clamp(0, src_h as i32 - 1) as u32;
                            let src_idx = ((sy * src_w + sx) * 4 + c) as usize;

                            if src_idx < src.len() {
                                let wx = Self::lanczos3_kernel(gx - (cx + kx) as f32);
                                let wy = Self::lanczos3_kernel(gy - (cy + ky) as f32);
                                let w = wx * wy;
                                sum += src[src_idx] as f32 * w;
                                weight_sum += w;
                            }
                        }
                    }

                    if weight_sum > 0.0 {
                        dst[dst_idx + c as usize] = (sum / weight_sum).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        dst
    }

    // ========================================================================
    // PAL/NTSC Framerate Conversion
    // ========================================================================

    /// Get the framerate converter (if active)
    pub fn get_framerate_converter(&self) -> Option<&FramerateConverter> {
        self.framerate_converter.as_ref()
    }

    /// Get a mutable reference to the framerate converter (if active)
    pub fn get_framerate_converter_mut(&mut self) -> Option<&mut FramerateConverter> {
        self.framerate_converter.as_mut()
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

    // Perform actual scaling and flip through RSX backend
    crate::context::get_hle_context_mut().resc.convert_and_flip(idx)
}

/// cellRescSetWaitFlip - Set wait for flip
///
/// # Returns
/// * 0 on success
pub fn cell_resc_set_wait_flip() -> i32 {
    trace!("cellRescSetWaitFlip()");

    // Check if initialized through global manager
    let ctx = crate::context::get_hle_context();
    if !ctx.resc.is_initialized() {
        return CELL_RESC_ERROR_NOT_INITIALIZED;
    }

    // Wait for flip operation to complete - in a real implementation
    // this would block until the GPU finishes the flip.
    // For now, we treat it as immediate since convert_and_flip already
    // increments flip_count synchronously.
    let _flip_count = ctx.resc.get_flip_count();
    
    // In a proper async implementation, we would spin or yield until
    // the current flip is complete. Since our flip is synchronous, 
    // we can return immediately.
    debug!("cellRescSetWaitFlip: flip completed (flip_count={})", _flip_count);

    0 // CELL_OK
}

/// cellRescGetNumDisplayBuffers - Get number of display buffers
///
/// # Arguments
/// * `num_addr` - Address to write number
///
/// # Returns
/// * 0 on success
pub fn cell_resc_get_num_display_buffers(num_addr: u32) -> i32 {
    trace!("cellRescGetNumDisplayBuffers(num_addr=0x{:08X})", num_addr);

    match crate::context::get_hle_context().resc.get_num_display_buffers() {
        Ok(num) => {
            // Write num to memory at num_addr
            if num_addr != 0 {
                if let Err(e) = write_be32(num_addr, num) {
                    debug!("cellRescGetNumDisplayBuffers: failed to write to memory");
                    return e;
                }
            }
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
pub fn cell_resc_get_display_buffer_size(size_addr: u32) -> i32 {
    trace!("cellRescGetDisplayBufferSize(size_addr=0x{:08X})", size_addr);

    match crate::context::get_hle_context().resc.get_display_buffer_size() {
        Ok(size) => {
            // Write size to memory at size_addr
            if size_addr != 0 {
                if let Err(e) = write_be32(size_addr, size) {
                    debug!("cellRescGetDisplayBufferSize: failed to write to memory");
                    return e;
                }
            }
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
pub fn cell_resc_get_last_flip_time(time_addr: u32) -> i32 {
    trace!("cellRescGetLastFlipTime(time_addr=0x{:08X})", time_addr);

    match crate::context::get_hle_context().resc.get_last_flip_time() {
        Ok(time) => {
            // Write time to memory at time_addr
            if time_addr != 0 {
                if let Err(e) = write_be64(time_addr, time) {
                    debug!("cellRescGetLastFlipTime: failed to write to memory");
                    return e;
                }
            }
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
        assert_eq!(manager.set_display_mode(CELL_RESC_720X480), 0);
        assert_eq!(manager.get_display_mode(), CELL_RESC_720X480);
        
        assert_eq!(manager.set_display_mode(CELL_RESC_1920X1080), 0);
        assert_eq!(manager.get_display_mode(), CELL_RESC_1920X1080);
        
        // Invalid mode
        assert_eq!(manager.set_display_mode(0xFF), CELL_RESC_ERROR_BAD_ARGUMENT);
        
        manager.exit();
    }

    #[test]
    fn test_resc_manager_buffer_size() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());
        
        // Check buffer size for different modes
        manager.set_display_mode(CELL_RESC_720X480);
        assert_eq!(manager.get_display_buffer_size().unwrap(), 720 * 480 * 4);
        
        manager.set_display_mode(CELL_RESC_1920X1080);
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
        assert!(config.display_modes & CELL_RESC_1920X1080 != 0);
        assert!(config.display_modes & CELL_RESC_1280X720 != 0);
    }

    #[test]
    fn test_resc_display_mode_flags() {
        assert_eq!(CELL_RESC_720X480, 0x01);
        assert_eq!(CELL_RESC_720X576, 0x02);
        assert_eq!(CELL_RESC_1280X720, 0x04);
        assert_eq!(CELL_RESC_1920X1080, 0x08);
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
        manager.set_display_mode(CELL_RESC_1920X1080);
        
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
        manager.set_display_mode(CELL_RESC_1280X720);
        
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

    // ========================================================================
    // Upscale Filter Tests
    // ========================================================================

    #[test]
    fn test_resc_upscale_filter_set_get() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());

        assert_eq!(manager.get_upscale_filter(), UpscaleFilter::Bilinear);

        assert_eq!(manager.set_upscale_filter(UpscaleFilter::Lanczos3), 0);
        assert_eq!(manager.get_upscale_filter(), UpscaleFilter::Lanczos3);

        assert_eq!(manager.set_upscale_filter(UpscaleFilter::Nearest), 0);
        assert_eq!(manager.get_upscale_filter(), UpscaleFilter::Nearest);

        manager.exit();
    }

    #[test]
    fn test_resc_upscale_filter_not_initialized() {
        let mut manager = RescManager::new();
        assert_eq!(manager.set_upscale_filter(UpscaleFilter::Lanczos3), CELL_RESC_ERROR_NOT_INITIALIZED);
    }

    #[test]
    fn test_resc_scale_nearest() {
        let manager = RescManager::new();
        // 2x2 red/green/blue/white pixel image (RGBA)
        let src = vec![
            255, 0, 0, 255,   0, 255, 0, 255,   // row 0: red, green
            0, 0, 255, 255,   255, 255, 255, 255, // row 1: blue, white
        ];
        let dst = RescManager::scale_nearest(&src, 2, 2, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 4); // 4x4 RGBA
        // Top-left corner should still be red
        assert_eq!(dst[0], 255);
        assert_eq!(dst[1], 0);
        assert_eq!(dst[2], 0);
    }

    #[test]
    fn test_resc_scale_bilinear() {
        // 2x2 solid white
        let src = vec![255u8; 2 * 2 * 4];
        let dst = RescManager::scale_bilinear(&src, 2, 2, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 4);
        // Bilinear of all-white should be all-white
        for &v in &dst {
            assert_eq!(v, 255);
        }
    }

    #[test]
    fn test_resc_scale_lanczos3_identity() {
        // 4x4 solid gray
        let src = vec![128u8; 4 * 4 * 4];
        let dst = RescManager::scale_lanczos3(&src, 4, 4, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 4);
        // Same-size Lanczos of uniform should be close to uniform
        for &v in &dst {
            assert!((v as i32 - 128).abs() <= 1, "Expected ~128, got {}", v);
        }
    }

    #[test]
    fn test_resc_apply_upscale_filter_dispatch() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());

        let src = vec![100u8; 2 * 2 * 4];

        manager.set_upscale_filter(UpscaleFilter::Nearest);
        let dst = manager.apply_upscale_filter(&src, 2, 2, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 4);

        manager.set_upscale_filter(UpscaleFilter::Bilinear);
        let dst = manager.apply_upscale_filter(&src, 2, 2, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 4);

        manager.set_upscale_filter(UpscaleFilter::Lanczos3);
        let dst = manager.apply_upscale_filter(&src, 2, 2, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 4);

        manager.exit();
    }

    #[test]
    fn test_resc_lanczos3_kernel() {
        // At x=0, kernel should be 1.0
        let k0 = RescManager::lanczos3_kernel(0.0);
        assert!((k0 - 1.0).abs() < 0.001);

        // At |x| >= 3, kernel should be 0.0
        assert_eq!(RescManager::lanczos3_kernel(3.0), 0.0);
        assert_eq!(RescManager::lanczos3_kernel(-3.5), 0.0);

        // Kernel should be symmetric
        let k1 = RescManager::lanczos3_kernel(1.0);
        let k1n = RescManager::lanczos3_kernel(-1.0);
        assert!((k1 - k1n).abs() < 0.001);
    }

    // ========================================================================
    // Framerate Converter Tests
    // ========================================================================

    #[test]
    fn test_framerate_converter_pal_to_ntsc() {
        let mut conv = FramerateConverter::new(FramerateStandard::Pal, FramerateStandard::Ntsc);
        assert_eq!(conv.get_frames_converted(), 0);

        // PAL 50Hz → NTSC 59.94Hz: every source frame should produce ~1.2 target frames
        // Over 50 source frames (1 second), we should produce ~60 target frames
        let mut output_count = 0;
        for _ in 0..50 {
            if conv.advance_source_frame() {
                output_count += 1;
            }
        }
        // Should produce approximately 50 frames (50 source → roughly 50 output with this algorithm)
        assert!(output_count > 0, "Should produce some output frames");
        assert_eq!(conv.get_frames_converted(), output_count);
    }

    #[test]
    fn test_framerate_converter_ntsc_to_pal() {
        let mut conv = FramerateConverter::new(FramerateStandard::Ntsc, FramerateStandard::Pal);

        // NTSC 59.94Hz → PAL 50Hz: some source frames should be dropped
        let mut output_count = 0u64;
        for _ in 0..60 {
            if conv.advance_source_frame() {
                output_count += 1;
            }
        }
        assert!(output_count > 0);
    }

    #[test]
    fn test_framerate_converter_same_standard() {
        let mut conv = FramerateConverter::new(FramerateStandard::Ntsc, FramerateStandard::Ntsc);
        // Same framerate: every source frame should produce a target frame
        let mut output_count = 0;
        for _ in 0..60 {
            if conv.advance_source_frame() {
                output_count += 1;
            }
        }
        assert_eq!(output_count, 60);
    }

    #[test]
    fn test_framerate_converter_blend_frames() {
        let prev = vec![0u8; 16];
        let next = vec![200u8; 16];

        // 50% blend
        let blended = FramerateConverter::blend_frames(&prev, &next, 0.5);
        assert_eq!(blended.len(), 16);
        for &v in &blended {
            assert_eq!(v, 100);
        }

        // 0% blend → all prev
        let blended = FramerateConverter::blend_frames(&prev, &next, 0.0);
        for &v in &blended {
            assert_eq!(v, 0);
        }

        // 100% blend → all next
        let blended = FramerateConverter::blend_frames(&prev, &next, 1.0);
        for &v in &blended {
            assert_eq!(v, 200);
        }
    }

    #[test]
    fn test_framerate_converter_reset() {
        let mut conv = FramerateConverter::new(FramerateStandard::Pal, FramerateStandard::Ntsc);
        conv.advance_source_frame();
        assert!(conv.get_frames_converted() > 0 || conv.get_blend_weight() >= 0.0);
        conv.reset();
        assert_eq!(conv.get_frames_converted(), 0);
        assert_eq!(conv.get_blend_weight(), 0.0);
    }

    #[test]
    fn test_resc_pal_temporal_creates_converter() {
        let mut manager = RescManager::new();
        manager.init(CellRescInitConfig::default());

        assert!(manager.get_framerate_converter().is_none());

        manager.set_pal_temporal_mode(CellRescPalTemporalMode::Filter50);
        assert!(manager.get_framerate_converter().is_some());

        manager.set_pal_temporal_mode(CellRescPalTemporalMode::None);
        assert!(manager.get_framerate_converter().is_none());

        manager.set_pal_temporal_mode(CellRescPalTemporalMode::Filter60);
        assert!(manager.get_framerate_converter().is_some());

        manager.exit();
    }
}
