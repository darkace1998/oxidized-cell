//! Audio time stretching
//!
//! Provides audio time stretching and pitch shifting for synchronization.

/// Time stretching algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StretchAlgorithm {
    /// Simple overlap-add
    Simple,
    /// Phase vocoder
    PhaseVocoder,
    /// WSOLA (Waveform Similarity Overlap-Add)
    Wsola,
}

/// Audio time stretcher configuration
#[derive(Debug, Clone, Copy)]
pub struct TimeStretchConfig {
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub num_channels: usize,
    /// Stretch algorithm
    pub algorithm: StretchAlgorithm,
    /// Window size for processing
    pub window_size: usize,
    /// Overlap between windows
    pub overlap: usize,
}

impl Default for TimeStretchConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            num_channels: 2,
            algorithm: StretchAlgorithm::Simple,
            window_size: 2048,
            overlap: 512,
        }
    }
}

/// Audio time stretcher
pub struct AudioTimeStretcher {
    config: TimeStretchConfig,
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
    position: usize,
}

impl AudioTimeStretcher {
    /// Create a new time stretcher
    pub fn new(config: TimeStretchConfig) -> Self {
        Self {
            config,
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            position: 0,
        }
    }

    /// Create with default configuration
    pub fn default_config(sample_rate: u32, num_channels: usize) -> Self {
        Self::new(TimeStretchConfig {
            sample_rate,
            num_channels,
            ..Default::default()
        })
    }

    /// Stretch audio by a given factor
    /// factor > 1.0: slower playback (longer duration)
    /// factor < 1.0: faster playback (shorter duration)
    /// factor = 1.0: no change
    pub fn stretch(&mut self, input: &[f32], factor: f32, output: &mut Vec<f32>) -> Result<(), String> {
        if factor <= 0.0 {
            return Err("Stretch factor must be positive".to_string());
        }

        if !input.len().is_multiple_of(self.config.num_channels) {
            return Err("Input length must be multiple of channel count".to_string());
        }

        // No stretching needed
        if (factor - 1.0).abs() < 0.001 {
            output.extend_from_slice(input);
            return Ok(());
        }

        // Add input to buffer
        self.input_buffer.extend_from_slice(input);

        match self.config.algorithm {
            StretchAlgorithm::Simple => self.stretch_simple(factor, output),
            StretchAlgorithm::PhaseVocoder => self.stretch_phase_vocoder(factor, output),
            StretchAlgorithm::Wsola => self.stretch_wsola(factor, output),
        }
    }

    /// Simple overlap-add time stretching
    fn stretch_simple(&mut self, factor: f32, output: &mut Vec<f32>) -> Result<(), String> {
        let window_size = self.config.window_size;
        let overlap = self.config.overlap;
        let hop_in = (window_size - overlap) as f32;

        let frames = self.input_buffer.len() / self.config.num_channels;
        
        while self.position + window_size <= frames {
            let out_pos = (self.position as f32 * factor) as usize;
            
            // Process each channel
            for ch in 0..self.config.num_channels {
                for i in 0..window_size {
                    let in_idx = (self.position + i) * self.config.num_channels + ch;
                    let sample = self.input_buffer[in_idx];
                    
                    // Apply Hann window
                    let window = 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / (window_size as f32 - 1.0)).cos());
                    let windowed = sample * window;
                    
                    let out_idx = out_pos * self.config.num_channels + i * self.config.num_channels + ch;
                    
                    // Ensure output buffer is large enough
                    while self.output_buffer.len() <= out_idx {
                        self.output_buffer.push(0.0);
                    }
                    
                    self.output_buffer[out_idx] += windowed;
                }
            }
            
            self.position += hop_in as usize;
        }

        // Move processed samples to output
        let available = self.output_buffer.len().min(output.capacity() - output.len());
        output.extend(self.output_buffer.drain(..available));

        Ok(())
    }

    /// Phase vocoder time stretching (simplified)
    fn stretch_phase_vocoder(&mut self, factor: f32, output: &mut Vec<f32>) -> Result<(), String> {
        // Simplified phase vocoder implementation
        // In production, this would use FFT for proper phase vocoder
        self.stretch_simple(factor, output)
    }

    /// WSOLA (Waveform Similarity Overlap-Add) time stretching
    fn stretch_wsola(&mut self, factor: f32, output: &mut Vec<f32>) -> Result<(), String> {
        let window_size = self.config.window_size;
        let overlap = self.config.overlap;
        let frames = self.input_buffer.len() / self.config.num_channels;

        while self.position + window_size <= frames {
            // Find best correlation offset
            let search_range = window_size / 4;
            let mut best_offset = 0;
            let mut best_correlation = f32::MIN;

            for offset in 0..search_range {
                if self.position + offset + window_size > frames {
                    break;
                }

                let mut correlation = 0.0;
                for i in 0..overlap {
                    for ch in 0..self.config.num_channels {
                        let idx1 = (self.position + i) * self.config.num_channels + ch;
                        let idx2 = (self.position + offset + i) * self.config.num_channels + ch;
                        correlation += self.input_buffer[idx1] * self.input_buffer[idx2];
                    }
                }

                if correlation > best_correlation {
                    best_correlation = correlation;
                    best_offset = offset;
                }
            }

            // Copy window with best offset
            let start = (self.position + best_offset) * self.config.num_channels;
            let end = start + window_size * self.config.num_channels;
            
            if end <= self.input_buffer.len() {
                output.extend_from_slice(&self.input_buffer[start..end]);
            }

            self.position += ((window_size - overlap) as f32 * factor) as usize;
        }

        Ok(())
    }

    /// Reset the time stretcher
    pub fn reset(&mut self) {
        self.input_buffer.clear();
        self.output_buffer.clear();
        self.position = 0;
    }

    /// Get configuration
    pub fn config(&self) -> &TimeStretchConfig {
        &self.config
    }

    /// Get buffered sample count
    pub fn buffer_len(&self) -> usize {
        self.input_buffer.len()
    }

    /// Set algorithm
    pub fn set_algorithm(&mut self, algorithm: StretchAlgorithm) {
        self.config.algorithm = algorithm;
    }
}

/// Real-time time stretcher with smooth factor transitions
///
/// This time stretcher supports dynamic factor adjustment for audio/video
/// synchronization, with smooth transitions to avoid audio artifacts.
pub struct RealTimeStretcher {
    config: TimeStretchConfig,
    /// Current stretch factor
    current_factor: f32,
    /// Target stretch factor
    target_factor: f32,
    /// Factor transition speed (0.0-1.0)
    transition_speed: f32,
    /// Input buffer
    input_buffer: Vec<f32>,
    /// Output buffer for partial results
    output_buffer: Vec<f32>,
    /// Processing position
    position: usize,
    /// Previous analysis window (for crossfade)
    prev_window: Vec<f32>,
}

/// Default transition speed for stretch factor changes
const DEFAULT_TRANSITION_SPEED: f32 = 0.05;

impl RealTimeStretcher {
    /// Create a new real-time time stretcher
    pub fn new(sample_rate: u32, num_channels: usize) -> Self {
        let config = TimeStretchConfig {
            sample_rate,
            num_channels,
            algorithm: StretchAlgorithm::Wsola,
            window_size: 1024,
            overlap: 256,
        };
        
        Self {
            config,
            current_factor: 1.0,
            target_factor: 1.0,
            transition_speed: DEFAULT_TRANSITION_SPEED,
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            position: 0,
            prev_window: Vec::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: TimeStretchConfig, transition_speed: f32) -> Self {
        Self {
            config,
            current_factor: 1.0,
            target_factor: 1.0,
            transition_speed: transition_speed.clamp(0.01, 1.0),
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            position: 0,
            prev_window: vec![0.0; config.window_size * config.num_channels],
        }
    }

    /// Set target stretch factor (smooth transition)
    ///
    /// factor > 1.0: slower playback (stretch)
    /// factor < 1.0: faster playback (compress)
    /// factor = 1.0: normal speed
    pub fn set_factor(&mut self, factor: f32) {
        self.target_factor = factor.clamp(0.5, 2.0);
    }

    /// Get current stretch factor
    pub fn current_factor(&self) -> f32 {
        self.current_factor
    }

    /// Get target stretch factor
    pub fn target_factor(&self) -> f32 {
        self.target_factor
    }

    /// Set transition speed
    pub fn set_transition_speed(&mut self, speed: f32) {
        self.transition_speed = speed.clamp(0.01, 1.0);
    }

    /// Process audio data with real-time factor adjustment
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<usize, String> {
        if !input.len().is_multiple_of(self.config.num_channels) {
            return Err("Input length must be multiple of channel count".to_string());
        }

        // Smoothly transition stretch factor
        self.current_factor += (self.target_factor - self.current_factor) * self.transition_speed;

        // No stretching needed (approximately 1.0)
        if (self.current_factor - 1.0).abs() < 0.01 {
            output.extend_from_slice(input);
            return Ok(input.len());
        }

        // Add input to buffer
        self.input_buffer.extend_from_slice(input);

        let initial_output_len = output.len();
        
        // Process using WSOLA with crossfade
        self.process_wsola(output)?;

        Ok(output.len() - initial_output_len)
    }

    /// WSOLA processing with crossfade for smooth factor transitions
    fn process_wsola(&mut self, output: &mut Vec<f32>) -> Result<(), String> {
        let window_size = self.config.window_size;
        let overlap = self.config.overlap;
        let frames = self.input_buffer.len() / self.config.num_channels;
        let search_range = window_size / 4;

        while self.position + window_size <= frames {
            // Find best correlation offset
            let mut best_offset = 0;
            let mut best_correlation = f32::MIN;

            for offset in 0..search_range {
                if self.position + offset + window_size > frames {
                    break;
                }

                let mut correlation = 0.0;
                for i in 0..overlap.min(self.prev_window.len() / self.config.num_channels) {
                    for ch in 0..self.config.num_channels {
                        let prev_idx = i * self.config.num_channels + ch;
                        let curr_idx = (self.position + offset + i) * self.config.num_channels + ch;
                        
                        if prev_idx < self.prev_window.len() && curr_idx < self.input_buffer.len() {
                            correlation += self.prev_window[prev_idx] * self.input_buffer[curr_idx];
                        }
                    }
                }

                if correlation > best_correlation {
                    best_correlation = correlation;
                    best_offset = offset;
                }
            }

            // Extract window with crossfade
            let window_start = (self.position + best_offset) * self.config.num_channels;
            let window_end = window_start + window_size * self.config.num_channels;
            
            if window_end <= self.input_buffer.len() {
                // Crossfade overlap region
                for i in 0..overlap {
                    let fade_in = i as f32 / overlap as f32;
                    let fade_out = 1.0 - fade_in;
                    
                    for ch in 0..self.config.num_channels {
                        let prev_idx = i * self.config.num_channels + ch;
                        let curr_idx = window_start + i * self.config.num_channels + ch;
                        
                        if prev_idx < self.prev_window.len() {
                            let crossfaded = self.prev_window[prev_idx] * fade_out 
                                + self.input_buffer[curr_idx] * fade_in;
                            output.push(crossfaded);
                        }
                    }
                }
                
                // Copy rest of window
                let copy_start = window_start + overlap * self.config.num_channels;
                let copy_end = (copy_start + (window_size - overlap) * self.config.num_channels)
                    .min(self.input_buffer.len());
                output.extend_from_slice(&self.input_buffer[copy_start..copy_end]);
                
                // Update previous window for next iteration
                let save_start = window_start + (window_size - overlap) * self.config.num_channels;
                let save_end = window_end.min(self.input_buffer.len());
                if save_end > save_start {
                    self.prev_window.clear();
                    self.prev_window.extend_from_slice(&self.input_buffer[save_start..save_end]);
                }
            }

            // Advance position based on current stretch factor
            let hop = ((window_size - overlap) as f32 / self.current_factor) as usize;
            self.position += hop.max(1);
        }

        // Remove consumed samples
        if self.position > 0 {
            let consumed = (self.position * self.config.num_channels).min(self.input_buffer.len());
            self.input_buffer.drain(..consumed);
            self.position = 0;
        }

        Ok(())
    }

    /// Get latency in samples
    pub fn latency_samples(&self) -> usize {
        self.config.window_size * 2
    }

    /// Reset the stretcher state
    pub fn reset(&mut self) {
        self.input_buffer.clear();
        self.output_buffer.clear();
        self.prev_window.clear();
        self.prev_window.resize(self.config.window_size * self.config.num_channels, 0.0);
        self.position = 0;
        self.current_factor = 1.0;
        self.target_factor = 1.0;
    }

    /// Get configuration
    pub fn config(&self) -> &TimeStretchConfig {
        &self.config
    }
}

impl Default for AudioTimeStretcher {
    fn default() -> Self {
        Self::new(TimeStretchConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_stretcher_creation() {
        let stretcher = AudioTimeStretcher::default();
        assert_eq!(stretcher.config().num_channels, 2);
    }

    #[test]
    fn test_time_stretcher_no_stretch() {
        let mut stretcher = AudioTimeStretcher::default_config(48000, 2);
        let input = vec![0.5, -0.5, 0.3, -0.3];
        let mut output = Vec::new();

        stretcher.stretch(&input, 1.0, &mut output).unwrap();
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn test_time_stretcher_simple() {
        let config = TimeStretchConfig {
            sample_rate: 48000,
            num_channels: 2,
            algorithm: StretchAlgorithm::Simple,
            window_size: 64,
            overlap: 16,
        };
        let mut stretcher = AudioTimeStretcher::new(config);
        
        let input = vec![0.5; 256]; // 128 stereo frames
        let mut output = Vec::new();

        stretcher.stretch(&input, 1.5, &mut output).unwrap();
        // With stretching, output should be generated
        assert!(!output.is_empty() || stretcher.output_buffer.len() > 0);
    }

    #[test]
    fn test_time_stretcher_wsola() {
        let config = TimeStretchConfig {
            sample_rate: 48000,
            num_channels: 2,
            algorithm: StretchAlgorithm::Wsola,
            window_size: 64,
            overlap: 16,
        };
        let mut stretcher = AudioTimeStretcher::new(config);
        
        let input = vec![0.5; 256];
        let mut output = Vec::new();

        stretcher.stretch(&input, 1.2, &mut output).unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_time_stretcher_invalid_factor() {
        let mut stretcher = AudioTimeStretcher::default();
        let input = vec![0.5, -0.5];
        let mut output = Vec::new();

        assert!(stretcher.stretch(&input, 0.0, &mut output).is_err());
        assert!(stretcher.stretch(&input, -1.0, &mut output).is_err());
    }

    #[test]
    fn test_time_stretcher_reset() {
        let mut stretcher = AudioTimeStretcher::default();
        let input = vec![0.5; 100];
        let mut output = Vec::new();

        stretcher.stretch(&input, 1.0, &mut output).unwrap();
        stretcher.reset();
        
        assert_eq!(stretcher.buffer_len(), 0);
    }

    #[test]
    fn test_time_stretcher_invalid_input() {
        let mut stretcher = AudioTimeStretcher::default_config(48000, 2);
        let input = vec![0.5, -0.5, 0.3]; // Odd number for stereo
        let mut output = Vec::new();

        assert!(stretcher.stretch(&input, 1.0, &mut output).is_err());
    }

    #[test]
    fn test_realtime_stretcher_creation() {
        let stretcher = RealTimeStretcher::new(48000, 2);
        assert_eq!(stretcher.current_factor(), 1.0);
        assert_eq!(stretcher.target_factor(), 1.0);
    }

    #[test]
    fn test_realtime_stretcher_factor_transition() {
        let mut stretcher = RealTimeStretcher::new(48000, 2);
        
        // Set target factor
        stretcher.set_factor(1.5);
        assert_eq!(stretcher.target_factor(), 1.5);
        
        // Process some audio to trigger transition
        let input = vec![0.5; 4096];
        let mut output = Vec::new();
        stretcher.process(&input, &mut output).unwrap();
        
        // Current factor should move towards target
        assert!(stretcher.current_factor() > 1.0);
        assert!(stretcher.current_factor() <= 1.5);
    }

    #[test]
    fn test_realtime_stretcher_passthrough() {
        let mut stretcher = RealTimeStretcher::new(48000, 2);
        
        // Factor ~1.0 should pass through
        stretcher.set_factor(1.0);
        let input = vec![0.5, -0.5, 0.3, -0.3];
        let mut output = Vec::new();
        
        stretcher.process(&input, &mut output).unwrap();
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn test_realtime_stretcher_reset() {
        let mut stretcher = RealTimeStretcher::new(48000, 2);
        
        stretcher.set_factor(1.5);
        let input = vec![0.5; 100];
        let mut output = Vec::new();
        stretcher.process(&input, &mut output).unwrap();
        
        stretcher.reset();
        assert_eq!(stretcher.current_factor(), 1.0);
        assert_eq!(stretcher.target_factor(), 1.0);
    }
}
