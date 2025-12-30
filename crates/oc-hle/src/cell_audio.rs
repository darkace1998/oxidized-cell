//! cellAudio HLE - Audio Output System
//!
//! This module provides HLE implementations for PS3 audio output.
//! It provides full audio mixing support compatible with the oc-audio subsystem.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, trace};
use crate::memory::{read_be32, write_be32, write_be64};

// ============================================================================
// Local Audio Types (compatible with oc-audio::mixer when integrated)
// ============================================================================

/// Audio sample format
pub type Sample = f32;

/// Audio source identifier
pub type SourceId = u32;

/// Audio channel configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround51,
    Surround71,
}

impl ChannelLayout {
    pub fn num_channels(&self) -> usize {
        match self {
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
            ChannelLayout::Surround51 => 6,
            ChannelLayout::Surround71 => 8,
        }
    }
}

/// Audio source for mixing
pub struct AudioSource {
    /// Source ID
    pub id: SourceId,
    /// Channel layout
    pub layout: ChannelLayout,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
    /// Audio buffer
    pub buffer: Vec<Sample>,
}

impl AudioSource {
    pub fn new(id: SourceId, layout: ChannelLayout) -> Self {
        Self {
            id,
            layout,
            volume: 1.0,
            buffer: Vec::new(),
        }
    }

    pub fn write_samples(&mut self, samples: &[Sample]) {
        self.buffer.extend_from_slice(samples);
    }

    pub fn read_samples(&mut self, count: usize) -> Vec<Sample> {
        let available = self.buffer.len().min(count);
        self.buffer.drain(..available).collect()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Audio mixer for multiple sources
pub struct HleAudioMixer {
    /// Audio sources
    sources: HashMap<SourceId, AudioSource>,
    /// Master volume
    master_volume: f32,
    /// Output channel layout
    output_layout: ChannelLayout,
    /// Next source ID
    next_id: SourceId,
}

impl HleAudioMixer {
    /// Create a new audio mixer
    pub fn new(output_layout: ChannelLayout) -> Self {
        Self {
            sources: HashMap::new(),
            master_volume: 1.0,
            output_layout,
            next_id: 0,
        }
    }

    /// Add a new audio source
    pub fn add_source(&mut self, layout: ChannelLayout) -> SourceId {
        let id = self.next_id;
        self.next_id += 1;
        
        let source = AudioSource::new(id, layout);
        self.sources.insert(id, source);
        
        debug!("Audio source {} added with {:?} layout", id, layout);
        id
    }

    /// Remove an audio source
    pub fn remove_source(&mut self, id: SourceId) -> bool {
        if self.sources.remove(&id).is_some() {
            debug!("Audio source {} removed", id);
            true
        } else {
            false
        }
    }

    /// Write samples to a source
    pub fn write_to_source(&mut self, id: SourceId, samples: &[Sample]) -> Result<(), String> {
        if let Some(source) = self.sources.get_mut(&id) {
            source.write_samples(samples);
            Ok(())
        } else {
            Err(format!("Source {} not found", id))
        }
    }

    /// Set source volume
    pub fn set_source_volume(&mut self, id: SourceId, volume: f32) -> Result<(), String> {
        if let Some(source) = self.sources.get_mut(&id) {
            source.volume = volume.clamp(0.0, 1.0);
            Ok(())
        } else {
            Err(format!("Source {} not found", id))
        }
    }

    /// Set master volume
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    /// Get master volume
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Mix audio sources into output buffer
    pub fn mix(&mut self, output: &mut [Sample], frames: usize) {
        let channels = self.output_layout.num_channels();
        let samples_needed = frames * channels;
        
        // Clear output buffer
        for sample in output.iter_mut().take(samples_needed) {
            *sample = 0.0;
        }

        // Mix all sources
        for source in self.sources.values_mut() {
            let source_samples = source.read_samples(samples_needed);
            
            // Apply volume and mix into output
            for (i, &sample) in source_samples.iter().enumerate() {
                if i < samples_needed {
                    output[i] += sample * source.volume * self.master_volume;
                }
            }
        }

        // Clamp output to prevent clipping
        for sample in output.iter_mut().take(samples_needed) {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }

    /// Clear all sources
    pub fn clear_all(&mut self) {
        for source in self.sources.values_mut() {
            source.clear();
        }
    }
}

impl Default for HleAudioMixer {
    fn default() -> Self {
        Self::new(ChannelLayout::Stereo)
    }
}

/// OC-Audio mixer backend reference
pub type AudioBackend = Option<Arc<RwLock<HleAudioMixer>>>;

/// Maximum number of audio ports
pub const CELL_AUDIO_PORT_MAX: usize = 8;

/// Audio block count (8 blocks per frame)
pub const CELL_AUDIO_BLOCK_8: u32 = 8;
pub const CELL_AUDIO_BLOCK_16: u32 = 16;
pub const CELL_AUDIO_BLOCK_32: u32 = 32;

/// Audio block samples (256 samples per block)
pub const CELL_AUDIO_BLOCK_SAMPLES: usize = 256;

/// Audio port types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellAudioPortType {
    /// 2 channel audio
    Audio2Ch = 2,
    /// 8 channel audio (7.1)
    Audio8Ch = 8,
}

/// Audio port state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPortState {
    Closed,
    Open,
    Started,
}

/// Audio port
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct AudioPort {
    /// Port state
    state: AudioPortState,
    /// Number of channels
    num_channels: u32,
    /// Number of blocks
    num_blocks: u32,
    /// Port tag
    tag: u64,
    /// Buffer address
    buffer_addr: u32,
    /// Volume level (0.0 to 1.0)
    volume: f32,
    /// OC-Audio mixer source ID
    mixer_source_id: Option<SourceId>,
}

impl Default for AudioPort {
    fn default() -> Self {
        Self {
            state: AudioPortState::Closed,
            num_channels: 0,
            num_blocks: 0,
            tag: 0,
            buffer_addr: 0,
            volume: 1.0,
            mixer_source_id: None,
        }
    }
}

/// Audio manager
pub struct AudioManager {
    /// Audio ports
    ports: [AudioPort; CELL_AUDIO_PORT_MAX],
    /// Initialization flag
    initialized: bool,
    /// OC-Audio mixer backend
    audio_backend: AudioBackend,
    /// Master volume (0.0 to 1.0)
    master_volume: f32,
    /// Audio block index (for timing)
    block_index: u64,
}

/// Public port info for querying
#[derive(Debug, Clone, Copy)]
pub struct PortInfo {
    pub started: bool,
    pub channels: u32,
    pub block_count: u32,
}

impl AudioManager {
    /// Create a new audio manager
    pub fn new() -> Self {
        Self {
            ports: [AudioPort::default(); CELL_AUDIO_PORT_MAX],
            initialized: false,
            audio_backend: None,
            master_volume: 1.0,
            block_index: 0,
        }
    }

    /// Initialize audio system
    pub fn init(&mut self) -> i32 {
        if self.initialized {
            return 0x80310701u32 as i32; // CELL_AUDIO_ERROR_ALREADY_INIT
        }

        debug!("cellAudioInit: initializing audio system");
        self.initialized = true;
        self.block_index = 0;

        // Initialize backend if connected
        if let Some(backend) = &self.audio_backend {
            if let Ok(mut mixer) = backend.write() {
                mixer.set_master_volume(self.master_volume);
            }
        }

        0 // CELL_OK
    }

    /// Quit audio system
    pub fn quit(&mut self) -> i32 {
        if !self.initialized {
            return 0x80310702u32 as i32; // CELL_AUDIO_ERROR_AUDIOSYSTEM
        }

        debug!("cellAudioQuit: shutting down audio system");
        
        // Close all open ports and remove mixer sources
        for port in &mut self.ports {
            if let Some(source_id) = port.mixer_source_id {
                if let Some(backend) = &self.audio_backend {
                    if let Ok(mut mixer) = backend.write() {
                        mixer.remove_source(source_id);
                    }
                }
            }
            port.state = AudioPortState::Closed;
            port.mixer_source_id = None;
        }
        
        self.initialized = false;

        0 // CELL_OK
    }

    /// Open an audio port
    pub fn port_open(
        &mut self,
        num_channels: u32,
        num_blocks: u32,
        _attr: u32,
        level: f32,
    ) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80310702u32 as i32); // CELL_AUDIO_ERROR_AUDIOSYSTEM
        }

        // Find a free port
        let port_num = self
            .ports
            .iter()
            .position(|p| p.state == AudioPortState::Closed)
            .ok_or(0x80310705u32 as i32)?; // CELL_AUDIO_ERROR_PORT_FULL

        debug!(
            "cellAudioPortOpen: opening port {} with {} channels, {} blocks",
            port_num, num_channels, num_blocks
        );

        // Determine channel layout based on channel count
        let layout = match num_channels {
            1 => ChannelLayout::Mono,
            2 => ChannelLayout::Stereo,
            6 => ChannelLayout::Surround51,
            8 => ChannelLayout::Surround71,
            _ => ChannelLayout::Stereo, // Default to stereo
        };

        // Create mixer source if backend is available
        let source_id = if let Some(backend) = &self.audio_backend {
            if let Ok(mut mixer) = backend.write() {
                let id = mixer.add_source(layout);
                let _ = mixer.set_source_volume(id, level);
                Some(id)
            } else {
                None
            }
        } else {
            None
        };

        // Configure the port
        self.ports[port_num].state = AudioPortState::Open;
        self.ports[port_num].num_channels = num_channels;
        self.ports[port_num].num_blocks = num_blocks;
        self.ports[port_num].volume = level;
        self.ports[port_num].mixer_source_id = source_id;

        Ok(port_num as u32)
    }

    /// Close an audio port
    pub fn port_close(&mut self, port_num: u32) -> i32 {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return 0x80310704u32 as i32; // CELL_AUDIO_ERROR_PARAM
        }

        let port = &mut self.ports[port_num as usize];
        if port.state == AudioPortState::Closed {
            return 0x80310703u32 as i32; // CELL_AUDIO_ERROR_PORT_NOT_OPEN
        }

        debug!("cellAudioPortClose: closing port {}", port_num);

        // Remove mixer source if it exists
        if let Some(source_id) = port.mixer_source_id {
            if let Some(backend) = &self.audio_backend {
                if let Ok(mut mixer) = backend.write() {
                    mixer.remove_source(source_id);
                }
            }
        }

        port.state = AudioPortState::Closed;
        port.mixer_source_id = None;

        0 // CELL_OK
    }

    /// Start audio port
    pub fn port_start(&mut self, port_num: u32) -> i32 {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return 0x80310704u32 as i32; // CELL_AUDIO_ERROR_PARAM
        }

        let port = &mut self.ports[port_num as usize];
        if port.state == AudioPortState::Closed {
            return 0x80310703u32 as i32; // CELL_AUDIO_ERROR_PORT_NOT_OPEN
        }

        trace!("cellAudioPortStart: starting port {}", port_num);

        port.state = AudioPortState::Started;

        0 // CELL_OK
    }

    /// Stop audio port
    pub fn port_stop(&mut self, port_num: u32) -> i32 {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return 0x80310704u32 as i32; // CELL_AUDIO_ERROR_PARAM
        }

        let port = &mut self.ports[port_num as usize];
        if port.state == AudioPortState::Closed {
            return 0x80310703u32 as i32; // CELL_AUDIO_ERROR_PORT_NOT_OPEN
        }

        trace!("cellAudioPortStop: stopping port {}", port_num);

        port.state = AudioPortState::Open;

        0 // CELL_OK
    }

    // ========================================================================
    // OC-Audio Backend Integration
    // ========================================================================

    /// Set the oc-audio mixer backend
    /// 
    /// Connects the AudioManager to the HLE audio mixer,
    /// enabling actual audio playback through the system audio device.
    /// 
    /// # Arguments
    /// * `backend` - Shared reference to HleAudioMixer
    pub fn set_audio_backend(&mut self, backend: Arc<RwLock<HleAudioMixer>>) {
        debug!("AudioManager::set_audio_backend - connecting to oc-audio mixer");
        self.audio_backend = Some(backend);
    }

    /// Check if the audio backend is connected
    pub fn has_audio_backend(&self) -> bool {
        self.audio_backend.is_some()
    }

    /// Submit audio buffer to backend
    /// 
    /// # Arguments
    /// * `port_num` - Audio port number
    /// * `buffer` - Audio samples to submit
    pub fn submit_audio(&mut self, port_num: u32, buffer: &[f32]) -> i32 {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return 0x80310704u32 as i32; // CELL_AUDIO_ERROR_PARAM
        }

        let port = &self.ports[port_num as usize];
        if port.state != AudioPortState::Started {
            return 0x80310703u32 as i32; // CELL_AUDIO_ERROR_PORT_NOT_OPEN
        }

        trace!("AudioManager::submit_audio: port={}, samples={}", port_num, buffer.len());

        // Submit to oc-audio mixer backend
        if let Some(source_id) = port.mixer_source_id {
            if let Some(backend) = &self.audio_backend {
                if let Ok(mut mixer) = backend.write() {
                    // Apply port volume before submitting
                    let scaled: Vec<f32> = buffer.iter().map(|s| s * port.volume).collect();
                    let _ = mixer.write_to_source(source_id, &scaled);
                }
            }
        }

        0 // CELL_OK
    }

    /// Set port volume
    /// 
    /// # Arguments
    /// * `port_num` - Audio port number
    /// * `volume` - Volume level (0.0 to 1.0)
    pub fn set_port_volume(&mut self, port_num: u32, volume: f32) -> i32 {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return 0x80310704u32 as i32; // CELL_AUDIO_ERROR_PARAM
        }

        let port = &mut self.ports[port_num as usize];
        if port.state == AudioPortState::Closed {
            return 0x80310703u32 as i32; // CELL_AUDIO_ERROR_PORT_NOT_OPEN
        }

        port.volume = volume.clamp(0.0, 1.0);

        // Update mixer source volume
        if let Some(source_id) = port.mixer_source_id {
            if let Some(backend) = &self.audio_backend {
                if let Ok(mut mixer) = backend.write() {
                    let _ = mixer.set_source_volume(source_id, port.volume);
                }
            }
        }

        debug!("Set port {} volume to {}", port_num, port.volume);

        0 // CELL_OK
    }

    /// Get port volume
    /// 
    /// # Arguments
    /// * `port_num` - Audio port number
    pub fn get_port_volume(&self, port_num: u32) -> Result<f32, i32> {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return Err(0x80310704u32 as i32); // CELL_AUDIO_ERROR_PARAM
        }

        let port = &self.ports[port_num as usize];
        if port.state == AudioPortState::Closed {
            return Err(0x80310703u32 as i32); // CELL_AUDIO_ERROR_PORT_NOT_OPEN
        }

        Ok(port.volume)
    }

    /// Set master volume
    /// 
    /// # Arguments
    /// * `volume` - Master volume level (0.0 to 1.0)
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);

        // Update mixer master volume
        if let Some(backend) = &self.audio_backend {
            if let Ok(mut mixer) = backend.write() {
                mixer.set_master_volume(self.master_volume);
            }
        }

        debug!("Set master volume to {}", self.master_volume);
    }

    /// Get master volume
    pub fn get_master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Get port information
    /// 
    /// # Arguments
    /// * `port_num` - Port number
    /// 
    /// # Returns
    /// * Some(PortInfo) if port is open, None otherwise
    pub fn get_port(&self, port_num: u32) -> Option<PortInfo> {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return None;
        }

        let port = &self.ports[port_num as usize];
        if port.state == AudioPortState::Closed {
            return None;
        }

        Some(PortInfo {
            started: port.state == AudioPortState::Started,
            channels: port.num_channels,
            block_count: port.num_blocks,
        })
    }

    /// Check if audio system is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Mix audio from multiple ports
    /// 
    /// Mixes audio from all active ports into a single output buffer.
    /// This is called by the audio thread to generate the final output.
    /// 
    /// # Arguments
    /// * `output` - Output buffer to fill with mixed audio
    /// * `frames` - Number of audio frames to mix
    pub fn mix_audio(&mut self, output: &mut [f32], frames: usize) -> i32 {
        if !self.initialized {
            return 0x80310702u32 as i32; // CELL_AUDIO_ERROR_AUDIOSYSTEM
        }

        trace!("AudioManager::mix_audio: frames={}", frames);

        // Use mixer backend if available
        if let Some(backend) = &self.audio_backend {
            if let Ok(mut mixer) = backend.write() {
                mixer.mix(output, frames);
            }
        }

        // Increment block index for timing
        self.block_index += 1;

        0 // CELL_OK
    }

    /// Get the current audio block index
    /// 
    /// Used for synchronization with the audio hardware.
    pub fn get_block_index(&self) -> u64 {
        self.block_index
    }

    /// Get number of active ports
    pub fn get_active_port_count(&self) -> usize {
        self.ports.iter().filter(|p| p.state == AudioPortState::Started).count()
    }

    /// Check if backend is connected
    pub fn is_backend_connected(&self) -> bool {
        self.audio_backend.is_some()
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellAudioInit - Initialize audio system
///
/// # Returns
/// * 0 on success
pub fn cell_audio_init() -> i32 {
    debug!("cellAudioInit()");

    crate::context::get_hle_context_mut().audio.init()
}

/// cellAudioQuit - Quit audio system
///
/// # Returns
/// * 0 on success
pub fn cell_audio_quit() -> i32 {
    debug!("cellAudioQuit()");

    crate::context::get_hle_context_mut().audio.quit()
}

/// cellAudioPortOpen - Open audio port
///
/// # Arguments
/// * `param_addr` - Port parameters address
/// * `port_num_addr` - Address to write port number to
///
/// # Returns
/// * 0 on success
pub fn cell_audio_port_open(param_addr: u32, port_num_addr: u32) -> i32 {
    debug!("cellAudioPortOpen(param_addr=0x{:08X}, port_num_addr=0x{:08X})", param_addr, port_num_addr);

    // Read parameters from memory (CellAudioPortParam structure)
    // nChannel (8 bytes) + nBlock (8 bytes) + attr (8 bytes) + level (4 bytes float)
    let n_channel = match read_be32(param_addr) {
        Ok(v) => v,
        Err(e) => {
            trace!("cellAudioPortOpen: failed to read nChannel, using default stereo (error: 0x{:08X})", e as u32);
            2 // Default to stereo
        }
    };
    let n_block = match read_be32(param_addr + 8) {
        Ok(v) => v,
        Err(e) => {
            trace!("cellAudioPortOpen: failed to read nBlock, using default (error: 0x{:08X})", e as u32);
            CELL_AUDIO_BLOCK_8 // Default to 8 blocks
        }
    };
    let attr = match read_be32(param_addr + 16) {
        Ok(v) => v,
        Err(e) => {
            trace!("cellAudioPortOpen: failed to read attr, using default (error: 0x{:08X})", e as u32);
            0 // No special attributes
        }
    };
    
    let level = 1.0f32; // Default to full volume
    
    let mut ctx = crate::context::get_hle_context_mut();
    match ctx.audio.port_open(n_channel, n_block, attr, level) {
        Ok(port_num) => {
            // Write port number to memory
            if let Err(e) = write_be32(port_num_addr, port_num) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellAudioPortClose - Close audio port
///
/// # Arguments
/// * `port_num` - Port number
///
/// # Returns
/// * 0 on success
pub fn cell_audio_port_close(port_num: u32) -> i32 {
    debug!("cellAudioPortClose(port_num={})", port_num);

    crate::context::get_hle_context_mut().audio.port_close(port_num)
}

/// cellAudioPortStart - Start audio port
///
/// # Arguments
/// * `port_num` - Port number
///
/// # Returns
/// * 0 on success
pub fn cell_audio_port_start(port_num: u32) -> i32 {
    trace!("cellAudioPortStart(port_num={})", port_num);

    crate::context::get_hle_context_mut().audio.port_start(port_num)
}

/// cellAudioPortStop - Stop audio port
///
/// # Arguments
/// * `port_num` - Port number
///
/// # Returns
/// * 0 on success
pub fn cell_audio_port_stop(port_num: u32) -> i32 {
    trace!("cellAudioPortStop(port_num={})", port_num);

    crate::context::get_hle_context_mut().audio.port_stop(port_num)
}

/// cellAudioGetPortConfig - Get port configuration
///
/// # Arguments
/// * `port_num` - Port number
/// * `config_addr` - Address to write configuration to
///
/// # Returns
/// * 0 on success
pub fn cell_audio_get_port_config(port_num: u32, config_addr: u32) -> i32 {
    trace!("cellAudioGetPortConfig(port_num={}, config_addr=0x{:08X})", port_num, config_addr);

    let ctx = crate::context::get_hle_context();
    if !ctx.audio.is_initialized() {
        return 0x80310702u32 as i32; // CELL_AUDIO_ERROR_AUDIOSYSTEM
    }
    
    // Get port configuration and write to memory
    // CellAudioPortConfig: readIndexAddr(8), status(4), nChannel(8), nBlock(8), portSize(4), portAddr(8)
    if let Some(port) = ctx.audio.get_port(port_num) {
        // Write status
        if let Err(e) = write_be32(config_addr + 8, if port.started { 1 } else { 0 }) {
            return e;
        }
        // Write nChannel
        if let Err(e) = write_be64(config_addr + 12, port.channels as u64) {
            return e;
        }
        // Write nBlock
        if let Err(e) = write_be64(config_addr + 20, port.block_count as u64) {
            return e;
        }
        0 // CELL_OK
    } else {
        0x80310701u32 as i32 // CELL_AUDIO_ERROR_PARAM
    }
}

/// cellAudioCreateNotifyEventQueue - Create event queue for audio notifications
///
/// # Arguments
/// * `id_addr` - Address to write event queue ID to
/// * `key` - Event queue key
///
/// # Returns
/// * 0 on success
pub fn cell_audio_create_notify_event_queue(id_addr: u32, key: u64) -> i32 {
    debug!("cellAudioCreateNotifyEventQueue(id_addr=0x{:08X}, key=0x{:016X})", id_addr, key);

    // For now, create a fake event queue ID based on the key
    let queue_id = (key & 0xFFFFFFFF) as u32;
    
    // Write event queue ID to memory
    if let Err(e) = write_be32(id_addr, queue_id) {
        return e;
    }

    0 // CELL_OK
}

/// cellAudioSetNotifyEventQueue - Set notification event queue
///
/// # Arguments
/// * `key` - Event queue key
///
/// # Returns
/// * 0 on success
pub fn cell_audio_set_notify_event_queue(key: u64) -> i32 {
    debug!("cellAudioSetNotifyEventQueue(key=0x{:016X})", key);

    // TODO: Set notification event queue for audio manager

    0 // CELL_OK
}

/// cellAudioRemoveNotifyEventQueue - Remove notification event queue
///
/// # Arguments
/// * `key` - Event queue key
///
/// # Returns
/// * 0 on success
pub fn cell_audio_remove_notify_event_queue(key: u64) -> i32 {
    debug!("cellAudioRemoveNotifyEventQueue(key=0x{:016X})", key);

    // TODO: Remove notification event queue from audio manager

    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_manager() {
        let mut manager = AudioManager::new();
        assert_eq!(manager.init(), 0);
        assert_eq!(manager.quit(), 0);
    }

    #[test]
    fn test_audio_port_lifecycle() {
        let mut manager = AudioManager::new();
        manager.init();

        let port = manager.port_open(2, CELL_AUDIO_BLOCK_8, 0, 1.0);
        assert!(port.is_ok());

        let port_num = port.unwrap();
        assert_eq!(manager.port_start(port_num), 0);
        assert_eq!(manager.port_stop(port_num), 0);
        assert_eq!(manager.port_close(port_num), 0);

        manager.quit();
    }

    #[test]
    fn test_audio_constants() {
        assert_eq!(CELL_AUDIO_PORT_MAX, 8);
        assert_eq!(CELL_AUDIO_BLOCK_SAMPLES, 256);
        assert_eq!(CELL_AUDIO_BLOCK_8, 8);
    }

    #[test]
    fn test_audio_init() {
        let result = cell_audio_init();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_audio_port_type() {
        assert_eq!(CellAudioPortType::Audio2Ch as u32, 2);
        assert_eq!(CellAudioPortType::Audio8Ch as u32, 8);
    }
}
