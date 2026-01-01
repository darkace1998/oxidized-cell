//! Audio codec support
//!
//! Provides support for various audio codecs used in PS3 games.

use std::io::Cursor;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_AAC};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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

/// AAC decoder using symphonia
pub struct AacDecoder {
    config: CodecConfig,
    /// Symphonia decoder instance (boxed to avoid type complexity)
    #[allow(dead_code)]
    decoder: Option<Box<dyn symphonia::core::codecs::Decoder>>,
    /// Sample buffer for decoded audio
    sample_buf: Option<SampleBuffer<f32>>,
}

impl AacDecoder {
    pub fn new() -> Self {
        Self {
            config: CodecConfig {
                codec: AudioCodec::Aac,
                ..Default::default()
            },
            decoder: None,
            sample_buf: None,
        }
    }

    /// Create an ADTS header for raw AAC data
    /// This is needed because symphonia expects ADTS-framed AAC
    fn create_adts_header(&self, aac_data_len: usize) -> Vec<u8> {
        let sample_rate = self.config.sample_rate;
        let channels = self.config.num_channels;
        
        // ADTS sampling frequency index
        let freq_index = match sample_rate {
            96000 => 0,
            88200 => 1,
            64000 => 2,
            48000 => 3,
            44100 => 4,
            32000 => 5,
            24000 => 6,
            22050 => 7,
            16000 => 8,
            12000 => 9,
            11025 => 10,
            8000 => 11,
            7350 => 12,
            _ => 4, // Default to 44100
        };

        // Channel configuration
        let channel_config = match channels {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            5 => 5,
            6 => 6,
            8 => 7,
            _ => 2, // Default to stereo
        };

        // ADTS frame length (header + data)
        let frame_len = 7 + aac_data_len;

        // Build ADTS header (7 bytes)
        let mut header = Vec::with_capacity(7);
        
        // Syncword (12 bits), ID (1 bit), Layer (2 bits), Protection absent (1 bit)
        header.push(0xFF); // 11111111
        header.push(0xF1); // 1111 0 00 1 (MPEG-4, layer 0, no CRC)
        
        // Profile (2 bits), Sampling freq index (4 bits), Private (1 bit), Channel config (3 bits) [first 1 bit]
        // AAC LC = profile 1 (stored as 0 in ADTS, 2-bit value is profile - 1)
        let byte2 = ((0 & 0x03) << 6) | ((freq_index & 0x0F) << 2) | (0 << 1) | ((channel_config >> 2) & 0x01);
        header.push(byte2);
        
        // Channel config (2 bits), Original copy (1 bit), Home (1 bit), 
        // Copyright ID bit (1 bit), Copyright ID start (1 bit), Frame length (13 bits) [first 2 bits]
        let byte3 = ((channel_config & 0x03) << 6) | ((frame_len >> 11) & 0x03) as u8;
        header.push(byte3);
        
        // Frame length (11 bits), Buffer fullness (11 bits) [first 5 bits]
        let byte4 = ((frame_len >> 3) & 0xFF) as u8;
        header.push(byte4);
        
        // Frame length (3 bits), Buffer fullness (11 bits) [remaining 6 bits]
        let byte5 = (((frame_len & 0x07) << 5) | 0x1F) as u8; // 0x1F = VBR
        header.push(byte5);
        
        // Buffer fullness (5 bits), Number of raw data blocks (2 bits)
        header.push(0xFC); // 11111 00 (1 raw data block)
        
        header
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
        tracing::info!(
            "AAC decoder initialized: {}Hz, {} channels",
            config.sample_rate,
            config.num_channels
        );
        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<f32>) -> Result<usize, String> {
        if input.is_empty() {
            return Ok(0);
        }

        // Check if input already has ADTS header (0xFFF sync word)
        let aac_data = if input.len() >= 2 && input[0] == 0xFF && (input[1] & 0xF0) == 0xF0 {
            // Already has ADTS header
            input.to_vec()
        } else {
            // Add ADTS header for raw AAC data
            let mut data = self.create_adts_header(input.len());
            data.extend_from_slice(input);
            data
        };

        // Create a media source from the input data
        let cursor = Cursor::new(aac_data);
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        // Create probe hint for ADTS format
        let mut hint = Hint::new();
        hint.with_extension("aac");

        // Probe the format
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let probed = match symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts) {
            Ok(probed) => probed,
            Err(e) => {
                tracing::warn!("AAC probe failed: {}, returning silence", e);
                // Return silence for undecodable data
                let samples = 1024 * self.config.num_channels;
                output.resize(output.len() + samples, 0.0);
                return Ok(samples);
            }
        };

        let mut format = probed.format;

        // Find the AAC track
        let track = format.tracks()
            .iter()
            .find(|t| t.codec_params.codec == CODEC_TYPE_AAC)
            .or_else(|| format.tracks().first());

        let track = match track {
            Some(t) => t,
            None => {
                tracing::warn!("No AAC track found, returning silence");
                let samples = 1024 * self.config.num_channels;
                output.resize(output.len() + samples, 0.0);
                return Ok(samples);
            }
        };

        let track_id = track.id;
        let codec_params = track.codec_params.clone();

        // Create decoder
        let dec_opts = DecoderOptions::default();
        let mut decoder = match symphonia::default::get_codecs().make(&codec_params, &dec_opts) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("AAC decoder creation failed: {}, returning silence", e);
                let samples = 1024 * self.config.num_channels;
                output.resize(output.len() + samples, 0.0);
                return Ok(samples);
            }
        };

        let mut total_samples = 0;

        // Decode all packets
        loop {
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(ref e)) 
                    if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(_) => break,
            };

            if packet.track_id() != track_id {
                continue;
            }

            let decoded = match decoder.decode(&packet) {
                Ok(d) => d,
                Err(e) => {
                    tracing::debug!("AAC decode error: {}", e);
                    continue;
                }
            };

            // Get audio specification
            let spec = *decoded.spec();
            let duration = decoded.capacity() as u64;

            // Create or resize sample buffer
            let sample_buf = self.sample_buf.get_or_insert_with(|| {
                SampleBuffer::<f32>::new(duration, spec)
            });

            // Copy samples to buffer
            sample_buf.copy_interleaved_ref(decoded);

            // Append to output
            let samples = sample_buf.samples();
            output.extend_from_slice(samples);
            total_samples += samples.len();
        }

        if total_samples == 0 {
            // If no samples were decoded, return a frame of silence
            let samples = 1024 * self.config.num_channels;
            output.resize(output.len() + samples, 0.0);
            return Ok(samples);
        }

        tracing::trace!("AAC decoded {} samples", total_samples);
        Ok(total_samples)
    }

    fn reset(&mut self) {
        self.decoder = None;
        self.sample_buf = None;
        tracing::debug!("AAC decoder reset");
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
