//! Resolution scaling and upscaling for RSX
//!
//! This module provides resolution scaling capabilities for the emulator,
//! including upscaling, downscaling, and render scale management.

/// Scaling mode for resolution management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingMode {
    /// Native PS3 resolution (typically 720p or 1080p)
    Native,
    /// Integer scaling (1x, 2x, 3x, 4x)
    Integer(u8),
    /// Custom resolution with aspect ratio preserved
    AspectPreserved,
    /// Custom resolution with stretching
    Stretch,
    /// Fit to window with letterboxing/pillarboxing
    FitToWindow,
}

/// Upscaling algorithm to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpscaleAlgorithm {
    /// Nearest neighbor (fastest, pixelated)
    Nearest,
    /// Bilinear interpolation
    Bilinear,
    /// Bicubic interpolation
    Bicubic,
    /// Lanczos filter (high quality)
    Lanczos,
    /// FSR 1.0 (AMD FidelityFX Super Resolution)
    Fsr1,
    /// FSR 2.0 (temporal upscaling)
    Fsr2,
    /// DLSS-like temporal upscaling (placeholder for future GPU support)
    TemporalUpscale,
    /// xBRZ for pixel art style games
    XbrZ,
    /// HQx for pixel art style games
    Hqx,
}

/// Downscaling algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownscaleAlgorithm {
    /// Average (box filter)
    Average,
    /// Bilinear
    Bilinear,
    /// Lanczos (high quality)
    Lanczos,
}

/// Internal render scale (percentage of output resolution)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderScale {
    /// Scale percentage (25-400%)
    pub percentage: f32,
}

impl RenderScale {
    /// Create a new render scale
    pub fn new(percentage: f32) -> Self {
        Self {
            percentage: percentage.clamp(25.0, 400.0),
        }
    }

    /// Native resolution (100%)
    pub fn native() -> Self {
        Self { percentage: 100.0 }
    }

    /// Performance mode (50%)
    pub fn performance() -> Self {
        Self { percentage: 50.0 }
    }

    /// Quality mode (100%)
    pub fn quality() -> Self {
        Self { percentage: 100.0 }
    }

    /// Ultra quality mode (200% - supersampling)
    pub fn ultra() -> Self {
        Self { percentage: 200.0 }
    }

    /// Get the scale factor (1.0 = 100%)
    pub fn factor(&self) -> f32 {
        self.percentage / 100.0
    }

    /// Calculate scaled dimensions
    pub fn scale_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        let scaled_width = ((width as f32) * self.factor()).round() as u32;
        let scaled_height = ((height as f32) * self.factor()).round() as u32;
        (scaled_width.max(1), scaled_height.max(1))
    }
}

impl Default for RenderScale {
    fn default() -> Self {
        Self::native()
    }
}

/// Resolution scaler configuration
#[derive(Debug, Clone)]
pub struct ResolutionScaler {
    /// Source width (native PS3 resolution)
    pub source_width: u32,
    /// Source height
    pub source_height: u32,
    /// Target width (output resolution)
    pub target_width: u32,
    /// Target height
    pub target_height: u32,
    /// Internal render scale
    pub render_scale: RenderScale,
    /// Scaling mode
    pub mode: ScalingMode,
    /// Upscaling algorithm
    pub upscale_algorithm: UpscaleAlgorithm,
    /// Downscaling algorithm
    pub downscale_algorithm: DownscaleAlgorithm,
    /// Whether to maintain aspect ratio
    pub maintain_aspect_ratio: bool,
    /// Sharpening amount after upscale (0.0 to 1.0)
    pub sharpening: f32,
}

impl ResolutionScaler {
    /// Create a new resolution scaler
    pub fn new(source_width: u32, source_height: u32) -> Self {
        Self {
            source_width,
            source_height,
            target_width: source_width,
            target_height: source_height,
            render_scale: RenderScale::native(),
            mode: ScalingMode::Native,
            upscale_algorithm: UpscaleAlgorithm::Bilinear,
            downscale_algorithm: DownscaleAlgorithm::Bilinear,
            maintain_aspect_ratio: true,
            sharpening: 0.0,
        }
    }

    /// Create a scaler for 720p source
    pub fn from_720p() -> Self {
        Self::new(1280, 720)
    }

    /// Create a scaler for 1080p source
    pub fn from_1080p() -> Self {
        Self::new(1920, 1080)
    }

    /// Set target resolution
    pub fn set_target_resolution(&mut self, width: u32, height: u32) {
        self.target_width = width;
        self.target_height = height;
    }

    /// Set render scale
    pub fn set_render_scale(&mut self, scale: RenderScale) {
        self.render_scale = scale;
    }

    /// Set scaling mode
    pub fn set_mode(&mut self, mode: ScalingMode) {
        self.mode = mode;
    }

    /// Set upscaling algorithm
    pub fn set_upscale_algorithm(&mut self, algorithm: UpscaleAlgorithm) {
        self.upscale_algorithm = algorithm;
    }

    /// Get internal render dimensions
    pub fn internal_dimensions(&self) -> (u32, u32) {
        self.render_scale.scale_dimensions(self.source_width, self.source_height)
    }

    /// Get final output dimensions based on scaling mode
    pub fn output_dimensions(&self) -> (u32, u32) {
        match self.mode {
            ScalingMode::Native => (self.source_width, self.source_height),
            ScalingMode::Integer(factor) => {
                let f = factor.max(1) as u32;
                (self.source_width * f, self.source_height * f)
            }
            ScalingMode::Stretch => (self.target_width, self.target_height),
            ScalingMode::AspectPreserved | ScalingMode::FitToWindow => {
                self.fit_with_aspect_ratio()
            }
        }
    }

    /// Calculate dimensions that fit within target while preserving aspect ratio
    fn fit_with_aspect_ratio(&self) -> (u32, u32) {
        let source_aspect = self.source_width as f32 / self.source_height as f32;
        let target_aspect = self.target_width as f32 / self.target_height as f32;

        if source_aspect > target_aspect {
            // Source is wider - fit to width
            let width = self.target_width;
            let height = (width as f32 / source_aspect).round() as u32;
            (width, height.max(1))
        } else {
            // Source is taller - fit to height
            let height = self.target_height;
            let width = (height as f32 * source_aspect).round() as u32;
            (width.max(1), height)
        }
    }

    /// Calculate letterbox/pillarbox offsets for centered output
    pub fn output_offset(&self) -> (i32, i32) {
        let (out_w, out_h) = self.output_dimensions();
        let x_offset = (self.target_width as i32 - out_w as i32) / 2;
        let y_offset = (self.target_height as i32 - out_h as i32) / 2;
        (x_offset, y_offset)
    }

    /// Get the scale factor between internal and output
    pub fn scale_factor(&self) -> f32 {
        let (out_w, _) = self.output_dimensions();
        let (int_w, _) = self.internal_dimensions();
        out_w as f32 / int_w as f32
    }

    /// Check if upscaling is needed
    pub fn needs_upscale(&self) -> bool {
        self.scale_factor() > 1.0
    }

    /// Check if downscaling is needed
    pub fn needs_downscale(&self) -> bool {
        self.scale_factor() < 1.0
    }

    /// Apply sharpening post-upscale
    pub fn set_sharpening(&mut self, amount: f32) {
        self.sharpening = amount.clamp(0.0, 1.0);
    }

    /// Get statistics about the current configuration
    pub fn stats(&self) -> ScalerStats {
        let (int_w, int_h) = self.internal_dimensions();
        let (out_w, out_h) = self.output_dimensions();
        
        ScalerStats {
            source_resolution: (self.source_width, self.source_height),
            internal_resolution: (int_w, int_h),
            output_resolution: (out_w, out_h),
            target_resolution: (self.target_width, self.target_height),
            render_scale_percent: self.render_scale.percentage,
            scale_factor: self.scale_factor(),
            needs_upscale: self.needs_upscale(),
            needs_downscale: self.needs_downscale(),
        }
    }
}

impl Default for ResolutionScaler {
    fn default() -> Self {
        Self::from_720p()
    }
}

/// Statistics about the resolution scaler
#[derive(Debug, Clone)]
pub struct ScalerStats {
    /// Original PS3 resolution
    pub source_resolution: (u32, u32),
    /// Internal render resolution
    pub internal_resolution: (u32, u32),
    /// Final output resolution (after scaling)
    pub output_resolution: (u32, u32),
    /// Target display resolution
    pub target_resolution: (u32, u32),
    /// Render scale percentage
    pub render_scale_percent: f32,
    /// Overall scale factor
    pub scale_factor: f32,
    /// Whether upscaling is applied
    pub needs_upscale: bool,
    /// Whether downscaling is applied
    pub needs_downscale: bool,
}

/// Common resolution presets
pub struct ResolutionPresets;

impl ResolutionPresets {
    /// 720p (HD)
    pub const HD: (u32, u32) = (1280, 720);
    /// 1080p (Full HD)
    pub const FHD: (u32, u32) = (1920, 1080);
    /// 1440p (QHD)
    pub const QHD: (u32, u32) = (2560, 1440);
    /// 4K (Ultra HD)
    pub const UHD_4K: (u32, u32) = (3840, 2160);
    /// 8K
    pub const UHD_8K: (u32, u32) = (7680, 4320);
    /// PS3 standard (480p)
    pub const PS3_SD: (u32, u32) = (720, 480);
    /// PS3 HD
    pub const PS3_HD: (u32, u32) = (1280, 720);
    /// PS3 Full HD (some games)
    pub const PS3_FHD: (u32, u32) = (1920, 1080);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_scale_creation() {
        let scale = RenderScale::new(150.0);
        assert_eq!(scale.percentage, 150.0);
        assert_eq!(scale.factor(), 1.5);
    }

    #[test]
    fn test_render_scale_clamping() {
        let low = RenderScale::new(10.0);
        assert_eq!(low.percentage, 25.0);

        let high = RenderScale::new(500.0);
        assert_eq!(high.percentage, 400.0);
    }

    #[test]
    fn test_render_scale_dimensions() {
        let scale = RenderScale::new(200.0);
        let (w, h) = scale.scale_dimensions(1280, 720);
        assert_eq!(w, 2560);
        assert_eq!(h, 1440);
    }

    #[test]
    fn test_resolution_scaler_creation() {
        let scaler = ResolutionScaler::from_720p();
        assert_eq!(scaler.source_width, 1280);
        assert_eq!(scaler.source_height, 720);
    }

    #[test]
    fn test_resolution_scaler_integer_scaling() {
        let mut scaler = ResolutionScaler::from_720p();
        scaler.set_mode(ScalingMode::Integer(2));
        
        let (w, h) = scaler.output_dimensions();
        assert_eq!(w, 2560);
        assert_eq!(h, 1440);
    }

    #[test]
    fn test_resolution_scaler_aspect_ratio() {
        let mut scaler = ResolutionScaler::from_720p();
        scaler.set_target_resolution(3840, 2160);
        scaler.set_mode(ScalingMode::FitToWindow);
        
        let (w, h) = scaler.output_dimensions();
        // 16:9 aspect ratio should fit 4K at 3840x2160
        assert_eq!(w, 3840);
        assert_eq!(h, 2160);
    }

    #[test]
    fn test_resolution_scaler_stats() {
        let mut scaler = ResolutionScaler::from_720p();
        scaler.set_target_resolution(1920, 1080);
        scaler.set_render_scale(RenderScale::new(50.0));
        
        let stats = scaler.stats();
        assert_eq!(stats.source_resolution, (1280, 720));
        assert_eq!(stats.internal_resolution, (640, 360));
    }

    #[test]
    fn test_resolution_presets() {
        assert_eq!(ResolutionPresets::HD, (1280, 720));
        assert_eq!(ResolutionPresets::FHD, (1920, 1080));
        assert_eq!(ResolutionPresets::UHD_4K, (3840, 2160));
    }

    #[test]
    fn test_upscale_detection() {
        let mut scaler = ResolutionScaler::from_720p();
        scaler.set_mode(ScalingMode::Integer(2));
        
        assert!(scaler.needs_upscale());
        assert!(!scaler.needs_downscale());
    }

    #[test]
    fn test_internal_resolution_supersampling() {
        // Test supersampling scenario: internal render at 200% of native
        let mut scaler = ResolutionScaler::from_1080p();
        scaler.set_render_scale(RenderScale::new(200.0)); // 3840x2160 internal
        scaler.set_target_resolution(1920, 1080);
        scaler.set_mode(ScalingMode::Native);
        
        // Internal resolution is 200% of source (3840x2160)
        // Output is native source resolution (1920x1080)
        let stats = scaler.stats();
        assert_eq!(stats.source_resolution, (1920, 1080));
        assert_eq!(stats.internal_resolution, (3840, 2160));
        assert_eq!(stats.output_resolution, (1920, 1080));
    }
}
