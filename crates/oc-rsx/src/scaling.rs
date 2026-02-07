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

// ============================================================================
// Bilinear Interpolation
// ============================================================================

/// Bilinear interpolation sampler
#[derive(Debug, Clone, Copy)]
pub struct BilinearSampler {
    /// Width of the source image
    pub src_width: u32,
    /// Height of the source image
    pub src_height: u32,
}

impl BilinearSampler {
    /// Create a new bilinear sampler
    pub fn new(src_width: u32, src_height: u32) -> Self {
        Self { src_width, src_height }
    }

    /// Linear interpolation between two values
    #[inline]
    pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    /// Bilinear interpolation of four samples
    /// 
    /// Samples are ordered: top-left, top-right, bottom-left, bottom-right
    pub fn sample_bilinear(tl: f32, tr: f32, bl: f32, br: f32, tx: f32, ty: f32) -> f32 {
        let top = Self::lerp(tl, tr, tx);
        let bottom = Self::lerp(bl, br, tx);
        Self::lerp(top, bottom, ty)
    }

    /// Calculate source coordinates for a destination pixel
    pub fn map_coords(&self, dst_x: u32, dst_y: u32, dst_width: u32, dst_height: u32) -> (f32, f32) {
        // Map destination pixel to source coordinates
        let src_x = (dst_x as f32 + 0.5) * (self.src_width as f32 / dst_width as f32) - 0.5;
        let src_y = (dst_y as f32 + 0.5) * (self.src_height as f32 / dst_height as f32) - 0.5;
        (src_x.max(0.0), src_y.max(0.0))
    }

    /// Get the four sample positions and weights for bilinear interpolation
    pub fn get_sample_info(&self, src_x: f32, src_y: f32) -> BilinearSampleInfo {
        let x0 = src_x.floor() as u32;
        let y0 = src_y.floor() as u32;
        let x1 = (x0 + 1).min(self.src_width.saturating_sub(1));
        let y1 = (y0 + 1).min(self.src_height.saturating_sub(1));
        let tx = src_x - src_x.floor();
        let ty = src_y - src_y.floor();

        BilinearSampleInfo {
            x0, y0, x1, y1, tx, ty,
        }
    }
}

/// Sample positions and weights for bilinear interpolation
#[derive(Debug, Clone, Copy)]
pub struct BilinearSampleInfo {
    /// Top-left X coordinate
    pub x0: u32,
    /// Top-left Y coordinate
    pub y0: u32,
    /// Bottom-right X coordinate
    pub x1: u32,
    /// Bottom-right Y coordinate
    pub y1: u32,
    /// Horizontal interpolation factor
    pub tx: f32,
    /// Vertical interpolation factor
    pub ty: f32,
}

// ============================================================================
// Bicubic Interpolation
// ============================================================================

/// Bicubic interpolation sampler
/// Uses Catmull-Rom spline (a = -0.5)
#[derive(Debug, Clone, Copy)]
pub struct BicubicSampler {
    /// Width of the source image
    pub src_width: u32,
    /// Height of the source image
    pub src_height: u32,
    /// Sharpness parameter (default: -0.5 for Catmull-Rom)
    pub a: f32,
}

impl BicubicSampler {
    /// Create a new bicubic sampler with Catmull-Rom spline
    pub fn new(src_width: u32, src_height: u32) -> Self {
        Self { 
            src_width, 
            src_height,
            a: -0.5, // Catmull-Rom
        }
    }

    /// Create with custom sharpness parameter
    pub fn with_sharpness(src_width: u32, src_height: u32, a: f32) -> Self {
        Self { src_width, src_height, a }
    }

    /// Cubic kernel function
    /// 
    /// The cubic kernel is defined as:
    /// - For |t| <= 1: (a+2)|t|^3 - (a+3)|t|^2 + 1
    /// - For 1 < |t| < 2: a|t|^3 - 5a|t|^2 + 8a|t| - 4a
    /// - Otherwise: 0
    pub fn cubic_kernel(&self, t: f32) -> f32 {
        let t_abs = t.abs();
        let a = self.a;

        if t_abs <= 1.0 {
            (a + 2.0) * t_abs.powi(3) - (a + 3.0) * t_abs.powi(2) + 1.0
        } else if t_abs < 2.0 {
            a * t_abs.powi(3) - 5.0 * a * t_abs.powi(2) + 8.0 * a * t_abs - 4.0 * a
        } else {
            0.0
        }
    }

    /// Get kernel weights for 4 samples
    pub fn get_weights(&self, t: f32) -> [f32; 4] {
        [
            self.cubic_kernel(t + 1.0),
            self.cubic_kernel(t),
            self.cubic_kernel(t - 1.0),
            self.cubic_kernel(t - 2.0),
        ]
    }

    /// Calculate source coordinates for a destination pixel
    pub fn map_coords(&self, dst_x: u32, dst_y: u32, dst_width: u32, dst_height: u32) -> (f32, f32) {
        let src_x = (dst_x as f32 + 0.5) * (self.src_width as f32 / dst_width as f32) - 0.5;
        let src_y = (dst_y as f32 + 0.5) * (self.src_height as f32 / dst_height as f32) - 0.5;
        (src_x.max(0.0), src_y.max(0.0))
    }

    /// Get the 16 sample positions for bicubic interpolation
    pub fn get_sample_positions(&self, src_x: f32, src_y: f32) -> BicubicSampleInfo {
        let x_center = src_x.floor() as i32;
        let y_center = src_y.floor() as i32;
        let tx = src_x - src_x.floor();
        let ty = src_y - src_y.floor();

        // Calculate clamped coordinates
        let max_x = self.src_width.saturating_sub(1) as i32;
        let max_y = self.src_height.saturating_sub(1) as i32;

        let x = [
            (x_center - 1).clamp(0, max_x) as u32,
            x_center.clamp(0, max_x) as u32,
            (x_center + 1).clamp(0, max_x) as u32,
            (x_center + 2).clamp(0, max_x) as u32,
        ];

        let y = [
            (y_center - 1).clamp(0, max_y) as u32,
            y_center.clamp(0, max_y) as u32,
            (y_center + 1).clamp(0, max_y) as u32,
            (y_center + 2).clamp(0, max_y) as u32,
        ];

        BicubicSampleInfo {
            x,
            y,
            weights_x: self.get_weights(tx),
            weights_y: self.get_weights(ty),
        }
    }

    /// Interpolate 4 values using the cubic kernel
    /// 
    /// If the weight sum is near zero (which shouldn't happen with valid inputs),
    /// falls back to samples[1] which is the center-left sample closest to the
    /// interpolation point (since samples are indexed -1, 0, +1, +2 relative to center).
    pub fn interpolate_1d(samples: [f32; 4], weights: [f32; 4]) -> f32 {
        let sum: f32 = weights.iter().sum();
        if sum.abs() < 1e-6 {
            // Fallback to center-left sample (index 1 corresponds to floor(x))
            return samples[1];
        }
        (samples[0] * weights[0] + samples[1] * weights[1] + 
         samples[2] * weights[2] + samples[3] * weights[3]) / sum
    }
}

/// Sample positions and weights for bicubic interpolation
#[derive(Debug, Clone, Copy)]
pub struct BicubicSampleInfo {
    /// X coordinates for 4 samples
    pub x: [u32; 4],
    /// Y coordinates for 4 samples
    pub y: [u32; 4],
    /// Horizontal kernel weights
    pub weights_x: [f32; 4],
    /// Vertical kernel weights
    pub weights_y: [f32; 4],
}

// ============================================================================
// Lanczos Resampling
// ============================================================================

/// Lanczos resampling sampler
#[derive(Debug, Clone, Copy)]
pub struct LanczosSampler {
    /// Width of the source image
    pub src_width: u32,
    /// Height of the source image
    pub src_height: u32,
    /// Number of lobes (typically 2, 3, or 4)
    pub lobes: u32,
}

impl LanczosSampler {
    /// Create a new Lanczos sampler with 3 lobes (common choice)
    pub fn new(src_width: u32, src_height: u32) -> Self {
        Self { src_width, src_height, lobes: 3 }
    }

    /// Create with custom number of lobes
    pub fn with_lobes(src_width: u32, src_height: u32, lobes: u32) -> Self {
        Self { src_width, src_height, lobes: lobes.clamp(1, 8) }
    }

    /// Sinc function: sin(πx) / (πx)
    fn sinc(x: f32) -> f32 {
        if x.abs() < 1e-6 {
            1.0
        } else {
            let pi_x = std::f32::consts::PI * x;
            pi_x.sin() / pi_x
        }
    }

    /// Lanczos kernel
    pub fn lanczos_kernel(&self, x: f32) -> f32 {
        let a = self.lobes as f32;
        if x.abs() >= a {
            0.0
        } else {
            Self::sinc(x) * Self::sinc(x / a)
        }
    }

    /// Get kernel weights for the given number of samples
    pub fn get_weights(&self, t: f32) -> Vec<f32> {
        let a = self.lobes as i32;
        let mut weights = Vec::with_capacity((2 * a) as usize);
        
        for i in (-a + 1)..=a {
            weights.push(self.lanczos_kernel(t - i as f32));
        }
        
        weights
    }
}

// ============================================================================
// Aspect Ratio Handling
// ============================================================================

/// Aspect ratio helper
#[derive(Debug, Clone, Copy)]
pub struct AspectRatioHelper {
    /// Source aspect ratio
    pub source_aspect: f32,
    /// Target aspect ratio
    pub target_aspect: f32,
}

impl AspectRatioHelper {
    /// Common aspect ratios
    pub const RATIO_4_3: f32 = 4.0 / 3.0;
    pub const RATIO_16_9: f32 = 16.0 / 9.0;
    pub const RATIO_16_10: f32 = 16.0 / 10.0;
    pub const RATIO_21_9: f32 = 21.0 / 9.0;

    /// Create from width/height
    pub fn new(src_width: u32, src_height: u32, dst_width: u32, dst_height: u32) -> Self {
        Self {
            source_aspect: src_width as f32 / src_height as f32,
            target_aspect: dst_width as f32 / dst_height as f32,
        }
    }

    /// Create from explicit ratios
    pub fn from_ratios(source: f32, target: f32) -> Self {
        Self {
            source_aspect: source,
            target_aspect: target,
        }
    }

    /// Check if aspect ratios match (within tolerance)
    pub fn ratios_match(&self, tolerance: f32) -> bool {
        (self.source_aspect - self.target_aspect).abs() < tolerance
    }

    /// Calculate correction mode
    pub fn correction_mode(&self) -> AspectCorrectionMode {
        if self.ratios_match(0.01) {
            AspectCorrectionMode::None
        } else if self.source_aspect > self.target_aspect {
            AspectCorrectionMode::Letterbox
        } else {
            AspectCorrectionMode::Pillarbox
        }
    }

    /// Calculate letterbox/pillarbox bars
    pub fn calculate_bars(&self, target_width: u32, target_height: u32) -> (u32, u32) {
        match self.correction_mode() {
            AspectCorrectionMode::None => (0, 0),
            AspectCorrectionMode::Letterbox => {
                // Source is wider - add bars top/bottom
                let content_height = (target_width as f32 / self.source_aspect).round() as u32;
                let bar_height = target_height.saturating_sub(content_height) / 2;
                (0, bar_height)
            }
            AspectCorrectionMode::Pillarbox => {
                // Source is taller - add bars left/right
                let content_width = (target_height as f32 * self.source_aspect).round() as u32;
                let bar_width = target_width.saturating_sub(content_width) / 2;
                (bar_width, 0)
            }
        }
    }

    /// Calculate content rectangle within target
    pub fn content_rect(&self, target_width: u32, target_height: u32) -> ContentRect {
        let (bar_x, bar_y) = self.calculate_bars(target_width, target_height);
        
        ContentRect {
            x: bar_x,
            y: bar_y,
            width: target_width.saturating_sub(bar_x * 2),
            height: target_height.saturating_sub(bar_y * 2),
        }
    }
}

/// Aspect ratio correction mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AspectCorrectionMode {
    /// No correction needed
    None,
    /// Black bars on top and bottom
    Letterbox,
    /// Black bars on left and right
    Pillarbox,
}

/// Content rectangle after aspect ratio correction
#[derive(Debug, Clone, Copy)]
pub struct ContentRect {
    /// X offset
    pub x: u32,
    /// Y offset
    pub y: u32,
    /// Content width
    pub width: u32,
    /// Content height
    pub height: u32,
}

// ============================================================================
// FSR 1.0 Configuration
// ============================================================================

/// FSR 1.0 (FidelityFX Super Resolution) configuration
/// This is a spatial upscaling solution with edge-adaptive sharpening
#[derive(Debug, Clone, Copy)]
pub struct Fsr1Config {
    /// Sharpness (0.0 = sharpest, 2.0 = least sharp)
    pub sharpness: f32,
    /// Input sharpening before upscale
    pub input_sharpening: f32,
}

impl Default for Fsr1Config {
    fn default() -> Self {
        Self {
            sharpness: 0.2, // Balanced default
            input_sharpening: 0.0,
        }
    }
}

impl Fsr1Config {
    /// Maximum sharpness preset
    pub fn max_sharpness() -> Self {
        Self {
            sharpness: 0.0,
            input_sharpening: 0.1,
        }
    }

    /// Balanced preset
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Soft preset (less sharpening)
    pub fn soft() -> Self {
        Self {
            sharpness: 1.0,
            input_sharpening: 0.0,
        }
    }

    /// Quality presets based on upscale ratio
    /// 
    /// The percentages in comments indicate the internal render resolution as
    /// a percentage of the target resolution. A scale of 2.0x means rendering
    /// at 50% resolution and upscaling to 100%.
    /// 
    /// - scale >= 2.0: Performance (50% internal resolution, most aggressive upscaling)
    /// - scale >= 1.5: Balanced (67% internal resolution)
    /// - scale >= 1.3: Quality (77% internal resolution)
    /// - scale < 1.3: Ultra Quality (83%+ internal resolution)
    pub fn for_quality_mode(scale: f32) -> Self {
        if scale >= 2.0 {
            // Performance mode (50% internal res) - needs more sharpening
            Self { sharpness: 0.0, input_sharpening: 0.1 }
        } else if scale >= 1.5 {
            // Balanced mode (67% internal res)
            Self { sharpness: 0.2, input_sharpening: 0.0 }
        } else if scale >= 1.3 {
            // Quality mode (77% internal res)
            Self { sharpness: 0.5, input_sharpening: 0.0 }
        } else {
            // Ultra quality (83%+ internal res)
            Self { sharpness: 1.0, input_sharpening: 0.0 }
        }
    }
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

    #[test]
    fn test_bilinear_lerp() {
        assert_eq!(BilinearSampler::lerp(0.0, 1.0, 0.0), 0.0);
        assert_eq!(BilinearSampler::lerp(0.0, 1.0, 1.0), 1.0);
        assert_eq!(BilinearSampler::lerp(0.0, 1.0, 0.5), 0.5);
    }

    #[test]
    fn test_bilinear_sample_info() {
        let sampler = BilinearSampler::new(100, 100);
        let info = sampler.get_sample_info(50.5, 50.5);
        
        assert_eq!(info.x0, 50);
        assert_eq!(info.x1, 51);
        assert!((info.tx - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_bicubic_kernel() {
        let sampler = BicubicSampler::new(100, 100);
        
        // At center (t=0), kernel should be 1.0
        assert!((sampler.cubic_kernel(0.0) - 1.0).abs() < 0.001);
        // At t=1, kernel should be 0 for Catmull-Rom
        assert!(sampler.cubic_kernel(1.0).abs() < 0.001);
        // At t>=2, kernel should be 0
        assert_eq!(sampler.cubic_kernel(2.0), 0.0);
    }

    #[test]
    fn test_lanczos_kernel() {
        let sampler = LanczosSampler::new(100, 100);
        
        // At center (x=0), should be 1.0
        assert!((sampler.lanczos_kernel(0.0) - 1.0).abs() < 0.001);
        // At x >= lobes, should be 0
        assert_eq!(sampler.lanczos_kernel(3.0), 0.0);
    }

    #[test]
    fn test_aspect_ratio_letterbox() {
        // 16:9 source to 4:3 target should be letterboxed
        let helper = AspectRatioHelper::from_ratios(16.0/9.0, 4.0/3.0);
        assert_eq!(helper.correction_mode(), AspectCorrectionMode::Letterbox);
    }

    #[test]
    fn test_aspect_ratio_pillarbox() {
        // 4:3 source to 16:9 target should be pillarboxed
        let helper = AspectRatioHelper::from_ratios(4.0/3.0, 16.0/9.0);
        assert_eq!(helper.correction_mode(), AspectCorrectionMode::Pillarbox);
    }

    #[test]
    fn test_fsr1_config_presets() {
        let max = Fsr1Config::max_sharpness();
        let soft = Fsr1Config::soft();
        
        // Max sharpness has lower sharpness value (0.0 = sharpest)
        assert!(max.sharpness < soft.sharpness);
    }
}
