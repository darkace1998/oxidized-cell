//! Audio resampling
//!
//! Provides audio resampling to convert between different sample rates.

use std::f32::consts::PI;

/// Audio resampler quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResamplerQuality {
    /// Low quality, fast (linear interpolation)
    Low,
    /// Medium quality (4-point interpolation)
    Medium,
    /// High quality (sinc interpolation)
    High,
}

/// Audio resampler
pub struct AudioResampler {
    /// Input sample rate
    input_rate: u32,
    /// Output sample rate
    output_rate: u32,
    /// Resampling quality
    quality: ResamplerQuality,
    /// Number of channels
    num_channels: usize,
    /// Input buffer for partial samples
    input_buffer: Vec<f32>,
    /// Current position in resampling
    position: f64,
}

impl AudioResampler {
    /// Create a new audio resampler
    pub fn new(input_rate: u32, output_rate: u32, num_channels: usize) -> Self {
        Self::with_quality(input_rate, output_rate, num_channels, ResamplerQuality::Medium)
    }

    /// Create with specific quality
    pub fn with_quality(
        input_rate: u32,
        output_rate: u32,
        num_channels: usize,
        quality: ResamplerQuality,
    ) -> Self {
        Self {
            input_rate,
            output_rate,
            quality,
            num_channels,
            input_buffer: Vec::new(),
            position: 0.0,
        }
    }

    /// Get resampling ratio
    pub fn ratio(&self) -> f64 {
        self.input_rate as f64 / self.output_rate as f64
    }

    /// Resample audio data
    pub fn resample(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<(), String> {
        if !input.len().is_multiple_of(self.num_channels) {
            return Err("Input length must be multiple of channel count".to_string());
        }

        // Append new input to buffer
        self.input_buffer.extend_from_slice(input);

        let ratio = self.ratio();
        let input_frames = self.input_buffer.len() / self.num_channels;

        // Calculate how many output frames we can generate
        let mut output_frames = 0;
        while self.position + ratio < input_frames as f64 {
            output_frames += 1;
            self.position += ratio;
        }

        // Reserve space for output
        output.reserve(output_frames * self.num_channels);

        // Reset position for resampling
        let mut pos = 0.0;
        for _ in 0..output_frames {
            match self.quality {
                ResamplerQuality::Low => {
                    self.resample_linear(pos, output);
                }
                ResamplerQuality::Medium => {
                    self.resample_cubic(pos, output);
                }
                ResamplerQuality::High => {
                    self.resample_sinc(pos, output);
                }
            }
            pos += ratio;
        }

        // Remove consumed samples from buffer
        let consumed_frames = pos.floor() as usize;
        let consumed_samples = consumed_frames * self.num_channels;
        if consumed_samples > 0 && consumed_samples <= self.input_buffer.len() {
            self.input_buffer.drain(..consumed_samples);
            self.position = pos - consumed_frames as f64;
        }

        Ok(())
    }

    /// Linear interpolation resampling
    fn resample_linear(&self, pos: f64, output: &mut Vec<f32>) {
        let idx0 = pos.floor() as usize;
        let idx1 = (idx0 + 1).min(self.input_buffer.len() / self.num_channels - 1);
        let frac = (pos - idx0 as f64) as f32;

        for ch in 0..self.num_channels {
            let sample0 = self.input_buffer[idx0 * self.num_channels + ch];
            let sample1 = self.input_buffer[idx1 * self.num_channels + ch];
            let interpolated = sample0 + (sample1 - sample0) * frac;
            output.push(interpolated);
        }
    }

    /// Cubic interpolation resampling (4-point)
    fn resample_cubic(&self, pos: f64, output: &mut Vec<f32>) {
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let frames = self.input_buffer.len() / self.num_channels;

        for ch in 0..self.num_channels {
            // Get 4 surrounding points
            let idx0 = if idx > 0 { idx - 1 } else { idx };
            let idx1 = idx;
            let idx2 = (idx + 1).min(frames - 1);
            let idx3 = (idx + 2).min(frames - 1);

            let y0 = self.input_buffer[idx0 * self.num_channels + ch];
            let y1 = self.input_buffer[idx1 * self.num_channels + ch];
            let y2 = self.input_buffer[idx2 * self.num_channels + ch];
            let y3 = self.input_buffer[idx3 * self.num_channels + ch];

            // Cubic interpolation (Catmull-Rom)
            let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
            let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c = -0.5 * y0 + 0.5 * y2;
            let d = y1;

            let interpolated = ((a * frac + b) * frac + c) * frac + d;
            output.push(interpolated);
        }
    }

    /// Sinc interpolation resampling (windowed sinc)
    fn resample_sinc(&self, pos: f64, output: &mut Vec<f32>) {
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let frames = self.input_buffer.len() / self.num_channels;
        
        // Simplified sinc with 8-tap filter
        const TAPS: usize = 8;
        const HALF_TAPS: i32 = (TAPS / 2) as i32;

        for ch in 0..self.num_channels {
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for i in -HALF_TAPS..HALF_TAPS {
                let sample_idx = (idx as i32 + i).clamp(0, frames as i32 - 1) as usize;
                let x = frac - i as f32;
                
                // Windowed sinc function
                let sinc_val = if x.abs() < 0.001 {
                    1.0
                } else {
                    let pi_x = PI * x;
                    (pi_x.sin() / pi_x) * (0.5 + 0.5 * (PI * x / TAPS as f32).cos()) // Hamming window
                };

                let sample = self.input_buffer[sample_idx * self.num_channels + ch];
                sum += sample * sinc_val;
                weight_sum += sinc_val;
            }

            let interpolated = if weight_sum > 0.0 { sum / weight_sum } else { 0.0 };
            output.push(interpolated);
        }
    }

    /// Reset the resampler state
    pub fn reset(&mut self) {
        self.input_buffer.clear();
        self.position = 0.0;
    }

    /// Get input sample rate
    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    /// Get output sample rate
    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    /// Get number of channels
    pub fn num_channels(&self) -> usize {
        self.num_channels
    }

    /// Get quality setting
    pub fn quality(&self) -> ResamplerQuality {
        self.quality
    }

    /// Set quality level
    pub fn set_quality(&mut self, quality: ResamplerQuality) {
        self.quality = quality;
    }
}

/// Adaptive audio resampler for real-time rate adjustment
///
/// This resampler supports dynamic rate adjustment to maintain audio/video sync.
/// It can smoothly adjust the resampling ratio without artifacts.
pub struct AdaptiveResampler {
    /// Base input sample rate
    base_input_rate: u32,
    /// Base output sample rate
    base_output_rate: u32,
    /// Current rate adjustment factor (1.0 = no adjustment)
    rate_adjustment: f64,
    /// Target rate adjustment (for smooth transitions)
    target_adjustment: f64,
    /// Adjustment smoothing factor (0.0-1.0, lower = smoother)
    smoothing_factor: f64,
    /// Number of channels
    num_channels: usize,
    /// Quality setting
    quality: ResamplerQuality,
    /// Input buffer for partial samples
    input_buffer: Vec<f32>,
    /// Current position in resampling
    position: f64,
}

/// Default smoothing factor for rate adjustment (provides good stability)
const DEFAULT_SMOOTHING_FACTOR: f64 = 0.1;

impl AdaptiveResampler {
    /// Create a new adaptive resampler
    pub fn new(base_input_rate: u32, base_output_rate: u32, num_channels: usize) -> Self {
        Self {
            base_input_rate,
            base_output_rate,
            rate_adjustment: 1.0,
            target_adjustment: 1.0,
            smoothing_factor: DEFAULT_SMOOTHING_FACTOR,
            num_channels,
            quality: ResamplerQuality::Medium,
            input_buffer: Vec::new(),
            position: 0.0,
        }
    }

    /// Create with custom quality and smoothing
    pub fn with_options(
        base_input_rate: u32,
        base_output_rate: u32,
        num_channels: usize,
        quality: ResamplerQuality,
        smoothing_factor: f64,
    ) -> Self {
        Self {
            base_input_rate,
            base_output_rate,
            rate_adjustment: 1.0,
            target_adjustment: 1.0,
            smoothing_factor: smoothing_factor.clamp(0.01, 1.0),
            num_channels,
            quality,
            input_buffer: Vec::new(),
            position: 0.0,
        }
    }

    /// Get current effective resampling ratio
    pub fn ratio(&self) -> f64 {
        (self.base_input_rate as f64 / self.base_output_rate as f64) * self.rate_adjustment
    }

    /// Set target rate adjustment
    ///
    /// Values > 1.0 speed up playback (skip samples)
    /// Values < 1.0 slow down playback (interpolate more)
    /// Value = 1.0 is normal playback speed
    pub fn set_rate_adjustment(&mut self, adjustment: f64) {
        // Clamp to reasonable range (0.5x to 2.0x)
        self.target_adjustment = adjustment.clamp(0.5, 2.0);
    }

    /// Get current rate adjustment
    pub fn rate_adjustment(&self) -> f64 {
        self.rate_adjustment
    }

    /// Set smoothing factor
    pub fn set_smoothing(&mut self, factor: f64) {
        self.smoothing_factor = factor.clamp(0.01, 1.0);
    }

    /// Resample audio data with adaptive rate adjustment
    pub fn resample(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<(), String> {
        if !input.len().is_multiple_of(self.num_channels) {
            return Err("Input length must be multiple of channel count".to_string());
        }

        // Smoothly transition rate adjustment towards target
        self.rate_adjustment += (self.target_adjustment - self.rate_adjustment) * self.smoothing_factor;

        // Append new input to buffer
        self.input_buffer.extend_from_slice(input);

        let ratio = self.ratio();
        let input_frames = self.input_buffer.len() / self.num_channels;

        // Calculate how many output frames we can generate
        let mut output_frames = 0;
        while self.position + ratio < input_frames as f64 {
            output_frames += 1;
            self.position += ratio;
        }

        // Reserve space for output
        output.reserve(output_frames * self.num_channels);

        // Reset position for resampling
        let mut pos = 0.0;
        for _ in 0..output_frames {
            match self.quality {
                ResamplerQuality::Low => {
                    self.resample_linear(pos, output);
                }
                ResamplerQuality::Medium => {
                    self.resample_cubic(pos, output);
                }
                ResamplerQuality::High => {
                    self.resample_sinc(pos, output);
                }
            }
            pos += ratio;
        }

        // Remove consumed samples from buffer
        let consumed_frames = pos.floor() as usize;
        let consumed_samples = consumed_frames * self.num_channels;
        if consumed_samples > 0 && consumed_samples <= self.input_buffer.len() {
            self.input_buffer.drain(..consumed_samples);
            self.position = pos - consumed_frames as f64;
        }

        Ok(())
    }

    /// Linear interpolation resampling
    fn resample_linear(&self, pos: f64, output: &mut Vec<f32>) {
        let idx0 = pos.floor() as usize;
        let idx1 = (idx0 + 1).min(self.input_buffer.len() / self.num_channels - 1);
        let frac = (pos - idx0 as f64) as f32;

        for ch in 0..self.num_channels {
            let sample0 = self.input_buffer[idx0 * self.num_channels + ch];
            let sample1 = self.input_buffer[idx1 * self.num_channels + ch];
            let interpolated = sample0 + (sample1 - sample0) * frac;
            output.push(interpolated);
        }
    }

    /// Cubic interpolation resampling (4-point)
    fn resample_cubic(&self, pos: f64, output: &mut Vec<f32>) {
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let frames = self.input_buffer.len() / self.num_channels;

        for ch in 0..self.num_channels {
            let idx0 = if idx > 0 { idx - 1 } else { idx };
            let idx1 = idx;
            let idx2 = (idx + 1).min(frames - 1);
            let idx3 = (idx + 2).min(frames - 1);

            let y0 = self.input_buffer[idx0 * self.num_channels + ch];
            let y1 = self.input_buffer[idx1 * self.num_channels + ch];
            let y2 = self.input_buffer[idx2 * self.num_channels + ch];
            let y3 = self.input_buffer[idx3 * self.num_channels + ch];

            // Cubic interpolation (Catmull-Rom)
            let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
            let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c = -0.5 * y0 + 0.5 * y2;
            let d = y1;

            let interpolated = ((a * frac + b) * frac + c) * frac + d;
            output.push(interpolated);
        }
    }

    /// Sinc interpolation resampling
    fn resample_sinc(&self, pos: f64, output: &mut Vec<f32>) {
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let frames = self.input_buffer.len() / self.num_channels;
        
        const TAPS: usize = 8;
        const HALF_TAPS: i32 = (TAPS / 2) as i32;

        for ch in 0..self.num_channels {
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for i in -HALF_TAPS..HALF_TAPS {
                let sample_idx = (idx as i32 + i).clamp(0, frames as i32 - 1) as usize;
                let x = frac - i as f32;
                
                // Windowed sinc function with bounds checking
                // The Hann window term (0.5 + 0.5 * cos(...)) is bounded [0, 1]
                // by construction since cos is bounded [-1, 1]
                let sinc_val = if x.abs() < 0.001 {
                    1.0
                } else {
                    let pi_x = PI * x;
                    let sinc = pi_x.sin() / pi_x;
                    // Hann window: 0.5 + 0.5 * cos(π * x / N) where N = TAPS
                    // Clamp window value to [0, 1] for numerical safety
                    let window = (0.5 + 0.5 * (PI * x / TAPS as f32).cos()).clamp(0.0, 1.0);
                    sinc * window
                };

                let sample = self.input_buffer[sample_idx * self.num_channels + ch];
                sum += sample * sinc_val;
                weight_sum += sinc_val;
            }

            let interpolated = if weight_sum > 0.0 { sum / weight_sum } else { 0.0 };
            output.push(interpolated);
        }
    }

    /// Reset the resampler state
    pub fn reset(&mut self) {
        self.input_buffer.clear();
        self.position = 0.0;
        self.rate_adjustment = 1.0;
        self.target_adjustment = 1.0;
    }

    /// Get number of channels
    pub fn num_channels(&self) -> usize {
        self.num_channels
    }

    /// Get base input rate
    pub fn base_input_rate(&self) -> u32 {
        self.base_input_rate
    }

    /// Get base output rate
    pub fn base_output_rate(&self) -> u32 {
        self.base_output_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_creation() {
        let resampler = AudioResampler::new(44100, 48000, 2);
        assert_eq!(resampler.input_rate(), 44100);
        assert_eq!(resampler.output_rate(), 48000);
        assert_eq!(resampler.num_channels(), 2);
    }

    #[test]
    fn test_resampler_ratio() {
        let resampler = AudioResampler::new(44100, 48000, 2);
        let ratio = resampler.ratio();
        assert!((ratio - 0.91875).abs() < 0.001);
    }

    #[test]
    fn test_resampler_linear() {
        let mut resampler = AudioResampler::with_quality(
            44100,
            48000,
            2,
            ResamplerQuality::Low,
        );

        let input = vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0];
        let mut output = Vec::new();

        resampler.resample(&input, &mut output).unwrap();
        assert!(!output.is_empty());
        assert_eq!(output.len() % 2, 0); // Stereo
    }

    #[test]
    fn test_resampler_cubic() {
        let mut resampler = AudioResampler::with_quality(
            44100,
            48000,
            2,
            ResamplerQuality::Medium,
        );

        let input = vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0];
        let mut output = Vec::new();

        resampler.resample(&input, &mut output).unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_resampler_sinc() {
        let mut resampler = AudioResampler::with_quality(
            44100,
            48000,
            2,
            ResamplerQuality::High,
        );

        let input = vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0];
        let mut output = Vec::new();

        resampler.resample(&input, &mut output).unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_resampler_reset() {
        let mut resampler = AudioResampler::new(44100, 48000, 2);
        let input = vec![0.5, -0.5, 0.3, -0.3];
        let mut output = Vec::new();

        resampler.resample(&input, &mut output).unwrap();
        resampler.reset();
        
        assert_eq!(resampler.position, 0.0);
    }

    #[test]
    fn test_resampler_invalid_input() {
        let mut resampler = AudioResampler::new(44100, 48000, 2);
        let input = vec![0.5, -0.5, 0.3]; // Odd number for stereo
        let mut output = Vec::new();

        assert!(resampler.resample(&input, &mut output).is_err());
    }

    #[test]
    fn test_adaptive_resampler_creation() {
        let resampler = AdaptiveResampler::new(44100, 48000, 2);
        assert_eq!(resampler.base_input_rate(), 44100);
        assert_eq!(resampler.base_output_rate(), 48000);
        assert_eq!(resampler.num_channels(), 2);
        assert!((resampler.rate_adjustment() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_adaptive_resampler_rate_adjustment() {
        let mut resampler = AdaptiveResampler::new(44100, 48000, 2);
        
        // Set rate adjustment
        resampler.set_rate_adjustment(1.1);
        
        // Do some resampling to trigger smoothing
        let input = vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0];
        let mut output = Vec::new();
        resampler.resample(&input, &mut output).unwrap();
        
        // Rate should have moved towards target
        assert!(resampler.rate_adjustment() > 1.0);
        assert!(resampler.rate_adjustment() <= 1.1);
    }

    #[test]
    fn test_adaptive_resampler_clamping() {
        let mut resampler = AdaptiveResampler::new(44100, 48000, 2);
        
        // Try setting extreme values - should be clamped
        resampler.set_rate_adjustment(5.0);
        let input = vec![0.0, 0.0, 0.5, 0.5];
        let mut output = Vec::new();
        resampler.resample(&input, &mut output).unwrap();
        
        // Should be clamped to 2.0 max
        assert!(resampler.rate_adjustment() <= 2.0);
    }

    #[test]
    fn test_adaptive_resampler_reset() {
        let mut resampler = AdaptiveResampler::new(44100, 48000, 2);
        
        resampler.set_rate_adjustment(1.5);
        let input = vec![0.5, -0.5, 0.3, -0.3];
        let mut output = Vec::new();
        resampler.resample(&input, &mut output).unwrap();
        
        resampler.reset();
        assert!((resampler.rate_adjustment() - 1.0).abs() < 0.001);
    }
}
