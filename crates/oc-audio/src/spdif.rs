//! S/PDIF (Sony/Philips Digital Interface) output support
//!
//! Provides digital audio output via S/PDIF interface.

/// S/PDIF output configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpdifConfig {
    /// Sample rate (typically 48000 Hz)
    pub sample_rate: u32,
    /// Number of channels (2 for S/PDIF)
    pub num_channels: usize,
    /// Enable AC3 pass-through
    pub ac3_passthrough: bool,
    /// Enable DTS pass-through
    pub dts_passthrough: bool,
}

impl Default for SpdifConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            num_channels: 2, // S/PDIF is stereo
            ac3_passthrough: false,
            dts_passthrough: false,
        }
    }
}

/// S/PDIF output state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpdifState {
    Disabled,
    Enabled,
    Active,
}

/// S/PDIF output handler
pub struct SpdifOutput {
    config: SpdifConfig,
    state: SpdifState,
    buffer: Vec<f32>,
}

impl SpdifOutput {
    /// Create a new S/PDIF output handler
    pub fn new() -> Self {
        Self::with_config(SpdifConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: SpdifConfig) -> Self {
        Self {
            config,
            state: SpdifState::Disabled,
            buffer: Vec::new(),
        }
    }

    /// Enable S/PDIF output
    pub fn enable(&mut self) -> Result<(), String> {
        if self.state == SpdifState::Disabled {
            self.state = SpdifState::Enabled;
            tracing::info!("S/PDIF output enabled at {} Hz", self.config.sample_rate);
            Ok(())
        } else {
            Err("S/PDIF already enabled".to_string())
        }
    }

    /// Disable S/PDIF output
    pub fn disable(&mut self) -> Result<(), String> {
        self.state = SpdifState::Disabled;
        self.buffer.clear();
        tracing::info!("S/PDIF output disabled");
        Ok(())
    }

    /// Start S/PDIF output
    pub fn start(&mut self) -> Result<(), String> {
        if self.state == SpdifState::Enabled {
            self.state = SpdifState::Active;
            tracing::info!("S/PDIF output started");
            Ok(())
        } else {
            Err("S/PDIF not enabled".to_string())
        }
    }

    /// Stop S/PDIF output
    pub fn stop(&mut self) -> Result<(), String> {
        if self.state == SpdifState::Active {
            self.state = SpdifState::Enabled;
            tracing::info!("S/PDIF output stopped");
            Ok(())
        } else {
            Err("S/PDIF not active".to_string())
        }
    }

    /// Write samples to S/PDIF output
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<(), String> {
        if self.state != SpdifState::Active {
            return Err("S/PDIF output not active".to_string());
        }

        // Convert to stereo if needed
        if !samples.len().is_multiple_of(2) {
            return Err("S/PDIF requires even number of samples (stereo)".to_string());
        }

        self.buffer.extend_from_slice(samples);
        Ok(())
    }

    /// Read samples from buffer
    pub fn read_samples(&mut self, count: usize) -> Vec<f32> {
        let available = self.buffer.len().min(count);
        self.buffer.drain(..available).collect()
    }

    /// Get S/PDIF state
    pub fn state(&self) -> SpdifState {
        self.state
    }

    /// Get configuration
    pub fn config(&self) -> &SpdifConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: SpdifConfig) -> Result<(), String> {
        if self.state != SpdifState::Disabled {
            return Err("Cannot change S/PDIF config while enabled".to_string());
        }
        self.config = config;
        Ok(())
    }

    /// Get buffer length
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Clear buffer
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }
}

impl Default for SpdifOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spdif_creation() {
        let spdif = SpdifOutput::new();
        assert_eq!(spdif.state(), SpdifState::Disabled);
    }

    #[test]
    fn test_spdif_enable_disable() {
        let mut spdif = SpdifOutput::new();
        
        assert!(spdif.enable().is_ok());
        assert_eq!(spdif.state(), SpdifState::Enabled);
        
        assert!(spdif.disable().is_ok());
        assert_eq!(spdif.state(), SpdifState::Disabled);
    }

    #[test]
    fn test_spdif_lifecycle() {
        let mut spdif = SpdifOutput::new();
        
        assert!(spdif.enable().is_ok());
        assert!(spdif.start().is_ok());
        assert_eq!(spdif.state(), SpdifState::Active);
        
        let samples = vec![0.5, -0.5, 0.3, -0.3];
        assert!(spdif.write_samples(&samples).is_ok());
        assert_eq!(spdif.buffer_len(), 4);
        
        assert!(spdif.stop().is_ok());
        assert_eq!(spdif.state(), SpdifState::Enabled);
    }

    #[test]
    fn test_spdif_config() {
        let mut config = SpdifConfig::default();
        config.ac3_passthrough = true;
        config.dts_passthrough = true;
        
        let spdif = SpdifOutput::with_config(config);
        assert_eq!(spdif.config().ac3_passthrough, true);
        assert_eq!(spdif.config().dts_passthrough, true);
    }

    #[test]
    fn test_spdif_write_odd_samples() {
        let mut spdif = SpdifOutput::new();
        spdif.enable().unwrap();
        spdif.start().unwrap();
        
        let samples = vec![0.5, -0.5, 0.3]; // Odd number
        assert!(spdif.write_samples(&samples).is_err());
    }

    #[test]
    fn test_spdif_read_samples() {
        let mut spdif = SpdifOutput::new();
        spdif.enable().unwrap();
        spdif.start().unwrap();
        
        let samples = vec![0.5, -0.5, 0.3, -0.3];
        spdif.write_samples(&samples).unwrap();
        
        let read = spdif.read_samples(2);
        assert_eq!(read.len(), 2);
        assert_eq!(spdif.buffer_len(), 2);
    }
}
