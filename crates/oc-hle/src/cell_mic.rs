//! cellMic HLE - Microphone Input
//!
//! This module provides HLE implementations for PS3 microphone audio capture.
//! It supports device enumeration and audio capture with full oc-input integration.

use std::sync::{Arc, RwLock};
use tracing::{debug, trace};
use oc_input::microphone::{Microphone, MicrophoneManager, MicrophoneState, AudioBuffer};

/// OC-Input microphone backend reference
pub type MicrophoneBackend = Option<Arc<RwLock<MicrophoneManager>>>;

/// Error codes
pub const CELL_MIC_ERROR_NOT_INITIALIZED: i32 = 0x80140101u32 as i32;
pub const CELL_MIC_ERROR_ALREADY_INITIALIZED: i32 = 0x80140102u32 as i32;
pub const CELL_MIC_ERROR_NO_DEVICE: i32 = 0x80140103u32 as i32;
pub const CELL_MIC_ERROR_INVALID_PARAMETER: i32 = 0x80140104u32 as i32;
pub const CELL_MIC_ERROR_DEVICE_BUSY: i32 = 0x80140105u32 as i32;
pub const CELL_MIC_ERROR_NO_MEMORY: i32 = 0x80140106u32 as i32;

/// Maximum number of microphones
pub const CELL_MIC_MAX_DEVICES: usize = 4;

/// Audio sample rates
pub const CELL_MIC_SAMPLE_RATE_48K: u32 = 48000;
pub const CELL_MIC_SAMPLE_RATE_32K: u32 = 32000;
pub const CELL_MIC_SAMPLE_RATE_24K: u32 = 24000;
pub const CELL_MIC_SAMPLE_RATE_16K: u32 = 16000;

/// Device type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CellMicDeviceType {
    /// USB microphone
    #[default]
    Usb = 0,
    /// Bluetooth headset
    Bluetooth = 1,
    /// Camera microphone (PlayStation Eye)
    Camera = 2,
}


/// Device state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CellMicDeviceState {
    /// Device is closed
    #[default]
    Closed = 0,
    /// Device is open
    Open = 1,
    /// Device is capturing
    Capturing = 2,
}


/// Device info
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMicDeviceInfo {
    /// Device ID
    pub device_id: u32,
    /// Device type
    pub device_type: u32,
    /// Number of channels (1 for mono, 2 for stereo)
    pub num_channels: u32,
    /// Sample rate
    pub sample_rate: u32,
    /// Device state
    pub state: u32,
    /// Device name
    pub name: [u8; 64],
}

impl Default for CellMicDeviceInfo {
    fn default() -> Self {
        Self {
            device_id: 0,
            device_type: CellMicDeviceType::Usb as u32,
            num_channels: 1,
            sample_rate: CELL_MIC_SAMPLE_RATE_48K,
            state: CellMicDeviceState::Closed as u32,
            name: [0; 64],
        }
    }
}

/// Capture parameters
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellMicCaptureParam {
    /// Sample rate
    pub sample_rate: u32,
    /// Channels
    pub channels: u32,
    /// Buffer size in samples
    pub buffer_size: u32,
}

impl Default for CellMicCaptureParam {
    fn default() -> Self {
        Self {
            sample_rate: CELL_MIC_SAMPLE_RATE_48K,
            channels: 1,
            buffer_size: 256,
        }
    }
}

/// Device entry
#[derive(Debug, Clone)]
struct DeviceEntry {
    /// Device info
    info: CellMicDeviceInfo,
    /// Current state
    state: CellMicDeviceState,
    /// Capture parameters
    params: CellMicCaptureParam,
}

/// Microphone manager
pub struct MicManager {
    /// Initialization flag
    initialized: bool,
    /// Devices
    devices: [Option<DeviceEntry>; CELL_MIC_MAX_DEVICES],
    /// Number of connected devices
    num_devices: u32,
    /// OC-Input microphone backend
    capture_backend: MicrophoneBackend,
    /// Audio buffers for each device
    audio_buffers: [Vec<i16>; CELL_MIC_MAX_DEVICES],
    /// Buffer read positions
    buffer_positions: [usize; CELL_MIC_MAX_DEVICES],
}

impl MicManager {
    /// Create a new microphone manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            devices: [None, None, None, None],
            num_devices: 0,
            capture_backend: None,
            audio_buffers: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            buffer_positions: [0; CELL_MIC_MAX_DEVICES],
        }
    }

    /// Initialize microphone system
    pub fn init(&mut self) -> i32 {
        if self.initialized {
            return CELL_MIC_ERROR_ALREADY_INITIALIZED;
        }

        debug!("MicManager::init");

        self.initialized = true;

        // Simulate one device connected
        let mut info = CellMicDeviceInfo::default();
        info.device_id = 0;
        info.device_type = CellMicDeviceType::Usb as u32;
        let name = b"USB Microphone\0";
        info.name[..name.len()].copy_from_slice(name);

        self.devices[0] = Some(DeviceEntry {
            info,
            state: CellMicDeviceState::Closed,
            params: CellMicCaptureParam::default(),
        });
        self.num_devices = 1;

        0 // CELL_OK
    }

    /// Shutdown microphone system
    pub fn end(&mut self) -> i32 {
        if !self.initialized {
            return CELL_MIC_ERROR_NOT_INITIALIZED;
        }

        debug!("MicManager::end");

        // Close all open devices
        for device in &mut self.devices {
            if let Some(entry) = device {
                if entry.state != CellMicDeviceState::Closed {
                    entry.state = CellMicDeviceState::Closed;
                }
            }
            *device = None;
        }

        self.initialized = false;
        self.num_devices = 0;

        0 // CELL_OK
    }

    /// Get device count
    pub fn get_device_count(&self) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_MIC_ERROR_NOT_INITIALIZED);
        }

        Ok(self.num_devices)
    }

    /// Get device info
    pub fn get_device_info(&self, device_id: u32) -> Result<CellMicDeviceInfo, i32> {
        if !self.initialized {
            return Err(CELL_MIC_ERROR_NOT_INITIALIZED);
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return Err(CELL_MIC_ERROR_INVALID_PARAMETER);
        }

        match &self.devices[device_id as usize] {
            Some(entry) => Ok(entry.info),
            None => Err(CELL_MIC_ERROR_NO_DEVICE),
        }
    }

    /// Open device
    pub fn open(&mut self, device_id: u32) -> i32 {
        if !self.initialized {
            return CELL_MIC_ERROR_NOT_INITIALIZED;
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return CELL_MIC_ERROR_INVALID_PARAMETER;
        }

        let device = match &mut self.devices[device_id as usize] {
            Some(entry) => entry,
            None => return CELL_MIC_ERROR_NO_DEVICE,
        };

        if device.state != CellMicDeviceState::Closed {
            return CELL_MIC_ERROR_DEVICE_BUSY;
        }

        debug!("MicManager::open: device_id={}", device_id);

        device.state = CellMicDeviceState::Open;
        device.info.state = CellMicDeviceState::Open as u32;

        0 // CELL_OK
    }

    /// Close device
    pub fn close(&mut self, device_id: u32) -> i32 {
        if !self.initialized {
            return CELL_MIC_ERROR_NOT_INITIALIZED;
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return CELL_MIC_ERROR_INVALID_PARAMETER;
        }

        let device = match &mut self.devices[device_id as usize] {
            Some(entry) => entry,
            None => return CELL_MIC_ERROR_NO_DEVICE,
        };

        debug!("MicManager::close: device_id={}", device_id);

        device.state = CellMicDeviceState::Closed;
        device.info.state = CellMicDeviceState::Closed as u32;

        0 // CELL_OK
    }

    /// Start capture
    pub fn start(&mut self, device_id: u32) -> i32 {
        if !self.initialized {
            return CELL_MIC_ERROR_NOT_INITIALIZED;
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return CELL_MIC_ERROR_INVALID_PARAMETER;
        }

        let device = match &mut self.devices[device_id as usize] {
            Some(entry) => entry,
            None => return CELL_MIC_ERROR_NO_DEVICE,
        };

        if device.state != CellMicDeviceState::Open {
            return CELL_MIC_ERROR_DEVICE_BUSY;
        }

        debug!("MicManager::start: device_id={}", device_id);

        device.state = CellMicDeviceState::Capturing;
        device.info.state = CellMicDeviceState::Capturing as u32;

        // TODO: Start actual audio capture

        0 // CELL_OK
    }

    /// Stop capture
    pub fn stop(&mut self, device_id: u32) -> i32 {
        if !self.initialized {
            return CELL_MIC_ERROR_NOT_INITIALIZED;
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return CELL_MIC_ERROR_INVALID_PARAMETER;
        }

        let device = match &mut self.devices[device_id as usize] {
            Some(entry) => entry,
            None => return CELL_MIC_ERROR_NO_DEVICE,
        };

        if device.state != CellMicDeviceState::Capturing {
            return 0; // Already stopped, not an error
        }

        debug!("MicManager::stop: device_id={}", device_id);

        device.state = CellMicDeviceState::Open;
        device.info.state = CellMicDeviceState::Open as u32;

        // TODO: Stop actual audio capture

        0 // CELL_OK
    }

    /// Read captured data
    pub fn read(&self, device_id: u32, _buffer: &mut [u8]) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_MIC_ERROR_NOT_INITIALIZED);
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return Err(CELL_MIC_ERROR_INVALID_PARAMETER);
        }

        let device = match &self.devices[device_id as usize] {
            Some(entry) => entry,
            None => return Err(CELL_MIC_ERROR_NO_DEVICE),
        };

        if device.state != CellMicDeviceState::Capturing {
            return Err(CELL_MIC_ERROR_DEVICE_BUSY);
        }

        trace!("MicManager::read: device_id={}", device_id);

        // TODO: Read actual captured data

        Ok(0) // No data available in stub
    }

    /// Set parameters
    pub fn set_param(&mut self, device_id: u32, param: CellMicCaptureParam) -> i32 {
        if !self.initialized {
            return CELL_MIC_ERROR_NOT_INITIALIZED;
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return CELL_MIC_ERROR_INVALID_PARAMETER;
        }

        let device = match &mut self.devices[device_id as usize] {
            Some(entry) => entry,
            None => return CELL_MIC_ERROR_NO_DEVICE,
        };

        trace!("MicManager::set_param: device_id={}, sample_rate={}", device_id, param.sample_rate);

        device.params = param;
        device.info.sample_rate = param.sample_rate;
        device.info.num_channels = param.channels;

        0 // CELL_OK
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    // ========================================================================
    // Audio Capture Backend Integration
    // ========================================================================

    /// Set the oc-input microphone backend
    /// 
    /// Connects the MicManager to the oc-input microphone system,
    /// enabling actual audio capture.
    /// 
    /// # Arguments
    /// * `backend` - Shared reference to MicrophoneManager
    pub fn set_capture_backend(&mut self, backend: Arc<RwLock<MicrophoneManager>>) {
        debug!("MicManager::set_capture_backend - connecting to oc-input");
        self.capture_backend = Some(backend);
    }

    /// Check if the capture backend is connected
    pub fn has_capture_backend(&self) -> bool {
        self.capture_backend.is_some()
    }

    /// Enumerate audio capture devices from backend
    /// 
    /// Queries the oc-input backend for available microphone devices.
    pub fn enumerate_devices(&mut self) -> i32 {
        if !self.initialized {
            return CELL_MIC_ERROR_NOT_INITIALIZED;
        }

        debug!("MicManager::enumerate_devices");

        // Get backend or use simulated device
        let backend = match &self.capture_backend {
            Some(b) => b.clone(),
            None => {
                // No backend, keep simulated device
                return 0;
            }
        };

        // Lock backend for reading
        let manager = match backend.read() {
            Ok(m) => m,
            Err(e) => {
                debug!("MicManager::enumerate_devices - failed to lock backend: {}", e);
                return 0;
            }
        };

        // Clear existing devices (except the simulated one if no real devices)
        self.num_devices = 0;

        // Enumerate devices from backend using list_registered
        for (index, name) in manager.list_registered() {
            if self.num_devices >= CELL_MIC_MAX_DEVICES as u32 {
                break;
            }

            // Get the microphone to access its config
            if let Some(mic) = manager.get(index) {
                let mut info = CellMicDeviceInfo::default();
                info.device_id = index as u32;
                info.device_type = CellMicDeviceType::Usb as u32;
                info.num_channels = mic.config.channels as u32;
                info.sample_rate = mic.config.sample_rate.hz();
                info.state = match mic.state() {
                    MicrophoneState::Closed => CellMicDeviceState::Closed as u32,
                    MicrophoneState::Open => CellMicDeviceState::Open as u32,
                    MicrophoneState::Recording => CellMicDeviceState::Capturing as u32,
                    MicrophoneState::Error => CellMicDeviceState::Closed as u32,
                };

                // Copy device name
                let name_bytes = name.as_bytes();
                let copy_len = name_bytes.len().min(63);
                info.name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

                let device_idx = self.num_devices as usize;
                self.devices[device_idx] = Some(DeviceEntry {
                    info,
                    state: CellMicDeviceState::Closed,
                    params: CellMicCaptureParam::default(),
                });
                self.num_devices += 1;
            }
        }

        debug!("MicManager::enumerate_devices - found {} devices", self.num_devices);

        0 // CELL_OK
    }

    /// Start audio capture on backend
    fn backend_start_capture(&mut self, device_id: u32) -> i32 {
        trace!("MicManager::backend_start_capture: device_id={}", device_id);

        let backend = match &self.capture_backend {
            Some(b) => b.clone(),
            None => return 0, // No backend, stub mode
        };

        let mut manager = match backend.write() {
            Ok(m) => m,
            Err(e) => {
                debug!("MicManager::backend_start_capture - failed to lock backend: {}", e);
                return CELL_MIC_ERROR_DEVICE_BUSY;
            }
        };

        if let Some(mic) = manager.get_mut(device_id as u8) {
            if let Err(e) = mic.start_recording() {
                debug!("MicManager::backend_start_capture - failed to start: {:?}", e);
                return CELL_MIC_ERROR_DEVICE_BUSY;
            }
        }

        0 // CELL_OK
    }

    /// Stop audio capture on backend
    fn backend_stop_capture(&mut self, device_id: u32) -> i32 {
        trace!("MicManager::backend_stop_capture: device_id={}", device_id);

        let backend = match &self.capture_backend {
            Some(b) => b.clone(),
            None => return 0, // No backend, stub mode
        };

        let mut manager = match backend.write() {
            Ok(m) => m,
            Err(e) => {
                debug!("MicManager::backend_stop_capture - failed to lock backend: {}", e);
                return CELL_MIC_ERROR_DEVICE_BUSY;
            }
        };

        if let Some(mic) = manager.get_mut(device_id as u8) {
            let _ = mic.stop_recording();
        }

        0 // CELL_OK
    }

    /// Read captured audio data from backend
    /// 
    /// # Arguments
    /// * `device_id` - Device ID
    /// * `buffer` - Buffer to fill with captured audio
    pub fn backend_read_data(&mut self, device_id: u32, buffer: &mut [u8]) -> Result<u32, i32> {
        trace!("MicManager::backend_read_data: device_id={}", device_id);

        let backend = match &self.capture_backend {
            Some(b) => b.clone(),
            None => return Ok(0), // No backend, no data
        };

        let mut manager = match backend.write() {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };

        if let Some(mic) = manager.get_mut(device_id as u8) {
            if let Some(audio_buffer) = mic.read_audio() {
                // Convert and copy audio data
                let samples: Vec<i16> = audio_buffer.as_i16_samples();
                let device_idx = device_id as usize;

                // Store in our buffer
                self.audio_buffers[device_idx].extend_from_slice(&samples);

                // Copy to output buffer
                let available = self.audio_buffers[device_idx].len() * 2; // bytes
                let to_copy = available.min(buffer.len());

                for (i, sample) in self.audio_buffers[device_idx].drain(..(to_copy / 2)).enumerate() {
                    let bytes = sample.to_le_bytes();
                    buffer[i * 2] = bytes[0];
                    buffer[i * 2 + 1] = bytes[1];
                }

                return Ok(to_copy as u32);
            }
        }

        Ok(0) // No data available
    }

    /// Get audio level from backend
    /// 
    /// Returns the current audio input level (0.0 - 1.0)
    pub fn get_audio_level(&self, device_id: u32) -> Result<f32, i32> {
        if !self.initialized {
            return Err(CELL_MIC_ERROR_NOT_INITIALIZED);
        }

        if device_id >= CELL_MIC_MAX_DEVICES as u32 {
            return Err(CELL_MIC_ERROR_INVALID_PARAMETER);
        }

        let backend = match &self.capture_backend {
            Some(b) => b.clone(),
            None => return Ok(0.0), // No backend, no level
        };

        let manager = match backend.read() {
            Ok(m) => m,
            Err(_) => return Ok(0.0),
        };

        if let Some(mic) = manager.get(device_id as u8) {
            return Ok(mic.levels().rms);
        }

        Ok(0.0)
    }

    /// Check if backend is connected
    pub fn is_backend_connected(&self) -> bool {
        self.capture_backend.is_some()
    }
}

impl Default for MicManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellMicInit - Initialize microphone system
///
/// # Returns
/// * 0 on success
pub fn cell_mic_init() -> i32 {
    debug!("cellMicInit()");

    crate::context::get_hle_context_mut().mic.init()
}

/// cellMicEnd - Shutdown microphone system
///
/// # Returns
/// * 0 on success
pub fn cell_mic_end() -> i32 {
    debug!("cellMicEnd()");

    crate::context::get_hle_context_mut().mic.end()
}

/// cellMicGetDeviceCount - Get number of connected microphones
///
/// # Arguments
/// * `count_addr` - Address to write count
///
/// # Returns
/// * 0 on success
pub fn cell_mic_get_device_count(_count_addr: u32) -> i32 {
    trace!("cellMicGetDeviceCount()");

    match crate::context::get_hle_context().mic.get_device_count() {
        Ok(_count) => {
            // TODO: Write count to memory at _count_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellMicGetDeviceInfo - Get device information
///
/// # Arguments
/// * `device_id` - Device ID
/// * `info_addr` - Address to write info
///
/// # Returns
/// * 0 on success
pub fn cell_mic_get_device_info(device_id: u32, _info_addr: u32) -> i32 {
    trace!("cellMicGetDeviceInfo(device_id={})", device_id);

    match crate::context::get_hle_context().mic.get_device_info(device_id) {
        Ok(_info) => {
            // TODO: Write info to memory at _info_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellMicOpen - Open a microphone device
///
/// # Arguments
/// * `device_id` - Device ID
///
/// # Returns
/// * 0 on success
pub fn cell_mic_open(device_id: u32) -> i32 {
    debug!("cellMicOpen(device_id={})", device_id);

    crate::context::get_hle_context_mut().mic.open(device_id)
}

/// cellMicClose - Close a microphone device
///
/// # Arguments
/// * `device_id` - Device ID
///
/// # Returns
/// * 0 on success
pub fn cell_mic_close(device_id: u32) -> i32 {
    debug!("cellMicClose(device_id={})", device_id);

    crate::context::get_hle_context_mut().mic.close(device_id)
}

/// cellMicStart - Start audio capture
///
/// # Arguments
/// * `device_id` - Device ID
///
/// # Returns
/// * 0 on success
pub fn cell_mic_start(device_id: u32) -> i32 {
    debug!("cellMicStart(device_id={})", device_id);

    crate::context::get_hle_context_mut().mic.start(device_id)
}

/// cellMicStop - Stop audio capture
///
/// # Arguments
/// * `device_id` - Device ID
///
/// # Returns
/// * 0 on success
pub fn cell_mic_stop(device_id: u32) -> i32 {
    debug!("cellMicStop(device_id={})", device_id);

    crate::context::get_hle_context_mut().mic.stop(device_id)
}

/// cellMicRead - Read captured audio data
///
/// # Arguments
/// * `device_id` - Device ID
/// * `buffer_addr` - Buffer address
/// * `buffer_size` - Buffer size
/// * `read_size_addr` - Address to write bytes read
///
/// # Returns
/// * 0 on success
pub fn cell_mic_read(device_id: u32, _buffer_addr: u32, _buffer_size: u32, _read_size_addr: u32) -> i32 {
    trace!("cellMicRead(device_id={})", device_id);

    // Read data
    // Check if operation would be valid
    let ctx = crate::context::get_hle_context();
    match ctx.mic.get_device_info(device_id) {
        Ok(info) => {
            if info.state != CellMicDeviceState::Capturing as u32 {
                return CELL_MIC_ERROR_DEVICE_BUSY;
            }
            // TODO: Read actual captured data to buffer
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mic_manager_lifecycle() {
        let mut manager = MicManager::new();
        
        assert_eq!(manager.init(), 0);
        assert!(manager.is_initialized());
        
        // Double init should fail
        assert_eq!(manager.init(), CELL_MIC_ERROR_ALREADY_INITIALIZED);
        
        assert_eq!(manager.end(), 0);
        assert!(!manager.is_initialized());
        
        // Double end should fail
        assert_eq!(manager.end(), CELL_MIC_ERROR_NOT_INITIALIZED);
    }

    #[test]
    fn test_mic_manager_device_count() {
        let mut manager = MicManager::new();
        manager.init();
        
        let count = manager.get_device_count().unwrap();
        assert_eq!(count, 1); // Simulated device
        
        manager.end();
    }

    #[test]
    fn test_mic_manager_device_info() {
        let mut manager = MicManager::new();
        manager.init();
        
        let info = manager.get_device_info(0).unwrap();
        assert_eq!(info.device_id, 0);
        assert_eq!(info.device_type, CellMicDeviceType::Usb as u32);
        
        // Invalid device
        assert_eq!(manager.get_device_info(99), Err(CELL_MIC_ERROR_INVALID_PARAMETER));
        
        manager.end();
    }

    #[test]
    fn test_mic_manager_capture_lifecycle() {
        let mut manager = MicManager::new();
        manager.init();
        
        // Open device
        assert_eq!(manager.open(0), 0);
        
        // Open again should fail
        assert_eq!(manager.open(0), CELL_MIC_ERROR_DEVICE_BUSY);
        
        // Start capture
        assert_eq!(manager.start(0), 0);
        
        // Stop capture
        assert_eq!(manager.stop(0), 0);
        
        // Close device
        assert_eq!(manager.close(0), 0);
        
        manager.end();
    }

    #[test]
    fn test_mic_manager_read() {
        let mut manager = MicManager::new();
        manager.init();
        
        // Open and start
        manager.open(0);
        manager.start(0);
        
        // Read data
        let mut buffer = [0u8; 256];
        let result = manager.read(0, &mut buffer);
        assert!(result.is_ok());
        
        manager.stop(0);
        manager.close(0);
        manager.end();
    }

    #[test]
    fn test_mic_manager_set_param() {
        let mut manager = MicManager::new();
        manager.init();
        
        let param = CellMicCaptureParam {
            sample_rate: CELL_MIC_SAMPLE_RATE_16K,
            channels: 2,
            buffer_size: 512,
        };
        
        assert_eq!(manager.set_param(0, param), 0);
        
        let info = manager.get_device_info(0).unwrap();
        assert_eq!(info.sample_rate, CELL_MIC_SAMPLE_RATE_16K);
        assert_eq!(info.num_channels, 2);
        
        manager.end();
    }

    #[test]
    fn test_mic_sample_rates() {
        assert_eq!(CELL_MIC_SAMPLE_RATE_48K, 48000);
        assert_eq!(CELL_MIC_SAMPLE_RATE_32K, 32000);
        assert_eq!(CELL_MIC_SAMPLE_RATE_16K, 16000);
    }

    #[test]
    fn test_mic_device_type() {
        assert_eq!(CellMicDeviceType::Usb as u32, 0);
        assert_eq!(CellMicDeviceType::Bluetooth as u32, 1);
        assert_eq!(CellMicDeviceType::Camera as u32, 2);
    }

    #[test]
    fn test_mic_device_state() {
        assert_eq!(CellMicDeviceState::Closed as u32, 0);
        assert_eq!(CellMicDeviceState::Open as u32, 1);
        assert_eq!(CellMicDeviceState::Capturing as u32, 2);
    }
}
