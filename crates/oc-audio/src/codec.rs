//! Audio codec support
//!
//! Provides support for various audio codecs used in PS3 games.

/// Audio codec type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioCodec {
    /// PCM (uncompressed)
    Pcm,
    /// LPCM (Linear PCM)
    Lpcm,
    /// AAC (Advanced Audio Coding)
    Aac,
    /// AT3 (Adaptive Transform Acoustic Coding 3)
    At3,
    /// AT3+ (Adaptive Transform Acoustic Coding 3 Plus)
    At3Plus,
    /// MP3 (MPEG-1 Audio Layer 3)
    Mp3,
    /// AC3 (Dolby Digital)
    Ac3,
    /// DTS (Digital Theater Systems)
    Dts,
}

impl AudioCodec {
    /// Check if codec is compressed
    pub fn is_compressed(&self) -> bool {
        !matches!(self, AudioCodec::Pcm | AudioCodec::Lpcm)
    }

    /// Check if codec supports multi-channel
    pub fn supports_multichannel(&self) -> bool {
        matches!(
            self,
            AudioCodec::Aac | AudioCodec::Ac3 | AudioCodec::Dts
        )
    }

    /// Get codec name
    pub fn name(&self) -> &str {
        match self {
            AudioCodec::Pcm => "PCM",
            AudioCodec::Lpcm => "LPCM",
            AudioCodec::Aac => "AAC",
            AudioCodec::At3 => "AT3",
            AudioCodec::At3Plus => "AT3+",
            AudioCodec::Mp3 => "MP3",
            AudioCodec::Ac3 => "AC3",
            AudioCodec::Dts => "DTS",
        }
    }
}

/// Audio codec configuration
#[derive(Debug, Clone, Copy)]
pub struct CodecConfig {
    /// Codec type
    pub codec: AudioCodec,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub num_channels: usize,
    /// Bit rate (for compressed codecs)
    pub bit_rate: Option<u32>,
    /// Bits per sample (for PCM)
    pub bits_per_sample: Option<u8>,
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            codec: AudioCodec::Pcm,
            sample_rate: 48000,
            num_channels: 2,
            bit_rate: None,
            bits_per_sample: Some(16),
        }
    }
}

/// Audio decoder trait
pub trait AudioDecoder: Send + Sync {
    /// Initialize decoder
    fn init(&mut self, config: CodecConfig) -> Result<(), String>;

    /// Decode audio data
    fn decode(&mut self, input: &[u8], output: &mut Vec<f32>) -> Result<usize, String>;

    /// Reset decoder state
    fn reset(&mut self);

    /// Get decoder configuration
    fn config(&self) -> &CodecConfig;
}

/// PCM decoder (pass-through)
pub struct PcmDecoder {
    config: CodecConfig,
}

impl PcmDecoder {
    pub fn new() -> Self {
        Self {
            config: CodecConfig::default(),
        }
    }
}

impl Default for PcmDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDecoder for PcmDecoder {
    fn init(&mut self, config: CodecConfig) -> Result<(), String> {
        if config.codec != AudioCodec::Pcm && config.codec != AudioCodec::Lpcm {
            return Err("PCM decoder only supports PCM/LPCM codecs".to_string());
        }
        self.config = config;
        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<f32>) -> Result<usize, String> {
        let bits_per_sample = self.config.bits_per_sample.unwrap_or(16);
        
        match bits_per_sample {
            16 => {
                // Convert 16-bit PCM to f32
                let samples = input.len() / 2;
                output.reserve(samples);
                
                for i in (0..input.len()).step_by(2) {
                    if i + 1 < input.len() {
                        let sample_i16 = i16::from_le_bytes([input[i], input[i + 1]]);
                        let sample_f32 = sample_i16 as f32 / 32768.0;
                        output.push(sample_f32);
                    }
                }
                Ok(samples)
            }
            24 => {
                // Convert 24-bit PCM to f32
                let samples = input.len() / 3;
                output.reserve(samples);
                
                for i in (0..input.len()).step_by(3) {
                    if i + 2 < input.len() {
                        // 24-bit signed integer (little-endian)
                        let sample_i32 = i32::from_le_bytes([
                            input[i],
                            input[i + 1],
                            input[i + 2],
                            if input[i + 2] & 0x80 != 0 { 0xFF } else { 0x00 },
                        ]);
                        let sample_f32 = sample_i32 as f32 / 8388608.0;
                        output.push(sample_f32);
                    }
                }
                Ok(samples)
            }
            32 => {
                // Convert 32-bit PCM to f32
                let samples = input.len() / 4;
                output.reserve(samples);
                
                for i in (0..input.len()).step_by(4) {
                    if i + 3 < input.len() {
                        let sample_i32 = i32::from_le_bytes([
                            input[i],
                            input[i + 1],
                            input[i + 2],
                            input[i + 3],
                        ]);
                        let sample_f32 = sample_i32 as f32 / 2147483648.0;
                        output.push(sample_f32);
                    }
                }
                Ok(samples)
            }
            _ => Err(format!("Unsupported bits per sample: {}", bits_per_sample)),
        }
    }

    fn reset(&mut self) {
        // PCM decoder has no state to reset
    }

    fn config(&self) -> &CodecConfig {
        &self.config
    }
}

/// AAC decoder (stub)
pub struct AacDecoder {
    config: CodecConfig,
}

impl AacDecoder {
    pub fn new() -> Self {
        Self {
            config: CodecConfig {
                codec: AudioCodec::Aac,
                ..Default::default()
            },
        }
    }
}

impl Default for AacDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDecoder for AacDecoder {
    fn init(&mut self, config: CodecConfig) -> Result<(), String> {
        if config.codec != AudioCodec::Aac {
            return Err("AAC decoder only supports AAC codec".to_string());
        }
        self.config = config;
        tracing::warn!("AAC decoder is not fully implemented");
        Ok(())
    }

    fn decode(&mut self, _input: &[u8], _output: &mut Vec<f32>) -> Result<usize, String> {
        // TODO: Implement AAC decoding
        tracing::warn!("AAC decoding not yet implemented, returning silence");
        Ok(0)
    }

    fn reset(&mut self) {
        // TODO: Reset AAC decoder state
    }

    fn config(&self) -> &CodecConfig {
        &self.config
    }
}

/// AT3 decoder (stub)
pub struct At3Decoder {
    config: CodecConfig,
}

impl At3Decoder {
    pub fn new() -> Self {
        Self {
            config: CodecConfig {
                codec: AudioCodec::At3,
                ..Default::default()
            },
        }
    }
}

impl Default for At3Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDecoder for At3Decoder {
    fn init(&mut self, config: CodecConfig) -> Result<(), String> {
        if config.codec != AudioCodec::At3 && config.codec != AudioCodec::At3Plus {
            return Err("AT3 decoder only supports AT3/AT3+ codecs".to_string());
        }
        self.config = config;
        tracing::warn!("AT3 decoder is not fully implemented");
        Ok(())
    }

    fn decode(&mut self, _input: &[u8], _output: &mut Vec<f32>) -> Result<usize, String> {
        // TODO: Implement AT3 decoding
        tracing::warn!("AT3 decoding not yet implemented, returning silence");
        Ok(0)
    }

    fn reset(&mut self) {
        // TODO: Reset AT3 decoder state
    }

    fn config(&self) -> &CodecConfig {
        &self.config
    }
}

/// Get appropriate decoder for codec
pub fn get_decoder(codec: AudioCodec) -> Box<dyn AudioDecoder> {
    match codec {
        AudioCodec::Pcm | AudioCodec::Lpcm => Box::new(PcmDecoder::new()),
        AudioCodec::Aac => Box::new(AacDecoder::new()),
        AudioCodec::At3 | AudioCodec::At3Plus => Box::new(At3Decoder::new()),
        _ => {
            tracing::warn!("No decoder available for {:?}, using PCM", codec);
            Box::new(PcmDecoder::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_properties() {
        assert!(!AudioCodec::Pcm.is_compressed());
        assert!(AudioCodec::Aac.is_compressed());
        assert!(AudioCodec::Ac3.supports_multichannel());
        assert!(!AudioCodec::At3.supports_multichannel());
    }

    #[test]
    fn test_codec_names() {
        assert_eq!(AudioCodec::Pcm.name(), "PCM");
        assert_eq!(AudioCodec::Aac.name(), "AAC");
        assert_eq!(AudioCodec::At3.name(), "AT3");
    }

    #[test]
    fn test_pcm_decoder_16bit() {
        let mut decoder = PcmDecoder::new();
        let config = CodecConfig {
            codec: AudioCodec::Pcm,
            sample_rate: 48000,
            num_channels: 2,
            bit_rate: None,
            bits_per_sample: Some(16),
        };
        
        decoder.init(config).unwrap();
        
        // Create test PCM data (16-bit)
        let input = vec![0x00, 0x00, 0xFF, 0x7F]; // 0, 32767
        let mut output = Vec::new();
        
        let samples = decoder.decode(&input, &mut output).unwrap();
        assert_eq!(samples, 2);
        assert_eq!(output.len(), 2);
        assert!((output[0] - 0.0).abs() < 0.001);
        assert!((output[1] - 0.99997).abs() < 0.001);
    }

    #[test]
    fn test_pcm_decoder_24bit() {
        let mut decoder = PcmDecoder::new();
        let config = CodecConfig {
            codec: AudioCodec::Pcm,
            sample_rate: 48000,
            num_channels: 2,
            bit_rate: None,
            bits_per_sample: Some(24),
        };
        
        decoder.init(config).unwrap();
        
        // Create test PCM data (24-bit)
        let input = vec![0x00, 0x00, 0x00, 0xFF, 0xFF, 0x7F]; // 0, 8388607
        let mut output = Vec::new();
        
        let samples = decoder.decode(&input, &mut output).unwrap();
        assert_eq!(samples, 2);
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_aac_decoder_init() {
        let mut decoder = AacDecoder::new();
        let config = CodecConfig {
            codec: AudioCodec::Aac,
            sample_rate: 48000,
            num_channels: 2,
            bit_rate: Some(256000),
            bits_per_sample: None,
        };
        
        assert!(decoder.init(config).is_ok());
    }

    #[test]
    fn test_get_decoder() {
        let pcm_decoder = get_decoder(AudioCodec::Pcm);
        assert_eq!(pcm_decoder.config().codec, AudioCodec::Pcm);
        
        let aac_decoder = get_decoder(AudioCodec::Aac);
        assert_eq!(aac_decoder.config().codec, AudioCodec::Aac);
    }
}
