//! Enhanced Microphone Input Support
//!
//! Audio capture backend for microphone input handling.
//! Supports multiple microphones for karaoke games (SingStar, etc.)

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Sample rate options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleRate {
    /// 8 kHz (telephony quality)
    Rate8000 = 8000,
    /// 16 kHz (wideband voice)
    Rate16000 = 16000,
    /// 22.05 kHz
    Rate22050 = 22050,
    /// 44.1 kHz (CD quality)
    Rate44100 = 44100,
    /// 48 kHz (professional)
    Rate48000 = 48000,
}

impl SampleRate {
    pub fn hz(&self) -> u32 {
        *self as u32
    }
}

/// Audio channel configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioChannels {
    Mono = 1,
    Stereo = 2,
}

/// Sample format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    /// 8-bit unsigned
    U8,
    /// 16-bit signed little-endian
    S16LE,
    /// 32-bit float
    F32,
}

impl SampleFormat {
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            SampleFormat::U8 => 1,
            SampleFormat::S16LE => 2,
            SampleFormat::F32 => 4,
        }
    }
}

/// Microphone configuration
#[derive(Debug, Clone)]
pub struct MicrophoneConfig {
    /// Sample rate
    pub sample_rate: SampleRate,
    /// Number of channels
    pub channels: AudioChannels,
    /// Sample format
    pub format: SampleFormat,
    /// Buffer size in samples
    pub buffer_size: usize,
    /// Input gain (0.0 - 2.0)
    pub gain: f32,
    /// Noise gate threshold (0.0 - 1.0)
    pub noise_gate: f32,
    /// Enable echo cancellation
    pub echo_cancel: bool,
    /// Enable noise reduction
    pub noise_reduction: bool,
}

impl Default for MicrophoneConfig {
    fn default() -> Self {
        Self {
            sample_rate: SampleRate::Rate48000,
            channels: AudioChannels::Mono,
            format: SampleFormat::S16LE,
            buffer_size: 1024,
            gain: 1.0,
            noise_gate: 0.01,
            echo_cancel: false,
            noise_reduction: false,
        }
    }
}

/// Audio buffer for storing samples
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Raw sample data
    pub data: Vec<u8>,
    /// Timestamp when captured
    pub timestamp: Duration,
    /// Sample count
    pub sample_count: usize,
}

impl AudioBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            timestamp: Duration::ZERO,
            sample_count: 0,
        }
    }

    /// Get samples as i16 (assuming S16LE format)
    pub fn as_i16_samples(&self) -> Vec<i16> {
        self.data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    }

    /// Calculate RMS level (0.0 - 1.0)
    pub fn rms_level(&self) -> f32 {
        let samples = self.as_i16_samples();
        if samples.is_empty() {
            return 0.0;
        }

        let sum_squares: f64 = samples
            .iter()
            .map(|&s| (s as f64).powi(2))
            .sum();
        
        let rms = (sum_squares / samples.len() as f64).sqrt();
        (rms / 32768.0) as f32
    }

    /// Calculate peak level (0.0 - 1.0)
    pub fn peak_level(&self) -> f32 {
        let samples = self.as_i16_samples();
        let max = samples
            .iter()
            .map(|&s| s.unsigned_abs())
            .max()
            .unwrap_or(0);
        
        max as f32 / 32768.0
    }
}

/// Microphone state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneState {
    /// Device not opened
    Closed,
    /// Device opened but not recording
    Open,
    /// Actively recording
    Recording,
    /// Error state
    Error,
}

/// Audio level analysis
#[derive(Debug, Clone, Default)]
pub struct AudioLevels {
    /// Current RMS level (0.0 - 1.0)
    pub rms: f32,
    /// Peak level (0.0 - 1.0)
    pub peak: f32,
    /// Peak hold level
    pub peak_hold: f32,
    /// Voice activity detected
    pub voice_detected: bool,
}

/// Callback for audio data
pub type AudioCallback = Box<dyn Fn(&AudioBuffer) + Send + Sync>;

/// Microphone device
pub struct Microphone {
    /// Device index
    pub index: u8,
    /// Device name
    pub name: String,
    /// Configuration
    pub config: MicrophoneConfig,
    /// Current state
    state: MicrophoneState,
    /// Audio buffer ring
    buffer_ring: VecDeque<AudioBuffer>,
    /// Current audio levels
    levels: AudioLevels,
    /// Recording start time
    start_time: Option<Instant>,
    /// Total samples recorded
    total_samples: u64,
    /// Audio callback
    callback: Option<Arc<Mutex<AudioCallback>>>,
}

impl std::fmt::Debug for Microphone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Microphone")
            .field("index", &self.index)
            .field("name", &self.name)
            .field("config", &self.config)
            .field("state", &self.state)
            .field("total_samples", &self.total_samples)
            .finish()
    }
}

impl Microphone {
    pub fn new(index: u8, name: &str) -> Self {
        Self {
            index,
            name: name.to_string(),
            config: MicrophoneConfig::default(),
            state: MicrophoneState::Closed,
            buffer_ring: VecDeque::with_capacity(16),
            levels: AudioLevels::default(),
            start_time: None,
            total_samples: 0,
            callback: None,
        }
    }

    /// Open the microphone
    pub fn open(&mut self) -> Result<(), MicrophoneError> {
        if self.state != MicrophoneState::Closed {
            return Err(MicrophoneError::AlreadyOpen);
        }

        self.state = MicrophoneState::Open;
        tracing::info!("Microphone {} ({}) opened", self.index, self.name);
        Ok(())
    }

    /// Configure the microphone
    pub fn configure(&mut self, config: MicrophoneConfig) -> Result<(), MicrophoneError> {
        if self.state == MicrophoneState::Recording {
            return Err(MicrophoneError::CantConfigureWhileRecording);
        }

        self.config = config;
        Ok(())
    }

    /// Start recording
    pub fn start_recording(&mut self) -> Result<(), MicrophoneError> {
        if self.state == MicrophoneState::Closed {
            return Err(MicrophoneError::NotOpen);
        }
        if self.state == MicrophoneState::Recording {
            return Err(MicrophoneError::AlreadyRecording);
        }

        self.start_time = Some(Instant::now());
        self.total_samples = 0;
        self.buffer_ring.clear();
        self.state = MicrophoneState::Recording;
        
        tracing::info!("Microphone {} started recording", self.index);
        Ok(())
    }

    /// Stop recording
    pub fn stop_recording(&mut self) -> Result<(), MicrophoneError> {
        if self.state != MicrophoneState::Recording {
            return Err(MicrophoneError::NotRecording);
        }

        self.state = MicrophoneState::Open;
        tracing::info!(
            "Microphone {} stopped recording ({} samples)",
            self.index,
            self.total_samples
        );
        Ok(())
    }

    /// Close the microphone
    pub fn close(&mut self) {
        if self.state == MicrophoneState::Recording {
            let _ = self.stop_recording();
        }
        self.state = MicrophoneState::Closed;
        tracing::info!("Microphone {} closed", self.index);
    }

    /// Get current state
    pub fn state(&self) -> MicrophoneState {
        self.state
    }

    /// Get current audio levels
    pub fn levels(&self) -> &AudioLevels {
        &self.levels
    }

    /// Set audio callback
    pub fn set_callback(&mut self, callback: AudioCallback) {
        self.callback = Some(Arc::new(Mutex::new(callback)));
    }

    /// Push audio data (from backend)
    pub fn push_audio(&mut self, data: &[u8]) {
        if self.state != MicrophoneState::Recording {
            return;
        }

        let elapsed = self.start_time.map(|t| t.elapsed()).unwrap_or_default();
        let bytes_per_sample = self.config.format.bytes_per_sample() 
            * self.config.channels as usize;
        let sample_count = data.len() / bytes_per_sample;

        // Apply gain
        let processed_data: Vec<u8> = if self.config.gain != 1.0 {
            // Process as S16LE
            data.chunks_exact(2)
                .flat_map(|chunk| {
                    let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                    let gained = (sample as f32 * self.config.gain).clamp(-32768.0, 32767.0) as i16;
                    gained.to_le_bytes()
                })
                .collect()
        } else {
            data.to_vec()
        };

        let buffer = AudioBuffer {
            data: processed_data,
            timestamp: elapsed,
            sample_count,
        };

        // Update levels
        self.levels.rms = buffer.rms_level();
        self.levels.peak = buffer.peak_level();
        if self.levels.peak > self.levels.peak_hold {
            self.levels.peak_hold = self.levels.peak;
        }
        self.levels.voice_detected = self.levels.rms > self.config.noise_gate;

        // Invoke callback
        if let Some(ref cb) = self.callback {
            if let Ok(callback) = cb.lock() {
                callback(&buffer);
            }
        }

        // Store in ring buffer
        if self.buffer_ring.len() >= 16 {
            self.buffer_ring.pop_front();
        }
        self.buffer_ring.push_back(buffer);
        
        self.total_samples += sample_count as u64;
    }

    /// Read available audio data
    pub fn read_audio(&mut self) -> Option<AudioBuffer> {
        self.buffer_ring.pop_front()
    }

    /// Generate test tone (for testing without real mic)
    pub fn generate_test_tone(&mut self, frequency: f32, duration_ms: u32) {
        if self.state != MicrophoneState::Recording {
            return;
        }

        let sample_rate = self.config.sample_rate.hz();
        let samples = (sample_rate * duration_ms / 1000) as usize;
        let mut data = Vec::with_capacity(samples * 2);

        for i in 0..samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (t * frequency * 2.0 * std::f32::consts::PI).sin();
            let value = (sample * 16000.0) as i16;
            data.extend_from_slice(&value.to_le_bytes());
        }

        self.push_audio(&data);
    }
}

impl Drop for Microphone {
    fn drop(&mut self) {
        self.close();
    }
}

/// Microphone errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneError {
    /// Device not open
    NotOpen,
    /// Already open
    AlreadyOpen,
    /// Already recording
    AlreadyRecording,
    /// Not recording
    NotRecording,
    /// Cannot configure while recording
    CantConfigureWhileRecording,
    /// Device not found
    NotFound,
    /// Device busy
    Busy,
    /// Permission denied
    PermissionDenied,
}

impl std::fmt::Display for MicrophoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MicrophoneError::NotOpen => write!(f, "Microphone not open"),
            MicrophoneError::AlreadyOpen => write!(f, "Microphone already open"),
            MicrophoneError::AlreadyRecording => write!(f, "Already recording"),
            MicrophoneError::NotRecording => write!(f, "Not recording"),
            MicrophoneError::CantConfigureWhileRecording => {
                write!(f, "Cannot configure while recording")
            }
            MicrophoneError::NotFound => write!(f, "Microphone not found"),
            MicrophoneError::Busy => write!(f, "Microphone busy"),
            MicrophoneError::PermissionDenied => write!(f, "Permission denied"),
        }
    }
}

impl std::error::Error for MicrophoneError {}

/// Microphone manager for multi-mic support (karaoke games)
pub struct MicrophoneManager {
    /// Available microphones
    microphones: [Option<Microphone>; 8],
}

impl MicrophoneManager {
    pub fn new() -> Self {
        Self {
            microphones: Default::default(),
        }
    }

    /// Register a microphone
    pub fn register(&mut self, index: u8, name: &str) -> Result<(), MicrophoneError> {
        if index >= 8 {
            return Err(MicrophoneError::NotFound);
        }

        let mut mic = Microphone::new(index, name);
        mic.open()?;
        self.microphones[index as usize] = Some(mic);
        Ok(())
    }

    /// Unregister a microphone
    pub fn unregister(&mut self, index: u8) {
        if index < 8 {
            if let Some(ref mut mic) = self.microphones[index as usize] {
                mic.close();
            }
            self.microphones[index as usize] = None;
        }
    }

    /// Get microphone
    pub fn get(&self, index: u8) -> Option<&Microphone> {
        self.microphones.get(index as usize)?.as_ref()
    }

    /// Get mutable microphone
    pub fn get_mut(&mut self, index: u8) -> Option<&mut Microphone> {
        self.microphones.get_mut(index as usize)?.as_mut()
    }

    /// List registered microphones
    pub fn list_registered(&self) -> Vec<(u8, &str)> {
        self.microphones
            .iter()
            .enumerate()
            .filter_map(|(i, m)| m.as_ref().map(|mic| (i as u8, mic.name.as_str())))
            .collect()
    }

    /// Start recording on all microphones
    pub fn start_all(&mut self) -> Result<(), MicrophoneError> {
        for mic in self.microphones.iter_mut().flatten() {
            mic.start_recording()?;
        }
        Ok(())
    }

    /// Stop recording on all microphones  
    pub fn stop_all(&mut self) -> Result<(), MicrophoneError> {
        for mic in self.microphones.iter_mut().flatten() {
            mic.stop_recording()?;
        }
        Ok(())
    }
}

impl Default for MicrophoneManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_rms() {
        let mut buffer = AudioBuffer::new(100);
        // Add some 16-bit samples
        for _ in 0..10 {
            buffer.data.extend_from_slice(&1000i16.to_le_bytes());
        }
        buffer.sample_count = 10;
        
        let rms = buffer.rms_level();
        assert!(rms > 0.0);
        assert!(rms < 1.0);
    }

    #[test]
    fn test_microphone_lifecycle() {
        let mut mic = Microphone::new(0, "Test Mic");
        
        assert_eq!(mic.state(), MicrophoneState::Closed);
        
        mic.open().unwrap();
        assert_eq!(mic.state(), MicrophoneState::Open);
        
        mic.start_recording().unwrap();
        assert_eq!(mic.state(), MicrophoneState::Recording);
        
        // Generate test tone
        mic.generate_test_tone(440.0, 100);
        
        mic.stop_recording().unwrap();
        assert_eq!(mic.state(), MicrophoneState::Open);
        
        mic.close();
        assert_eq!(mic.state(), MicrophoneState::Closed);
    }

    #[test]
    fn test_microphone_manager() {
        let mut manager = MicrophoneManager::new();
        
        manager.register(0, "Mic 1").unwrap();
        manager.register(1, "Mic 2").unwrap();
        
        let registered = manager.list_registered();
        assert_eq!(registered.len(), 2);
    }
}
