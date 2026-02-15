//! cellAudio HLE - Audio Output System
//!
//! This module provides HLE implementations for PS3 audio output.
//! It provides full audio mixing support compatible with the oc-audio subsystem.

use std::collections::{HashMap, VecDeque};
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

/// Audio sample rate (48 kHz)
pub const CELL_AUDIO_SAMPLE_RATE: u64 = 48000;

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
    /// Notification event queues (keyed by event queue key)
    notify_event_queues: Vec<u64>,
    /// Decoded audio queue from cellAdec — PCM frames ready for mixing
    decoded_audio_queue: VecDeque<Vec<f32>>,
    /// Reusable buffer for backend mixing (avoids allocation per mix_audio call)
    backend_mix_buf: Vec<f32>,
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
            notify_event_queues: Vec::new(),
            decoded_audio_queue: VecDeque::new(),
            backend_mix_buf: Vec::new(),
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
        
        // Clear all notification event queues
        self.notify_event_queues.clear();
        
        // Clear decoded audio queue
        self.decoded_audio_queue.clear();
        
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
    /// Reads PCM samples from port shared memory buffers (where games write
    /// samples directly) and decoded audio queue (from cellAdec), mixes them
    /// into a single output buffer. This is called by the audio thread.
    /// 
    /// # Arguments
    /// * `output` - Output buffer to fill with mixed audio
    /// * `frames` - Number of audio frames to mix
    pub fn mix_audio(&mut self, output: &mut [f32], frames: usize) -> i32 {
        if !self.initialized {
            return 0x80310702u32 as i32; // CELL_AUDIO_ERROR_AUDIOSYSTEM
        }

        let samples_needed = frames * 2; // stereo output
        trace!("AudioManager::mix_audio: frames={}, samples_needed={}", frames, samples_needed);

        // Zero output buffer
        for s in output.iter_mut().take(samples_needed) {
            *s = 0.0;
        }

        // Read PCM from port shared memory buffers and mix into output
        for port in &self.ports {
            if port.state != AudioPortState::Started {
                continue;
            }

            // Read PCM from shared memory if buffer address is set
            if port.buffer_addr != 0 && port.num_blocks > 0 {
                let block_offset = (self.block_index as usize % port.num_blocks as usize)
                    * CELL_AUDIO_BLOCK_SAMPLES
                    * port.num_channels as usize;
                let samples_to_read = (CELL_AUDIO_BLOCK_SAMPLES * port.num_channels as usize)
                    .min(samples_needed);
                let base_addr = port.buffer_addr
                    .wrapping_add(block_offset as u32 * 4);

                for i in 0..samples_to_read {
                    let addr = base_addr.wrapping_add(i as u32 * 4);
                    if let Ok(bits) = read_be32(addr) {
                        let sample = f32::from_bits(bits) * port.volume * self.master_volume;
                        let out_idx = if port.num_channels <= 2 {
                            i
                        } else {
                            // Downmix multi-channel to stereo
                            i % 2
                        };
                        if out_idx < samples_needed {
                            output[out_idx] += sample;
                        }
                    }
                }
            }
        }

        // Mix decoded audio from cellAdec queue
        if let Some(decoded) = self.decoded_audio_queue.pop_front() {
            for (i, &sample) in decoded.iter().enumerate() {
                if i < samples_needed {
                    output[i] += sample * self.master_volume;
                }
            }
        }

        // Use mixer backend for any additional sources
        if let Some(backend) = &self.audio_backend {
            if let Ok(mut mixer) = backend.write() {
                // Reuse backend buffer to avoid allocation per mix call
                self.backend_mix_buf.resize(samples_needed, 0.0);
                for s in self.backend_mix_buf.iter_mut() {
                    *s = 0.0;
                }
                mixer.mix(&mut self.backend_mix_buf, frames);
                for (i, &s) in self.backend_mix_buf.iter().enumerate() {
                    if i < samples_needed {
                        output[i] += s;
                    }
                }
            }
        }

        // Clamp to prevent clipping
        for s in output.iter_mut().take(samples_needed) {
            *s = s.clamp(-1.0, 1.0);
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

    /// Get the timestamp for a given audio block tag
    /// 
    /// Returns the timestamp in microseconds derived from the block tag.
    /// Each audio block is 256 samples at 48 kHz, so one block = 256/48000 ≈ 5333.33 µs.
    /// 
    /// # Arguments
    /// * `port_num` - Audio port number
    /// * `tag` - Audio block tag (write index)
    /// 
    /// # Returns
    /// * `Ok(timestamp_us)` on success
    /// * `Err(error_code)` if port is invalid
    pub fn get_port_timestamp(&self, port_num: u32, tag: u64) -> Result<u64, i32> {
        if port_num >= CELL_AUDIO_PORT_MAX as u32 {
            return Err(0x80310704u32 as i32); // CELL_AUDIO_ERROR_PARAM
        }

        let port = &self.ports[port_num as usize];
        if port.state == AudioPortState::Closed {
            return Err(0x80310703u32 as i32); // CELL_AUDIO_ERROR_PORT_NOT_OPEN
        }

        // Each block is 256 samples at 48000 Hz
        // Time per block = 256 / 48000 seconds = 5333.33... µs
        // Using saturating arithmetic to avoid silent overflow for very large tags.
        // At 48 kHz with 256-sample blocks, u64 overflow occurs after ~72 billion years.
        let timestamp_us = tag.saturating_mul(CELL_AUDIO_BLOCK_SAMPLES as u64)
            .saturating_mul(1_000_000)
            / CELL_AUDIO_SAMPLE_RATE;

        Ok(timestamp_us)
    }

    /// Advance audio timing
    /// 
    /// Called each frame to increment the block index. This drives
    /// audio timing for A/V sync and ring buffer management.
    pub fn advance_block_index(&mut self) {
        if self.initialized {
            self.block_index += 1;
        }
    }

    /// Get number of active ports
    pub fn get_active_port_count(&self) -> usize {
        self.ports.iter().filter(|p| p.state == AudioPortState::Started).count()
    }

    /// Check if backend is connected
    pub fn is_backend_connected(&self) -> bool {
        self.audio_backend.is_some()
    }

    // ========================================================================
    // Decoded Audio Queue (cellAdec integration)
    // ========================================================================

    /// Submit decoded PCM audio from cellAdec to the mixer queue
    ///
    /// cellAdec decoders (ATRAC3, AAC, MP3, etc.) produce PCM samples
    /// which are queued here for mixing into the audio output.
    ///
    /// # Arguments
    /// * `pcm_samples` - Decoded PCM samples (f32, interleaved stereo)
    pub fn submit_decoded_audio(&mut self, pcm_samples: Vec<f32>) {
        if pcm_samples.is_empty() {
            return;
        }
        trace!("AudioManager: queued {} decoded PCM samples", pcm_samples.len());
        self.decoded_audio_queue.push_back(pcm_samples);
    }

    /// Get the number of pending decoded audio buffers
    pub fn decoded_audio_pending(&self) -> usize {
        self.decoded_audio_queue.len()
    }

    /// Clear the decoded audio queue
    pub fn clear_decoded_audio(&mut self) {
        self.decoded_audio_queue.clear();
    }

    // ========================================================================
    // Notification Event Queue Management
    // ========================================================================

    /// Set notification event queue for audio manager
    /// 
    /// Registers an event queue key for receiving audio notifications.
    /// The audio system will send events to this queue when audio blocks
    /// need to be filled.
    /// 
    /// # Arguments
    /// * `key` - Event queue key to register
    /// 
    /// # Returns
    /// * 0 on success
    /// * CELL_AUDIO_ERROR_AUDIOSYSTEM if not initialized
    pub fn set_notify_event_queue(&mut self, key: u64) -> i32 {
        if !self.initialized {
            return 0x80310702u32 as i32; // CELL_AUDIO_ERROR_AUDIOSYSTEM
        }

        // Check if already registered
        if !self.notify_event_queues.contains(&key) {
            self.notify_event_queues.push(key);
            debug!("AudioManager: registered notification event queue key=0x{:016X}, total={}", 
                   key, self.notify_event_queues.len());
        } else {
            trace!("AudioManager: event queue key=0x{:016X} already registered", key);
        }

        0 // CELL_OK
    }

    /// Remove notification event queue from audio manager
    /// 
    /// Unregisters an event queue key from receiving audio notifications.
    /// 
    /// # Arguments
    /// * `key` - Event queue key to unregister
    /// 
    /// # Returns
    /// * 0 on success
    /// * CELL_AUDIO_ERROR_AUDIOSYSTEM if not initialized
    /// * CELL_AUDIO_ERROR_PARAM if key not found
    pub fn remove_notify_event_queue(&mut self, key: u64) -> i32 {
        if !self.initialized {
            return 0x80310702u32 as i32; // CELL_AUDIO_ERROR_AUDIOSYSTEM
        }

        if let Some(pos) = self.notify_event_queues.iter().position(|&k| k == key) {
            self.notify_event_queues.remove(pos);
            debug!("AudioManager: removed notification event queue key=0x{:016X}, remaining={}", 
                   key, self.notify_event_queues.len());
            0 // CELL_OK
        } else {
            trace!("AudioManager: event queue key=0x{:016X} not found", key);
            0x80310704u32 as i32 // CELL_AUDIO_ERROR_PARAM
        }
    }

    /// Get the registered notification event queue keys
    /// 
    /// Returns a copy of all registered event queue keys.
    pub fn get_notify_event_queues(&self) -> Vec<u64> {
        self.notify_event_queues.clone()
    }

    /// Check if any notification event queues are registered
    pub fn has_notify_event_queues(&self) -> bool {
        !self.notify_event_queues.is_empty()
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

    crate::context::get_hle_context_mut().audio.set_notify_event_queue(key)
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

    crate::context::get_hle_context_mut().audio.remove_notify_event_queue(key)
}

/// cellAudioGetPortTimestamp - Get audio port timestamp
///
/// Returns the timestamp (in microseconds) for a specific audio block tag.
/// Games use this for A/V synchronization by comparing audio playback
/// position against video frame timing.
///
/// The timestamp is derived from the block index: each audio block is
/// 256 samples at 48 kHz, so one block = 256/48000 ≈ 5333.33 µs.
///
/// # Arguments
/// * `port_num` - Audio port number
/// * `tag` - Audio block tag (write index)
/// * `stamp_addr` - Address to write the 64-bit timestamp (microseconds)
///
/// # Returns
/// * 0 on success
pub fn cell_audio_get_port_timestamp(port_num: u32, tag: u64, stamp_addr: u32) -> i32 {
    trace!("cellAudioGetPortTimestamp(port_num={}, tag={}, stamp_addr=0x{:08X})", port_num, tag, stamp_addr);

    let ctx = crate::context::get_hle_context();
    if !ctx.audio.is_initialized() {
        return 0x80310702u32 as i32; // CELL_AUDIO_ERROR_AUDIOSYSTEM
    }

    match ctx.audio.get_port_timestamp(port_num, tag) {
        Ok(timestamp) => {
            if let Err(e) = write_be64(stamp_addr, timestamp) {
                trace!("cellAudioGetPortTimestamp: failed to write timestamp to 0x{:08X}: 0x{:08X}", stamp_addr, e as u32);
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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

    #[test]
    fn test_notify_event_queue_set_and_remove() {
        let mut manager = AudioManager::new();
        manager.init();

        // Set event queue
        let key = 0x1234567890ABCDEF;
        assert_eq!(manager.set_notify_event_queue(key), 0);
        assert!(manager.has_notify_event_queues());
        assert_eq!(manager.get_notify_event_queues().len(), 1);

        // Remove event queue
        assert_eq!(manager.remove_notify_event_queue(key), 0);
        assert!(!manager.has_notify_event_queues());

        manager.quit();
    }

    #[test]
    fn test_notify_event_queue_duplicate_set() {
        let mut manager = AudioManager::new();
        manager.init();

        let key = 0xDEADBEEFCAFEBABE;
        assert_eq!(manager.set_notify_event_queue(key), 0);
        assert_eq!(manager.set_notify_event_queue(key), 0); // Duplicate should be OK
        assert_eq!(manager.get_notify_event_queues().len(), 1); // Still only one

        manager.quit();
    }

    #[test]
    fn test_notify_event_queue_remove_nonexistent() {
        let mut manager = AudioManager::new();
        manager.init();

        // Try to remove non-existent key
        let result = manager.remove_notify_event_queue(0x12345678);
        assert_eq!(result, 0x80310704u32 as i32); // CELL_AUDIO_ERROR_PARAM

        manager.quit();
    }

    #[test]
    fn test_notify_event_queue_cleared_on_quit() {
        let mut manager = AudioManager::new();
        manager.init();

        manager.set_notify_event_queue(0x1111);
        manager.set_notify_event_queue(0x2222);
        assert_eq!(manager.get_notify_event_queues().len(), 2);

        manager.quit();
        
        // Re-init and check queues are cleared
        manager.init();
        assert!(!manager.has_notify_event_queues());
        
        manager.quit();
    }

    #[test]
    fn test_port_timestamp_basic() {
        let mut manager = AudioManager::new();
        manager.init();

        let port_num = manager.port_open(2, CELL_AUDIO_BLOCK_8, 0, 1.0).unwrap();
        manager.port_start(port_num);

        // Tag 0 → timestamp 0
        let ts = manager.get_port_timestamp(port_num, 0).unwrap();
        assert_eq!(ts, 0);

        // Tag 1 → 256 samples at 48 kHz = 5333 µs
        let ts1 = manager.get_port_timestamp(port_num, 1).unwrap();
        assert_eq!(ts1, 256 * 1_000_000 / CELL_AUDIO_SAMPLE_RATE);

        // Tag 9 (one full 8-block frame) → ~48000 µs
        let ts9 = manager.get_port_timestamp(port_num, 9).unwrap();
        assert_eq!(ts9, 9 * 256 * 1_000_000 / CELL_AUDIO_SAMPLE_RATE);

        manager.quit();
    }

    #[test]
    fn test_port_timestamp_closed_port() {
        let mut manager = AudioManager::new();
        manager.init();

        // Port 0 is closed — should fail
        let result = manager.get_port_timestamp(0, 0);
        assert!(result.is_err());

        manager.quit();
    }

    #[test]
    fn test_port_timestamp_invalid_port() {
        let mut manager = AudioManager::new();
        manager.init();

        // Invalid port number
        let result = manager.get_port_timestamp(99, 0);
        assert!(result.is_err());

        manager.quit();
    }

    #[test]
    fn test_advance_block_index() {
        let mut manager = AudioManager::new();
        manager.init();

        assert_eq!(manager.get_block_index(), 0);
        manager.advance_block_index();
        assert_eq!(manager.get_block_index(), 1);
        manager.advance_block_index();
        assert_eq!(manager.get_block_index(), 2);

        manager.quit();
    }

    #[test]
    fn test_advance_block_not_initialized() {
        let mut manager = AudioManager::new();

        // Not initialized — should not advance
        manager.advance_block_index();
        assert_eq!(manager.get_block_index(), 0);
    }

    #[test]
    fn test_audio_mixer_integration() {
        let mut manager = AudioManager::new();
        
        // Create and connect mixer backend
        let mixer = Arc::new(RwLock::new(HleAudioMixer::default()));
        manager.set_audio_backend(mixer.clone());
        assert!(manager.has_audio_backend());
        
        manager.init();

        // Open port — should create mixer source
        let port_num = manager.port_open(2, CELL_AUDIO_BLOCK_8, 0, 1.0).unwrap();
        manager.port_start(port_num);

        // Submit audio samples — should forward to mixer
        let samples = vec![0.5f32; 512];
        let result = manager.submit_audio(port_num, &samples);
        assert_eq!(result, 0);

        // Verify mixer has data
        {
            let mut m = mixer.write().unwrap();
            let mut output = vec![0.0f32; 512];
            m.mix(&mut output, 256);
            // Should have non-zero output after mixing
            let has_audio = output.iter().any(|&s| s != 0.0);
            assert!(has_audio, "Mixer should have audio data after submit");
        }

        manager.quit();
    }

    #[test]
    fn test_sample_rate_constant() {
        assert_eq!(CELL_AUDIO_SAMPLE_RATE, 48000);
    }

    #[test]
    fn test_mix_audio_produces_output() {
        let mut manager = AudioManager::new();
        let mixer = Arc::new(RwLock::new(HleAudioMixer::default()));
        manager.set_audio_backend(mixer.clone());
        manager.init();

        let port_num = manager.port_open(2, CELL_AUDIO_BLOCK_8, 0, 1.0).unwrap();
        manager.port_start(port_num);

        // Submit samples through submit_audio (writes to mixer source)
        let samples = vec![0.5f32; 512];
        manager.submit_audio(port_num, &samples);

        // mix_audio should produce non-zero output from backend
        let mut output = vec![0.0f32; 512];
        let result = manager.mix_audio(&mut output, 256);
        assert_eq!(result, 0);
        let has_audio = output.iter().any(|&s| s != 0.0);
        assert!(has_audio, "mix_audio should produce non-zero output from submitted samples");

        manager.quit();
    }

    #[test]
    fn test_mix_audio_increments_block_index() {
        let mut manager = AudioManager::new();
        manager.init();

        assert_eq!(manager.get_block_index(), 0);
        let mut output = vec![0.0f32; 512];
        manager.mix_audio(&mut output, 256);
        assert_eq!(manager.get_block_index(), 1);
        manager.mix_audio(&mut output, 256);
        assert_eq!(manager.get_block_index(), 2);

        manager.quit();
    }

    #[test]
    fn test_decoded_audio_queue() {
        let mut manager = AudioManager::new();
        manager.init();

        assert_eq!(manager.decoded_audio_pending(), 0);

        // Submit decoded PCM from cellAdec
        let pcm = vec![0.25f32; 512];
        manager.submit_decoded_audio(pcm);
        assert_eq!(manager.decoded_audio_pending(), 1);

        // Mix should drain the decoded queue
        let mut output = vec![0.0f32; 512];
        manager.mix_audio(&mut output, 256);
        assert_eq!(manager.decoded_audio_pending(), 0);

        // Output should contain the decoded audio
        let has_audio = output.iter().any(|&s| s != 0.0);
        assert!(has_audio, "mix_audio should include decoded audio queue samples");

        manager.quit();
    }

    #[test]
    fn test_decoded_audio_empty_ignored() {
        let mut manager = AudioManager::new();
        manager.init();

        // Empty samples should be ignored
        manager.submit_decoded_audio(Vec::new());
        assert_eq!(manager.decoded_audio_pending(), 0);

        manager.quit();
    }

    #[test]
    fn test_decoded_audio_cleared_on_quit() {
        let mut manager = AudioManager::new();
        manager.init();

        manager.submit_decoded_audio(vec![0.5f32; 256]);
        assert_eq!(manager.decoded_audio_pending(), 1);

        manager.quit();

        // Re-init and verify queue is cleared
        manager.init();
        assert_eq!(manager.decoded_audio_pending(), 0);
        manager.quit();
    }

    #[test]
    fn test_mix_audio_clamps_output() {
        let mut manager = AudioManager::new();
        let mixer = Arc::new(RwLock::new(HleAudioMixer::default()));
        manager.set_audio_backend(mixer.clone());
        manager.init();

        let port_num = manager.port_open(2, CELL_AUDIO_BLOCK_8, 0, 1.0).unwrap();
        manager.port_start(port_num);

        // Submit very loud samples
        let samples = vec![2.0f32; 512];
        manager.submit_audio(port_num, &samples);

        let mut output = vec![0.0f32; 512];
        manager.mix_audio(&mut output, 256);

        // All samples should be clamped to [-1.0, 1.0]
        for &s in &output[..512] {
            assert!(s >= -1.0 && s <= 1.0, "Sample {} should be clamped", s);
        }

        manager.quit();
    }

    #[test]
    fn test_block_timestamp_consistency() {
        let mut manager = AudioManager::new();
        manager.init();
        let port_num = manager.port_open(2, CELL_AUDIO_BLOCK_8, 0, 1.0).unwrap();

        // Block 0 = 0µs, Block 1 = 5333µs, Block 9 = 48000µs
        let t0 = manager.get_port_timestamp(port_num, 0).unwrap();
        let t1 = manager.get_port_timestamp(port_num, 1).unwrap();
        let t9 = manager.get_port_timestamp(port_num, 9).unwrap();

        assert_eq!(t0, 0);
        assert_eq!(t1, 5333); // 256 * 1_000_000 / 48000
        assert_eq!(t9, 48000); // 9 * 256 * 1_000_000 / 48000
        assert!(t9 > t1);
        assert!(t1 > t0);

        manager.quit();
    }
}
