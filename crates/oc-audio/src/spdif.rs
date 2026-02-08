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

    /// Check if currently in passthrough mode
    pub fn is_passthrough(&self) -> bool {
        self.config.ac3_passthrough || self.config.dts_passthrough
    }
}

/// Bitstream format for passthrough
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitstreamFormat {
    /// AC3 (Dolby Digital)
    Ac3,
    /// DTS (Digital Theater Systems)
    Dts,
    /// EAC3 (Enhanced AC3 / Dolby Digital Plus)
    Eac3,
    /// True HD (Dolby TrueHD)
    TrueHd,
    /// DTS-HD Master Audio
    DtsHd,
}

impl BitstreamFormat {
    /// Get burst preamble for IEC 61937 encapsulation
    pub fn burst_preamble(&self) -> [u16; 2] {
        // IEC 61937 preamble: Pa = 0xF872, Pb = 0x4E1F
        [0xF872, 0x4E1F]
    }

    /// Get data type code for IEC 61937
    pub fn data_type(&self) -> u16 {
        match self {
            BitstreamFormat::Ac3 => 0x0001,
            BitstreamFormat::Dts => 0x000B,
            BitstreamFormat::Eac3 => 0x0015,
            BitstreamFormat::TrueHd => 0x0016,
            BitstreamFormat::DtsHd => 0x0011,
        }
    }
}

/// S/PDIF passthrough handler for bitstream audio
///
/// Handles IEC 61937 encapsulation of compressed audio bitstreams
/// for pass-through to external receivers/AVRs.
pub struct SpdifPassthrough {
    format: BitstreamFormat,
    state: SpdifState,
    /// Output buffer for IEC 61937 frames
    output_buffer: Vec<u8>,
    /// Current burst position
    burst_position: usize,
    /// Frame counter
    frame_count: u64,
}

/// Size of IEC 61937 burst for AC3 (6144 bytes = 1536 stereo 16-bit samples)
const AC3_BURST_SIZE: usize = 6144;

/// Size of IEC 61937 burst for DTS (varies by stream type)
const DTS_BURST_SIZE: usize = 8192;

impl SpdifPassthrough {
    /// Create a new S/PDIF passthrough handler
    pub fn new(format: BitstreamFormat) -> Self {
        Self {
            format,
            state: SpdifState::Disabled,
            output_buffer: Vec::new(),
            burst_position: 0,
            frame_count: 0,
        }
    }

    /// Enable passthrough
    pub fn enable(&mut self) -> Result<(), String> {
        if self.state == SpdifState::Disabled {
            self.state = SpdifState::Enabled;
            tracing::info!("S/PDIF passthrough enabled for {:?}", self.format);
            Ok(())
        } else {
            Err("S/PDIF passthrough already enabled".to_string())
        }
    }

    /// Disable passthrough
    pub fn disable(&mut self) {
        self.state = SpdifState::Disabled;
        self.output_buffer.clear();
        self.burst_position = 0;
        tracing::info!("S/PDIF passthrough disabled");
    }

    /// Start passthrough
    pub fn start(&mut self) -> Result<(), String> {
        if self.state == SpdifState::Enabled {
            self.state = SpdifState::Active;
            tracing::info!("S/PDIF passthrough started");
            Ok(())
        } else {
            Err("S/PDIF passthrough not enabled".to_string())
        }
    }

    /// Stop passthrough
    pub fn stop(&mut self) -> Result<(), String> {
        if self.state == SpdifState::Active {
            self.state = SpdifState::Enabled;
            tracing::info!("S/PDIF passthrough stopped");
            Ok(())
        } else {
            Err("S/PDIF passthrough not active".to_string())
        }
    }

    /// Get burst size for current format
    fn burst_size(&self) -> usize {
        match self.format {
            BitstreamFormat::Ac3 | BitstreamFormat::Eac3 => AC3_BURST_SIZE,
            BitstreamFormat::Dts | BitstreamFormat::DtsHd => DTS_BURST_SIZE,
            BitstreamFormat::TrueHd => AC3_BURST_SIZE * 2,
        }
    }

    /// Encapsulate compressed audio frame in IEC 61937 burst
    ///
    /// Returns the IEC 61937 encapsulated data ready for S/PDIF output.
    pub fn encapsulate(&mut self, compressed_data: &[u8]) -> Result<Vec<u8>, String> {
        if self.state != SpdifState::Active {
            return Err("S/PDIF passthrough not active".to_string());
        }

        let burst_size = self.burst_size();
        let data_len = compressed_data.len();
        
        if data_len > burst_size - 8 {
            return Err(format!(
                "Compressed frame too large: {} bytes (max {})",
                data_len,
                burst_size - 8
            ));
        }

        let mut burst = vec![0u8; burst_size];
        
        // IEC 61937 preamble (Pa, Pb)
        let preamble = self.format.burst_preamble();
        burst[0] = (preamble[0] & 0xFF) as u8;
        burst[1] = (preamble[0] >> 8) as u8;
        burst[2] = (preamble[1] & 0xFF) as u8;
        burst[3] = (preamble[1] >> 8) as u8;
        
        // Pc: Data type and error flag
        let data_type = self.format.data_type();
        burst[4] = (data_type & 0xFF) as u8;
        burst[5] = (data_type >> 8) as u8;
        
        // Pd: Length in bits
        let length_bits = (data_len * 8) as u16;
        burst[6] = (length_bits & 0xFF) as u8;
        burst[7] = (length_bits >> 8) as u8;
        
        // Copy compressed data with byte swapping for IEC 61937 compatibility
        // IEC 61937 requires 16-bit words in big-endian order within the S/PDIF stream,
        // but S/PDIF is transmitted LSB first. This byte swap converts from the source
        // codec's native byte order to the required transmission order.
        for i in 0..data_len / 2 {
            let src_idx = i * 2;
            let dst_idx = 8 + i * 2;
            if src_idx + 1 < compressed_data.len() && dst_idx + 1 < burst.len() {
                // Swap bytes within each 16-bit word
                burst[dst_idx] = compressed_data[src_idx + 1];
                burst[dst_idx + 1] = compressed_data[src_idx];
            }
        }
        
        // Handle odd byte
        if data_len % 2 == 1 {
            let last_idx = 8 + (data_len / 2) * 2;
            if last_idx < burst.len() {
                burst[last_idx] = 0;
                burst[last_idx + 1] = compressed_data[data_len - 1];
            }
        }
        
        self.frame_count += 1;
        tracing::trace!("IEC 61937 burst created: {} bytes, frame #{}", burst_size, self.frame_count);
        
        Ok(burst)
    }

    /// Get format
    pub fn format(&self) -> BitstreamFormat {
        self.format
    }

    /// Set format
    pub fn set_format(&mut self, format: BitstreamFormat) -> Result<(), String> {
        if self.state != SpdifState::Disabled {
            return Err("Cannot change format while active".to_string());
        }
        self.format = format;
        Ok(())
    }

    /// Get state
    pub fn state(&self) -> SpdifState {
        self.state
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Reset
    pub fn reset(&mut self) {
        self.output_buffer.clear();
        self.burst_position = 0;
        self.frame_count = 0;
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

    #[test]
    fn test_bitstream_format_data_types() {
        assert_eq!(BitstreamFormat::Ac3.data_type(), 0x0001);
        assert_eq!(BitstreamFormat::Dts.data_type(), 0x000B);
        assert_eq!(BitstreamFormat::Eac3.data_type(), 0x0015);
    }

    #[test]
    fn test_spdif_passthrough_creation() {
        let passthrough = SpdifPassthrough::new(BitstreamFormat::Ac3);
        assert_eq!(passthrough.format(), BitstreamFormat::Ac3);
        assert_eq!(passthrough.state(), SpdifState::Disabled);
    }

    #[test]
    fn test_spdif_passthrough_lifecycle() {
        let mut passthrough = SpdifPassthrough::new(BitstreamFormat::Ac3);
        
        assert!(passthrough.enable().is_ok());
        assert_eq!(passthrough.state(), SpdifState::Enabled);
        
        assert!(passthrough.start().is_ok());
        assert_eq!(passthrough.state(), SpdifState::Active);
        
        assert!(passthrough.stop().is_ok());
        assert_eq!(passthrough.state(), SpdifState::Enabled);
        
        passthrough.disable();
        assert_eq!(passthrough.state(), SpdifState::Disabled);
    }

    #[test]
    fn test_spdif_passthrough_encapsulate() {
        let mut passthrough = SpdifPassthrough::new(BitstreamFormat::Ac3);
        passthrough.enable().unwrap();
        passthrough.start().unwrap();
        
        // Simulate an AC3 frame (small for testing)
        let frame_data = vec![0x0B, 0x77, 0x12, 0x34]; // AC3 sync word + dummy data
        
        let burst = passthrough.encapsulate(&frame_data).unwrap();
        
        // AC3 burst should be 6144 bytes
        assert_eq!(burst.len(), 6144);
        
        // Check preamble
        let pa = u16::from_le_bytes([burst[0], burst[1]]);
        assert_eq!(pa, 0xF872);
        
        let pb = u16::from_le_bytes([burst[2], burst[3]]);
        assert_eq!(pb, 0x4E1F);
    }

    #[test]
    fn test_spdif_passthrough_not_active() {
        let mut passthrough = SpdifPassthrough::new(BitstreamFormat::Ac3);
        
        let frame_data = vec![0x0B, 0x77];
        assert!(passthrough.encapsulate(&frame_data).is_err());
    }

    #[test]
    fn test_spdif_passthrough_reset() {
        let mut passthrough = SpdifPassthrough::new(BitstreamFormat::Dts);
        passthrough.enable().unwrap();
        passthrough.start().unwrap();
        
        let frame_data = vec![0x7F, 0xFE]; // DTS sync
        passthrough.encapsulate(&frame_data).unwrap();
        
        passthrough.reset();
        assert_eq!(passthrough.frame_count(), 0);
    }
}
