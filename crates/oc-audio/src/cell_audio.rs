//! cellAudio HLE (High-Level Emulation)
//!
//! Emulates the PS3's cellAudio library for audio output.

use std::sync::Arc;
use parking_lot::RwLock;

/// Audio port configuration
#[derive(Debug, Clone, Copy)]
pub struct AudioPortConfig {
    /// Number of channels (1, 2, 6, or 8)
    pub num_channels: u32,
    /// Number of blocks
    pub num_blocks: u32,
    /// Port attributes
    pub attributes: u64,
}

impl Default for AudioPortConfig {
    fn default() -> Self {
        Self {
            num_channels: 2,  // Stereo by default
            num_blocks: 8,
            attributes: 0,
        }
    }
}

/// Audio port state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPortState {
    Closed,
    Opened,
    Started,
}

/// Audio port
#[derive(Clone)]
pub struct AudioPort {
    /// Port number
    pub port_num: u32,
    /// Configuration
    pub config: AudioPortConfig,
    /// State
    pub state: AudioPortState,
    /// Read index
    pub read_index: u64,
    /// Write index (tag)
    pub tag: u64,
}

impl AudioPort {
    pub fn new(port_num: u32, config: AudioPortConfig) -> Self {
        Self {
            port_num,
            config,
            state: AudioPortState::Opened,
            read_index: 0,
            tag: 0,
        }
    }

    pub fn start(&mut self) {
        self.state = AudioPortState::Started;
    }

    pub fn stop(&mut self) {
        self.state = AudioPortState::Opened;
    }
}

/// cellAudio system configuration
pub struct CellAudioConfig {
    /// Sample rate (typically 48000 Hz)
    pub sample_rate: u32,
    /// Buffer size
    pub buffer_size: usize,
}

impl Default for CellAudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 256,
        }
    }
}

/// cellAudio HLE implementation
pub struct CellAudio {
    config: CellAudioConfig,
    ports: Arc<RwLock<Vec<Option<AudioPort>>>>,
}

impl CellAudio {
    /// Create a new cellAudio instance
    pub fn new() -> Self {
        Self::with_config(CellAudioConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: CellAudioConfig) -> Self {
        Self {
            config,
            ports: Arc::new(RwLock::new(vec![None; 8])), // Max 8 ports
        }
    }

    /// Initialize audio system (cellAudioInit)
    pub fn init(&self) -> Result<(), String> {
        tracing::info!("cellAudio initialized with sample rate: {} Hz", self.config.sample_rate);
        Ok(())
    }

    /// Quit audio system (cellAudioQuit)
    pub fn quit(&self) -> Result<(), String> {
        let mut ports = self.ports.write();
        for port in ports.iter_mut() {
            *port = None;
        }
        tracing::info!("cellAudio quit");
        Ok(())
    }

    /// Open an audio port (cellAudioPortOpen)
    pub fn port_open(&self, config: AudioPortConfig) -> Result<u32, String> {
        let mut ports = self.ports.write();
        
        for (idx, port) in ports.iter_mut().enumerate() {
            if port.is_none() {
                let port_num = idx as u32;
                *port = Some(AudioPort::new(port_num, config));
                tracing::debug!("Audio port {} opened", port_num);
                return Ok(port_num);
            }
        }
        
        Err("No available audio ports".to_string())
    }

    /// Close an audio port (cellAudioPortClose)
    pub fn port_close(&self, port_num: u32) -> Result<(), String> {
        let mut ports = self.ports.write();
        
        if let Some(port) = ports.get_mut(port_num as usize) {
            *port = None;
            tracing::debug!("Audio port {} closed", port_num);
            Ok(())
        } else {
            Err("Invalid port number".to_string())
        }
    }

    /// Start an audio port (cellAudioPortStart)
    pub fn port_start(&self, port_num: u32) -> Result<(), String> {
        let mut ports = self.ports.write();
        
        if let Some(Some(port)) = ports.get_mut(port_num as usize) {
            port.start();
            tracing::debug!("Audio port {} started", port_num);
            Ok(())
        } else {
            Err("Invalid port number".to_string())
        }
    }

    /// Stop an audio port (cellAudioPortStop)
    pub fn port_stop(&self, port_num: u32) -> Result<(), String> {
        let mut ports = self.ports.write();
        
        if let Some(Some(port)) = ports.get_mut(port_num as usize) {
            port.stop();
            tracing::debug!("Audio port {} stopped", port_num);
            Ok(())
        } else {
            Err("Invalid port number".to_string())
        }
    }

    /// Get port config
    pub fn get_port_config(&self, port_num: u32) -> Option<AudioPortConfig> {
        let ports = self.ports.read();
        ports.get(port_num as usize)?.as_ref().map(|p| p.config)
    }
}

impl Default for CellAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_audio_init() {
        let audio = CellAudio::new();
        assert!(audio.init().is_ok());
        assert!(audio.quit().is_ok());
    }

    #[test]
    fn test_audio_port_lifecycle() {
        let audio = CellAudio::new();
        audio.init().unwrap();
        
        let config = AudioPortConfig::default();
        let port_num = audio.port_open(config).unwrap();
        assert_eq!(port_num, 0);
        
        assert!(audio.port_start(port_num).is_ok());
        assert!(audio.port_stop(port_num).is_ok());
        assert!(audio.port_close(port_num).is_ok());
    }

    #[test]
    fn test_multiple_ports() {
        let audio = CellAudio::new();
        let config = AudioPortConfig::default();
        
        let port1 = audio.port_open(config).unwrap();
        let port2 = audio.port_open(config).unwrap();
        
        assert_ne!(port1, port2);
        assert!(audio.port_close(port1).is_ok());
        assert!(audio.port_close(port2).is_ok());
    }
}
